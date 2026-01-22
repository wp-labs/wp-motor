// src/applications/config/constants/fields.rs
//! 配置字段名相关常量
#![allow(dead_code)]

/// 参数字段名
pub const FIELD_PARAMS: &str = "params";

/// 参数覆写字段名
pub const FIELD_PARAMS_OVERRIDE: &str = "params_override";

/// 标签字段名
pub const FIELD_TAGS: &str = "tags";

/// 格式字段名
pub const FIELD_FMT: &str = "fmt";

/// OML 匹配字段名
pub const FIELD_OML: &str = "oml";

/// 规则匹配字段名
pub const FIELD_RULE: &str = "rule";

/// 名称字段名
pub const FIELD_NAME: &str = "name";

/// 连接/使用字段名（及其别名）
pub const FIELD_CONNECT: &str = "connect";
pub const FIELD_USE: &str = "use";
pub const FIELD_CONNECTOR: &str = "connector";

/// 类型字段名
pub const FIELD_TYPE: &str = "type";

/// 允许覆写字段名
pub const FIELD_ALLOW_OVERRIDE: &str = "allow_override";

/// 并行度字段名
pub const FIELD_PARALLEL: &str = "parallel";

/// 期望字段名
pub const FIELD_EXPECT: &str = "expect";

/// 过滤字段名
pub const FIELD_FILTER: &str = "filter";

/// 版本字段名
pub const FIELD_VERSION: &str = "version";

/// 基础路径字段名
pub const FIELD_BASE: &str = "base";

/// 文件名字段名
pub const FIELD_FILE: &str = "file";

/// 配置字段别名映射
pub const FIELD_ALIASES: &[(&str, &str)] =
    &[(FIELD_USE, FIELD_CONNECT), (FIELD_CONNECTOR, FIELD_CONNECT)];

/// 常见误嵌套字段名（用于验证）
pub const NESTED_FIELD_BLACKLIST: &[&str] = &[FIELD_PARAMS, FIELD_PARAMS_OVERRIDE];
