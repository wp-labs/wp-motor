//! Data picking loop: pulls frames from sources, applies prehooks, and dispatches to parsers.
//!
//! 设计说明（What & Why）
//! - 本子模块承载“采集→排队→投递”的核心逻辑，目标是在高吞吐下依然可控：
//!   1) 尽量让每次投递的批（SourceBatch）变大，减少“按批计数”的通道占用；
//!   2) 在出现背压时，优先合并小批、早停拉，避免 pending 无界增长，控制 RSS；
//!   3) 通过策略（Pull/PostPolicy）把抖动控制在水位附近，而不是持续塞满。
//! - 背压来源：解析通道按“批数”计数；当批很碎、CPU 满时，通道极易满 → 批回填 pending → 内存上升。
//!   因此“批更大/合并小批/水位控制”是必要的设计取舍。

mod actor;
mod dispatch;
mod fetch;
mod policy;
mod post;
mod round;
mod worker;
pub use actor::JMActPicker;
pub use worker::SourceWorker;

// Re-export reader for tests
pub use super::reader::read_data;

#[cfg(test)]
mod tests;

use std::path::Path;
use tokio::fs::File;
use wp_conf::RunArgs;

/// Minimal helper used by integration tests to open a sample data file
/// and return default run arguments. Kept here to avoid leaking internal paths.
pub async fn async_test_prepare<P: AsRef<Path>>(sample_path: P) -> anyhow::Result<(RunArgs, File)> {
    let args = RunArgs::default();
    let input = File::open(sample_path).await?;
    Ok((args, input))
}
