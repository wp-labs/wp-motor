//! 统一的配置加载接口
//!
//! 提供 `ConfigLoader` trait，为所有配置类型提供一致的加载体验。
//!
//! # 设计原则
//!
//! - **一致性**: 所有配置类型使用相同的加载方法
//! - **可扩展**: 易于添加新的配置类型
//! - **可测试**: 统一的接口便于编写测试
//!
//! ```

use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use std::path::Path;

/// 统一的配置加载接口
///
/// 所有配置类型都应实现此 trait 以提供一致的加载体验。
pub trait ConfigLoader: Sized {
    /// 配置类型名称（用于错误消息）
    ///
    /// 返回人类可读的配置类型名称，例如 "Sources", "Sink Routes", "Syslog Sink"。
    fn config_type_name() -> &'static str;

    /// 从文件路径加载配置
    ///
    /// 这是加载配置的主要入口点。它会：
    /// 1. 读取文件内容
    /// 2. 调用 `load_from_str` 进行解析
    /// 3. 调用 `validate` 进行验证
    ///
    /// # 参数
    /// - `path`: 配置文件路径（绝对路径或相对路径）
    /// - `dict`: 环境变量字典，用于变量替换
    ///
    /// # 错误
    /// - 文件不存在或无法读取
    /// - 文件内容格式错误（TOML 解析失败等）
    /// - 验证失败
    ///
    /// # 示例
    /// ```no_run
    /// # use wp_conf::loader::traits::ConfigLoader;
    /// # use wp_conf::structure::SourceInstanceConf;
    /// # use orion_variate::EnvDict;
    /// # use std::path::Path;
    /// let sources = Vec::<SourceInstanceConf>::load_from_path(
    ///     Path::new("sources.toml"),
    ///     &EnvDict::default(),
    /// )?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn load_from_path(path: &Path, dict: &EnvDict) -> OrionConfResult<Self>
    where
        Self: serde::Serialize,
    {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfIOReason::from_validation(format!(
                "无法读取 {} 配置文件 {:?}: {}",
                Self::config_type_name(),
                path,
                e
            ))
            .to_err()
        })?;

        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let config = Self::load_from_str(&content, base, dict)?;

        // 检测未替换的变量
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            super::validate_vars::check_unresolved_variables(&config, filename)?;
        }

        // 自动验证
        config.validate()?;

        Ok(config)
    }

    /// 从字符串内容加载配置
    ///
    /// 解析配置文件内容并构建配置对象。这是核心的解析逻辑。
    ///
    /// # 参数
    /// - `content`: 配置文件内容（通常是 TOML 格式）
    /// - `base`: 基准路径，用于解析相对路径引用
    /// - `dict`: 环境变量字典，用于变量替换
    ///
    /// # 返回
    /// 返回解析后的配置对象（未验证）。
    ///
    /// # 错误
    /// - TOML 解析错误
    /// - 必需字段缺失
    /// - 类型转换错误
    fn load_from_str(content: &str, base: &Path, dict: &EnvDict) -> OrionConfResult<Self>;

    /// 验证配置（可选，默认不验证）
    ///
    /// 在加载后自动调用，用于检查配置的合法性。
    ///
    /// # 错误
    /// 如果配置不合法，返回描述性错误。
    ///
    /// # 示例
    /// ```no_run
    /// # use wp_conf::loader::traits::ConfigLoader;
    /// # use orion_conf::error::OrionConfResult;
    /// # struct MyConfig;
    /// impl ConfigLoader for MyConfig {
    ///     fn config_type_name() -> &'static str { "MyConfig" }
    ///     fn load_from_str(content: &str, base: &std::path::Path, dict: &orion_variate::EnvDict) -> OrionConfResult<Self> {
    ///         Ok(MyConfig)
    ///     }
    ///     fn validate(&self) -> OrionConfResult<()> {
    ///         // 自定义验证逻辑
    ///         Ok(())
    ///     }
    /// }
    /// ```
    fn validate(&self) -> OrionConfResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ForTest;
    use serde::Serialize;

    // 用于测试的简单配置类型
    #[derive(Serialize)]
    struct TestConfig {
        value: String,
    }

    impl ConfigLoader for TestConfig {
        fn config_type_name() -> &'static str {
            "TestConfig"
        }

        fn load_from_str(_content: &str, _base: &Path, _dict: &EnvDict) -> OrionConfResult<Self> {
            Ok(TestConfig {
                value: "test".to_string(),
            })
        }

        fn validate(&self) -> OrionConfResult<()> {
            if self.value.is_empty() {
                return Err(ConfIOReason::from_validation("value 不能为空").to_err());
            }
            Ok(())
        }
    }

    #[test]
    fn config_loader_load_from_path_reads_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();

        let result = TestConfig::load_from_path(file.path(), &EnvDict::test_default());
        assert!(result.is_ok(), "应该成功加载");
    }

    #[test]
    fn config_loader_validate_called() {
        // 创建一个会失败验证的配置
        #[derive(Debug, Serialize)]
        struct InvalidConfig;

        impl ConfigLoader for InvalidConfig {
            fn config_type_name() -> &'static str {
                "InvalidConfig"
            }

            fn load_from_str(_: &str, _: &Path, _: &EnvDict) -> OrionConfResult<Self> {
                Ok(InvalidConfig)
            }

            fn validate(&self) -> OrionConfResult<()> {
                Err(ConfIOReason::from_validation("验证失败").to_err())
            }
        }

        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "content").unwrap();

        let result = InvalidConfig::load_from_path(file.path(), &EnvDict::test_default());
        assert!(result.is_err(), "验证失败应该返回错误");
        assert!(result.unwrap_err().to_string().contains("验证失败"));
    }

    #[test]
    fn config_loader_error_on_missing_file() {
        let result = TestConfig::load_from_path(
            Path::new("/nonexistent/file.toml"),
            &EnvDict::test_default(),
        );
        assert!(result.is_err(), "不存在的文件应该返回错误");
    }
}
