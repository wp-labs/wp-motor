//! 单个数据包解析逻辑

use super::types::ProcessResult;
use crate::core::parser::wpl_engine::pipeline::WplPipeline;
use crate::{core::parser::ParseOption, stat::MonSend};
use orion_conf::ToStructError;
use orion_error::{UvsDataFrom, UvsReason};
use std::sync::Arc;
use wp_connector_api::SourceEvent;
use wp_model_core::model::data::Field;
use wpl::{WparseError, WparseReason, WparseResult};

/// 数据包解析器
#[derive(Clone, getset::Getters)]
#[get = "pub"]
pub struct MultiParser {
    pipelines: Vec<WplPipeline>,
}

impl MultiParser {
    pub fn new(pipelines: Vec<WplPipeline>) -> Self {
        Self { pipelines }
    }

    /// 处理单个事件
    pub fn parse_event(&mut self, event: &SourceEvent, setting: &ParseOption) -> ProcessResult {
        let mut max_depth = 0;
        let mut best_wpl = String::new();
        let mut best_error = None;
        let rule_cnt = self.pipelines.len();

        // 尝试用每个规则处理事件
        for (idx, wpl_line) in self.pipelines.iter_mut().enumerate() {
            let is_last = idx == rule_cnt - 1;

            // 调用 WPL 处理
            match wpl_line.proc(event, max_depth) {
                Ok((mut tdo_crate, un_parsed)) => {
                    if *setting.gen_msg_id() {
                        tdo_crate.set_id(event.event_id);
                        tdo_crate.append(Field::from_chars("wp_src_key", event.src_key.as_str()));
                        if let Some(ups_ip) = event.ups_ip {
                            tdo_crate.append(Field::from_ip("wp_src_ip", ups_ip));
                        }
                    }
                    wpl_line.hit_cnt += 1;

                    let wpl_key = wpl_line.wpl_key().to_string();

                    // 根据是否有残留数据返回不同的结果
                    if un_parsed.is_empty() || un_parsed.is_empty() {
                        let record = Arc::new(tdo_crate);
                        return ProcessResult::Success { wpl_key, record };
                    } else {
                        let parsed_len = event.payload.len() - un_parsed.len();
                        if un_parsed.len() as f64 / event.payload.len() as f64 > 0.2 {
                            info_data!(
                                "wpl parse not complete: {}\n data:{}",
                                wpl_line.wpl_key(),
                                event.payload,
                            );
                            if parsed_len > max_depth {
                                max_depth = parsed_len;
                                best_wpl = wpl_line.wpl_key().clone();
                                best_error = Some(
                                    WparseReason::from_data("not complete", Some(parsed_len))
                                        .to_err(),
                                );
                            }
                        } else {
                            let record = Arc::new(tdo_crate);
                            return ProcessResult::Partial {
                                wpl_key,
                                record,
                                residue: un_parsed.to_string(),
                            };
                        }
                    }
                }
                Err(e) => {
                    info_data!(
                        "wpl parse fail: {}\n data:{}\n{}",
                        wpl_line.wpl_key(),
                        event.payload,
                        e
                    );
                    // 记录解析深度最高的错误
                    if let WparseReason::Uvs(UvsReason::DataError(_, Some(pos))) = e.reason() {
                        if *pos > max_depth {
                            max_depth = *pos;
                            best_wpl = wpl_line.wpl_key().clone();
                            best_error = Some(e.clone());
                        }
                    } else if best_error.is_none() {
                        // 如果不是 DataError，作为备选记录第一个错误
                        best_wpl = wpl_line.wpl_key().clone();
                        best_error = Some(e.clone());
                        break;
                    }

                    if is_last {
                        break;
                    }
                }
            }
        }

        // 所有规则都失败，返回深度最高的失败信息
        let best_error = best_error.unwrap_or_else(|| {
            WparseError::from(WparseReason::Uvs(UvsReason::SystemError(
                "No matching rule".to_string(),
            )))
        });
        ProcessResult::Miss(super::types::ParseFailInfo::new(
            best_wpl, best_error, max_depth,
        ))
    }
    pub fn stop(&mut self) {
        self.pipelines.iter_mut().for_each(|i| i.stop());
    }

    pub fn optimized(&mut self, _count: usize) {
        if self.pipelines.is_empty() {
            return;
        }

        self.pipelines.sort_by(|a, b| {
            b.hit_cnt
                .cmp(&a.hit_cnt)
                .then_with(|| a.index().cmp(b.index()))
        });

        // 下一窗口重新记录命中情况
        for pipeline in &mut self.pipelines {
            pipeline.hit_cnt = 0;
        }
    }

    /// 更新规则命中计数并排序
    pub fn hit_count_sort(&mut self) {
        for x in &mut self.pipelines {
            x.access_cnt = 0;
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        for i in self.pipelines.iter_mut() {
            i.send_stat(mon_send).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::wpl_engine::pipeline::WplPipeline;
    use crate::sinks::SinkGroupAgent;
    use wpl::WplEvaluator;

    fn dummy_pipeline(idx: usize, hit: usize) -> WplPipeline {
        let evaluator = WplEvaluator::from_code("rule dummy { ( _ ) }").expect("build wpl");
        let parser = evaluator;
        let mut pipeline = WplPipeline::new(
            idx,
            format!("rule-{}", idx),
            Vec::new(),
            parser,
            vec![SinkGroupAgent::null()],
            Vec::new(),
        );
        pipeline.hit_cnt = hit;
        pipeline
    }

    #[test]
    fn optimized_reorders_by_hit_count() {
        let pipelines = vec![
            dummy_pipeline(0, 1),
            dummy_pipeline(1, 5),
            dummy_pipeline(2, 3),
        ];

        let mut parser = MultiParser::new(pipelines);
        parser.optimized(0);

        let order: Vec<_> = parser
            .pipelines
            .iter()
            .map(|p| p.wpl_key().to_string())
            .collect();
        assert_eq!(order, vec!["rule-1", "rule-2", "rule-0"]);
        assert!(parser.pipelines.iter().all(|p| p.hit_cnt == 0));
    }
}

// 重新导出主要类型
