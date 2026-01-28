//! TCP Syslog source implementation
//!
//! This module provides the TCP-based syslog source that can receive syslog messages
//! over TCP protocol with connection management, automatic framing, and message distribution.

use crate::sources::event_id::next_event_id;
use std::sync::Arc;

use bytes::BytesMut;
use tokio::sync::mpsc::Sender;
use wp_connector_api::{
    CtrlRx, DataSource, EventPreHook, SourceBatch, SourceError, SourceEvent, SourceReason,
    SourceResult, Tags,
};
use wp_parse_api::RawData;

use super::normalize;
use crate::sources::syslog::constants::Message;
use crate::sources::tcp::framing::{self};
use crate::sources::tcp::{TcpSource, ZcpMessage};

// 分帧由公共模块提供；此处不再定义局部 framing 常量

/// TCP Syslog data source with full lifecycle management
///
/// Manages TCP connections, line framing, and message distribution
pub struct TcpSyslogSource {
    key: String,
    tags: Tags,
    strip_header: bool,
    attach_meta_tags: bool,
    fast_strip: bool,
    inner: TcpSource,
    // 复用预处理闭包，避免每条事件都构建一次
    preproc_hook: Option<EventPreHook>,
}

impl TcpSyslogSource {
    /// Create a new TCP syslog source with a pre-built TCP aggregator source
    pub async fn new(
        key: String,
        tags: Tags,
        strip_header: bool,
        attach_meta_tags: bool,
        fast_strip: bool,
        inner: TcpSource,
    ) -> SourceResult<Self> {
        // 先初始化，再构建可复用的预处理闭包
        let mut this = Self {
            key,
            tags,
            strip_header,
            attach_meta_tags,
            fast_strip,
            inner,
            preproc_hook: None,
        };
        this.preproc_hook = this.build_preproc_hook();
        Ok(this)
    }

    fn base_source_tags(&self) -> Tags {
        let mut stags = Tags::new();
        for (k, v) in self.tags.iter() {
            stags.set(k, v);
        }
        stags
    }

    // 连接管理由通用 TcpServer 承担；此处只保留事件构造与预处理逻辑。

