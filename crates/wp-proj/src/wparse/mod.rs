use orion_conf::error::ConfIOReason;
use orion_error::{
    ErrorOweSource, ErrorWith, OperationContext, StructError, UvsFrom, WrapStructError,
};
use orion_variate::EnvDict;
use std::path::Path;
use wp_error::diagnostic_meta::{OperationContextMetaExt, OperationKind, RuntimeStage};

use crate::utils::LogHandler;

pub mod samples;

fn wparse_clean_context(path: &Path, operation: OperationKind) -> OperationContext {
    OperationContext::want("clean wparse data")
        .with_meta_value(RuntimeStage::SystemOperations)
        .with_meta_value(operation)
        .with_dir_path(path)
}

/// WParse 管理器
#[derive(Debug, Clone)]
pub struct WParseManager {
    work_root: std::path::PathBuf,
}

#[allow(dead_code)]
impl WParseManager {
    /// 创建新的 WParse 管理器
    pub fn new<P: AsRef<std::path::Path>>(work_root: P) -> Self {
        Self {
            work_root: work_root.as_ref().to_path_buf(),
        }
    }

    /// 获取 WParse 配置路径
    pub fn get_config_path(&self) -> std::path::PathBuf {
        self.work_root.join("conf").join("wparse.toml")
    }

    /// 清理 WParse 相关数据
    pub fn clean_data(&self, dict: &EnvDict) -> wp_error::run_error::RunResult<bool> {
        let mut did_clean_any = false;

        // 1. 清理 .run 目录
        let run_dir = self.work_root.join(".run");
        if run_dir.exists() {
            std::fs::remove_dir_all(&run_dir)
                .owe_conf_source()
                .with(wparse_clean_context(
                    &run_dir,
                    OperationKind::LoadConfigFile,
                ))
                .with(&run_dir)
                .want("remove wparse runtime dir")
                .map_err(|e: StructError<ConfIOReason>| {
                    e.wrap(wp_error::RunReason::from_conf())
                        .with_detail("清理 wparse 运行目录失败")
                })?;
            println!("✓ Cleaned .run directory");
            did_clean_any = true;
        }

        // 2. 使用 WpEngine 的 load_main 清理日志文件
        let log_cleaned = LogHandler::clean_logs_via_config(&self.work_root, dict)
            .map_err(|e| e.with_detail("清理 wparse 日志失败"))?;
        if log_cleaned {
            did_clean_any = true;
        }

        Ok(did_clean_any)
    }

    /// 检查是否有 WParse 数据需要清理
    pub fn has_pending_cleanup(&self) -> bool {
        // 1. 检查 .run 目录
        let run_dir = self.work_root.join(".run");
        if run_dir.exists() {
            return true;
        }

        // 2. 简化：只检查 .run 目录，日志清理操作会自动处理
        false
    }

    /// 获取 WParse 数据根目录
    pub fn get_data_root(&self) -> std::path::PathBuf {
        self.work_root.join("data")
    }

    /// 获取 WParse 工作目录
    pub fn get_work_root(&self) -> &std::path::Path {
        &self.work_root
    }

    /// 获取 WParse 工作目录字符串（向后兼容）
    pub fn get_work_root_str(&self) -> &str {
        self.work_root.to_str().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{temp_workdir, write_basic_wparse_config};
    use wp_conf::test_support::ForTest;
    use wp_error::diagnostic_meta::{MetaValue, RuntimeStage, first_meta_str, key};

    #[test]
    fn manager_reports_paths_relative_to_work_root() {
        let temp = temp_workdir();
        let manager = WParseManager::new(temp.path());
        assert_eq!(
            manager.get_config_path(),
            temp.path().join("conf/wparse.toml")
        );
        assert_eq!(manager.get_data_root(), temp.path().join("data"));
    }

    #[test]
    fn has_pending_cleanup_detects_run_directory() {
        let temp = temp_workdir();
        let manager = WParseManager::new(temp.path());
        assert!(!manager.has_pending_cleanup());
        std::fs::create_dir_all(temp.path().join(".run")).unwrap();
        assert!(manager.has_pending_cleanup());
    }

    #[test]
    fn clean_data_removes_run_and_logs() {
        let temp = temp_workdir();
        write_basic_wparse_config(temp.path());
        let manager = WParseManager::new(temp.path());
        let run_dir = temp.path().join(".run");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("tmp"), "state").unwrap();
        let log_dir = temp.path().join("data/logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("wp.log"), "line").unwrap();
        let dict = EnvDict::test_default();

        let cleaned = manager.clean_data(&dict).expect("clean data");
        assert!(cleaned);
        assert!(!run_dir.exists());
    }

    #[test]
    fn clean_data_reports_run_dir_failure_with_source_metadata() {
        let temp = temp_workdir();
        write_basic_wparse_config(temp.path());
        let manager = WParseManager::new(temp.path());
        let run_dir = temp.path().join(".run");
        std::fs::write(&run_dir, "not a dir").expect("blocking file");
        let dict = EnvDict::test_default();

        let err = manager
            .clean_data(&dict)
            .expect_err("file at .run should fail runtime directory cleanup");
        let report = err.report();
        let run_dir_str = run_dir.display().to_string();

        assert_eq!(
            first_meta_str(&report, key::RUNTIME_STAGE),
            Some(RuntimeStage::SystemOperations.as_str())
        );
        assert_eq!(
            first_meta_str(&report, key::DIR_PATH),
            Some(run_dir_str.as_str())
        );
        assert!(err.display_chain().contains("remove wparse runtime dir"));
        assert!(
            !report.source_frames.is_empty(),
            "remove_dir_all failure should keep io source frame"
        );
    }
}
