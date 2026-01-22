//! Generic TCP server framework with zero-copy message processing
//!
//! This module provides a modular TCP server implementation split into
//! logical components for better maintainability and testing.

pub mod batch;
pub mod config;
pub mod connection;
//pub mod server;

// Re-export main types for convenience
pub use batch::BatchProcessor;
pub use config::TcpTunables;
pub use connection::ConnectionManager;
//pub use server::TcpServer;
