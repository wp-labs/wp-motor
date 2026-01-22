use std::sync::Arc;

use wp_parse_api::{RawData, WparseResult};

/// Trait for pipeline data processing operations.
///
/// This trait defines the interface for components that process RawData
/// within a data pipeline, transforming it from one format to another
/// (e.g., base64 decoding, hex decoding, string unescaping, etc.).
///
/// Pipeline processors are executed in sequence as part of a data processing
/// pipeline, with the output of one processor becoming the input of the next.
pub trait PipeProcessor {
    /// Process the input data and return the transformed result.
    ///
    /// # Arguments
    /// * `data` - The input data to be processed
    ///
    /// # Returns
    /// The processed data in the appropriate output format
    fn process(&self, data: RawData) -> WparseResult<RawData>;

    /// Get the name/identifier of this pipeline processor.
    ///
    /// # Returns
    /// A string slice representing the processor name
    fn name(&self) -> &'static str;
}

pub type PipeHold = Arc<dyn PipeProcessor + Send + Sync>;
