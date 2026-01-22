//! TCP server implementation for zero-copy message processing
//!
//! This module provides the main TCP server structure that handles
//! connection acceptance and overall server lifecycle management.

use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use wp_connector_api::{ControlEvent, CtrlRx, SourceError, SourceReason, SourceResult};

use crate::sources::tcp::MessageBatch;
use crate::sources::tcp::framing::{FramingMode, STOP_CHANNEL_CAPACITY, default_channel_capacity};
use crate::sources::tcp::zc::types::BatchConfig;

use super::connection::ConnectionManager;

/// Generic TCP server with zero-copy message processing
pub struct TcpServer {
    address: String,
    tcp_recv_bytes: usize,
    framing: FramingMode,
    acceptor: Option<JoinHandle<()>>,
    stop_tx: Option<broadcast::Sender<()>>,
    connection_manager: ConnectionManager,
    zcp_tx: mpsc::Sender<MessageBatch>,
    zcp_rx: mpsc::Receiver<MessageBatch>,
    batch_config: BatchConfig,
}

impl TcpServer {
    /// Create a new TCP server with the given configuration
    pub fn new(address: String, tcp_recv_bytes: usize, framing: FramingMode) -> Self {
        let (zcp_tx, zcp_rx) = tokio::sync::mpsc::channel(default_channel_capacity());
        Self {
            address,
            tcp_recv_bytes,
            framing,
            acceptor: None,
            stop_tx: None,
            connection_manager: ConnectionManager::new(),
            zcp_tx,
            zcp_rx,
            batch_config: BatchConfig::default(),
        }
    }

    /// Set batch configuration for message processing
    pub fn with_batch_config(mut self, batch_config: BatchConfig) -> Self {
        self.batch_config = batch_config;
        self
    }

    /// Get the server address
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Start the server and bridge control signals
    pub async fn start(&mut self, key: &str, mut ctrl_rx: CtrlRx) -> SourceResult<()> {
        if self.acceptor.is_some() {
            return Err(SourceReason::SupplierError("tcp server already started".into()).into());
        }

        let (stx, srx) = broadcast::channel::<()>(STOP_CHANNEL_CAPACITY);
        self.stop_tx = Some(stx.clone());

        // Bridge control -> stop signals
        tokio::spawn(async move {
            while let Ok(evt) = ctrl_rx.recv().await {
                match evt {
                    ControlEvent::Stop | ControlEvent::Isolate(true) => {
                        let _ = stx.send(());
                        break;
                    }
                    _ => {}
                }
            }
        });

        let addr = self.address.clone();
        let bytes = self.tcp_recv_bytes;
        let framing = self.framing;
        let key_s = key.to_string();
        let batch_config = self.batch_config;
        let zcp_tx = self.zcp_tx.clone();
        let connection_manager = self.connection_manager.clone();

        let acceptor = tokio::spawn(async move {
            if let Err(e) = Self::run_acceptor(
                &key_s,
                addr,
                bytes,
                framing,
                zcp_tx,
                srx,
                connection_manager,
                batch_config,
            )
            .await
            {
                error_data!("TCP '{}' acceptor failed: {}", key_s, e);
            }
        });

        self.acceptor = Some(acceptor);
        info_data!("TCP '{}' started", key);
        Ok(())
    }

