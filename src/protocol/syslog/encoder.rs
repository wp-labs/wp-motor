use bytes::Bytes;
use chrono::{DateTime, Local, Utc};

/// Static description of a syslog line used for encoding.
#[derive(Debug, Clone)]
pub struct EmitMessage<'a> {
    pub priority: u8,
    pub hostname: Option<&'a str>,
    pub app_name: Option<&'a str>,
    pub message: &'a str,
    pub timestamp: Option<DateTime<Utc>>,
    pub append_newline: bool,
}

impl<'a> EmitMessage<'a> {
    pub fn new(message: &'a str) -> Self {
        Self {
            priority: 13,
            hostname: None,
            app_name: None,
            message,
            timestamp: None,
            append_newline: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SyslogEncoder;

impl SyslogEncoder {
    pub fn new() -> Self {
        Self
    }

    /// Encode a simple RFC3164 line.
    pub fn encode_rfc3164(&self, msg: &EmitMessage<'_>) -> Bytes {
        let ts = msg
            .timestamp
            .unwrap_or_else(Utc::now)
            .with_timezone(&Local)
            .format("%b %d %H:%M:%S")
            .to_string();
        let hostname = msg.hostname.unwrap_or("localhost");
        let app_name = msg.app_name.unwrap_or("wp-engine");
        let mut line = format!(
            "<{}>{} {} {}: {}",
            msg.priority, ts, hostname, app_name, msg.message
        );
        if msg.append_newline && !line.ends_with('\n') {
            line.push('\n');
        }
        Bytes::from(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_default_line() {
        let encoder = SyslogEncoder::new();
        let msg = EmitMessage::new("hello world");
        let encoded = encoder.encode_rfc3164(&msg);
        let text = String::from_utf8(encoded.to_vec()).unwrap();
        assert!(text.contains("hello world"));
        assert!(text.contains("<13>"));
    }
}
