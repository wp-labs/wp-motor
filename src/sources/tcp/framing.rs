//! 分帧工具与常量

use bytes::{Bytes, BytesMut};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use wp_connector_api::{SourceError, SourceReason, SourceResult};

// 默认配置：与 syslog 源保持一致的接收缓存与通道容量
pub const DEFAULT_TCP_RECV_BYTES: usize = 10_485_760; // 10 MiB
pub const STOP_CHANNEL_CAPACITY: usize = 2;

// Octet-counting 相关限制
const MAX_LEN_DIGITS: usize = 10; // 长度前缀最多 10 位十进制
const MAX_FRAME_BYTES: usize = 10_000_000; // 单帧 10MB 上限

/// 事件消息：client-ip + payload
pub type Message = (Arc<str>, Bytes);
/// 批消息类型（微批）：使用 Vec，避免引入新依赖
pub type MessageBatch = Vec<Message>;

/// 分帧模式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramingMode {
    /// 自动选择：默认优先 length 前缀（若 length 不在进行中则回退到按行）
    Auto,
    /// 按换行分帧
    Line,
    /// 按 RFC6587 风格 length 前缀分帧（`<len> <payload>`）
    Len,
}

// Provide compatibility extractor struct API for modules expecting `FramingExtractor`
pub mod extractor;
pub use extractor::FramingExtractor;

/// 是否处于 length 前缀进行中的状态（即已看到 `<digits><space>`，但 payload 未收齐）
pub fn octet_in_progress(buf: &BytesMut) -> bool {
    if buf.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < buf.len() && i < MAX_LEN_DIGITS {
        if !buf[i].is_ascii_digit() {
            break;
        }
        i += 1;
    }
    if i == 0 || i >= buf.len() {
        return false;
    }
    if buf[i] != b' ' {
        return false;
    }
    let Some(n) = std::str::from_utf8(&buf[..i])
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    else {
        return false;
    };
    // 只在合理范围内才认为是进行中的长度前缀；过大的 n 直接视为无效 → 允许回退到按行
    if n == 0 || n >= MAX_FRAME_BYTES {
        return false;
    }
    buf.len() < i + 1 + n
}

/// 提取一条按行分帧的消息（丢弃行末 CR/space/tab），无换行时返回 None
pub fn extract_newline(buf: &mut BytesMut) -> Option<Bytes> {
    let nl = buf.iter().position(|&b| b == b'\n')?;
    let mut chunk = buf.split_to(nl + 1);
    let mut end = nl;
    while end > 0 {
        match chunk[end - 1] {
            b'\r' | b' ' | b'\t' => end -= 1,
            _ => break,
        }
    }
    Some(chunk.split_to(end).freeze())
}

/// 提取一条按长度前缀分帧的消息；不足时返回 None
pub fn extract_octet_counted(buf: &mut BytesMut) -> Option<Bytes> {
    if buf.is_empty() {
        return None;
    }
    let mut i = 0;
    while i < buf.len() && i < MAX_LEN_DIGITS {
        if !buf[i].is_ascii_digit() {
            break;
        }
        i += 1;
    }
    if i == 0 || i >= buf.len() {
        return None;
    }
    if buf[i] != b' ' {
        return None;
    }
    let length_str = std::str::from_utf8(&buf[..i]).ok()?;
    let msg_len = length_str.parse::<usize>().ok()?;
    if msg_len == 0 || msg_len >= MAX_FRAME_BYTES {
        return None;
    }
    let total = i + 1 + msg_len;
    if buf.len() < total {
        return None;
    }
    let _ = buf.split_to(i + 1); // 丢弃 "<len> "
    let msg = buf.split_to(msg_len);
    Some(msg.freeze())
}

/// 按行持续抽取并发送
/// 尝试尽可能多地发送按行分帧的消息；通道满时返回首条未发送的消息，交由上层决定处理策略
// 旧行为（发送版）保留在 drain_* 中；新引入 collect_* 用于批量收集
pub async fn drain_by_line(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    sender: &Sender<Message>,
) -> Option<Message> {
    while let Some(line) = extract_newline(buf) {
        if sender.try_send((client_ip.clone(), line.clone())).is_err() {
            // 注意：此处 line 已从 buf 中抽取；为避免丢失，将其作为 pending 返回给调用方
            return Some((client_ip.clone(), line));
        }
    }
    None
}

/// 按长度前缀持续抽取并发送
pub async fn drain_by_len(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    sender: &Sender<Message>,
) -> Option<Message> {
    while let Some(msg) = extract_octet_counted(buf) {
        if sender.try_send((client_ip.clone(), msg.clone())).is_err() {
            return Some((client_ip.clone(), msg));
        }
    }
    None
}

