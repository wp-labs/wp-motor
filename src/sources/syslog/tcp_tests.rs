// Tests for TCP framing logic
#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use tokio::sync::mpsc::channel;

    use crate::sources::syslog::{TcpSyslogSource, constants::Message};

    #[tokio::test]
    async fn test_framing_octet_counting() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Test octet counting: "34 <165>1 2023-01-01T00:00:00Z msg"
        let msg = "<165>1 2023-01-01T00:00:00Z msg";
        let frame = format!("{} {}", msg.len(), msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (ip, received) = rx.recv().await.unwrap();
        assert_eq!(ip.as_ref(), "127.0.0.1");
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_framing_newline_delimiter() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Test newline framing
        let msg = "<165>1 2023-01-01T00:00:00Z simple message";
        let frame = format!("{}\n", msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "192.168.1.1", &tx)
            .await
            .unwrap();

        let (ip, received) = rx.recv().await.unwrap();
        assert_eq!(ip.as_ref(), "192.168.1.1");
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_framing_message_with_newlines() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Test message containing newlines using octet counting
        let msg = "<165>1 2023-01-01T00:00:00Z - - - msg with\nnewlines\n";
        let frame = format!("{} {}", msg.len(), msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "10.0.0.1", &tx)
            .await
            .unwrap();

        let (ip, received) = rx.recv().await.unwrap();
        assert_eq!(ip.as_ref(), "10.0.0.1");
        assert_eq!(received, msg);
        // Verify newlines are preserved
        assert!(received.contains(&b'\n'));
    }

    #[tokio::test]
    async fn test_framing_partial_messages() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Send partial octet-counted message
        let msg = "<165>1 2023-01-01T00:00:00Z complete";
        let frame = format!("{} {}", msg.len(), msg);

        // Split the frame into two parts
        let (part1, part2) = frame.split_at(15);

        // Send first part - should not receive anything yet
        TcpSyslogSource::process_buffer(&mut buffer, part1.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        assert!(rx.try_recv().is_err()); // No complete message yet

        // Send second part - should receive the complete message
        TcpSyslogSource::process_buffer(&mut buffer, part2.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_framing_multiple_messages() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Send two messages in one buffer
        let msg1 = "<165>1 2023-01-01T00:00:00Z first";
        let msg2 = "<165>1 2023-01-01T00:00:01Z second";
        let frame = format!("{} {}{} {}", msg1.len(), msg1, msg2.len(), msg2);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received1) = rx.recv().await.unwrap();
        assert_eq!(received1, msg1);

        let (_, received2) = rx.recv().await.unwrap();
        assert_eq!(received2, msg2);
    }

    // ===== Edge Cases and Boundary Conditions =====

    #[tokio::test]
    async fn test_framing_empty_lines_newline_delimiter() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Empty lines should be ignored
        let frame = "\n\n\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should not receive any messages
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_framing_whitespace_only_lines() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Lines with only whitespace should be ignored after trimming
        let frame = "   \n\t\t\n  \n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should not receive any messages
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_framing_carriage_return_handling() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // CRLF line endings should be handled correctly
        let msg = "<165>1 2023-01-01T00:00:00Z test";
        let frame = format!("{}\r\n", msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        // Should trim \r as well
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_framing_zero_length_message() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Zero-length octet count should be ignored (invalid per RFC 6587)
        let frame = "0 ";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should not receive any messages (zero-length is invalid)
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_framing_large_valid_message() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Large message (1MB)
        let msg = "X".repeat(1_000_000);
        let frame = format!("{} {}", msg.len(), msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received.len(), 1_000_000);
        assert_eq!(received, msg);
    }

    // ===== Mixed Framing Methods =====

    #[tokio::test]
    async fn test_framing_mixed_octet_and_newline() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Mix octet-counted and newline-delimited messages
        let msg1 = "<165>1 2023-01-01T00:00:00Z octet";
        let msg2 = "<165>1 2023-01-01T00:00:01Z newline";
        let frame = format!("{} {}{}\n", msg1.len(), msg1, msg2);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received1) = rx.recv().await.unwrap();
        assert_eq!(received1, msg1);

        let (_, received2) = rx.recv().await.unwrap();
        assert_eq!(received2, msg2);
    }

    #[tokio::test]
    async fn test_framing_newline_then_octet() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Newline-delimited message followed by octet-counted
        let msg1 = "<165>1 2023-01-01T00:00:00Z newline";
        let msg2 = "<165>1 2023-01-01T00:00:01Z octet";
        let frame = format!("{}\n{} {}", msg1, msg2.len(), msg2);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received1) = rx.recv().await.unwrap();
        assert_eq!(received1, msg1);

        let (_, received2) = rx.recv().await.unwrap();
        assert_eq!(received2, msg2);
    }

    #[tokio::test]
    async fn test_framing_alternating_methods() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Alternate between octet-counted and newline-delimited
        let msg1 = "msg1";
        let msg2 = "msg2";
        let msg3 = "msg3";
        let frame = format!("{} {}{}\n{} {}", msg1.len(), msg1, msg2, msg3.len(), msg3);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        assert_eq!(rx.recv().await.unwrap().1, msg1);
        assert_eq!(rx.recv().await.unwrap().1, msg2);
        assert_eq!(rx.recv().await.unwrap().1, msg3);
    }

    // ===== Error Handling and Invalid Input =====

    #[tokio::test]
    async fn test_framing_invalid_length_prefix_non_numeric() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Invalid length prefix (contains non-digits)
        let frame = "12a <165>1 2023-01-01T00:00:00Z msg\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should fall back to newline framing
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "12a <165>1 2023-01-01T00:00:00Z msg");
    }

    #[tokio::test]
    async fn test_framing_invalid_length_prefix_negative() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Negative length is not valid (no '-' sign in digits check)
        let frame = "-10 test\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should fall back to newline framing
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "-10 test");
    }

    #[tokio::test]
    async fn test_framing_length_mismatch() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Length says 100 but message is shorter, followed by a newline message
        let frame = "100 short\nactual message\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should wait for more data for octet-counted, but we have newline messages
        // Buffer doesn't have 100 bytes after "100 ", so it should skip to newline parsing
        // Actually, this will wait for more data. Let's test this properly
        assert!(rx.try_recv().is_err()); // Still waiting for the 100 bytes
    }

    #[tokio::test]
    async fn test_framing_extremely_large_length_prefix() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Length > 10MB should be rejected
        let frame = "99999999 msg\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should fall back to newline framing since length is too large
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "99999999 msg");
    }

    #[tokio::test]
    async fn test_framing_buffer_overflow_protection() {
        let (tx, _rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Create data larger than 10MB without any valid framing
        let large_data = vec![b'X'; 11_000_000];

        let result =
            TcpSyslogSource::process_buffer(&mut buffer, &large_data, "127.0.0.1", &tx).await;

        // Should return an error due to buffer overflow
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_framing_no_space_in_length_prefix() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // No space within first 10 bytes, should use newline delimiter
        let frame = "12345678901234567890\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "12345678901234567890");
    }

    #[tokio::test]
    async fn test_octet_in_progress_does_not_fallback_newline() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // First chunk: has a valid length prefix (12) but only 6 bytes + a newline
        // Should NOT fallback to newline; must wait for more data
        let first = b"12 short\n"; // after "12 ", only 6 bytes available, need 12
        TcpSyslogSource::process_buffer(&mut buffer, first, "127.0.0.1", &tx)
            .await
            .unwrap();
        assert!(rx.try_recv().is_err());

        // Second chunk: provide the remaining 6 bytes
        let second = b"remain"; // total 12 bytes now => "short\nremain"
        TcpSyslogSource::process_buffer(&mut buffer, second, "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "short\nremain");
    }

    #[tokio::test]
    async fn test_framing_length_with_leading_zeros() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();
        let frame = "0005 hello"; // length=5, message("hello")
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "hello");
    }

    #[tokio::test]
    async fn test_zero_length_prefix_fallback_newline() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();
        let frame = "0 test\n"; // zero length is invalid => newline fallback
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "0 test");
    }

    #[tokio::test]
    async fn test_double_space_after_prefix() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();
        let frame = "6  hello"; // we skip only first space; message starts with a space (len=6)
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, " hello");
    }

    #[tokio::test]
    async fn test_prefix_longer_than_search_window_fallback_newline() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();
        // 11 digits before space -> beyond search limit => newline fallback path
        let frame = "12345678901 msg\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "12345678901 msg");
    }

    // ===== Mixed Scenarios (multi-segment + mixed framing) =====

    #[tokio::test]
    async fn test_mixed_octet_then_newline_one_buffer() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        let msg1 = "hello";
        let msg2 = "line_after";
        let frame = format!("{} {}\n{}\n", msg1.len(), msg1, msg2);
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, r1) = rx.recv().await.unwrap();
        let (_, r2) = rx.recv().await.unwrap();
        assert_eq!(r1, msg1);
        assert_eq!(r2, msg2);
    }

    #[tokio::test]
    async fn test_mixed_newline_then_octet_one_buffer() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        let m1 = "first_line";
        let m2 = "second";
        let frame = format!("{}\n{} {}", m1, m2.len(), m2);
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, r1) = rx.recv().await.unwrap();
        let (_, r2) = rx.recv().await.unwrap();
        assert_eq!(r1, m1);
        assert_eq!(r2, m2);
    }

    #[tokio::test]
    async fn test_partial_header_across_chunks_then_newline() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Build 12-byte message and a following newline message
        let m_oct = "hello world!"; // 12 bytes
        let m_nl = "after_octet";

        TcpSyslogSource::process_buffer(&mut buffer, b"1", "127.0.0.1", &tx)
            .await
            .unwrap();
        assert!(rx.try_recv().is_err());

        TcpSyslogSource::process_buffer(&mut buffer, b"2 ", "127.0.0.1", &tx)
            .await
            .unwrap();
        assert!(rx.try_recv().is_err()); // header complete but payload missing

        let chunk3 = format!("{}\n{}\n", m_oct, m_nl);
        TcpSyslogSource::process_buffer(&mut buffer, chunk3.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, r1) = rx.recv().await.unwrap();
        assert_eq!(r1, m_oct);
        let (_, r2) = rx.recv().await.unwrap();
        assert_eq!(r2, m_nl);
    }

    #[tokio::test]
    async fn test_invalid_large_then_newline_then_valid_octet() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        let frame = "99999999 too_big\n7 welcome"; // first invalid -> newline fallback; then valid octet
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, r1) = rx.recv().await.unwrap();
        assert_eq!(r1, "99999999 too_big");
        let (_, r2) = rx.recv().await.unwrap();
        assert_eq!(r2, "welcome");
    }

    #[tokio::test]
    async fn test_many_mixed_messages_in_stream() {
        let (tx, mut rx) = channel::<Message>(50);
        let mut buffer = BytesMut::new();

        // Mix: NL, OCT, NL, OCT in a single stream
        let m1 = "alpha"; // NL
        let m2 = "bravo"; // OCT
        let m3 = "charlie"; // NL
        let m4 = "delta"; // OCT
        let frame = format!("{}\n{} {}\n{}\n{} {}", m1, m2.len(), m2, m3, m4.len(), m4);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let mut outs = Vec::new();
        for _ in 0..4 {
            outs.push(rx.recv().await.unwrap().1);
        }
        assert_eq!(
            outs,
            vec![
                m1.to_string(),
                m2.to_string(),
                m3.to_string(),
                m4.to_string()
            ]
        );
    }
    #[tokio::test]
    async fn test_framing_space_at_position_10() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Space at exactly position 10 (beyond search limit), should use newline
        let frame = "1234567890 msg\n";
        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Should fall back to newline since space is at position 10 (beyond limit)
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, "1234567890 msg");
    }

    // ===== Performance and Stress Tests =====

    #[tokio::test]
    async fn test_framing_many_small_messages() {
        let (tx, mut rx) = channel::<Message>(1000);
        let mut buffer = BytesMut::new();

        // Send 100 small messages using octet counting
        let mut frame = String::new();
        for i in 0..100 {
            let msg = format!("message_{}", i);
            frame.push_str(&format!("{} {}", msg.len(), msg));
        }

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Verify all messages received
        for i in 0..100 {
            let (_, received) = rx.recv().await.unwrap();
            assert_eq!(received, format!("message_{}", i));
        }
    }

    #[tokio::test]
    async fn test_framing_many_newline_messages() {
        let (tx, mut rx) = channel::<Message>(1000);
        let mut buffer = BytesMut::new();

        // Send 100 newline-delimited messages
        let mut frame = String::new();
        for i in 0..100 {
            frame.push_str(&format!("message_{}\n", i));
        }

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        // Verify all messages received
        for i in 0..100 {
            let (_, received) = rx.recv().await.unwrap();
            assert_eq!(received, format!("message_{}", i));
        }
    }

    #[tokio::test]
    async fn test_framing_incremental_large_message() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Send a large message in small chunks
        let msg = "A".repeat(100_000);
        let frame = format!("{} {}", msg.len(), msg);

        // Split into chunks of 1KB
        for chunk in frame.as_bytes().chunks(1024) {
            TcpSyslogSource::process_buffer(&mut buffer, chunk, "127.0.0.1", &tx)
                .await
                .unwrap();
        }

        // Should receive the complete message
        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received.len(), 100_000);
        assert_eq!(received, msg);
    }

    // ===== RFC 6587 Specific Test Cases =====

    #[tokio::test]
    async fn test_rfc6587_example_octet_counting() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Example from RFC 6587 Section 3.4.1
        let msg = "<34>1 2003-10-11T22:14:15.003Z mymachine.example.com su - ID47 - BOM'su root' failed for lonvick on /dev/pts/8";
        let frame = format!("{} {}", msg.len(), msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_rfc6587_octet_counting_preserves_all_data() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Message with embedded newlines, spaces, and special characters
        let msg = "line1\nline2\r\nline3\tspaces  multiple";
        let frame = format!("{} {}", msg.len(), msg);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        let (_, received) = rx.recv().await.unwrap();
        assert_eq!(received, msg);
        // Verify special characters are preserved
        assert!(received.contains(&b'\n'));
        assert!(received.contains(&b'\t'));
    }

    #[tokio::test]
    async fn test_rfc6587_trailer_behavior() {
        let (tx, mut rx) = channel::<Message>(10);
        let mut buffer = BytesMut::new();

        // Per RFC 6587, octet counting doesn't require a trailer
        // But if followed by another message, it should work
        let msg1 = "first";
        let msg2 = "second";
        let frame = format!("{} {}{} {}", msg1.len(), msg1, msg2.len(), msg2);

        TcpSyslogSource::process_buffer(&mut buffer, frame.as_bytes(), "127.0.0.1", &tx)
            .await
            .unwrap();

        assert_eq!(rx.recv().await.unwrap().1, msg1);
        assert_eq!(rx.recv().await.unwrap().1, msg2);
    }
}
