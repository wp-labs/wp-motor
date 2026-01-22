//! Lightweight syslog codec facade used by sources and sinks.
//!
//! The goal of this module is to expose a small, well-tested API that wraps the
//! existing normalization logic (`sources::syslog::normalize`).  By centralizing
//! encode/decode helpers here we avoid duplicating parsing logic inside every
//! source implementation.

mod decoder;
mod encoder;
mod message;

#[allow(unused_imports)]
pub use decoder::{DecodeError, SyslogDecoder};
#[allow(unused_imports)]
pub use encoder::{EmitMessage, SyslogEncoder};
#[allow(unused_imports)]
pub use message::SyslogFrame;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode_roundtrip() {
        let encoder = SyslogEncoder::new();
        let mut emit = EmitMessage::new("roundtrip message");
        emit.priority = 34;
        emit.hostname = Some("host1");
        emit.app_name = Some("app1");
        emit.append_newline = false;

        let encoded = encoder.encode_rfc3164(&emit);
        let decoder = SyslogDecoder;
        let frame = decoder.decode_bytes(encoded).expect("decode");
        assert_eq!(frame.message_str(), Some("roundtrip message"));
        assert_eq!(frame.meta().pri, Some(34));
    }
}
