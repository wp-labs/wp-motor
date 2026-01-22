use crate::sources::event_id::next_event_id;
use crate::sources::tcp::framing::{FramingExtractor, FramingMode};
use bytes::{Bytes, BytesMut};
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpStream;
use wp_connector_api::{SourceBatch, SourceEvent, SourceReason, SourceResult, Tags};
use wp_parse_api::RawData;

const DEFAULT_BATCH_CAPACITY: usize = 128;
const MAX_BATCH_BYTES: usize = 64 * 1024; // soft cap; single payload may exceed but only single event allowed
// When idle and buffer is large, shrink capacity to reduce RSS footprint.
// Balanced shrink thresholds：空闲时将过大的缓冲收缩到较小基线
const SHRINK_HIGH_WATER_BYTES: usize = 1024 * 1024; // 若 capacity 超过 1MiB 且 len==0 则收缩
const SHRINK_TARGET_BYTES: usize = 256 * 1024; // 收缩到 256KiB（降低扩容↔收缩抖动）

pub enum ReadOutcome {
    NoData,
    Produced(SourceBatch),
    Closed,
}

pub struct TcpConnection {
    stream: TcpStream,
    client_addr: SocketAddr,
    framing: FramingMode,
    batcher: BatchBuilder,
}

impl TcpConnection {
    fn raw_fd(&self) -> i32 {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            self.stream.as_raw_fd()
        }
        #[cfg(not(unix))]
        {
            -1
        }
    }
}

impl Drop for TcpConnection {
    fn drop(&mut self) {
        debug_data!(
            "Dropping TCP connection {} fd={} (pending_events={} pending_bytes={})",
            self.client_addr,
            self.raw_fd(),
            self.batcher.pending_len(),
            self.batcher.pending_bytes()
        );
    }
}

struct BatchBuilder {
    buffer: BytesMut,
    base_tags: Tags,
    batch_capacity: usize,
    source_key: String,
    pending_events: VecDeque<SourceEvent>,
    max_batch_bytes: usize,
}

impl TcpConnection {
    pub fn new(
        stream: TcpStream,
        client_addr: SocketAddr,
        framing: FramingMode,
        base_tags: Tags,
        tcp_recv_bytes: usize,
        source_key: String,
    ) -> Self {
        let capacity = tcp_recv_bytes.max(1024);
        let conn = Self {
            stream,
            client_addr,
            framing,
            batcher: BatchBuilder::new(
                BytesMut::with_capacity(capacity),
                base_tags,
                source_key,
                DEFAULT_BATCH_CAPACITY,
                MAX_BATCH_BYTES,
            ),
        };
        debug_data!(
            "Created TCP connection {} fd={}",
            conn.client_addr,
            conn.raw_fd()
        );
        conn
    }

    pub fn try_read_batch(&mut self) -> SourceResult<ReadOutcome> {
        let mut produced = SourceBatch::with_capacity(self.batcher.batch_capacity);
        let mut produced_bytes = 0usize;
        self.batcher
            .fill_batch_from_pending(&mut produced, &mut produced_bytes);
        if !produced.is_empty() {
            return Ok(ReadOutcome::Produced(produced));
        }
        loop {
            match self.stream.try_read_buf(self.batcher.buffer_mut()) {
                Ok(0) => {
                    info_data!(
                        "TCP conn {} try_read returned EOF (pending_events={} pending_bytes={})",
                        self.client_addr,
                        self.batcher.pending_len(),
                        self.batcher.pending_bytes()
                    );
                    return Ok(ReadOutcome::Closed);
                }
                Ok(_) => {
                    trace_data!(
                        "TCP conn {} try_read filled buffer (pending_before={} bytes_before={})",
                        self.client_addr,
                        self.batcher.pending_len(),
                        self.batcher.pending_bytes()
                    );
                    self.batcher.drain_messages(
                        self.framing,
                        self.client_addr.ip(),
                        &mut produced,
                        &mut produced_bytes,
                    );
                    if !produced.is_empty() {
                        return Ok(ReadOutcome::Produced(produced));
                    }
                    continue;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    if produced.is_empty() {
                        // No immediate data; opportunistically shrink buffer if idle
                        self.batcher.maybe_shrink();
                        return Ok(ReadOutcome::NoData);
                    } else {
                        return Ok(ReadOutcome::Produced(produced));
                    }
                }
                Err(e) => {
                    return Err(SourceReason::Disconnect(format!(
                        "tcp read error ({}): {}",
                        self.client_addr, e
                    ))
                    .into());
                }
            }
        }
    }

