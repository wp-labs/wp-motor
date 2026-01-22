//! TCP DataSource implementation: consume accepted `TcpStream` directly and batch events.

use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use tokio::sync::mpsc;
use wp_connector_api::{CtrlRx, DataSource, SourceBatch, SourceReason, SourceResult, Tags};

use super::ConnectionRegistry;
use super::conn::connection::{ReadOutcome, TcpConnection, batch_bytes};
use super::framing::FramingMode;
use super::worker::ConnectionRegistration;

struct ConnectionGuard<'a> {
    source: &'a mut TcpSource,
    conn_id: u64,
    conn: Option<TcpConnection>,
}

impl<'a> ConnectionGuard<'a> {
    fn new(source: &'a mut TcpSource, conn_id: u64) -> Option<Self> {
        source.connections.remove(&conn_id).map(|conn| Self {
            source,
            conn_id,
            conn: Some(conn),
        })
    }

    fn connection(&mut self) -> &mut TcpConnection {
        self.conn
            .as_mut()
            .expect("connection guard missing TcpConnection")
    }

    fn requeue(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.source.push_connection_back(self.conn_id, conn);
        }
    }

    fn deregister(&mut self, reason: &str) {
        self.source.deregister_connection(self.conn_id, reason);
        self.conn.take();
    }

    async fn read(mut self) -> SourceResult<Option<SourceBatch>> {
        let key = self.source.key.clone();
        let conn_id = self.conn_id;
        let (peer, try_result) = {
            let conn = self.connection();
            let peer = conn.client_ip();
            let pending = conn.pending_len();
            let pending_bytes = conn.pending_bytes();
            trace_data!(
                "TCP source '{}' begin try_read (conn={}, pending={}, bytes={})",
                key,
                conn_id,
                pending,
                pending_bytes
            );
            (peer, conn.try_read_batch())
        };

        match try_result {
            Err(e) => {
                error_ctrl!(
                    "TCP source '{}' conn {} try_read error (peer={}): {}",
                    self.source.key,
                    self.conn_id,
                    peer,
                    e
                );
                let reason = format!("try_read error from {}: {}", peer, e);
                self.deregister(&reason);
                return Err(e);
            }
            Ok(ReadOutcome::Produced(batch)) => {
                let pending_after = self
                    .conn
                    .as_ref()
                    .map(|conn| conn.pending_len())
                    .unwrap_or_default();
                trace_ctrl!(
                    "TCP source '{}' conn {} produced {} events via try_read (peer={}, batch_bytes={}, pending_after={})",
                    self.source.key,
                    self.conn_id,
                    batch.len(),
                    peer,
                    batch_bytes(&batch),
                    pending_after
                );
                self.requeue();
                return Ok(Some(batch));
            }
            Ok(ReadOutcome::Closed) => {
                let (pending_events, pending_bytes) = self
                    .conn
                    .as_ref()
                    .map(|conn| (conn.pending_len(), conn.pending_bytes()))
                    .unwrap_or((0, 0));
                info_ctrl!(
                    "TCP source '{}' conn {} closed during try_read (peer={}, pending_events={}, pending_bytes={})",
                    self.source.key,
                    self.conn_id,
                    peer,
                    pending_events,
                    pending_bytes
                );
                let reason = format!(
                    "peer {} closed during try_read (pending_events={} pending_bytes={})",
                    peer, pending_events, pending_bytes
                );
                self.deregister(&reason);
                return Ok(None);
            }
            Ok(ReadOutcome::NoData) => {}
        }

        let key = self.source.key.clone();
        let conn_id = self.conn_id;
        let peer = {
            let conn = self.connection();
            let peer = conn.client_ip();
            let pending = conn.pending_len();
            let pending_bytes = conn.pending_bytes();
            trace_data!(
                "TCP source '{}' begin blocking read (conn={}, pending={}, bytes={})",
                key,
                conn_id,
                pending,
                pending_bytes
            );
            peer
        };

        match self.connection().read_batch().await {
            Err(e) => {
                warn_data!(
                    "TCP source '{}' conn {} read error (peer={}): {}",
                    self.source.key,
                    self.conn_id,
                    peer,
                    e
                );
                let reason = format!("blocking read error from {}: {}", peer, e);
                self.deregister(&reason);
                Err(e)
            }
            Ok(ReadOutcome::Produced(batch)) => {
                let pending_after = self
                    .conn
                    .as_ref()
                    .map(|conn| conn.pending_len())
                    .unwrap_or_default();
                trace_data!(
                    "TCP source '{}' conn {} produced {} events via blocking read (peer={}, batch_bytes={}, pending_after={})",
                    self.source.key,
                    self.conn_id,
                    batch.len(),
                    peer,
                    batch_bytes(&batch),
                    pending_after
                );
                self.requeue();
                Ok(Some(batch))
            }
            Ok(ReadOutcome::NoData) => {
                self.requeue();
                Ok(None)
            }
            Ok(ReadOutcome::Closed) => {
                let (pending_events, pending_bytes) = self
                    .conn
                    .as_ref()
                    .map(|conn| (conn.pending_len(), conn.pending_bytes()))
                    .unwrap_or((0, 0));
                info_ctrl!(
                    "TCP source '{}' conn {} closed during blocking read (peer={}, pending_events={}, pending_bytes={})",
                    self.source.key,
                    self.conn_id,
                    peer,
                    pending_events,
                    pending_bytes
                );
                let reason = format!(
                    "peer {} closed during blocking read (pending_events={} pending_bytes={})",
                    peer, pending_events, pending_bytes
                );
                self.deregister(&reason);
                Ok(None)
            }
        }
    }
}

