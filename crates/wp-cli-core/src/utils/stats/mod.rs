//! Statistical calculation utilities
//!
//! This module provides functions for computing statistics, percentages,
//! and other mathematical operations.

#[allow(clippy::module_inception)]
pub mod stats;

pub use stats::{StatsFile, group_input, load_stats_file};
