use crate::runtime::collector::realtime::constants::{
    PICKER_PENDING_CAPACITY, picker_pending_max_bytes,
};
use crate::runtime::collector::realtime::picker::policy::PostPolicy;
use crate::runtime::collector::realtime::picker::policy::PullPolicy;
use crate::runtime::parser::workflow::ParseDispatchRouter;
use std::collections::VecDeque;

use wp_connector_api::SourceBatch;
use wp_connector_api::SourceEvent;
use wp_model_core::raw::RawData;
/// Picker state and constructor.
/// JM is my wife ,Thank JM for her support in WarpParse development.
#[derive(getset::Getters, getset::MutGetters)]
#[get = "pub"]
#[get_mut = "pub"]
pub struct JMActPicker {
    #[get = "pub"]
    parse_router: ParseDispatchRouter,
    pending: VecDeque<SourceBatch>,
    pending_bytes: usize,
    #[get_mut = "pub"]
    post_policy: PostPolicy,
    #[get_mut = "pub"]
    pull_policy: PullPolicy,
}

impl JMActPicker {
    /// 创建 ActPicker，并一次性注入解析订阅者集合（推荐）。
    /// 使用空集合可创建“无订阅者”的 picker。
    pub fn new(parse_router: ParseDispatchRouter) -> Self {
        let burst = Self::burst_max();
        Self {
            parse_router,
            pending: VecDeque::with_capacity(PICKER_PENDING_CAPACITY),
            pending_bytes: 0,
            post_policy: PostPolicy::new(burst),
            pull_policy: PullPolicy::new(burst),
        }
    }

    // 兼容遗留 API 的 subscribe 已移除；请在 new(...) 时传入集合，或由上层组件维护订阅者集合。

    #[inline]
    pub(crate) fn take_pending(&mut self) -> Option<SourceBatch> {
        let batch = self.pending.pop_front()?;
        self.pending_bytes = self.pending_bytes.saturating_sub(batch_bytes(&batch));
        Some(batch)
    }
    #[inline]
    pub(crate) fn set_pending_front(&mut self, batch: SourceBatch) {
        self.pending_bytes = self.pending_bytes.saturating_add(batch_bytes(&batch));
        self.pending.push_front(batch);
    }
    #[inline]
    pub(crate) fn extend_pending(&mut self, batch: SourceBatch) {
        let batch_bytes = batch_bytes(&batch);
        self.pending_bytes = self.pending_bytes.saturating_add(batch_bytes);
        self.pending.push_back(batch);
        // 当 pending 水位接近上限时，抽样打印，辅助定位“解析前积压”导致的内存增长
        const WARN_THRESHOLD: usize =
            crate::runtime::collector::realtime::constants::PICKER_PENDING_CAPACITY - 8;
        if self.pending.len() >= WARN_THRESHOLD {
            use crate::sample_log_with_hits;
            sample_log_with_hits!(
                PENDING_HI_HITS,
                warn_mtrc,
                "backpressure: picker pending high water: {} / {} (pending_bytes={} max_bytes={})",
                self.pending.len(),
                crate::runtime::collector::realtime::constants::PICKER_PENDING_CAPACITY,
                self.pending_bytes,
                picker_pending_max_bytes()
            );
        }
    }
    #[inline]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
    #[inline]
    pub(crate) fn pending_bytes_at_capacity(&self) -> bool {
        self.pending_bytes >= picker_pending_max_bytes()
    }

    /// 合并前端的多个小批次，尽量把事件数凑到 `max_events`，用于减少“批数”对解析通道的占用。
    /// 返回合并后的单批；若 pending 为空则返回 None。
    ///
    /// 设计要点：
    /// - 只处理队头，保持投递顺序稳定；
    /// - 超出上限时将剩余事件回推到队头，避免丢失与乱序；
    /// - 仅在 pending 堆积达到触发阈值时调用，正常路径不增加额外开销。
    pub(crate) fn coalesce_pending_front(&mut self, max_events: usize) -> Option<SourceBatch> {
        if self.pending.is_empty() {
            return None;
        }
        let mut merged: SourceBatch = SourceBatch::with_capacity(max_events);
        while merged.len() < max_events {
            match self.take_pending() {
                Some(mut b) => {
                    if merged.len() + b.len() <= max_events {
                        merged.append(&mut b);
                    } else {
                        // 超出上限：把多余的事件推回一个新的批次并放回队头
                        let remain = max_events - merged.len();
                        let rest = b.split_off(remain);
                        merged.append(&mut b);
                        self.set_pending_front(rest);
                        break;
                    }
                }
                None => break,
            }
        }
        if merged.is_empty() {
            None
        } else {
            Some(merged)
        }
    }
}

fn batch_bytes(batch: &SourceBatch) -> usize {
    batch.iter().map(event_payload_len).sum()
}

fn event_payload_len(ev: &SourceEvent) -> usize {
    match &ev.payload {
        RawData::String(s) => s.len(),
        RawData::Bytes(b) => b.len(),
        RawData::ArcBytes(b) => b.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::parser::workflow::ParseDispatchRouter;
    use crate::sources::event_id::next_event_id;
    use std::sync::Arc;
    use wp_connector_api::Tags;

    fn make_event_with_size(tag: &str, size: usize) -> SourceEvent {
        let mut tags = Tags::new();
        tags.set("tag", tag.to_string());
        SourceEvent::new(
            next_event_id(),
            tag,
            RawData::Bytes(vec![b'x'; size].into()),
            Arc::new(tags),
        )
    }

    #[test]
    fn picker_pending_bytes_tracks_queue_mutations() {
        let mut picker = JMActPicker::new(ParseDispatchRouter::empty());
        let first = vec![make_event_with_size("a", 8), make_event_with_size("b", 16)];
        let second = vec![make_event_with_size("c", 32)];

        picker.extend_pending(first.clone());
        picker.set_pending_front(second.clone());

        assert_eq!(picker.pending_count(), 2);
        assert_eq!(
            *picker.pending_bytes(),
            batch_bytes(&first) + batch_bytes(&second)
        );

        let front = picker.take_pending().expect("should take front batch");
        assert_eq!(front.len(), 1);
        assert_eq!(*picker.pending_bytes(), batch_bytes(&first));

        let tail = picker.take_pending().expect("should take tail batch");
        assert_eq!(tail.len(), 2);
        assert_eq!(*picker.pending_bytes(), 0);
        assert!(!picker.pending_bytes_at_capacity());
    }
}
