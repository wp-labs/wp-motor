//! 统一的配置加载模块
//!
//! 本模块提供统一的配置加载接口 [`traits::ConfigLoader`] trait。
//! 所有配置类型都实现此 trait，提供一致的 API。
//!
//! ## 示例
//!
//! ```no_run
//! use wp_conf::loader::traits::ConfigLoader;
//! use wp_conf::structure::SourceInstanceConf;
//! use orion_variate::EnvDict;
//! use std::path::Path;
//!
//! let sources = Vec::<SourceInstanceConf>::load_from_path(
//!     Path::new("config/sources.toml"),
//!     &EnvDict::default(),
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod batch;
pub mod delegate;
pub mod traits;
pub mod validate_vars;

pub use delegate::ConfDelegate;
pub use traits::ConfigLoader;
pub use validate_vars::check_unresolved_variables;
