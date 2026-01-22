use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use wp_engine::facade::config::load_warp_engine_confs;
use wp_error::run_error::RunResult;

use super::error_handler::ErrorHandler;

/// # 通用配置路径解析器
///
/// `ConfigPathResolver` 统一处理基于配置文件的路径解析逻辑，支持配置文件不存在时的回退机制。
///
/// ## 主要功能
///
/// - 提供统一的配置路径解析接口
/// - 支持配置文件不存在时的回退机制
/// - 自动创建目录和文件的辅助功能
/// - 避免在各模块中重复编写相同的路径解析逻辑
///
pub struct SpecConfPath;
impl SpecConfPath {
    pub fn topology(work_root: PathBuf, sub: &str) -> RunResult<PathBuf> {
        Ok(work_root.join("topology").join(sub))
    }
    pub fn models(work_root: PathBuf, sub: &str) -> RunResult<PathBuf> {
        Ok(work_root.join("models").join(sub))
    }
    pub fn conf(work_root: PathBuf, sub: &str) -> RunResult<PathBuf> {
        Ok(work_root.join("conf").join(sub))
    }
    pub fn data(work_root: PathBuf, sub: &str) -> RunResult<PathBuf> {
        Ok(work_root.join("data").join(sub))
    }
}
pub struct ConfigPathResolver;

impl ConfigPathResolver {
    /// 获取模型目录路径（兼容性方法）
    pub fn resolve_model_path(
        work_root: &str,
        model_type: &str,
        dict: &EnvDict,
    ) -> RunResult<PathBuf> {
        let fallback = format!("models/{}", model_type);

        match load_warp_engine_confs(work_root, dict) {
            Ok((conf_manager, main_conf)) => {
                let work_root_path = conf_manager.work_root_path();
                let model_root = match model_type {
                    "wpl" => main_conf.rule_root(),
                    "oml" => main_conf.oml_root(),
                    "sinks" => main_conf.sink_root(),
                    "sources" => main_conf.src_root(),
                    _ => &fallback,
                };

                // 检查 model_root 是否已经是绝对路径或包含完整工作目录
                let result = if Path::new(model_root).is_absolute() {
                    PathBuf::from(model_root)
                } else if model_root.starts_with(work_root) {
                    // 已经包含完整工作目录路径
                    PathBuf::from(model_root)
                } else {
                    Path::new(&work_root_path).join(model_root)
                };
                Ok(result)
            }
            Err(_) => {
                // 配置文件不存在时使用回退路径
                Ok(Path::new(work_root).join(fallback))
            }
        }
    }

    /// 创建目录（如果不存在）
    pub fn ensure_dir_exists(path: &Path) -> RunResult<()> {
        ErrorHandler::safe_create_dir(path)
    }

    /// 创建目录并写入文件
    pub fn write_file_with_dir(file_path: &Path, content: &str) -> RunResult<()> {
        ErrorHandler::safe_write_file(file_path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{temp_workdir, write_basic_wparse_config};
    use wp_conf::test_support::ForTest;

    #[test]
    fn resolve_model_path_falls_back_without_config() {
        let temp = temp_workdir();
        let path = ConfigPathResolver::resolve_model_path(
            temp.path().to_str().unwrap(),
            "wpl",
            &EnvDict::test_default(),
        )
        .expect("resolve path");
        assert!(path.ends_with("models/wpl"));
        assert!(path.starts_with(temp.path()));
    }

    #[test]
    fn resolve_model_path_uses_config_when_available() {
        let temp = temp_workdir();
        write_basic_wparse_config(temp.path());
        let path = ConfigPathResolver::resolve_model_path(
            temp.path().to_str().unwrap(),
            "sinks",
            &EnvDict::test_default(),
        )
        .expect("resolve path");
        assert_eq!(path, temp.path().join("topology/sinks"));
    }

    #[test]
    fn write_file_with_dir_creates_parent_directories() {
        let temp = temp_workdir();
        let target = temp.path().join("models/oml/example.toml");
        ConfigPathResolver::write_file_with_dir(&target, "demo = true").expect("write");
        assert!(target.exists());
        let body = std::fs::read_to_string(target).expect("read");
        assert_eq!(body, "demo = true");
    }
}
