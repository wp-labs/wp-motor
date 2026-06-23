//! 同步 Sink 相关的类型定义

use crate::runtime::actor::signal::ShutdownCmd;
use crate::sinks::{SinkDatYSender, SinkEndpoint};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc::error::TrySendError;

/// 同步数据接收点
#[derive(Clone)]
pub enum SinkTerminal {
    Channel(SinkDatYSender),
    BlackHole(super::BlackHoleSink),
    Debug(DebugView),
    Storage(SinkEndpoint),
}

impl SinkTerminal {
    pub fn null() -> Self {
        Self::BlackHole(super::BlackHoleSink::default())
    }
}

impl From<SinkEndpoint> for SinkTerminal {
    fn from(value: SinkEndpoint) -> Self {
        Self::Storage(value)
    }
}

/// 调试视图 sink
#[derive(Clone)]
pub struct DebugView {
    inner: std::sync::Arc<DebugViewInner>,
}

pub struct DebugViewInner {
    pub sender: tokio::sync::mpsc::Sender<String>,
    dropped: AtomicUsize,
    pub _shutdown: ShutdownCmd,
}

impl DebugView {
    pub fn new() -> (Self, ShutdownCmd) {
        let (_shutdown_tx, shutdown_rx): (tokio::sync::oneshot::Sender<ShutdownCmd>, _) =
            tokio::sync::oneshot::channel();
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<String>(wp_conf::limits::debug_view_channel_cap());
        let debug_view_batch_lines = wp_conf::limits::debug_view_batch_lines();

        // 在后台处理日志输出
        tokio::spawn(async move {
            let mut lines = Vec::with_capacity(debug_view_batch_lines);
            let mut shutdown_rx = shutdown_rx;
            loop {
                tokio::select! {
                    Some(line) = rx.recv() => {
                        lines.push(line);
                        if lines.len() >= debug_view_batch_lines {
                            // 批量输出
                            for line in lines.drain(..) {
                                println!("{}", line);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        // 退出前输出剩余内容
                        for line in lines {
                            println!("{}", line);
                        }
                        break;
                    }
                }
            }
        });

        let shutdown = ShutdownCmd::NoOp;
        let inner = DebugViewInner {
            sender: tx,
            dropped: AtomicUsize::new(0),
            _shutdown: ShutdownCmd::NoOp,
        };
        (
            Self {
                inner: Arc::new(inner),
            },
            shutdown,
        )
    }

    pub fn send(&self, msg: String) {
        match self.inner.sender.try_send(msg) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                let dropped = self.inner.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped.is_multiple_of(1024) {
                    warn_data!(
                        "debug view output channel full; dropped {} debug lines",
                        dropped
                    );
                }
            }
            Err(TrySendError::Closed(_)) => {
                let dropped = self.inner.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped.is_multiple_of(1024) {
                    warn_data!(
                        "debug view output channel closed; dropped {} debug lines",
                        dropped
                    );
                }
            }
        }
    }
}

impl Default for DebugView {
    fn default() -> Self {
        let (v, _) = Self::new();
        v
    }
}
