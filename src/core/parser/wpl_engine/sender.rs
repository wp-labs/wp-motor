//! 数据发送逻辑

use super::types::{ParseFailInfo, ParsedDatSet};
use crate::core::parser::WplEngine;
use crate::core::sinks::sync_sink::TrySendStatus;
use crate::core::sinks::sync_sink::traits::RecSyncSink;
use crate::facade::test_helpers::SinkTerminal;
use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use crate::runtime::errors::err4_send_to_sink;
use crate::sinks::{SinkDataEnum, SinkGroupAgent, SinkInfraAble, SinkPackage, SinkRecUnit};
use base64::Engine;
use base64::engine::general_purpose;
use orion_error::UvsReason;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::sleep;
use wp_connector_api::{SinkError, SourceEvent};
use wp_error::error_handling::{ErrorHandlingStrategy, sys_robust_mode};
use wp_parse_api::RawData;
use wpl::{PkgID, WparseError, WparseReason, WparseResult};

impl WplEngine {
    /// 发送批量处理后的数据
    pub async fn send_batched_data(&self, data: ParsedDatSet) -> WparseResult<()> {
        // 发送失败的数据到 miss sink（infra）
        self.send_miss_packets(data.missed_packets).await?;

        // 发送 residue 数据（infra）
        self.send_residue_data(data.residue_data).await?;

        // 若配置为 skip_sink，则仅保留 infra 通道（miss/residue/monitor/error）
        if crate::engine_flags::skip_sink() {
            return Ok(());
        }

        // 批量发送到业务 sinks
        self.send_to_sink_groups(data.sink_groups).await
    }

    /// 发送失败的数据包
    async fn send_miss_packets(
        &self,
        missed_packets: Vec<(SourceEvent, ParseFailInfo)>,
    ) -> WparseResult<()> {
        for (event, fail_info) in missed_packets {
            let err_msg = fail_info.format_error();

            info_data!("parse error:{}", err_msg);
            info_data!("event src_key:{}", event.src_key);

            // 直接发送 SourceEvent
            self.send_miss_event_async(&event, err_msg).await?;
        }

        Ok(())
    }

    /// 发送 residue 数据
    async fn send_residue_data(&self, residue_data: Vec<(PkgID, String)>) -> WparseResult<()> {
        for (pkg_id, raw_data) in residue_data {
            self.forward_raw_to_infra(|| self.residue(), pkg_id, raw_data)
                .await?;
        }

        Ok(())
    }

    /// 发送到 sink 组
    async fn send_to_sink_groups(
        &self,
        sink_groups: HashMap<String, SinkPackage>,
    ) -> WparseResult<()> {
        for (wpl_key, mut package) in sink_groups {
            if package.is_empty() {
                continue;
            }

            // 查找对应的 pipeline
            for wpl_line in self.pipelines.pipelines() {
                if wpl_line.wpl_key() == &wpl_key {
                    let endp = wpl_line.get_rolled_end().clone();

                    // 更新所有记录的元数据
                    let proc_meta = crate::sinks::ProcMeta::Rule(wpl_key.clone());
                    package.update_meta(proc_meta);

                    // 发送批量数据
                    self.sink_batch(&endp, package).await?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// 批量发送到单个 sink
    async fn sink_batch(&self, endp: &SinkTerminal, package: SinkPackage) -> WparseResult<()> {
        if !package.is_empty() {
            match endp {
                SinkTerminal::Channel(sender) => {
                    if let Err(e) = sender.send(package).await {
                        error_dfx!("批量发送失败: {}", e);
                    }
                }
                SinkTerminal::BlackHole(blackhole) => {
                    let units = package.into_inner();
                    let _ = blackhole.try_send_to_sink_batch(units);
                    // BlackHole 总是成功，不需要处理结果
                }
                _ => {
                    // 对于其他类型的 sink，使用默认的批量实现（逐个发送）
                    for unit in package {
                        let status = endp.try_send_record(
                            *unit.id(),
                            unit.meta().clone(),
                            unit.data().clone(),
                        );
                        match status {
                            TrySendStatus::Sended => {}
                            TrySendStatus::Fulfilled(_, _) => {
                                sleep(std::time::Duration::from_millis(ACTOR_IDLE_TICK_MS)).await;
                            }
                            TrySendStatus::Err(e) => {
                                Self::handle_try_send_status(e)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 统一处理 try_send_to_sink 的返回值，隐藏策略与 idle 退让细节
    fn handle_try_send_status(e: Arc<SinkError>) -> WparseResult<()> {
        match err4_send_to_sink(&e, &sys_robust_mode()) {
            ErrorHandlingStrategy::FixRetry => {
                // 这里的错误处理可能需要更复杂的逻辑
                let msg = format!("sink send error (fix_retry): {}", e);
                Err(WparseError::from(WparseReason::Uvs(
                    UvsReason::SystemError(msg),
                )))
            }
            ErrorHandlingStrategy::Throw => {
                let msg = format!("sink send error: {}", e);
                Err(WparseError::from(WparseReason::Uvs(
                    UvsReason::SystemError(msg),
                )))
            }
            ErrorHandlingStrategy::Terminate => Ok(()),
            ErrorHandlingStrategy::Tolerant | ErrorHandlingStrategy::Ignore => Ok(()),
        }
    }

    /// 发送失败的 SourceEvent
    async fn send_miss_event_async(
        &self,
        event: &SourceEvent,
        err_msg: String,
    ) -> WparseResult<()> {
        // 将 payload 转换为字符串用于显示
        let raw_str = match &event.payload {
            RawData::String(s) => s.clone(),
            RawData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            RawData::ArcBytes(b) => String::from_utf8_lossy(b).to_string(),
        };

        // 如果是字节数据，需要编码
        let display_str = match &event.payload {
            RawData::Bytes(_) => general_purpose::STANDARD.encode(raw_str.as_bytes()),
            _ => raw_str,
        };

        let raw_data = format!(
            "src_key: {}  | data:\n{}\n{}\n\n",
            event.src_key, display_str, err_msg
        );
        self.forward_raw_to_infra(|| self.miss(), event.event_id, raw_data)
            .await
    }

    async fn forward_raw_to_infra<F>(
        &self,
        fetch_agent: F,
        mut pkg_id: PkgID,
        raw: String,
    ) -> WparseResult<()>
    where
        F: Fn() -> SinkGroupAgent,
    {
        let mut pending = SinkDataEnum::from(raw);
        loop {
            let status = match pending {
                SinkDataEnum::Raw(body) => {
                    let agent = fetch_agent();
                    agent.end_point().try_send_raw(pkg_id, body)
                }
                SinkDataEnum::Rec(meta, record) => {
                    let agent = fetch_agent();
                    let unit = SinkRecUnit::new(pkg_id, meta, record);
                    agent.end_point().try_send_to_sink(unit)
                }
                SinkDataEnum::FFV(_) => {
                    error_data!("infra raw sink received FFV data, drop it");
                    return Ok(());
                }
            };

            match status {
                TrySendStatus::Sended => return Ok(()),
                TrySendStatus::Fulfilled(id, data) => {
                    pkg_id = id;
                    pending = data;
                    sleep(std::time::Duration::from_millis(ACTOR_IDLE_TICK_MS)).await;
                }
                TrySendStatus::Err(e) => {
                    Self::handle_try_send_status(e)?;
                    return Ok(());
                }
            }
        }
    }
}
