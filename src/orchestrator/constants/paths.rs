// src/applications/config/constants/paths.rs
//! 路径和文件名相关常量
#![allow(dead_code)]

/// 连接器配置根目录名
pub const PATH_CONNECTORS_DIR: &str = "connectors";

/// 连接器配置子目录名
pub const PATH_SINK_SUBDIR: &str = "sink.d";

/// 业务路由配置目录名
pub const PATH_BUSINESS_SUBDIR: &str = "business.d";

/// 基础设施路由配置目录名
pub const PATH_INFRA_SUBDIR: &str = "infra.d";

/// 默认配置文件名
pub const PATH_DEFAULTS_FILE: &str = "defaults.toml";

/// TOML 文件扩展名
pub const FILE_EXT_TOML: &str = "toml";

/// 配置目录层次结构
pub const CONFIG_DIR_HIERARCHY: &[&str] = &[PATH_CONNECTORS_DIR, PATH_SINK_SUBDIR];