/// auto 模式下的“尽可能多” drain：先尝试 length → 若 length 不在进行中则回退到按行
///
/// 对齐 syslog 的成熟实现：包含缓冲溢出防护（10MB）并在溢出时清空缓冲并返回错误。
pub async fn drain_auto_all(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    sender: &Sender<Message>,
) -> SourceResult<Option<Message>> {
    loop {
        if buf.is_empty() {
            break;
        }

        if let Some(msg) = extract_octet_counted(buf) {
            if sender.try_send((client_ip.clone(), msg.clone())).is_err() {
                return Ok(Some((client_ip.clone(), msg)));
            }
            continue;
        }
        if octet_in_progress(buf) {
            break;
        }
        if let Some(line) = extract_newline(buf) {
            if !line.is_empty() && sender.try_send((client_ip.clone(), line.clone())).is_err() {
                return Ok(Some((client_ip.clone(), line)));
            }
            continue;
        }

        if buf.len() > MAX_FRAME_BYTES {
            let preview_len = buf.len().min(256);
            let preview = String::from_utf8_lossy(&buf[..preview_len]);
            warn_data!(
                "syslog framing buffer overflow (peer={}, len={} bytes, preview='{}'); dropping connection",
                client_ip,
                buf.len(),
                preview
            );
            // 与 syslog 对齐：清空并返回错误，避免 OOM
            buf.clear();
            return Err(SourceError::from(SourceReason::SupplierError(
                "buffer overflow".to_string(),
            )));
        }
        break;
    }
    Ok(None)
}

/// 收集按行分帧的消息到 out，最多收集 `max_collect` 条
pub fn collect_by_line(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    out: &mut MessageBatch,
    max_collect: usize,
) {
    while out.len() < max_collect {
        if let Some(line) = extract_newline(buf) {
            out.push((client_ip.clone(), line));
        } else {
            break;
        }
    }
}

/// 收集按长度前缀分帧的消息到 out，最多收集 `max_collect` 条
pub fn collect_by_len(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    out: &mut MessageBatch,
    max_collect: usize,
) {
    while out.len() < max_collect {
        if let Some(msg) = extract_octet_counted(buf) {
            out.push((client_ip.clone(), msg));
        } else {
            break;
        }
    }
}

/// auto 模式下尽可能多地收集（受 `max_collect` 限制）
pub fn collect_auto_all(
    buf: &mut BytesMut,
    client_ip: &Arc<str>,
    out: &mut MessageBatch,
    max_collect: usize,
) -> SourceResult<()> {
    loop {
        if buf.is_empty() || out.len() >= max_collect {
            break;
        }
        if let Some(msg) = extract_octet_counted(buf) {
            out.push((client_ip.clone(), msg));
            continue;
        }
        if octet_in_progress(buf) {
            break;
        }
        if let Some(line) = extract_newline(buf) {
            if !line.is_empty() {
                out.push((client_ip.clone(), line));
            }
            continue;
        }
        if buf.len() > MAX_FRAME_BYTES {
            let preview_len = buf.len().min(256);
            let preview = String::from_utf8_lossy(&buf[..preview_len]);
            warn_data!(
                "syslog framing buffer overflow (peer={}, len={} bytes, preview='{}'); dropping connection",
                client_ip,
                buf.len(),
                preview
            );
            buf.clear();
            return Err(SourceError::from(SourceReason::SupplierError(
                "buffer overflow".to_string(),
            )));
        }
        break;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;

    #[test]
    fn newline_extracts_and_trims() {
        let mut b = BytesMut::from(&b"abc \t\r\nrest"[..]);
        let first = extract_newline(&mut b).unwrap();
        assert_eq!(&first[..], b"abc");
        assert_eq!(&b[..], b"rest");
    }

    #[test]
    fn octet_extracts_once_complete() {
        let mut b = BytesMut::from(&b"5 hello7 good"[..]); // second frame incomplete
        let m1 = extract_octet_counted(&mut b).unwrap();
        assert_eq!(&m1[..], b"hello");
        // second frame incomplete (missing payload) → None
        assert!(extract_octet_counted(&mut b).is_none());
        assert!(octet_in_progress(&b));
    }

    #[test]
    fn drain_len_two_frames() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut b = BytesMut::from(&b"5 hello5 world"[..]);
            let (tx, mut rx) = mpsc::channel::<Message>(8);
            let ip: Arc<str> = Arc::<str>::from("127.0.0.1");
            drain_by_len(&mut b, &ip, &tx).await;
            let m1 = rx.recv().await.unwrap();
            let m2 = rx.recv().await.unwrap();
            assert_eq!(&m1.1[..], b"hello");
            assert_eq!(&m2.1[..], b"world");
            assert!(rx.try_recv().is_err());
        });
    }

    #[test]
    fn auto_prefers_len_then_line() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut b = BytesMut::from(&b"5 hello\n"[..]);
            let (tx, mut rx) = mpsc::channel::<Message>(1);
            let ip: Arc<str> = Arc::<str>::from("127.0.0.1");
            drain_auto_all(&mut b, &ip, &tx).await.unwrap();
            let m = rx.recv().await.unwrap();
            assert_eq!(&m.1[..], b"hello");

            // fallback to newline when no length prefix
            let mut b_line = BytesMut::from(&b"abc\n"[..]);
            let (tx_line, mut rx_line) = mpsc::channel::<Message>(1);
            drain_auto_all(&mut b_line, &ip, &tx_line).await.unwrap();
            let line_msg = rx_line.recv().await.unwrap();
            assert_eq!(&line_msg.1[..], b"abc");
        });
    }

    #[test]
    fn auto_waits_when_len_in_progress() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut b = BytesMut::from(&b"7 incom"[..]); // incomplete length frame
            let (tx, mut rx) = mpsc::channel::<Message>(1);
            let ip: Arc<str> = Arc::<str>::from("127.0.0.1");
            drain_auto_all(&mut b, &ip, &tx).await.unwrap();
            assert!(rx.try_recv().is_err());
        });
    }
}
