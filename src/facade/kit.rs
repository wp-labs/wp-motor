//! Facade: 工具/工作台与同步处理对外入口（稳定 re-export）。

pub use crate::orchestrator::engine::definition::WplCodePKG;
pub use crate::orchestrator::sync_processor::{engine_check, engine_proc_file, wpl_workshop_parse};
