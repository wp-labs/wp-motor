//! Observability business logic for sources and sinks
//!
//! This module provides high-level business functions for collecting
//! observability data about sources and sinks.

mod sinks;
mod sources;
mod validate;

pub use sinks::{ResolvedSinkLite, collect_sink_statistics, process_group, process_group_v2};
pub use sources::{
    SrcLineItem, SrcLineReport, list_file_sources_with_lines, total_input_from_wpsrc,
};
pub use validate::build_groups_v2;
