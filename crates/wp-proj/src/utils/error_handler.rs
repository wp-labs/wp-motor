//! # 统一的错误处理工具
//!
//! 本模块提供 wp-proj 的标准错误处理模式和工具函数。
//!
//! ## 错误处理标准
//!
//! ### 1. 统一返回类型
//!
//! 所有可能失败的函数应返回 `RunResult<T>`：
//!
//! ```rust,ignore
//! use wp_error::run_error::RunResult;
//!
//! pub fn check(&self) -> RunResult<CheckStatus> {
//!     // 实现...
//! }
//! ```
//!
//! ### 2. 错误转换模式
//!
//! **模式 A: 使用 `.err_conv()`（推荐）**
//!
//! 对于实现了 `ErrorConv` trait 的类型（如 orion-error 家族的错误）：
//!
//! ```rust,ignore
//! use orion_error::ErrorConv;
//!
//! let config = WarpSources::env_load_toml(path, dict).err_conv()?;
//! ```
//!
//! **模式 B: 使用 `orion-error` 的 `.owe_*()` 进行归一化转换**
//!
//! 对于标准库错误或第三方错误，优先统一映射为领域错误：
//!
//! ```rust,ignore
//! use orion_error::ErrorOweSource;
//!
//! let content = fs::read_to_string(&path).owe_conf_source()?;
//! ```
//!
//! **模式 C: 使用 `ErrorHandler` 辅助函数**
//!
//! 对于常见操作（文件检查、目录创建等）：
//!
//! ```rust,ignore
//! use wp_proj::utils::error_handler::ErrorHandler;
//!
//! ErrorHandler::check_file_exists(&path, "配置文件")?;
//! ErrorHandler::safe_write_file(&path, content)?;
//! ```
//!
//! ### 3. 错误消息格式
//!
//! - **配置错误**：`"配置错误: <描述>"`
//! - **文件操作**：`"Failed to <operation>: <path>, error: <detail>"`
//! - **验证错误**：`"<component> 验证失败: <issue>"`
//!
//! ### 4. 避免的模式
//!
//! ❌ 使用 `.unwrap()` 或 `.expect()` 在生产代码中
//!
//! ❌ 返回 `Result<T, String>` 而不是 `RunResult<T>`
//!
//! ❌ 忽略错误或使用 `.ok()`
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use wp_proj::utils::error_handler::ErrorHandler;
//! # use std::path::PathBuf;
//! # use wp_error::run_error::RunResult;
//!
//! # fn demo() -> RunResult<()> {
//! let path = PathBuf::from("./conf/sample.toml");
//!
//! // 检查文件是否存在
//! let _ = ErrorHandler::check_file_exists(&path, "配置文件");
//!
//! // 安全的文件操作
//! ErrorHandler::safe_write_file(&path, "content")?;
//!
//! // 创建统一格式的错误
//! ErrorHandler::config_error("配置项缺失")?;
//! # Ok(())
//! # }
//! # let _ = demo();
//! ```

use orion_conf::ErrorWith;
use orion_error::{IntoAs, ToStructError, UvsFrom};
use std::path::Path;
use wp_error::run_error::{RunReason, RunResult};

/// `ErrorHandler` 提供一致的错误处理策略和错误信息格式，统一各模块的错误处理方式。
///
/// 详细的错误处理标准和最佳实践请参见模块级文档。
pub struct ErrorHandler;

#[allow(dead_code)]
impl ErrorHandler {
    /// 创建配置相关错误
    pub fn config_error(message: impl Into<String>) -> RunResult<()> {
        Err(RunReason::from_conf().to_err().with_detail(message.into()))
    }

    /// 创建文件操作相关错误
    pub fn file_error(operation: &str, path: &Path, cause: &str) -> RunResult<()> {
        Self::config_error(format!(
            "Failed to {}: {:?}, cause: {}",
            operation, path, cause
        ))
    }

