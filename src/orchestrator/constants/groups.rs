// src/applications/config/constants/groups.rs
//! 组名相关常量
#![allow(dead_code)]

// 直接复用配置层的组名常量，避免双份维护
pub use wp_conf::sinks::{GROUP_DEFAULT, GROUP_ERROR, GROUP_MISS, GROUP_MONITOR, GROUP_RESIDUE};
// 应用层沿用旧名别名，指向配置层集合
pub use wp_conf::sinks::INFRA_GROUPS as INFRA_GROUP_NAMES;

/// 业务组名排除列表（在构建 infra 组时排除）
pub const BUSINESS_GROUP_EXCLUSIONS: &[&str] = INFRA_GROUP_NAMES;