    pub async fn read_batch(&mut self) -> SourceResult<ReadOutcome> {
        let mut produced = SourceBatch::with_capacity(self.batcher.batch_capacity);
        let mut produced_bytes = 0usize;
        self.batcher
            .fill_batch_from_pending(&mut produced, &mut produced_bytes);
        if !produced.is_empty() {
            return Ok(ReadOutcome::Produced(produced));
        }
        loop {
            if let Err(e) = self.stream.readable().await {
                return Err(SourceReason::Disconnect(format!(
                    "tcp readable error ({}): {}",
                    self.client_addr, e
                ))
                .into());
            }
            match self.stream.try_read_buf(self.batcher.buffer_mut()) {
                Ok(0) => {
                    info_data!(
                        "TCP conn {} blocking read returned EOF (pending_events={} pending_bytes={})",
                        self.client_addr,
                        self.batcher.pending_len(),
                        self.batcher.pending_bytes()
                    );
                    return Ok(ReadOutcome::Closed);
                }
                Ok(_) => {
                    trace_data!(
                        "TCP conn {} blocking read filled buffer (pending_before={} bytes_before={})",
                        self.client_addr,
                        self.batcher.pending_len(),
                        self.batcher.pending_bytes()
                    );
                    self.batcher.drain_messages(
                        self.framing,
                        self.client_addr.ip(),
                        &mut produced,
                        &mut produced_bytes,
                    );
                    if !produced.is_empty() {
                        return Ok(ReadOutcome::Produced(produced));
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    if produced.is_empty() {
                        self.batcher.maybe_shrink();
                    }
                    continue;
                }
                Err(e) => {
                    return Err(SourceReason::Disconnect(format!(
                        "tcp read error ({}): {}",
                        self.client_addr, e
                    ))
                    .into());
                }
            }
        }
    }

    pub fn client_ip(&self) -> IpAddr {
        self.client_addr.ip()
    }

    pub fn pending_len(&self) -> usize {
        self.batcher.pending_len()
    }

    pub fn pending_bytes(&self) -> usize {
        self.batcher.pending_bytes()
    }

    pub fn has_pending(&self) -> bool {
        self.batcher.pending_len() > 0
    }
}

impl BatchBuilder {
    fn new(
        buffer: BytesMut,
        base_tags: Tags,
        source_key: String,
        batch_capacity: usize,
        max_batch_bytes: usize,
    ) -> Self {
        Self {
            buffer,
            base_tags,
            batch_capacity,
            source_key,
            pending_events: VecDeque::new(),
            max_batch_bytes,
        }
    }

    fn buffer_mut(&mut self) -> &mut BytesMut {
        &mut self.buffer
    }

    /// Opportunistically shrink the internal buffer when idle to reclaim memory.
    fn maybe_shrink(&mut self) {
        if self.buffer.is_empty() && self.buffer.capacity() > SHRINK_HIGH_WATER_BYTES {
            // Recreate with a smaller baseline capacity to actually release memory.
            self.buffer = BytesMut::with_capacity(SHRINK_TARGET_BYTES);
        }
    }

    fn fill_batch_from_pending(&mut self, batch: &mut SourceBatch, produced_bytes: &mut usize) {
        while let Some(event) = self.pending_events.pop_front() {
            let event_size = event_payload_len(&event);
            let would_exceed = *produced_bytes + event_size > self.max_batch_bytes;
            if batch.len() >= self.batch_capacity {
                self.pending_events.push_front(event);
                break;
            }
            if would_exceed && !batch.is_empty() {
                debug_data!(
                    "TCP source '{}' batch hit byte cap: current_bytes={} event_size={} limit={} pending_requeue={}",
                    self.source_key,
                    produced_bytes,
                    event_size,
                    self.max_batch_bytes,
                    self.pending_events.len() + 1
                );
                self.pending_events.push_front(event);
                break;
            }
            *produced_bytes = produced_bytes.saturating_add(event_size);
            batch.push(event);
            if would_exceed {
                break;
            }
        }
    }

