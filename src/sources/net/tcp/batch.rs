//! Batch processing for TCP messages
//!
//! This module handles the batching logic for zero-copy messages
//! to optimize channel throughput and reduce overhead.

use std::net::IpAddr;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::sources::tcp::zc::types::BatchConfig;
use crate::sources::tcp::{MessageBatch, ZcpMessage};

/// Batch processor for TCP zero-copy messages
pub struct BatchProcessor {
    batch: MessageBatch,
    last_send: Instant,
    batch_timeout: std::time::Duration,
    batch_config: BatchConfig,
}

impl BatchProcessor {
    /// Create a new batch processor with the given configuration
    pub fn new(batch_config: BatchConfig) -> Self {
        Self {
            batch: MessageBatch::with_capacity(batch_config.batch_capacity),
            last_send: Instant::now(),
            batch_timeout: std::time::Duration::from_millis(batch_config.batch_timeout_ms),
            batch_config,
        }
    }

    /// Add a message to the batch and send if needed
    pub fn add_message(
        &mut self,
        message: ZcpMessage,
        sender: &mpsc::Sender<MessageBatch>,
        key: &str,
        client_ip: IpAddr,
    ) -> bool {
        self.batch.push(message);
        self.send_batch_if_needed(sender, key, client_ip)
    }

    /// Send batch if it meets size or time criteria
    fn send_batch_if_needed(
        &mut self,
        sender: &mpsc::Sender<MessageBatch>,
        key: &str,
        client_ip: IpAddr,
    ) -> bool {
        // Check if batch needs sending: size limit reached or timeout occurred
        if self.batch.len() >= self.batch_config.max_batch_size
            || self.last_send.elapsed() > self.batch_timeout
        {
            let batch_to_send = std::mem::take(&mut self.batch);
            let batch_size = batch_to_send.len();
            match sender.try_send(batch_to_send) {
                Ok(_) => {
                    debug_data!(
                        "TCP '{}' sent batch of {} messages from {}",
                        key,
                        batch_size,
                        client_ip
                    );
                    self.last_send = Instant::now();
                    true
                }
                Err(_) => {
                    //TODO: error !! data will miss
                    warn_data!(
                        "TCP '{}' channel full, dropping batch from {}",
                        key,
                        client_ip
                    );
                    self.last_send = Instant::now();
                    true // Clear batch even if send failed
                }
            }
        } else {
            false
        }
    }

    /// Send any remaining messages in the batch
    pub fn flush(&mut self, sender: &mpsc::Sender<MessageBatch>, key: &str, client_ip: IpAddr) {
        if !self.batch.is_empty() {
            let remaining_batch = std::mem::take(&mut self.batch);
            let batch_size = remaining_batch.len();
            match sender.try_send(remaining_batch) {
                Ok(_) => {
                    trace_data!(
                        "TCP '{}' sent final batch of {} messages from {}",
                        key,
                        batch_size,
                        client_ip
                    );
                }
                Err(_) => {
                    warn_data!(
                        "TCP '{}' channel full, dropping final batch from {}",
                        key,
                        client_ip
                    );
                }
            }
        }
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Get current batch size
    pub fn batch_size(&self) -> usize {
        self.batch.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::tcp::zc::types::BatchConfig;

    #[test]
    fn test_batch_processor_new() {
        let config = BatchConfig::default();
        let processor = BatchProcessor::new(config);

        assert!(processor.is_empty());
        assert_eq!(processor.batch_size(), 0);
    }

    #[test]
    fn test_batch_processor_add_message() {
        let config = BatchConfig::default()
            .with_max_batch_size(2)
            .with_timeout_ms(1000);
        let mut processor = BatchProcessor::new(config);

        let (tx, mut rx) = mpsc::channel(10);

        let client_ip = "127.0.0.1".parse().unwrap();
        let msg = ZcpMessage::new(b"127.0.0.1", b"test message".to_vec());

        // Add first message - should not trigger send
        let sent = processor.add_message(msg.clone(), &tx, "test_key", client_ip);
        assert!(!sent);
        assert_eq!(processor.batch_size(), 1);
        assert!(rx.try_recv().is_err());

        // Add second message - should trigger send due to max_batch_size
        let sent = processor.add_message(msg, &tx, "test_key", client_ip);
        assert!(sent);
        assert!(processor.is_empty());

        // Receive the batch
        let batch = rx.try_recv().unwrap();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_processor_timeout() {
        let config = BatchConfig::default()
            .with_max_batch_size(10)
            .with_timeout_ms(1); // 1ms timeout
        let mut processor = BatchProcessor::new(config);

        let (tx, mut rx) = mpsc::channel(10);

        let client_ip = "127.0.0.1".parse().unwrap();
        let msg = ZcpMessage::new(b"127.0.0.1", b"test message".to_vec());

        // Add message
        processor.add_message(msg, &tx, "test_key", client_ip);
        assert_eq!(processor.batch_size(), 1);

        // Wait for timeout and send any remaining messages
        processor.flush(&tx, "test_key", client_ip);

        // Should receive the batch with 1 message
        let batch = rx.try_recv().unwrap();
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn test_batch_processor_flush() {
        let config = BatchConfig::default().with_max_batch_size(1);
        let mut processor = BatchProcessor::new(config);

        let (tx, mut rx) = mpsc::channel(10);

        let client_ip = "127.0.0.1".parse().unwrap();
        let msg = ZcpMessage::new(b"127.0.0.1", b"test message".to_vec());

        // Add message - should trigger send due to max_batch_size=1
        processor.add_message(msg, &tx, "test_key", client_ip);
        // Batch should be empty because message was sent
        assert!(processor.is_empty());

        // Should receive the message
        let batch = rx.try_recv().unwrap();
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn test_batch_processor_channel_full() {
        let config = BatchConfig::default().with_max_batch_size(1);
        let mut processor = BatchProcessor::new(config);

        // Create a channel to simulate full condition
        let (tx, _rx) = mpsc::channel(1);

        let client_ip = "127.0.0.1".parse().unwrap();
        let msg = ZcpMessage::new(b"127.0.0.1", b"test message".to_vec());

        // Add message - should return true (batch sent) even though channel is full
        let sent = processor.add_message(msg, &tx, "test_key", client_ip);
        assert!(sent);
        assert!(processor.is_empty());
    }
}
