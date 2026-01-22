use bytes::Bytes;
use std::ops::Range;
use thiserror::Error;

use crate::sources::syslog::normalize;

use super::message::SyslogFrame;

/// Errors returned by [`SyslogDecoder`].
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("empty syslog payload")]
    Empty,
    #[error("syslog payload is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}

/// Minimal decoder that leverages the mature normalization logic already used
/// by the TCP/UDP syslog sources.
#[derive(Debug, Default, Clone, Copy)]
pub struct SyslogDecoder;

impl SyslogDecoder {
    /// Decode a `Bytes` buffer into a [`SyslogFrame`].
    pub fn decode_bytes(&self, raw: Bytes) -> Result<SyslogFrame, DecodeError> {
        if raw.is_empty() {
            return Err(DecodeError::Empty);
        }
        let text = std::str::from_utf8(&raw)?;
        let slice = normalize::normalize_slice(text);
        let msg_end = slice.msg_end.min(raw.len());
        let msg_start = slice.msg_start.min(msg_end);
        let range = Range {
            start: msg_start,
            end: msg_end,
        };
        Ok(SyslogFrame::new(raw, range, slice.meta))
    }

    /// Convenience helper for callers that only have a byte slice.
    #[allow(dead_code)]
    pub fn decode_slice(&self, data: &[u8]) -> Result<SyslogFrame, DecodeError> {
        self.decode_bytes(Bytes::copy_from_slice(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_simple_rfc3164() {
        let decoder = SyslogDecoder;
        let input = Bytes::from_static(b"<13>Oct 11 22:14:15 mymachine su: hello world");
        let frame = decoder.decode_bytes(input).expect("decode");
        assert_eq!(frame.message_str(), Some("hello world"));
        assert_eq!(frame.meta().pri, Some(13));
    }

    #[test]
    fn decode_invalid_utf8() {
        let decoder = SyslogDecoder;
        let input = Bytes::from_static(&[0xff, 0xfe, 0xfd]);
        let err = decoder.decode_bytes(input).unwrap_err();
        matches!(err, DecodeError::InvalidUtf8(_));
    }
}