    fn drain_messages(
        &mut self,
        framing: FramingMode,
        peer_ip: IpAddr,
        batch: &mut SourceBatch,
        produced_bytes: &mut usize,
    ) {
        while let Some(payload) = extract_message(framing, &mut self.buffer) {
            let event = self.build_event(payload, peer_ip);
            let event_size = event_payload_len(&event);
            let would_exceed = *produced_bytes + event_size > self.max_batch_bytes;
            if batch.len() >= self.batch_capacity {
                self.pending_events.push_back(event);
                break;
            }
            if would_exceed && !batch.is_empty() {
                debug_data!(
                    "TCP source '{}' batch hit byte cap while draining buffer: current_bytes={} event_size={} limit={} pending_after={}",
                    self.source_key,
                    produced_bytes,
                    event_size,
                    self.max_batch_bytes,
                    self.pending_events.len()
                );
                self.pending_events.push_back(event);
                break;
            }
            *produced_bytes = produced_bytes.saturating_add(event_size);
            batch.push(event);
            if would_exceed {
                debug_data!(
                    "TCP source '{}' batch reached byte cap after push: total_bytes={} events={} limit={} pending_after={}",
                    self.source_key,
                    produced_bytes,
                    batch.len(),
                    self.max_batch_bytes,
                    self.pending_events.len()
                );
                break;
            }
        }
    }

    fn pending_len(&self) -> usize {
        self.pending_events.len()
    }

    fn pending_bytes(&self) -> usize {
        self.pending_events.iter().map(event_payload_len).sum()
    }

