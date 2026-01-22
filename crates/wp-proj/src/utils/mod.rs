//! # 通用工具模块
//!
//! 提供 wproj/core 中各模块共用的工具函数和辅助类。
//!
//! ## 模块组成
//!
//! - **config_path**: 统一的配置路径解析，支持回退机制
//! - **error_conv**: 错误类型转换辅助（anyhow/OrionConfResult → RunResult）
//! - **error_handler**: 统一的错误处理策略和错误信息格式化
//! - **fs**: 文件系统操作工具，提供统一的文件和目录操作接口
//! - **log_handler**: 通用的日志处理，基于 WpEngine LogConf 对象
//! - **path_resolver**: 路径解析 trait，用于将相对路径转换为绝对路径
//! - **template_init**: 模板文件初始化辅助工具

pub mod config_path;
pub mod error_conv;
pub mod error_handler;
pub mod fs;
pub mod log_handler;
pub mod path_resolver;
pub mod template_init;

// Re-export 主要类型以方便使用
pub use fs::FsOps;
pub use log_handler::LogHandler;
pub use path_resolver::PathResolvable;
pub use template_init::TemplateInitializer;