    /// Stop the server
    pub async fn stop(&mut self) -> SourceResult<()> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(acceptor) = self.acceptor.take() {
            let _ = acceptor.await;
        }
        info_data!("TCP server stopped");
        Ok(())
    }

    /// Take the zero-copy message batch receiver
    pub fn take_zcp_receiver(&mut self) -> tokio::sync::mpsc::Receiver<MessageBatch> {
        std::mem::replace(&mut self.zcp_rx, tokio::sync::mpsc::channel(1).1)
    }

    /// Close the server and cleanup all connections
    pub async fn close(&mut self, key: &str) -> SourceResult<()> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.acceptor.take() {
            let _ = t.await;
        }
        self.connection_manager.abort_all().await;
        info_data!("TCP '{}' closed", key);
        Ok(())
    }

    /// Run the main acceptor loop
    async fn run_acceptor(
        key: &str,
        address: String,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        zcp_tx: Sender<MessageBatch>,
        mut stop_rx: broadcast::Receiver<()>,
        connection_manager: ConnectionManager,
        batch_config: BatchConfig,
    ) -> SourceResult<()> {
        let listener = TcpListener::bind(&address).await.map_err(|e| {
            SourceError::from(SourceReason::Disconnect(format!(
                "failed to bind TCP socket to {}: {}",
                address, e
            )))
        })?;

        let local = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| address.clone());

        // 控制面与数据面双日志，便于在控制台/文件快速定位端口与绑定地址
        info_ctrl!("TCP listen '{}' addr={} local={}", key, address, local);

        loop {
            tokio::select! {
                res = listener.accept() => {
                    match res {
                        Ok((stream, addr)) => {
                            connection_manager.spawn_connection(
                                key,
                                tcp_recv_bytes,
                                framing,
                                stream,
                                addr.ip(),
                                zcp_tx.clone(),
                                stop_rx.resubscribe(),
                                batch_config,
                            ).await;
                        }
                        Err(e) => {
                            error_data!("TCP '{}' accept error: {}", key, e);
                        }
                    }
                }
                _ = stop_rx.recv() => {
                    info_data!("TCP '{}' acceptor stopped", key);
                    break;
                }
            }
        }

        connection_manager.abort_all().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_tcp_server_new() {
        let server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line);

        assert_eq!(server.address, "127.0.0.1:0");
        assert_eq!(server.tcp_recv_bytes, 8192);
        assert!(server.acceptor.is_none());
        assert!(server.stop_tx.is_none());
    }

    #[tokio::test]
    async fn test_tcp_server_with_batch_config() {
        let config = BatchConfig::default().with_max_batch_size(32);
        let server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line)
            .with_batch_config(config);

        assert_eq!(server.batch_config.max_batch_size, 32);
    }

    #[tokio::test]
    async fn test_tcp_server_start_stop() {
        let mut server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line);

        let (_tx, rx) = async_broadcast::broadcast::<ControlEvent>(1);
        let ctrl_rx = rx;

        // Start server
        server.start("test", ctrl_rx).await.unwrap();
        assert!(server.acceptor.is_some());
        assert!(server.stop_tx.is_some());

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Stop server
        server.stop().await.unwrap();
        assert!(server.acceptor.is_none());
        assert!(server.stop_tx.is_none());
    }

    #[tokio::test]
    async fn test_tcp_server_double_start_fails() {
        let mut server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line);

        let (_tx, rx) = async_broadcast::broadcast::<ControlEvent>(1);
        let ctrl_rx = rx;

        // Start server first time
        server.start("test", ctrl_rx).await.unwrap();

        // Try to start again - should fail
        let (_tx2, rx2) = async_broadcast::broadcast::<ControlEvent>(1);
        let ctrl_rx2 = rx2;
        let result = server.start("test", ctrl_rx2).await;
        assert!(result.is_err());

        // Cleanup
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_tcp_server_take_zcp_receiver() {
        let mut server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line);

        let receiver = server.take_zcp_receiver();
        assert!(receiver.capacity() > 0);
    }

    #[tokio::test]
    async fn test_tcp_server_close() {
        let mut server = TcpServer::new("127.0.0.1:0".to_string(), 8192, FramingMode::Line);

        let (_tx, rx) = async_broadcast::broadcast::<ControlEvent>(1);
        let ctrl_rx = rx;

        // Start server
        server.start("test", ctrl_rx).await.unwrap();

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Close server
        server.close("test").await.unwrap();
        assert!(server.acceptor.is_none());
        assert!(server.stop_tx.is_none());
    }
}
