//! Facade Test Helpers
//!
//! 轻量测试工具，帮助在用例里快速创建工作目录、连接器目录、临时文件等，
//! 避免对内部模块路径产生耦合。

use std::fs;
use std::path::{Path, PathBuf};

// Intentionally not re-exporting internal test_support to avoid cfg(test) issues.

/// 创建并返回 `$work/connectors/source.d` 目录路径。
pub fn create_connectors_dir<P: AsRef<Path>>(work: P) -> PathBuf {
    let p = work.as_ref().join("connectors").join("source.d");
    let _ = fs::create_dir_all(&p);
    p
}

/// 在指定的 connectors 目录下写入一个 TOML 文件。
pub fn write_connector_toml<P: AsRef<Path>>(connectors_dir: P, file: &str, content: &str) {
    let fp = connectors_dir.as_ref().join(file);
    fs::write(fp, content).expect("write connector toml");
}

/// 在系统临时目录创建一个带内容的文件并返回其路径。
pub fn write_tmp_file(name: &str, content: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    fs::write(&p, content).expect("write tmp file");
    p
}

// Re-export a minimal set of actor types for tests, so external tests
// do not need to depend on internal runtime paths directly.
pub use crate::runtime::actor::command::ActorCtrlCmd;
pub use crate::runtime::actor::signal::ShutdownCmd;

// Re-export common runtime helpers needed by integration tests
pub use crate::runtime::collector::{async_test_prepare, read_data};
pub use crate::runtime::generator::types::RuleGRA;
pub use crate::runtime::parser::act_parser::ActParser;
pub use crate::runtime::parser::workflow::{ActorWork, ParseWorkerSender};

// Core parser and types commonly used in tests/benches（从私有模块路径直接 re-export 类型本身）
pub use crate::core::generator::rules::GenRuleUnit;
pub use crate::core::parser::setting::ParseOption;
pub use crate::core::parser::wpl_engine::repo::WplRepository;
pub use crate::core::sinks::sync_sink::SinkTerminal;
pub use crate::sinks::BlackHoleSink;