    fn build_event(&self, payload: Bytes, peer_ip: IpAddr) -> SourceEvent {
        let mut event = SourceEvent::new(
            next_event_id(),
            &self.source_key,
            RawData::Bytes(payload),
            Arc::new(self.base_tags.clone()),
        );
        event.ups_ip = Some(peer_ip);
        event
    }
}

fn extract_message(framing: FramingMode, buffer: &mut BytesMut) -> Option<Bytes> {
    match framing {
        FramingMode::Line => FramingExtractor::extract_line_message(buffer),
        FramingMode::Len => FramingExtractor::extract_length_prefixed_message(buffer),
        FramingMode::Auto => FramingExtractor::extract_length_prefixed_message(buffer)
            .or_else(|| FramingExtractor::extract_line_message(buffer)),
    }
}

pub fn batch_bytes(batch: &SourceBatch) -> usize {
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
    use bytes::BufMut;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn try_read_batch_respects_payload_budget() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().unwrap();
        let writer = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(addr)
                .await
                .expect("connect client");
            let line = vec![b'a'; 4096];
            for _ in 0..10 {
                client.write_all(&line).await.unwrap();
                client.write_all(b"\n").await.unwrap();
            }
        });

        let (stream, peer) = listener.accept().await.expect("accept connection");
        let mut conn = TcpConnection::new(
            stream,
            peer,
            FramingMode::Line,
            Tags::new(),
            8192,
            "test".into(),
        );
        writer.await.unwrap();

        let first = conn.try_read_batch().expect("first batch should succeed");
        let mut total_bytes = 0usize;
        if let ReadOutcome::Produced(batch) = first {
            for ev in &batch {
                total_bytes += event_payload_len(ev);
            }
            assert!(
                total_bytes <= MAX_BATCH_BYTES,
                "first batch should not exceed byte limit"
            );
        } else {
            panic!("expected produced outcome");
        }

        let second = conn.try_read_batch().expect("second batch should succeed");
        if let ReadOutcome::Produced(batch) = second {
            assert!(!batch.is_empty());
        } else {
            panic!("expected remaining data");
        }
    }

    #[tokio::test]
    async fn test_length_prefixed_framing() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().unwrap();

        let writer = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(addr)
                .await
                .expect("connect client");

            // Send length-prefixed messages
            let messages = vec!["hello", "world", "length", "prefixed"];
            for msg in messages {
                client
                    .write_all(format!("{} {}", msg.len(), msg).as_bytes())
                    .await
                    .unwrap();
            }
        });

        let (stream, peer) = listener.accept().await.expect("accept connection");
        let mut conn = TcpConnection::new(
            stream,
            peer,
            FramingMode::Len,
            Tags::new(),
            8192,
            "test_len".into(),
        );

        writer.await.unwrap();

        let result = conn.try_read_batch().expect("read should succeed");
        if let ReadOutcome::Produced(batch) = result {
            assert_eq!(batch.len(), 4);

            let payloads: Vec<String> = batch
                .iter()
                .map(|ev| match &ev.payload {
                    RawData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                    _ => panic!("expected bytes payload"),
                })
                .collect();

            assert_eq!(payloads, vec!["hello", "world", "length", "prefixed"]);
        } else {
            panic!("expected produced outcome");
        }
    }

    #[tokio::test]
    async fn test_auto_framing_handles_mixed_modes() {
        if std::env::var("WP_NET_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().unwrap();

        let writer = tokio::spawn(async move {
            let mut client = tokio::net::TcpStream::connect(addr)
                .await
                .expect("connect client");

            // Mix of newline and length-prefixed messages
            client.write_all(b"line1\n").await.unwrap();
            client.write_all(b"5 hello").await.unwrap();
            client.write_all(b"\n").await.unwrap();
            client.write_all(b"7 message").await.unwrap();
        });

        let (stream, peer) = listener.accept().await.expect("accept connection");
        let mut conn = TcpConnection::new(
            stream,
            peer,
            FramingMode::Auto,
            Tags::new(),
            8192,
            "test_auto".into(),
        );

        writer.await.unwrap();

        let result = conn.try_read_batch().expect("read should succeed");
        if let ReadOutcome::Produced(batch) = result {
            assert_eq!(batch.len(), 4);

            let payloads: Vec<String> = batch
                .iter()
                .map(|ev| match &ev.payload {
                    RawData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                    _ => panic!("expected bytes payload"),
                })
                .collect();

            assert_eq!(payloads, vec!["line1", "hello", "", "message"]);
        } else {
            panic!("expected produced outcome");
        }
    }

    #[test]
    fn test_batch_builder_maybe_shrink() {
        let mut batcher = BatchBuilder::new(
            BytesMut::with_capacity(2 * 1024 * 1024), // 2MiB
            Tags::new(),
            "test".into(),
            10,
            64 * 1024,
        );

        // Fill buffer with data
        batcher.buffer.put(&[0u8; 1000][..]);
        assert_eq!(batcher.buffer.capacity(), 2 * 1024 * 1024);

        // Clear and try to shrink (should shrink because capacity > SHRINK_HIGH_WATER_BYTES)
        batcher.buffer.clear();
        batcher.maybe_shrink();
        assert_eq!(batcher.buffer.capacity(), SHRINK_TARGET_BYTES);

        // Fill with small buffer
        let mut batcher2 = BatchBuilder::new(
            BytesMut::with_capacity(100 * 1024), // 100KiB
            Tags::new(),
            "test".into(),
            10,
            64 * 1024,
        );

        batcher2.buffer.clear();
        batcher2.maybe_shrink();
        // Should not shrink because capacity is less than SHRINK_HIGH_WATER_BYTES
        assert_eq!(batcher2.buffer.capacity(), 100 * 1024);
    }

    #[test]
    fn test_fill_batch_from_pending_with_byte_limit() {
        let mut batcher = BatchBuilder::new(
            BytesMut::new(),
            Tags::new(),
            "test".into(),
            10,
            100, // Small byte limit for testing
        );

        // Create pending events that exceed byte limit
        let peer_ip = "127.0.0.1".parse().unwrap();
        let event1 = batcher.build_event(Bytes::from(vec![0u8; 60]), peer_ip);
        let event2 = batcher.build_event(Bytes::from(vec![0u8; 60]), peer_ip);
        let event3 = batcher.build_event(Bytes::from(vec![0u8; 20]), peer_ip);

        batcher.pending_events.push_back(event1);
        batcher.pending_events.push_back(event2);
        batcher.pending_events.push_back(event3);

        let mut batch = SourceBatch::new();
        let mut produced_bytes = 0;

        batcher.fill_batch_from_pending(&mut batch, &mut produced_bytes);

        // Should only include the first event (60 bytes) as second would exceed limit
        assert_eq!(batch.len(), 1);
        assert_eq!(produced_bytes, 60);
        assert_eq!(batcher.pending_events.len(), 2); // Two events remain
    }

    #[test]
    fn test_event_payload_len() {
        let id = next_event_id();
        let source_key = "test";
        let tags = Arc::new(Tags::new());

        // Test String payload
        let event_str = SourceEvent::new(
            id,
            source_key,
            RawData::String("hello world".to_string()),
            tags.clone(),
        );
        assert_eq!(event_payload_len(&event_str), 11);

        // Test Bytes payload
        let event_bytes = SourceEvent::new(
            id,
            source_key,
            RawData::Bytes(vec![0u8; 42].into()),
            tags.clone(),
        );
        assert_eq!(event_payload_len(&event_bytes), 42);

        // Test ArcBytes payload
        let event_arc = SourceEvent::new(
            id,
            source_key,
            RawData::ArcBytes(Arc::new(vec![0u8; 100])),
            tags,
        );
        assert_eq!(event_payload_len(&event_arc), 100);
    }
}
