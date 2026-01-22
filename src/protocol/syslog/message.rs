use bytes::Bytes;
use std::ops::Range;

use crate::sources::syslog::normalize::SyslogMeta;

/// Lightweight view of a syslog line.
#[derive(Debug, Clone)]
pub struct SyslogFrame {
    raw: Bytes,
    message_range: Range<usize>,
    #[allow(dead_code)]
    meta: SyslogMeta,
}

impl SyslogFrame {
    pub(crate) fn new(raw: Bytes, message_range: Range<usize>, meta: SyslogMeta) -> Self {
        Self {
            raw,
            message_range,
            meta,
        }
    }

    /// Access the full raw payload.
    #[allow(dead_code)]
    pub fn raw(&self) -> &Bytes {
        &self.raw
    }

    /// Byte slice pointing at the message body (no header stripping).
    pub fn message_bytes(&self) -> &[u8] {
        &self.raw[self.message_range.clone()]
    }

    /// Message body as UTF-8 string. Returns `None` if the slice cannot be
    /// interpreted as UTF-8 (should not happen because decoding already
    /// validated the entire line).
    #[cfg(test)]
    pub fn message_str(&self) -> Option<&str> {
        std::str::from_utf8(self.message_bytes()).ok()
    }

    #[cfg(test)]
    pub fn meta(&self) -> &SyslogMeta {
        &self.meta
    }
}
