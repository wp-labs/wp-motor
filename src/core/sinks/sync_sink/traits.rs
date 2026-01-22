//! SyncSink trait 定义

use crate::sinks::{SinkDataEnum, SinkRecUnit};
use std::sync::Arc;
use wp_connector_api::SinkResult;
use wpl::PkgID;

/// 控制同步 sink 的 trait
pub trait SyncCtrl {
    fn stop(&mut self) -> SinkResult<()> {
        Ok(())
    }
}

/// try_send_to_sink 的返回：要么已发送（不再持有数据），要么未发送（带回数据与原因）。
#[derive(Debug)]
pub enum TrySendStatus {
    /// 成功发送
    Sended,
    /// 已回退（带数据）
    Fulfilled(PkgID, SinkDataEnum),
    /// 发送出错
    Err(Arc<wp_connector_api::SinkError>),
}

impl Clone for TrySendStatus {
    fn clone(&self) -> Self {
        match self {
            TrySendStatus::Sended => TrySendStatus::Sended,
            TrySendStatus::Fulfilled(pkg_id, data) => {
                TrySendStatus::Fulfilled(*pkg_id, data.clone())
            }
            TrySendStatus::Err(e) => {
                // 使用 Arc 包装错误以支持克隆
                TrySendStatus::Err(e.clone())
            }
        }
    }
}

/// 接收记录数据的同步 sink trait
pub trait RecSyncSink {
    fn send_to_sink(&self, data: SinkRecUnit) -> SinkResult<()>;
    /// 非阻塞尝试发送；应尽量不阻塞当前线程。
    fn try_send_to_sink(&self, data: SinkRecUnit) -> TrySendStatus;

    /// 批量发送数据到 sink
    fn send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> SinkResult<()> {
        // 默认实现：逐个发送
        for item in data {
            self.send_to_sink(item)?;
        }
        Ok(())
    }
    /// 批量尝试发送数据到 sink
    fn try_send_to_sink_batch(&self, data: Vec<SinkRecUnit>) -> Vec<TrySendStatus> {
        // 默认实现：逐个尝试发送
        data.into_iter()
            .map(|item| self.try_send_to_sink(item))
            .collect()
    }
}