    /// 创建目录操作错误
    pub fn dir_error(operation: &str, path: &Path) -> RunResult<()> {
        Self::file_error(operation, path, "I/O error")
    }

    /// 安全地检查文件是否存在并返回统一错误
    pub fn check_file_exists(path: &Path, description: &str) -> RunResult<()> {
        if !path.exists() {
            return Self::config_error(format!("配置错误: {} 文件不存在: {:?}", description, path));
        }
        Ok(())
    }

    /// 检查文件是否为空
    pub fn check_file_not_empty(path: &Path, description: &str) -> RunResult<()> {
        let content = Self::safe_read_file(path)?;
        if content.trim().is_empty() {
            return Self::config_error(format!("配置错误: {} 文件为空: {:?}", description, path));
        }
        Ok(())
    }

    /// 安全执行文件操作
    pub fn safe_file_operation<T>(
        operation: &str,
        path: &Path,
        op: impl FnOnce() -> Result<T, std::io::Error>,
    ) -> RunResult<T> {
        op().into_as(RunReason::from_conf(), operation)
            .with(path)
            .doing(operation)
    }

    /// 安全创建目录
    pub fn safe_create_dir(path: &Path) -> RunResult<()> {
        if !path.exists() {
            Self::safe_file_operation("create directory", path, || std::fs::create_dir_all(path))?;
        }
        Ok(())
    }

    /// 安全写入文件（自动创建父目录）
    pub fn safe_write_file(path: &Path, content: &str) -> RunResult<()> {
        if let Some(parent) = path.parent() {
            Self::safe_create_dir(parent)?;
        }

        Self::safe_file_operation("write file", path, || std::fs::write(path, content))?;
        Ok(())
    }

    /// 安全读取文件
    pub fn safe_read_file(path: &Path) -> RunResult<String> {
        Self::safe_file_operation("read file", path, || std::fs::read_to_string(path))
    }

    /// 转换和包装错误
    pub fn wrap_error<T, E>(
        result: Result<T, E>,
        context: &str,
        actual_operation: impl Into<String>,
    ) -> RunResult<T>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        result.map_err(|err| {
            RunReason::from_conf()
                .to_err()
                .with_detail(actual_operation.into())
                .with_std_source(err)
                .with(context)
        })
    }

    /// 转换和包装错误 (支持 &str context)
    pub fn wrap_error_str<T, E>(
        result: Result<T, E>,
        context: &str,
        actual_operation: impl Into<String>,
    ) -> RunResult<T>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        result.map_err(|err| {
            RunReason::from_conf()
                .to_err()
                .with_detail(actual_operation.into())
                .with_std_source(err)
                .with(context)
        })
    }

    /// 创建验证错误
    pub fn validation_error(component: &str, issue: &str) -> RunResult<()> {
        Self::config_error(format!("{} 验证失败: {}", component, issue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_write_file_creates_missing_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let file_path = temp.path().join("nested/example.txt");
        ErrorHandler::safe_write_file(&file_path, "hello").expect("write");
        assert!(file_path.exists());
        let body = std::fs::read_to_string(file_path).expect("read");
        assert_eq!(body, "hello");
    }

    #[test]
    fn check_file_exists_reports_missing_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let missing = temp.path().join("none.txt");
        let err = ErrorHandler::check_file_exists(&missing, "missing").unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn wrap_error_formats_context() {
        let err = ErrorHandler::wrap_error(
            Err::<(), std::io::Error>(std::io::Error::other("boom")),
            "ctx",
            "run demo",
        )
        .unwrap_err();
        assert!(err.to_string().contains("ctx"));
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn check_file_not_empty_preserves_read_error() {
        let temp = tempfile::tempdir().expect("temp dir");
        let missing = temp.path().join("missing.txt");
        let err = ErrorHandler::check_file_not_empty(&missing, "missing").unwrap_err();
        assert!(err.to_string().contains("read file"));
        assert!(err.to_string().contains("missing.txt"));
    }
}