impl<'a> Drop for ConnectionGuard<'a> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let pending_events = conn.pending_len();
            let pending_bytes = conn.pending_bytes();
            warn_ctrl!(
                "TCP source '{}' guard dropped without disposition; requeueing conn {} (pending_events={}, pending_bytes={})",
                self.source.key,
                self.conn_id,
                pending_events,
                pending_bytes
            );
            self.source.push_connection_back(self.conn_id, conn);
        }
    }
}

pub struct TcpSource {
    key: String,
    base_tags: Tags,
    framing: FramingMode,
    tcp_recv_bytes: usize,
    registry: ConnectionRegistry,
    connection_rx: mpsc::Receiver<ConnectionRegistration>,
    connections: HashMap<u64, TcpConnection>,
    connection_order: VecDeque<u64>,
    started: bool,
    awaiting_logged: bool,
}

impl TcpSource {
    pub fn new(
        key: String,
        tags: Tags,
        _address: String,
        tcp_recv_bytes: usize,
        framing: FramingMode,
        registry: ConnectionRegistry,
        connection_rx: mpsc::Receiver<ConnectionRegistration>,
    ) -> SourceResult<Self> {
        let mut base_tags = Tags::new();
        for (k, v) in tags.iter() {
            base_tags.set(k, v);
        }
        Ok(Self {
            key,
            base_tags,
            framing,
            tcp_recv_bytes,
            registry,
            connection_rx,
            connections: HashMap::new(),
            connection_order: VecDeque::new(),
            started: false,
            awaiting_logged: false,
        })
    }

    pub fn active_connections(&self) -> usize {
        self.connections.len()
    }

    fn register_connection(&mut self, reg: ConnectionRegistration) {
        let connection = TcpConnection::new(
            reg.stream,
            reg.peer_addr,
            self.framing,
            self.base_tags.clone(),
            self.tcp_recv_bytes,
            self.key.clone(),
        );
        self.registry.lock().unwrap().insert(reg.connection_id);
        self.connections.insert(reg.connection_id, connection);
        self.connection_order.push_back(reg.connection_id);
        self.awaiting_logged = false;
        info_ctrl!(
            "TCP source '{}' registered connection {} (peer={}, active={})",
            self.key,
            reg.connection_id,
            reg.peer_addr,
            self.connections.len()
        );
    }

    fn deregister_connection(&mut self, conn_id: u64, reason: &str) {
        self.registry.lock().unwrap().remove(&conn_id);
        self.connections.remove(&conn_id);
        if let Some(pos) = self.connection_order.iter().position(|&id| id == conn_id) {
            self.connection_order.remove(pos);
        }
        info_ctrl!(
            "TCP source '{}' deregistered connection {} (reason={}, remaining={})",
            self.key,
            conn_id,
            reason,
            self.connections.len()
        );
    }

    fn drain_new_connections(&mut self) {
        while let Ok(reg) = self.connection_rx.try_recv() {
            self.register_connection(reg);
        }
    }

    async fn wait_for_connection(&mut self) -> SourceResult<bool> {
        match self.connection_rx.recv().await {
            Some(reg) => {
                info_ctrl!(
                    "TCP source '{}' received pending connection {} from {}",
                    self.key,
                    reg.connection_id,
                    reg.peer_addr
                );
                self.register_connection(reg);
                Ok(true)
            }
            None => {
                warn_ctrl!(
                    "TCP source '{}' connection channel closed; no further connectors will arrive",
                    self.key
                );
                Ok(false)
            }
        }
    }

    fn push_connection_back(&mut self, conn_id: u64, conn: TcpConnection) {
        self.connections.insert(conn_id, conn);
        self.connection_order.push_back(conn_id);
    }

    fn acquire_connection_guard(&mut self, conn_id: u64) -> Option<ConnectionGuard<'_>> {
        ConnectionGuard::new(self, conn_id)
    }
}

