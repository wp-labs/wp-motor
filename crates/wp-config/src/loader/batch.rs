//! 批量配置加载辅助函数
//!
//! 提供便捷的函数用于从目录或多个路径批量加载配置。
//!
//! # 示例
//!
//! ```no_run
//! ```

use super::traits::ConfigLoader;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use std::path::{Path, PathBuf};

/// 从目录加载所有匹配模式的配置文件
///
/// 遍历指定目录，加载所有匹配文件名模式的配置文件。
///
/// # 参数
/// - `dir`: 目录路径
/// - `pattern`: 文件名模式，支持：
///   - `"*.toml"`: 所有 .toml 文件
///   - `"*.json"`: 所有 .json 文件
///   - `"*"` 或 `"*.*"`: 所有文件
///   - 具体文件名（例如 `"config.toml"`）
/// - `dict`: 环境变量字典
///
/// # 返回
/// 返回成功加载的所有配置对象的向量。
///
/// # 错误
/// - 目录不存在或无法读取
/// - 任何配置文件加载失败
///
/// ```
pub fn load_all_from_dir<T>(dir: &Path, pattern: &str, dict: &EnvDict) -> OrionConfResult<Vec<T>>
where
    T: ConfigLoader + serde::Serialize,
{
    if !dir.exists() {
        return Err(ConfIOReason::from_validation(format!("目录不存在: {:?}", dir)).to_err());
    }

    if !dir.is_dir() {
        return Err(ConfIOReason::from_validation(format!("路径不是目录: {:?}", dir)).to_err());
    }

    let mut configs = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| {
        ConfIOReason::from_validation(format!("无法读取目录 {:?}: {}", dir, e)).to_err()
    })?;

    for entry in entries {
        let entry: std::fs::DirEntry = entry.map_err(|e| {
            ConfIOReason::from_validation(format!("读取目录项失败: {}", e)).to_err()
        })?;

        let path = entry.path();

        if path.is_file() && matches_pattern(&path, pattern) {
            let config = T::load_from_path(&path, dict)?;
            configs.push(config);
        }
    }

    Ok(configs)
}

/// 从多个路径加载配置
///
/// 加载指定路径列表中的所有配置文件。
///
/// # 参数
/// - `paths`: 配置文件路径列表
/// - `dict`: 环境变量字典
///
/// # 返回
/// 返回与输入路径对应的配置对象向量。
///
/// # 错误
/// 如果任何文件加载失败，返回第一个错误。
///
pub fn load_from_paths<T>(paths: &[PathBuf], dict: &EnvDict) -> OrionConfResult<Vec<T>>
where
    T: ConfigLoader + serde::Serialize,
{
    paths
        .iter()
        .map(|path| T::load_from_path(path, dict))
        .collect()
}

/// 检查文件路径是否匹配模式
///
/// # 支持的模式
/// - `"*"` 或 `"*.*"`: 匹配所有文件
/// - `"*.ext"`: 匹配指定扩展名
/// - `"filename"`: 精确匹配文件名
fn matches_pattern(path: &Path, pattern: &str) -> bool {
    // 匹配所有文件
    if pattern == "*" || pattern == "*.*" {
        return true;
    }

    // 匹配扩展名
    if let Some(ext_pattern) = pattern.strip_prefix("*.") {
        return path.extension().and_then(|e| e.to_str()) == Some(ext_pattern);
    }

    // 精确匹配文件名
    path.file_name().and_then(|n| n.to_str()) == Some(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ForTest;
    use serde::Serialize;
    use std::fs;
    use tempfile::tempdir;

    // 测试用的简单配置类型
    #[derive(Serialize)]
    struct TestConfig {
        name: String,
    }

    impl ConfigLoader for TestConfig {
        fn config_type_name() -> &'static str {
            "TestConfig"
        }

        fn load_from_str(content: &str, _base: &Path, _dict: &EnvDict) -> OrionConfResult<Self> {
            Ok(TestConfig {
                name: content.trim().to_string(),
            })
        }
    }

    #[test]
    fn load_all_from_dir_finds_matching_files() {
        let temp = tempdir().unwrap();

        // 创建测试文件
        fs::write(temp.path().join("a.toml"), "config_a").unwrap();
        fs::write(temp.path().join("b.toml"), "config_b").unwrap();
        fs::write(temp.path().join("c.txt"), "ignored").unwrap();

        let configs =
            load_all_from_dir::<TestConfig>(temp.path(), "*.toml", &EnvDict::test_default())
                .unwrap();

        assert_eq!(configs.len(), 2, "应该加载 2 个 .toml 文件");
    }

    #[test]
    fn load_all_from_dir_error_on_missing_directory() {
        let result = load_all_from_dir::<TestConfig>(
            Path::new("/nonexistent/directory"),
            "*.toml",
            &EnvDict::test_default(),
        );

        assert!(result.is_err(), "不存在的目录应该返回错误");
    }

    #[test]
    fn load_from_paths_handles_multiple() {
        let temp = tempdir().unwrap();

        let path1 = temp.path().join("config1.toml");
        let path2 = temp.path().join("config2.toml");

        fs::write(&path1, "config1").unwrap();
        fs::write(&path2, "config2").unwrap();

        let paths = vec![path1, path2];
        let configs = load_from_paths::<TestConfig>(&paths, &EnvDict::test_default()).unwrap();

        assert_eq!(configs.len(), 2, "应该加载 2 个配置");
        assert_eq!(configs[0].name, "config1");
        assert_eq!(configs[1].name, "config2");
    }

    #[test]
    fn matches_pattern_star_matches_all() {
        assert!(matches_pattern(Path::new("test.toml"), "*"));
        assert!(matches_pattern(Path::new("test.json"), "*"));
        assert!(matches_pattern(Path::new("test.txt"), "*.*"));
    }

    #[test]
    fn matches_pattern_extension() {
        assert!(matches_pattern(Path::new("test.toml"), "*.toml"));
        assert!(!matches_pattern(Path::new("test.json"), "*.toml"));
        assert!(matches_pattern(Path::new("a/b/c.toml"), "*.toml"));
    }

    #[test]
    fn matches_pattern_exact_name() {
        assert!(matches_pattern(Path::new("config.toml"), "config.toml"));
        assert!(!matches_pattern(Path::new("other.toml"), "config.toml"));
        assert!(matches_pattern(Path::new("dir/config.toml"), "config.toml"));
    }
}
