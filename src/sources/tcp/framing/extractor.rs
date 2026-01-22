use bytes::{Buf, Bytes, BytesMut};
use memchr::memchr;

/// Message framing extractor (line, length-prefixed RFC6587)
pub struct FramingExtractor;

impl FramingExtractor {
    /// Extract line-separated message from buffer (drops trailing \n and any \r)
    pub fn extract_line_message(buf: &mut BytesMut) -> Option<Bytes> {
        let newline_pos = memchr(b'\n', buf.as_ref())?;
        let mut chunk = buf.split_to(newline_pos + 1);
        chunk.truncate(newline_pos);
        if let Some(first_cr) = memchr(b'\r', chunk.as_ref()) {
            let mut write_idx = first_cr;
            for read_idx in (first_cr + 1)..chunk.len() {
                let byte = chunk[read_idx];
                if byte != b'\r' {
                    chunk[write_idx] = byte;
                    write_idx += 1;
                }
            }
            chunk.truncate(write_idx);
        }
        Some(chunk.freeze())
    }

    /// Extract length-prefixed message from buffer: "<len><SP><payload>"
    pub fn extract_length_prefixed_message(buf: &mut BytesMut) -> Option<Bytes> {
        let mut pos = 0;
        let mut len = 0;
        while pos < buf.len() && pos < 10 && buf[pos].is_ascii_digit() {
            len = len * 10 + (buf[pos] - b'0') as usize;
            pos += 1;
        }
        if pos == 0 || pos >= buf.len() || buf[pos] != b' ' {
            return None;
        }
        pos += 1;
        if pos + len > buf.len() {
            return None;
        }
        buf.advance(pos);
        let payload = buf.split_to(len);
        Some(payload.freeze())
    }

    pub fn has_length_prefix(buf: &BytesMut) -> bool {
        let mut pos = 0;
        let mut found_digit = false;
        while pos < buf.len() && pos < 10 {
            if buf[pos].is_ascii_digit() {
                found_digit = true;
                pos += 1;
            } else if buf[pos] == b' ' && found_digit {
                return true;
            } else {
                break;
            }
        }
        false
    }

    pub fn has_newline(buf: &BytesMut) -> bool {
        buf.contains(&b'\n')
    }
}

#[cfg(test)]
mod tests {
    use super::FramingExtractor;
    use bytes::{Buf, Bytes, BytesMut};

    // Line message extraction tests
    #[test]
    fn test_extract_line_message() {
        let mut buf = BytesMut::from("hello world\n");
        let result = FramingExtractor::extract_line_message(&mut buf);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from("hello world"));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_extract_line_message_with_crlf() {
        let mut buf = BytesMut::from("hello world\r\n");
        let result = FramingExtractor::extract_line_message(&mut buf);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from("hello world"));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_extract_line_message_no_newline() {
        let mut buf = BytesMut::from("hello world");
        let result = FramingExtractor::extract_line_message(&mut buf);

        assert!(result.is_none());
        assert_eq!(buf, BytesMut::from("hello world"));
    }

    #[test]
    fn test_extract_line_message_multiple_lines() {
        let mut buf = BytesMut::from("first\nsecond\n");

        let result1 = FramingExtractor::extract_line_message(&mut buf);
        assert_eq!(result1.unwrap(), Bytes::from("first"));

        let result2 = FramingExtractor::extract_line_message(&mut buf);
        assert_eq!(result2.unwrap(), Bytes::from("second"));

        assert_eq!(buf.len(), 0);
    }

    // Length-prefixed message extraction tests
    #[test]
    fn test_extract_length_prefixed_message() {
        let mut buf = BytesMut::from("5 hello");
        let result = FramingExtractor::extract_length_prefixed_message(&mut buf);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from("hello"));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_extract_length_prefixed_message_multi_digit() {
        let payload = "multi digit payload";
        let mut buf = BytesMut::from(&format!("{} {}", payload.len(), payload)[..]);
        let result = FramingExtractor::extract_length_prefixed_message(&mut buf);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from(payload));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_extract_length_prefixed_message_zero_length() {
        let mut buf = BytesMut::from("0 ");
        let result = FramingExtractor::extract_length_prefixed_message(&mut buf);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from(""));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_extract_length_prefixed_message_invalid() {
        let mut buf1 = BytesMut::from("5hello"); // missing space
        let mut buf2 = BytesMut::from("a hello"); // non-digit
        let mut buf3 = BytesMut::from("10 hello"); // incomplete payload

        assert!(FramingExtractor::extract_length_prefixed_message(&mut buf1).is_none());
        assert!(FramingExtractor::extract_length_prefixed_message(&mut buf2).is_none());
        assert!(FramingExtractor::extract_length_prefixed_message(&mut buf3).is_none());
    }

    // Helper function tests
    #[test]
    fn test_has_length_prefix() {
        assert!(FramingExtractor::has_length_prefix(&BytesMut::from(
            "5 hello"
        )));
        assert!(FramingExtractor::has_length_prefix(&BytesMut::from("0 ")));

        assert!(!FramingExtractor::has_length_prefix(&BytesMut::from("")));
        assert!(!FramingExtractor::has_length_prefix(&BytesMut::from(
            "hello"
        )));
        assert!(!FramingExtractor::has_length_prefix(&BytesMut::from(
            "5hello"
        )));
    }

    #[test]
    fn test_has_newline() {
        assert!(FramingExtractor::has_newline(&BytesMut::from("hello\n")));
        assert!(FramingExtractor::has_newline(&BytesMut::from("hello\r\n")));

        assert!(!FramingExtractor::has_newline(&BytesMut::from("")));
        assert!(!FramingExtractor::has_newline(&BytesMut::from("hello")));
        assert!(!FramingExtractor::has_newline(&BytesMut::from("hello\r")));
    }

    // Integration test
    #[test]
    fn test_mixed_scenarios() {
        // Test binary data
        let mut data = Vec::new();
        data.extend_from_slice(b"5 ");
        data.extend_from_slice(&[0x00, 0x01, 0x02, 0xFF, 0xFE]);
        let mut buf = BytesMut::from(&data[..]);
        let result = FramingExtractor::extract_length_prefixed_message(&mut buf);
        assert_eq!(
            result.unwrap(),
            Bytes::from(&[0x00, 0x01, 0x02, 0xFF, 0xFE][..])
        );

        // Test partial buffer with newline
        let mut buf = BytesMut::from("5 hello\nworld\n");
        let msg1 = FramingExtractor::extract_length_prefixed_message(&mut buf).unwrap();
        assert_eq!(msg1, Bytes::from("hello"));
        // After length-prefixed extraction, buffer should be "\nworld\n"
        assert!(buf.starts_with(b"\n"));
        buf.advance(1); // Skip the newline
        let msg2 = FramingExtractor::extract_line_message(&mut buf).unwrap();
        assert_eq!(msg2, Bytes::from("world"));
        assert_eq!(buf.len(), 0);
    }
}
