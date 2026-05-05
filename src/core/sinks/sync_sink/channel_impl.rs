//! Channel 类型 sink 的实现

use super::traits::{RecSyncSink, TrySendStatus};
use crate::sample_log_with_hits;
use crate::sinks::{SinkDataEnum, SinkPackage, SinkRecUnit};
use crate::stat::runtime_counters;
// stride uses crate::LOG_SAMPLE_STRIDE globally; no per-site stride import here
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use wp_connector_api::{SinkError, SinkReason, SinkResult};

// 周期性打印 sink 分发通道满的日志（宏内部维护静态计数器）

// 为方便使用，重新导出
use super::SinkDatYSender;

fn sink_channel_closed_error() -> SinkError {
    SinkReason::sink("sink channel closed")
}

fn sink_channel_full_error() -> SinkError {
    SinkReason::sink("sink channel full - cannot send batch package")
}

impl RecSyncSink for SinkDatYSender {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()> {
        // 非阻塞语义：失败直接返回错误，由上层策略处理背压/错误。
        match self.try_send_to_sink(data) {
            TrySendStatus::Sended => Ok(()),
            TrySendStatus::Fulfilled(_, _) => Err(SinkError::from(SinkReason::StgCtrl)),
            TrySendStatus::Err(_e) => Err(SinkError::from(SinkReason::StgCtrl)),
        }
    }

    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus {
        // 将单个 SinkRecUnit 打包成 SinkPackage 发送
        let package = SinkPackage::single(data);
        match self.try_send(package) {
            Ok(()) => TrySendStatus::Sended,
            Err(TrySendError::Full(package)) => {
                // Channel 满，提取数据返回
                let unit = package.into_iter().next().unwrap();
                runtime_counters::rec_sink_channel_full();
                sample_log_with_hits!(
                    SINK_CH_FULL_HITS,
                    warn_mtrc,
                    "backpressure: sink dispatcher channel full, batch=1"
                );
                TrySendStatus::Fulfilled(
                    *unit.id(),
                    SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()),
                )
            }
            Err(TrySendError::Closed(_)) => {
                runtime_counters::rec_sink_channel_closed();
                TrySendStatus::Err(Arc::new(sink_channel_closed_error()))
            }
        }
    }

    fn send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> SinkResult<()> {
        // 将所有 SinkRecUnit 打包成一个 SinkPackage 发送
        let package = SinkPackage::from_units(data);
        match self.try_send(package) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(p)) => {
                runtime_counters::rec_sink_channel_full();
                sample_log_with_hits!(
                    SINK_CH_FULL_HITS,
                    warn_mtrc,
                    "backpressure: sink dispatcher channel full, batch_size={}",
                    p.len()
                );
                Err(sink_channel_full_error())
            }
            Err(TrySendError::Closed(_)) => {
                runtime_counters::rec_sink_channel_closed();
                Err(sink_channel_closed_error())
            }
        }
    }

    fn try_send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> Vec<TrySendStatus> {
        let data_len = data.len();
        let mut results = Vec::with_capacity(data_len);

        // 将所有 SinkRecUnit 打包成一个 SinkPackage
        let package = SinkPackage::from_units(data);
        match self.try_send(package) {
            Ok(()) => {
                // 所有批量发送成功
                for _ in 0..data_len {
                    results.push(TrySendStatus::Sended);
                }
            }
            Err(TrySendError::Full(package)) => {
                // Channel 满，逐个返回
                runtime_counters::rec_sink_channel_full();
                for unit in package {
                    results.push(TrySendStatus::Fulfilled(
                        *unit.id(),
                        SinkDataEnum::Rec(unit.meta().clone(), unit.data().clone()),
                    ));
                }
            }
            Err(TrySendError::Closed(_)) => {
                // Channel 已关闭
                runtime_counters::rec_sink_channel_closed();
                for _ in 0..data_len {
                    results.push(TrySendStatus::Err(Arc::new(sink_channel_closed_error())));
                }
            }
        }

        results
    }
}
