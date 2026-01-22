//! TCP connection management and handling
//!
//! This module provides connection lifecycle management including
//! spawning, handling, and cleanup of TCP connections.

use crate::sources::event_id::next_event_id;
use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinHandle;
use wp_parse_api::RawData;

use wp_connector_api::{SourceReason, SourceResult};

use crate::sources::tcp::framing::FramingMode;
use crate::sources::tcp::zc::types::BatchConfig;
use crate::sources::tcp::{MessageBatch, ZcpMessage};
use wp_connector_api::{SourceEvent, Tags};

use super::batch::BatchProcessor;
use super::config::TcpTunables;
use crate::sources::tcp::framing::{FramingExtractor, octet_in_progress};

/// Callback function type for direct message processing
pub type MessageCallback = Arc<dyn Fn(SourceEvent) + Send + Sync>;

/// Connection manager for tracking active TCP connections
#[derive(Clone)]
pub struct ConnectionManager {
    connections: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new connection handler
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_connection(
        &self,
        key: &str,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        stream: TcpStream,
        client_ip: IpAddr,
        zcp_tx: mpsc::Sender<MessageBatch>,
        mut stop_rx: broadcast::Receiver<()>,
        batch_config: BatchConfig,
    ) {
        let ip_str = client_ip.to_string();
        let key_s = key.to_string();
        let connections_clone = self.connections.clone();
        let ip_str_for_log = ip_str.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::handle_connection(
                &key_s,
                tcp_recv_bytes,
                framing,
                stream,
                client_ip,
                zcp_tx,
                &mut stop_rx,
                batch_config,
            )
            .await
            {
                warn_data!("TCP '{}' conn {} error: {}", key_s, ip_str_for_log, e);
            }
            info_data!("TCP '{}' conn {} closed", key_s, ip_str_for_log);
            connections_clone.lock().await.remove(&ip_str_for_log);
        });

        self.connections.lock().await.insert(ip_str, handle);
    }

    /// Abort all active connections
    pub async fn abort_all(&self) {
        let mut m = self.connections.lock().await;
        for (_, h) in m.drain() {
            h.abort();
        }
    }

    /// Get count of active connections
    pub async fn active_count(&self) -> usize {
        self.connections.lock().await.len()
    }

    /// Spawn a new direct connection handler with callback to parser
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_direct_connection(
        &self,
        key: &str,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        stream: TcpStream,
        client_ip: IpAddr,
        event_tx: mpsc::Sender<SourceEvent>,
        mut stop_rx: broadcast::Receiver<()>,
        base_stags: Tags,
    ) {
        let ip_str = client_ip.to_string();
        let key_s = key.to_string();
        let connections_clone = self.connections.clone();
        let ip_str_for_log = ip_str.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::handle_direct_connection(
                &key_s,
                tcp_recv_bytes,
                framing,
                stream,
                client_ip,
                event_tx,
                &mut stop_rx,
                base_stags,
            )
            .await
            {
                warn_data!(
                    "TCP direct '{}' conn {} error: {}",
                    key_s,
                    ip_str_for_log,
                    e
                );
            }
            info_data!("TCP direct '{}' conn {} closed", key_s, ip_str_for_log);
            connections_clone.lock().await.remove(&ip_str_for_log);
        });

        self.connections.lock().await.insert(ip_str, handle);
    }

    /// Handle individual TCP connection with zero-copy message processing
    #[allow(clippy::too_many_arguments)]
    async fn handle_connection(
        key: &str,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        mut stream: TcpStream,
        client_ip: IpAddr,
        zcp_sender: mpsc::Sender<MessageBatch>,
        stop_rx: &mut broadcast::Receiver<()>,
        batch_config: BatchConfig,
    ) -> SourceResult<()> {
        let mut buf = BytesMut::with_capacity(tcp_recv_bytes);
        let mut batch_processor = BatchProcessor::new(batch_config);
        let tun = TcpTunables::default(); // Use default tunables for connection
        let mut scratch = vec![0u8; tun.read_chunk];

        loop {
            tokio::select! {
                r = stream.read_buf(&mut buf) => {
                    match r {
                        Ok(0) => {
                            info_data!("TCP '{}' client {} disconnected", key, client_ip);
                            return Err(SourceReason::Disconnect(format!("client {} closed", client_ip)).into());
                        }
                        Ok(_n) => {
                            // Read additional data in bursts
                            for _ in 0..tun.read_burst {
                                match stream.try_read(&mut scratch) {
                                    Ok(0) => break,
                                    Ok(extra) => {
                                        buf.extend_from_slice(&scratch[..extra]);
                                    }
                                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                                    Err(e) => {
                                        return Err(SourceReason::SupplierError(format!("read {} failed: {}", client_ip, e)).into());
                                    }
                                }
                            }

                            // Process framing
                            Self::process_framing(
                                &mut buf,
                                framing,
                                client_ip,
                                &mut batch_processor,
                                &zcp_sender,
                                key,
                            );
                            // Opportunistically shrink buffer when idle (no unframed residue)
                            if buf.is_empty() && buf.capacity() > NET_SHRINK_HIGH_WATER_BYTES {
                                buf = BytesMut::with_capacity(NET_SHRINK_TARGET_BYTES);
                            }
                        }
                        Err(e) => {
                            return Err(SourceReason::SupplierError(format!("read {} failed: {}", client_ip, e)).into());
                        }
                    }
                }
                _ = stop_rx.recv() => {
                    // Connection closing - flush remaining batch
                    batch_processor.flush(&zcp_sender, key, client_ip);
                    break;
                }
            }
        }

        // Normal exit - flush remaining batch
        batch_processor.flush(&zcp_sender, key, client_ip);
        Ok(())
    }

    /// Process data based on framing mode
    fn process_framing(
        buf: &mut BytesMut,
        framing: FramingMode,
        client_ip: IpAddr,
        batch_processor: &mut BatchProcessor,
        zcp_sender: &mpsc::Sender<MessageBatch>,
        key: &str,
    ) {
        match framing {
            FramingMode::Line => {
                while let Some(data) = FramingExtractor::extract_line_message(buf) {
                    let zcp_msg = ZcpMessage::from_ip_addr(client_ip, data.to_vec());
                    batch_processor.add_message(zcp_msg, zcp_sender, key, client_ip);
                }
            }
            FramingMode::Len => {
                while let Some(data) = FramingExtractor::extract_length_prefixed_message(buf) {
                    let zcp_msg = ZcpMessage::from_ip_addr(client_ip, data.to_vec());
                    batch_processor.add_message(zcp_msg, zcp_sender, key, client_ip);
                }
            }
            FramingMode::Auto => {
                let mut extracted_len = false;
                while let Some(data) = FramingExtractor::extract_length_prefixed_message(buf) {
                    let zcp_msg = ZcpMessage::from_ip_addr(client_ip, data.to_vec());
                    batch_processor.add_message(zcp_msg, zcp_sender, key, client_ip);
                    extracted_len = true;
                }
                // If we already processed a length-prefixed message or are mid-octet payload,
                // prefer length framing and skip line extraction for now.
                if extracted_len || octet_in_progress(buf) {
                    while buf.first().is_some_and(|b| matches!(*b, b'\n' | b'\r')) {
                        buf.advance(1);
                    }
                    return;
                }
                while let Some(data) = FramingExtractor::extract_line_message(buf) {
                    let zcp_msg = ZcpMessage::from_ip_addr(client_ip, data.to_vec());
                    batch_processor.add_message(zcp_msg, zcp_sender, key, client_ip);
                }
            }
        }
    }

    /// Handle individual TCP connection with direct callback to parser
    #[allow(clippy::too_many_arguments)]
    async fn handle_direct_connection(
        key: &str,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        mut stream: TcpStream,
        client_ip: IpAddr,
        event_tx: mpsc::Sender<SourceEvent>,
        stop_rx: &mut broadcast::Receiver<()>,
        base_stags: Tags,
    ) -> SourceResult<()> {
        let mut buf = bytes::BytesMut::with_capacity(tcp_recv_bytes);
        let tun = TcpTunables::default(); // Use default tunables for connection
        let mut scratch = vec![0u8; tun.read_chunk];

        loop {
            tokio::select! {
                r = stream.read_buf(&mut buf) => {
                    match r {
                        Ok(0) => {
                            info_data!("TCP direct '{}' client {} disconnected", key, client_ip);
                            return Err(SourceReason::Disconnect(format!("client {} closed", client_ip)).into());
                        }
                        Ok(_n) => {
                            // Read additional data in bursts
                            for _ in 0..tun.read_burst {
                                match stream.try_read(&mut scratch) {
                                    Ok(0) => break,
                                    Ok(extra) => {
                                        buf.extend_from_slice(&scratch[..extra]);
                                    }
                                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                                    Err(e) => {
                                        return Err(SourceReason::SupplierError(format!("read {} failed: {}", client_ip, e)).into());
                                    }
                                }
                            }

                            // Process framing and send events directly
                            Self::process_framing_direct(
                                &mut buf,
                                framing,
                                client_ip,
                                &event_tx,
                                key,
                                base_stags.clone(),
                            ).await;
                            if buf.is_empty() && buf.capacity() > NET_SHRINK_HIGH_WATER_BYTES {
                                buf = bytes::BytesMut::with_capacity(NET_SHRINK_TARGET_BYTES);
                            }
                        }
                        Err(e) => {
                            return Err(SourceReason::SupplierError(format!("read {} failed: {}", client_ip, e)).into());
                        }
                    }
                }
                _ = stop_rx.recv() => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process data based on framing mode and send events directly
    async fn process_framing_direct(
        buf: &mut bytes::BytesMut,
        framing: FramingMode,
        client_ip: IpAddr,
        event_tx: &mpsc::Sender<SourceEvent>,
        key: &str,
        base_stags: Tags,
    ) {
        match framing {
            FramingMode::Line => {
                while let Some(data) = FramingExtractor::extract_line_message(buf) {
                    let event =
                        Self::build_direct_source_event(key, client_ip, &data, base_stags.clone());
                    let _ = event_tx.try_send(event);
                }
            }
            FramingMode::Len => {
                while let Some(data) = FramingExtractor::extract_length_prefixed_message(buf) {
                    let event =
                        Self::build_direct_source_event(key, client_ip, &data, base_stags.clone());
                    let _ = event_tx.try_send(event);
                }
            }
            FramingMode::Auto => {
                while let Some(data) = FramingExtractor::extract_length_prefixed_message(buf) {
                    let event =
                        Self::build_direct_source_event(key, client_ip, &data, base_stags.clone());
                    let _ = event_tx.try_send(event);
                }
                while let Some(data) = FramingExtractor::extract_line_message(buf) {
                    let event =
                        Self::build_direct_source_event(key, client_ip, &data, base_stags.clone());
                    let _ = event_tx.try_send(event);
                }
            }
        }
    }

    /// Build SourceEvent directly from raw data with zero-copy
    fn build_direct_source_event(
        key: &str,
        client_ip: IpAddr,
        data: &[u8],
        mut base_stags: Tags,
    ) -> SourceEvent {
        // Add access_ip tag
        base_stags.set("access_ip", client_ip.to_string());

        // Create zero-copy payload using Arc
        let payload_arc = Arc::from(data.to_vec());

        // Build source event with zero-copy payload
        let mut event = SourceEvent::new(
            next_event_id(),
            key,
            RawData::from_arc_bytes(payload_arc),
            Arc::new(base_stags),
        );

        // Set ups_ip field
        event.ups_ip = Some(client_ip);

        event
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// Balanced shrink thresholds for per-connection read buffers (net/tcp)
const NET_SHRINK_HIGH_WATER_BYTES: usize = 1024 * 1024; // 1MiB 以上且空时收缩
const NET_SHRINK_TARGET_BYTES: usize = 256 * 1024; // 收缩到 256KiB

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_connection_manager_new() {
        let manager = ConnectionManager::new();
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_manager_default() {
        let manager = ConnectionManager::default();
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_manager_spawn_and_abort() {
        let manager = ConnectionManager::new();

        // Create a mock server to accept connections
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "skipping test_connection_manager_spawn_and_abort: bind failed: {}",
                    e
                );
                return;
            }
        };
        let addr = listener.local_addr().unwrap();

        // Spawn a mock connection
        let (stop_tx, _stop_rx) = broadcast::channel(1);
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                // Just keep connection open for a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                drop(stream);
            }
        });

        // Connect to the mock server
        let stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "skipping test_connection_manager_spawn_and_abort: connect failed: {}",
                    e
                );
                return;
            }
        };
        let client_ip = stream.peer_addr().unwrap().ip();

        let (zcp_tx, _zcp_rx) = mpsc::channel(32);
        let stop_rx = stop_tx.subscribe();

        manager
            .spawn_connection(
                "test_key",
                1024,
                FramingMode::Line,
                stream,
                client_ip,
                zcp_tx,
                stop_rx,
                BatchConfig::default(),
            )
            .await;

        // Should have one active connection
        assert_eq!(manager.active_count().await, 1);

        // Abort all connections
        manager.abort_all().await;

        // Give some time for cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should have no active connections
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_process_framing_line_mode() {
        let mut buf = BytesMut::from("hello\nworld\n");
        let client_ip = "127.0.0.1".parse().unwrap();
        let mut batch_processor =
            BatchProcessor::new(BatchConfig::default().with_max_batch_size(10));
        let (zcp_tx, mut zcp_rx) = mpsc::channel(10);

        ConnectionManager::process_framing(
            &mut buf,
            FramingMode::Line,
            client_ip,
            &mut batch_processor,
            &zcp_tx,
            "test_key",
        );

        // Buffer should be empty after processing
        assert!(buf.is_empty());

        // Flush to send remaining messages
        batch_processor.flush(&zcp_tx, "test_key", client_ip);

        // Try to receive messages
        let mut total_messages = 0;
        while let Ok(batch) = zcp_rx.try_recv() {
            total_messages += batch.len();
            assert!(batch.iter().all(|msg| msg.client_ip() == client_ip));
        }
        assert_eq!(total_messages, 2);
    }

    #[tokio::test]
    async fn test_process_framing_length_mode() {
        let mut buf = BytesMut::from("5 hello6 world!");
        let client_ip = "127.0.0.1".parse().unwrap();
        let mut batch_processor =
            BatchProcessor::new(BatchConfig::default().with_max_batch_size(10));
        let (zcp_tx, mut zcp_rx) = mpsc::channel(10);

        ConnectionManager::process_framing(
            &mut buf,
            FramingMode::Len,
            client_ip,
            &mut batch_processor,
            &zcp_tx,
            "test_key",
        );

        // Buffer should be empty after processing
        assert!(buf.is_empty());

        // Flush to send remaining messages
        batch_processor.flush(&zcp_tx, "test_key", client_ip);

        // Try to receive messages
        let mut total_messages = 0;
        while let Ok(batch) = zcp_rx.try_recv() {
            total_messages += batch.len();
        }
        assert_eq!(total_messages, 2);
    }

    #[tokio::test]
    async fn test_process_framing_auto_handles_newlines() {
        let mut buf = BytesMut::from("hello\n5 world");
        let client_ip = "127.0.0.1".parse().unwrap();
        let mut batch_processor =
            BatchProcessor::new(BatchConfig::default().with_max_batch_size(10));
        let (zcp_tx, mut zcp_rx) = mpsc::channel(10);

        ConnectionManager::process_framing(
            &mut buf,
            FramingMode::Auto,
            client_ip,
            &mut batch_processor,
            &zcp_tx,
            "test_key",
        );

        // Should extract line message, leaving length-prefixed message
        assert_eq!(buf, BytesMut::from("5 world"));

        // Flush to send line message
        batch_processor.flush(&zcp_tx, "test_key", client_ip);

        // Should receive one message
        let batch = zcp_rx.try_recv().unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload(), b"hello");
    }

    #[tokio::test]
    async fn test_process_framing_auto_prefers_length() {
        let mut buf = BytesMut::from("5 world\nhello");
        let client_ip = "127.0.0.1".parse().unwrap();
        let mut batch_processor =
            BatchProcessor::new(BatchConfig::default().with_max_batch_size(10));
        let (zcp_tx, mut zcp_rx) = mpsc::channel(10);

        ConnectionManager::process_framing(
            &mut buf,
            FramingMode::Auto,
            client_ip,
            &mut batch_processor,
            &zcp_tx,
            "test_key",
        );

        // Should extract length-prefixed message, leaving line message without newline
        assert_eq!(buf, BytesMut::from("hello"));

        // Flush to send length-prefixed message
        batch_processor.flush(&zcp_tx, "test_key", client_ip);

        // Should receive one message
        let batch = zcp_rx.try_recv().unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload(), b"world");
    }
}
