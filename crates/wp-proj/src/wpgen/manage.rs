use std::path::Path;

use orion_variate::EnvDict;
use wp_error::RunResult;

use crate::wpgen::core::clean_wpgen_output_file;

/// WPgen 管理器
#[derive(Debug, Clone)]
pub struct WpGenManager {
    work_root: std::path::PathBuf,
}

impl WpGenManager {
    /// 创建新的 WPgen 管理器
    pub fn new<P: AsRef<Path>>(work_root: P) -> Self {
        Self {
            work_root: work_root.as_ref().to_path_buf(),
        }
    }

    /// 数据清理（根据 wpgen.toml 配置中的 connect 字段确定数据位置）
    pub fn clean_outputs(&self, dict: &EnvDict) -> RunResult<bool> {
        // 检查配置文件是否存在
        let config_path = self.work_root.join("conf").join("wpgen.toml");
        if !config_path.exists() {
            return Ok(false);
        }

        // 使用已抽离的 wp_proj::cli_ops::wpgen::clean_output 函数
        // 这个函数会正确解析 wpgen.toml 并根据 connect 配置清理数据
        match clean_wpgen_output_file(
            self.work_root.to_string_lossy().as_ref(),
            "wpgen.toml",
            true,
            dict,
        ) {
            Ok(result) => {
                if let Some(path) = result.path {
                    if result.cleaned {
                        println!("✓ Cleaned wpgen data from: {}", path);
                        Ok(true)
                    } else if result.existed {
                        eprintln!("Warning: Failed to clean wpgen data from: {}", path);
                        Ok(false)
                    } else {
                        println!("✓ No wpgen data to clean at: {}", path);
                        Ok(false)
                    }
                } else if let Some(msg) = result.message {
                    println!("✓ Wpgen cleanup skipped: {}", msg);
                    Ok(false)
                } else {
                    println!("✓ No wpgen data to clean");
                    Ok(false)
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to clean wpgen data: {}", e);
                Ok(false)
            }
        }
    }

    /// 获取工作根目录的 Path 引用
    pub fn work_root(&self) -> &std::path::Path {
        &self.work_root
    }

    /// 获取工作根目录字符串（向后兼容）
    pub fn work_root_str(&self) -> &str {
        self.work_root.to_str().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use orion_error::TestAssertWithMsg;
    use wp_conf::test_support::{ForTest, TestCasePath};

    use super::*;
    use crate::project::{WarpProject, init::PrjScope};

    #[test]
    fn clean_outputs_remove_file_sink_outputs() {
        let case_path = TestCasePath::new("wgpen", "clean1").assert("test path");
        let mut project = WarpProject::bare(case_path.path());
        project
            .init_basic(PrjScope::Full)
            .assert("init project with connectors");

        let output_file = case_path.path().join("data/in_dat/gen.dat");
        std::fs::create_dir_all(output_file.parent().unwrap()).expect("dir");
        std::fs::write(&output_file, "payload").expect("write sample");
        assert!(output_file.exists());

        let manager = WpGenManager::new(case_path.path());
        let cleaned = manager
            .clean_outputs(&EnvDict::test_default())
            .expect("clean outputs");
        assert!(cleaned, "expected wpgen data clean to report work done");
        assert!(
            !output_file.exists(),
            "wpgen generated file should be removed"
        );
    }
}
