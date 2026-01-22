//! 常用路径/文件名常量：统一对外导出，便于多处复用。
//!
//! - 仅作为“约定路径”的声明处，不承担具体 IO 逻辑；
//! - 向后兼容：`wp_conf::builder::{OUT_FILE_PATH, GEN_RULE_FILE, GEN_FIELD_FILE}` 继续 re-export。

/// 输出文件目录（默认）
pub const OUT_FILE_PATH: &str = "./data/out_dat";
pub const RESCURE_FILE_PATH: &str = "./data/rescue";
/// 生成规则（WPL）默认文件名
pub const GEN_RULE_FILE: &str = "gen_rule.wpl";
/// 生成字段配置（TOML）默认文件名
pub const GEN_FIELD_FILE: &str = "gen_field.toml";

pub const SRC_FILE_PATH: &str = "./data/in_dat";
