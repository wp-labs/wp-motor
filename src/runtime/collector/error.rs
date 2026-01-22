#![allow(dead_code)]
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Channel communication error: {0}")]
    Channel(String),

    #[error("Data parsing error: {0}")]
    Parse(String),

    #[error("Recovery checkpoint error: {0}")]
    Checkpoint(String),

    #[error("Resource exhausted: {0}")]
    Resource(String),
}

pub type CollectorResult<T> = Result<T, CollectorError>;
