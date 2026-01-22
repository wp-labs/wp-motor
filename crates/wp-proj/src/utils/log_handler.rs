use orion_variate::EnvDict;
use std::path::Path;
use wp_engine::facade::config::load_warp_engine_confs;
use wp_error::run_error::RunResult;
use wp_log::conf::LogConf;

/// 通用日志处理器 - 基于WpEngine的LogConf对象进行日志处理
pub struct LogHandler;

impl LogHandler {
    /// 清理日志目录（基于WpEngine的LogConf对象）
    pub fn clean_logs<P: AsRef<Path>>(log_conf: &LogConf, work_root: P) -> RunResult<bool> {
        let work_root = work_root.as_ref();
        if let Some(log_path) = Self::log_path_from_conf(log_conf) {
            Self::clean_log_dir(work_root, &log_path)
        } else {
            Ok(false)
        }
    }

    /// 从工作目录加载配置并清理日志
    pub fn clean_logs_via_config<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> RunResult<bool> {
        let work_root = work_root.as_ref();
        match load_warp_engine_confs(work_root.to_string_lossy().as_ref(), dict) {
            Ok((_, main_conf)) => {
                let log_conf = main_conf.log_conf();
                Self::clean_logs(log_conf, work_root)
            }
            Err(e) => {
                eprintln!("Warning: Failed to load main config: {}", e);
                Ok(false)
            }
        }
    }

    /// 从WpEngine的LogConf提取日志路径
    fn log_path_from_conf(log_conf: &LogConf) -> Option<String> {
        log_conf.file.as_ref().and_then(|f| {
            let path = f.path.trim();
            if path.is_empty() {
                None
            } else {
                Some(path.to_string())
            }
        })
    }

    /// 规范化路径，移除 "." 和 ".." 组件
    fn normalize_path(path: &Path) -> String {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::CurDir => {
                    // 跳过 "." 组件
                }
                std::path::Component::ParentDir => {
                    // ".." 组件：弹出上一个组件（如果存在且不是根）
                    if !components.is_empty() {
                        components.pop();
                    }
                }
                _ => {
                    // 正常组件（根、前缀、普通路径）
                    components.push(component);
                }
            }
        }

        // 重新组装路径
        let mut result = std::path::PathBuf::new();
        for component in components {
            result.push(component);
        }
        result.to_string_lossy().to_string()
    }

    /// 清理日志目录
    fn clean_log_dir<P: AsRef<Path>>(work_root: P, log_path: &str) -> RunResult<bool> {
        let work_root = work_root.as_ref();
        let log_dir = Path::new(log_path);

        // 如果是相对路径，则与工作目录组合
        let full_log_dir = if log_dir.is_absolute() {
            log_dir.to_path_buf()
        } else {
            work_root.join(log_dir)
        };

        // 规范化路径，移除 "./" 和 "../" 组件
        let normalized_path = Self::normalize_path(&full_log_dir);

        if full_log_dir.exists() {
            match std::fs::remove_dir_all(&full_log_dir) {
                Ok(_) => {
                    println!("✓ Cleaned log directory: {}", normalized_path);
                    Ok(true)
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to remove log directory {}: {}",
                        normalized_path, e
                    );
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::temp_workdir;

    #[test]
    fn clean_logs_removes_relative_directory() {
        use wp_log::conf::{FileLogConf, LogConf, Output};

        let temp = temp_workdir();
        let log_dir = temp.path().join("data/logs");
        std::fs::create_dir_all(&log_dir).expect("log dir");
        std::fs::write(log_dir.join("test.log"), "line").expect("log file");

        let mut cfg = LogConf::default();
        cfg.output = Output::File;
        cfg.file = Some(FileLogConf {
            path: "./data/logs".to_string(),
        });

        let cleaned = LogHandler::clean_logs(&cfg, temp.path().to_str().unwrap()).unwrap();
        assert!(cleaned);
        assert!(!log_dir.exists());
    }

    #[test]
    fn normalize_path_removes_current_dir_components() {
        use std::path::PathBuf;

        // 测试 ././ 被规范化
        let path = PathBuf::from("/Users/test/./data/./logs");
        let normalized = LogHandler::normalize_path(&path);
        assert_eq!(normalized, "/Users/test/data/logs");

        // 测试 .. 被规范化
        let path = PathBuf::from("/Users/test/foo/../logs");
        let normalized = LogHandler::normalize_path(&path);
        assert_eq!(normalized, "/Users/test/logs");

        // 测试混合情况
        let path = PathBuf::from("/Users/./test/./foo/../logs");
        let normalized = LogHandler::normalize_path(&path);
        assert_eq!(normalized, "/Users/test/logs");
    }
}
