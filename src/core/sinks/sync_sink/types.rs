//! 同步 Sink 相关的类型定义

use crate::runtime::actor::signal::ShutdownCmd;
use crate::sinks::{SinkDatYSender, SinkEndpoint};
use std::sync::Arc;

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

#[derive(Clone)]
pub struct DebugViewInner {
    pub sender: tokio::sync::mpsc::UnboundedSender<String>,
    pub _shutdown: ShutdownCmd,
}

impl DebugView {
    pub fn new() -> (Self, ShutdownCmd) {
        let (_shutdown_tx, shutdown_rx): (tokio::sync::oneshot::Sender<ShutdownCmd>, _) =
            tokio::sync::oneshot::channel();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        // 在后台处理日志输出
        tokio::spawn(async move {
            let mut lines = Vec::with_capacity(100);
            let mut shutdown_rx = shutdown_rx;
            loop {
                tokio::select! {
                    Some(line) = rx.recv() => {
                        lines.push(line);
                        if lines.len() >= 100 {
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
        let _ = self.inner.sender.send(msg);
    }
}

impl Default for DebugView {
    fn default() -> Self {
        let (v, _) = Self::new();
        v
    }
}