#[async_trait]
impl DataSource for TcpSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        if !self.started {
            return Err(SourceReason::SupplierError("TCP source not started".into()).into());
        }
        loop {
            self.drain_new_connections();

            if self.connections.is_empty() {
                if !self.awaiting_logged {
                    debug_data!("TCP source '{}' awaiting connectors (current=0)", self.key);
                    self.awaiting_logged = true;
                }
                if !self.wait_for_connection().await? {
                    return Err(SourceReason::Disconnect(format!(
                        "TCP source '{}' no active connections",
                        self.key
                    ))
                    .into());
                }
                continue;
            }
            self.awaiting_logged = false;

            let conn_id = match self.connection_order.pop_front() {
                Some(id) => id,
                None => continue,
            };

            let Some(guard) = self.acquire_connection_guard(conn_id) else {
                warn_ctrl!(
                    "TCP source '{}' lost connection {} before processing; skipping",
                    self.key,
                    conn_id
                );
                continue;
            };

            if let Some(batch) = guard.read().await?
                && !batch.is_empty()
            {
                return Ok(batch);
            }
        }
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        None
    }

    fn can_try_receive(&mut self) -> bool {
        false
        //true
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }

    async fn start(&mut self, _ctrl_rx: CtrlRx) -> SourceResult<()> {
        if self.started {
            return Err(SourceReason::SupplierError("TCP source already started".into()).into());
        }
        self.started = true;
        self.awaiting_logged = false;
        info_data!("TCP source '{}' started", self.key);
        Ok(())
    }

    async fn close(&mut self) -> SourceResult<()> {
        self.connections.clear();
        self.connection_order.clear();
        self.started = false;
        self.awaiting_logged = false;
        self.registry.lock().unwrap().clear();
        info_data!("TCP source '{}' closed", self.key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_tcp_source_creation() {
        let registry = Arc::new(Mutex::new(HashSet::new()));
        let (_tx, rx) = mpsc::channel(8);
        let source = TcpSource::new(
            "test_tcp".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            8192,
            FramingMode::Line,
            registry,
            rx,
        );
        assert!(source.is_ok());
    }

    #[tokio::test]
    async fn receive_reads_from_registered_connection() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }
        let registry = Arc::new(Mutex::new(HashSet::new()));
        let (reg_tx, reg_rx) = mpsc::channel(8);
        let mut source = TcpSource::new(
            "test_tcp".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            8192,
            FramingMode::Line,
            registry.clone(),
            reg_rx,
        )
        .unwrap();
        let (_ctrl_tx, ctrl_rx) = async_broadcast::broadcast(1);
        source.start(ctrl_rx).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client =
            tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await.unwrap() });
        let (stream, peer_addr) = listener.accept().await.unwrap();
        let mut client_stream = client.await.unwrap();

        reg_tx
            .send(ConnectionRegistration {
                connection_id: 1,
                stream,
                peer_addr,
            })
            .await
            .unwrap();
        registry.lock().unwrap().insert(1);

        use tokio::io::AsyncWriteExt;
        client_stream.write_all(b"hello\n").await.unwrap();

        let batch = tokio::time::timeout(std::time::Duration::from_secs(1), source.receive())
            .await
            .expect("receive result")
            .unwrap();
        assert_eq!(batch.len(), 1);
    }

    #[tokio::test]
    async fn connection_requeued_for_multiple_batches() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }
        let registry = Arc::new(Mutex::new(HashSet::new()));
        let (reg_tx, reg_rx) = mpsc::channel(8);
        let mut source = TcpSource::new(
            "test_tcp".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            8192,
            FramingMode::Line,
            registry.clone(),
            reg_rx,
        )
        .unwrap();
        let (_ctrl_tx, ctrl_rx) = async_broadcast::broadcast(1);
        source.start(ctrl_rx).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client =
            tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await.unwrap() });
        let (stream, peer_addr) = listener.accept().await.unwrap();
        let mut client_stream = client.await.unwrap();

        reg_tx
            .send(ConnectionRegistration {
                connection_id: 2,
                stream,
                peer_addr,
            })
            .await
            .unwrap();
        registry.lock().unwrap().insert(2);

        use tokio::io::AsyncWriteExt;
        client_stream.write_all(b"first\n").await.unwrap();

        let first = tokio::time::timeout(std::time::Duration::from_secs(1), source.receive())
            .await
            .expect("first receive result")
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(source.active_connections(), 1);

        client_stream.write_all(b"second\n").await.unwrap();

        let second = tokio::time::timeout(std::time::Duration::from_secs(1), source.receive())
            .await
            .expect("second receive result")
            .unwrap();
        assert_eq!(second.len(), 1);
    }
}