    /// Build preprocessing hook for syslog normalize and strip/tag injection
    fn build_preproc_hook(&self) -> Option<EventPreHook> {
        let strip = self.strip_header;
        let attach = self.attach_meta_tags;
        let fast = self.fast_strip;
        if strip || attach {
            Some(std::sync::Arc::new(move |f: &mut SourceEvent| {
                let s_opt = match &f.payload {
                    RawData::String(s) => Some(s.as_str()),
                    RawData::Bytes(b) => std::str::from_utf8(b).ok(),
                    RawData::ArcBytes(b) => std::str::from_utf8(b).ok(),
                };
                if let Some(s) = s_opt {
                    // 快速裁剪路径：仅 strip、不打标签时，优先使用轻量规则避免完整解析
                    if fast && strip && !attach {
                        // 0) RFC5424 快路径：在 '>' 后，VERSION + 空格 + 5 个 token + structured-data（- 或 [...]）后是消息体
                        if let Some(gt) = s.find('>') {
                            let bytes = s.as_bytes();
                            let mut j = gt + 1;
                            // VERSION: 至少 1 位数字
                            let n = bytes.len();
                            let mut k = j;
                            while k < n && bytes[k].is_ascii_digit() {
                                k += 1;
                            }
                            if k > j && k < n && bytes[k] == b' ' {
                                j = k + 1; // 跳过 version 和一个空格
                                // 跳过 5 个以空格分隔的 token
                                let mut tok = 0;
                                while j < n && tok < 5 {
                                    // 跳过非空格
                                    while j < n && bytes[j] != b' ' {
                                        j += 1;
                                    }
                                    if j >= n {
                                        break;
                                    }
                                    // 跳过一个空格
                                    j += 1;
                                    tok += 1;
                                }
                                if tok == 5 && j <= n {
                                    // 结构化数据：'-' 或 '[' ... ']'
                                    if j < n && bytes[j] == b'-' {
                                        let mut start = j + 1;
                                        if start < n && bytes[start] == b' ' {
                                            start += 1;
                                        }
                                        // 直接裁剪 [start..n]
                                        match &mut f.payload {
                                            RawData::Bytes(b) => {
                                                if start <= b.len() {
                                                    *b = b.slice(start..n);
                                                }
                                            }
                                            RawData::String(st) => {
                                                if start <= st.len() {
                                                    *st = st[start..].to_string();
                                                }
                                            }
                                            RawData::ArcBytes(arc_b) => {
                                                // Convert ArcBytes to Bytes for modification
                                                if start <= arc_b.len() {
                                                    let new_bytes =
                                                        bytes::Bytes::copy_from_slice(&arc_b[start..]);
                                                    f.payload = RawData::Bytes(new_bytes);
                                                }
                                            }
                                        }
                                        return;
                                    }
                                    if j < n && bytes[j] == b'[' {
                                        // 找到配对的 ']'
                                        if let Some(close_rel) = s[j + 1..].find(']') {
                                            let mut start = j + 1 + close_rel + 1; // 右括号后一位
                                            if start < n && bytes[start] == b' ' {
                                                start += 1;
                                            }
                                            match &mut f.payload {
                                                RawData::Bytes(b) => {
                                                    if start <= b.len() {
                                                        *b = b.slice(start..n);
                                                    }
                                                }
                                                RawData::String(st) => {
                                                    if start <= st.len() {
                                                        *st = st[start..].to_string();
                                                    }
                                                }
                                                RawData::ArcBytes(arc_b) => {
                                                    // Convert ArcBytes to Bytes for modification
                                                    if start <= arc_b.len() {
                                                        let new_bytes =
                                                            bytes::Bytes::copy_from_slice(&arc_b[start..]);
                                                        f.payload = RawData::Bytes(new_bytes);
                                                    }
                                                }
                                            }
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        // 1) RFC3164 常见模式：在第一个 ": " 之后即为消息体（确保位于 PRI '>' 之后）
                        if let Some(col) = s.find(": ")
                            && let Some(gt) = s.find('>')
                            && col > gt
                        {
                            let start = col + 2;
                            match &mut f.payload {
                                RawData::Bytes(b) => {
                                    let len = b.len();
                                    if start <= len {
                                        *b = b.slice(start..len);
                                    }
                                }
                                RawData::String(st) => {
                                    if start <= st.len() {
                                        *st = st[start..].to_string();
                                    }
                                }
                                RawData::ArcBytes(arc_b) => {
                                    // Convert ArcBytes to Bytes for modification
                                    let len = arc_b.len();
                                    if start <= len {
                                        let new_bytes = bytes::Bytes::copy_from_slice(&arc_b[start..]);
                                        f.payload = RawData::Bytes(new_bytes);
                                    }
                                }
                            }
                            return;
                        }
                        // 2) 兼容历史生成样本的快剪：遇到 " wpgen: " 直接裁剪其后
                        if let Some(pos) = s.find(" wpgen: ") {
                            let start = pos + 8;
                            match &mut f.payload {
                                RawData::Bytes(b) => {
                                    let len = b.len();
                                    if start <= len {
                                        *b = b.slice(start..len);
                                    }
                                }
                                RawData::String(st) => {
                                    if start <= st.len() {
                                        *st = st[start..].to_string();
                                    }
                                }
                                RawData::ArcBytes(arc_b) => {
                                    // Convert ArcBytes to Bytes for modification
                                    let len = arc_b.len();
                                    if start <= len {
                                        let new_bytes = bytes::Bytes::copy_from_slice(&arc_b[start..]);
                                        f.payload = RawData::Bytes(new_bytes);
                                    }
                                }
                            }
                            return;
                        }
                    }
                    if fast && strip && !attach {
                        trace_data!(
                            "syslog fast strip fallback (key={}, preview='{}')",
                            f.src_key,
                            Self::syslog_preview(s)
                        );
                    }
                    let ns = normalize::normalize_slice(s);
                    if attach {
                        let tags = Arc::make_mut(&mut f.tags);
                        if let Some(pri) = ns.meta.pri {
                            tags.set("syslog.pri", pri.to_string());
                        }
                        if let Some(ref fac) = ns.meta.facility {
                            tags.set("syslog.facility", fac.clone());
                        }
                        if let Some(ref sev) = ns.meta.severity {
                            tags.set("syslog.severity", sev.clone());
                        }
                    }
                    if strip {
                        if ns.msg_start >= ns.msg_end {
                            trace_data!(
                                "syslog strip produced empty span (key={}, preview='{}')",
                                f.src_key,
                                Self::syslog_preview(s)
                            );
                        }
                        match &mut f.payload {
                            RawData::Bytes(b) => {
                                let start = ns.msg_start.min(b.len());
                                let end = ns.msg_end.min(b.len());
                                if start <= end {
                                    *b = b.slice(start..end);
                                }
                            }
                            RawData::String(st) => {
                                let start = ns.msg_start.min(st.len());
                                let end = ns.msg_end.min(st.len());
                                *st = st[start..end].to_string();
                            }
                            RawData::ArcBytes(arc_b) => {
                                // Convert ArcBytes to Bytes for modification
                                let start = ns.msg_start.min(arc_b.len());
                                let end = ns.msg_end.min(arc_b.len());
                                if start <= end {
                                    let new_bytes = bytes::Bytes::copy_from_slice(&arc_b[start..end]);
                                    f.payload = RawData::Bytes(new_bytes);
                                }
                            }
                        }
                    }
                }
            }) as EventPreHook)
        } else {
            None
        }
    }

    fn syslog_preview(data: &str) -> String {
        const LIMIT: usize = 120;
        if data.chars().count() <= LIMIT {
            data.to_string()
        } else {
            let mut out = String::with_capacity(LIMIT + 3);
            for (idx, ch) in data.char_indices() {
                if idx >= LIMIT {
                    break;
                }
                out.push(ch);
            }
            out.push_str("...");
            out
        }
    }

    fn decorate_batch(&self, mut batch: SourceBatch) -> SourceBatch {
        for event in batch.iter_mut() {
            // 仅在需要附加元标签时注入 access_ip，避免每条事件克隆 Tags
            if self.attach_meta_tags
                && let Some(ip) = event.ups_ip
            {
                Arc::make_mut(&mut event.tags).set("access_ip", ip.to_string());
            }
            // 复用预处理闭包，降低分配
            if self.strip_header || self.attach_meta_tags {
                event.preproc = self.preproc_hook.clone();
            }
        }
        batch
    }

    /// 基于零拷贝消息构造 SourceEvent，附加 access_ip/ups_ip 元信息
    pub fn build_zero_copy_frame(&self, msg: ZcpMessage) -> SourceEvent {
        let client_ip = msg.client_ip();
        let payload = RawData::ArcBytes(msg.into_payload_arc());
        let stags = self.base_source_tags();
        //stags.set("access_ip".to_string(), access_ip.clone());

        let mut event = SourceEvent::new(next_event_id(), &self.key, payload, Arc::new(stags));
        event.ups_ip = Some(client_ip);
        if self.strip_header || self.attach_meta_tags {
            event.preproc = self.preproc_hook.clone();
        }
        event
    }

    /// RFC6587 分帧辅助：供单元测试/诊断复现 framing 行为
    pub async fn process_buffer(
        buffer: &mut BytesMut,
        data: &[u8],
        client_ip: &str,
        sender: &Sender<Message>,
    ) -> SourceResult<()> {
        if !data.is_empty() {
            buffer.extend_from_slice(data);
        }

        let client_ip = Arc::<str>::from(client_ip);
        while let Some(pending) = framing::drain_auto_all(buffer, &client_ip, sender).await? {
            sender.send(pending).await.map_err(|e| {
                SourceError::from(SourceReason::Disconnect(format!(
                    "syslog framing channel closed: {}",
                    e
                )))
            })?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl DataSource for TcpSyslogSource {
    async fn receive(&mut self) -> SourceResult<SourceBatch> {
        let batch = self.inner.receive().await?;
        Ok(self.decorate_batch(batch))
    }

    fn try_receive(&mut self) -> Option<SourceBatch> {
        self.inner
            .try_receive()
            .map(|batch| self.decorate_batch(batch))
    }

    fn can_try_receive(&mut self) -> bool {
        self.inner.can_try_receive()
    }

    fn identifier(&self) -> String {
        self.key.clone()
    }

    async fn start(&mut self, ctrl_rx: CtrlRx) -> SourceResult<()> {
        self.inner.start(ctrl_rx).await
    }

    async fn close(&mut self) -> SourceResult<()> {
        self.inner.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::syslog::{EmitMessage, SyslogEncoder};

    #[tokio::test]
    async fn test_syslog_zero_copy_frame_builder() {
        // Build a minimal inner tcp aggregator
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test_syslog_zero_copy".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            65536,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let tcp_syslog = TcpSyslogSource::new(
            "test_syslog_zero_copy".to_string(),
            Tags::default(),
            false, // strip_header
            false, // attach_meta_tags
            false, // fast_strip
            inner,
        )
        .await
        .unwrap();

        // 创建测试消息
        let zcp_msg = ZcpMessage::new(
            b"192.168.1.200",
            b"<34>Oct 22 10:52:12 myhost test message".to_vec(),
        );

        // 使用零拷贝帧构建器
        let event = tcp_syslog.build_zero_copy_frame(zcp_msg);

        // 验证事件属性
        assert_eq!(event.src_key.as_str(), "test_syslog_zero_copy");
        assert!(event.ups_ip.is_some());

        match event.payload {
            RawData::Bytes(data) => {
                let message_str = String::from_utf8_lossy(&data);
                assert!(message_str.contains("test message"));
            }
            RawData::ArcBytes(data) => {
                let message_str = String::from_utf8_lossy(&data);
                assert!(message_str.contains("test message"));
            }
            RawData::String(s) => {
                assert!(s.contains("test message"));
            }
        }
    }

    #[tokio::test]
    async fn test_syslog_zero_copy_frame_with_meta() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test_syslog_meta".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            65536,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let tcp_syslog = TcpSyslogSource::new(
            "test_syslog_meta".to_string(),
            Tags::default(),
            false, // strip_header
            true,  // attach_meta_tags
            false, // fast_strip
            inner,
        )
        .await
        .unwrap();

        // 创建带有syslog优先级的消息
        let zcp_msg = ZcpMessage::new(
            b"10.0.0.1",
            b"<13>Oct 22 10:52:12 myhost test message with priority".to_vec(),
        );

        let event = tcp_syslog.build_zero_copy_frame(zcp_msg);

        // 验证事件属性
        assert_eq!(event.src_key.as_str(), "test_syslog_meta");

        match event.payload {
            RawData::Bytes(data) => {
                let message_str = String::from_utf8_lossy(&data);
                assert!(message_str.contains("test message with priority"));
            }
            RawData::ArcBytes(data) => {
                let message_str = String::from_utf8_lossy(&data);
                assert!(message_str.contains("test message with priority"));
            }
            RawData::String(s) => {
                assert!(s.contains("test message with priority"));
            }
        }
    }

    #[tokio::test]
    async fn test_syslog_zero_copy_frame_invalid_ip() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test_syslog_invalid_ip".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            65536,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let tcp_syslog = TcpSyslogSource::new(
            "test_syslog_invalid_ip".to_string(),
            Tags::default(),
            false,
            false,
            false,
            inner,
        )
        .await
        .unwrap();

        // 创建包含无效UTF-8的IP
        let invalid_client_ip = &[0xFF, 0xFE, 0xFD];
        let zcp_msg = ZcpMessage::new(
            invalid_client_ip,
            b"<14>Oct 22 10:52:12 myhost test".to_vec(),
        );

        let event = tcp_syslog.build_zero_copy_frame(zcp_msg);

        // 验证无效IP被正确处理
        assert_eq!(event.src_key.as_str(), "test_syslog_invalid_ip");
    }

    #[tokio::test]
    async fn preproc_strips_header() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test-strip".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            4096,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let source = TcpSyslogSource::new(
            "test-strip".to_string(),
            Tags::default(),
            true,
            true,
            false,
            inner,
        )
        .await
        .unwrap();

        let mut pre_event = SourceEvent::new(
            next_event_id(),
            "syslog",
            RawData::String("<13>Oct 11 22:14:15 host app: body".into()),
            Arc::new(Tags::new()),
        );
        let hook = source.build_preproc_hook().unwrap();
        hook(&mut pre_event);
        assert_eq!(pre_event.payload.to_string(), "body");
        assert_eq!(pre_event.tags.get("syslog.pri"), Some("13"));
    }

    #[tokio::test]
    async fn fast_strip_rfc3164_quick_path() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test-fast3164".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            4096,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        // strip_header=true, attach_meta_tags=false, fast_strip=true → 触发快路径
        let source = TcpSyslogSource::new(
            "test-fast3164".to_string(),
            Tags::default(),
            true,
            false,
            true,
            inner,
        )
        .await
        .unwrap();

        let mut pre_event = SourceEvent::new(
            next_event_id(),
            "syslog",
            RawData::String("<34>Oct 11 22:14:15 mymachine app: hello world".into()),
            Arc::new(Tags::new()),
        );
        let hook = source.build_preproc_hook().unwrap();
        hook(&mut pre_event);
        assert_eq!(pre_event.payload.to_string(), "hello world");
    }

    #[tokio::test]
    async fn syslog_sink_source_roundtrip_strips_header() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "roundtrip".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            4096,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let source = TcpSyslogSource::new(
            "roundtrip".to_string(),
            Tags::default(),
            true,
            true,
            true,
            inner,
        )
        .await
        .unwrap();

        let body = "monitor payload";
        let mut emit = EmitMessage::new(body);
        emit.priority = 13;
        emit.hostname = Some("unit-host");
        emit.app_name = Some("wpgen");
        emit.append_newline = true;
        let encoder = SyslogEncoder::new();
        let encoded = encoder.encode_rfc3164(&emit);

        let mut pre_event = SourceEvent::new(
            next_event_id(),
            "syslog",
            RawData::String(String::from_utf8(encoded.to_vec()).unwrap()),
            Arc::new(Tags::new()),
        );
        let hook = source.build_preproc_hook().unwrap();
        hook(&mut pre_event);

        let stripped = match &pre_event.payload {
            RawData::String(s) => s.clone(),
            RawData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            RawData::ArcBytes(b) => String::from_utf8_lossy(b).to_string(),
        };
        assert!(
            stripped.contains(body),
            "payload missing body: {}",
            stripped
        );
        assert!(stripped.ends_with('\n'));
        assert_eq!(pre_event.tags.get("syslog.pri"), Some("13"));
    }

    #[tokio::test]
    async fn fast_strip_rfc5424_quick_path() {
        let pool = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let (_tx, rx) = tokio::sync::mpsc::channel(8);
        let inner = TcpSource::new(
            "test-fast5424".to_string(),
            Tags::default(),
            "127.0.0.1:0".to_string(),
            4096,
            crate::sources::tcp::FramingMode::Line,
            pool,
            rx,
        )
        .unwrap();
        let source = TcpSyslogSource::new(
            "test-fast5424".to_string(),
            Tags::default(),
            true,  // strip
            false, // !attach
            true,  // fast_strip
            inner,
        )
        .await
        .unwrap();

        let mut pre_event = SourceEvent::new(
            next_event_id(),
            "syslog",
            RawData::String("<14>1 2024-10-05T12:34:56Z host app 123 - - hello world".into()),
            Arc::new(Tags::new()),
        );
        let hook = source.build_preproc_hook().unwrap();
        hook(&mut pre_event);
        assert_eq!(pre_event.payload.to_string(), "hello world");
    }
}
