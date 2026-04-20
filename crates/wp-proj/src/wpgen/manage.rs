use std::path::Path;

use orion_error::{ToStructError, UvsFrom, WrapStructError};
use orion_variate::EnvDict;
use wp_error::{RunReason, RunResult};

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
                        Err(RunReason::from_conf()
                            .to_err()
                            .with_detail(format!("清理 wpgen 数据失败: {}", path)))
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
            Err(e) => Err(e
                .wrap(RunReason::from_conf())
                .with_detail("清理 wpgen 数据失败")),
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
    use tempfile::tempdir;
    use wp_conf::test_support::ForTest;
    use wp_error::diagnostic_meta::{ComponentKind, ConfigKind, MetaValue, first_meta_str, key};

    use super::*;
    use crate::project::{WarpProject, init::PrjScope};
    use crate::wpgen::gen_conf_init;
    use crate::wpgen::load_wpgen_resolved;
    use wp_engine::facade::config::WarpConf;

    fn sharded_output(path: &Path, idx: usize) -> std::path::PathBuf {
        let parent = path.parent().expect("output parent");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("output file name");
        let sharded = if let Some((stem, ext)) = name.rsplit_once('.') {
            format!("{stem}-r{idx}.{ext}")
        } else {
            format!("{name}-r{idx}")
        };
        parent.join(sharded)
    }

    #[test]
    fn clean_outputs_remove_file_sink_outputs() {
        let case_path = tempdir().expect("test path");
        let mut project = WarpProject::bare(case_path.path());
        project
            .init_basic(PrjScope::Full)
            .assert("init project with connectors");

        let wpgen_conf = case_path.path().join("conf/wpgen.toml");
        if !wpgen_conf.exists() {
            gen_conf_init(case_path.path()).expect("init wpgen config");
        }

        let god = WarpConf::new(case_path.path());
        let resolved = load_wpgen_resolved("wpgen.toml", &god, &EnvDict::test_default())
            .expect("resolve wpgen output");
        let output_file = case_path
            .path()
            .join(resolved.out_sink.resolve_file_path().expect("output path"));
        std::fs::create_dir_all(output_file.parent().unwrap()).expect("dir");
        let shard0 = sharded_output(&output_file, 0);
        let shard1 = sharded_output(&output_file, 1);
        std::fs::write(&shard0, "payload-0").expect("write shard 0");
        std::fs::write(&shard1, "payload-1").expect("write shard 1");
        assert!(shard0.exists());
        assert!(shard1.exists());

        let manager = WpGenManager::new(case_path.path());
        let cleaned = manager
            .clean_outputs(&EnvDict::test_default())
            .expect("clean outputs");
        assert!(cleaned, "expected wpgen data clean to report work done");
        assert!(!shard0.exists(), "wpgen shard 0 should be removed");
        assert!(!shard1.exists(), "wpgen shard 1 should be removed");
    }

    #[test]
    fn clean_outputs_reports_delete_failures() {
        let case_path = tempdir().expect("test path");
        let mut project = WarpProject::bare(case_path.path());
        project
            .init_basic(PrjScope::Full)
            .assert("init project with connectors");

        let wpgen_conf = case_path.path().join("conf/wpgen.toml");
        if !wpgen_conf.exists() {
            gen_conf_init(case_path.path()).expect("init wpgen config");
        }

        let god = WarpConf::new(case_path.path());
        let resolved = load_wpgen_resolved("wpgen.toml", &god, &EnvDict::test_default())
            .expect("resolve wpgen output");
        let output_file = case_path
            .path()
            .join(resolved.out_sink.resolve_file_path().expect("output path"));
        std::fs::create_dir_all(&output_file).expect("create blocking dir");

        let manager = WpGenManager::new(case_path.path());
        let err = manager
            .clean_outputs(&EnvDict::test_default())
            .expect_err("directory at output path should fail remove_file");
        let detail = err.detail().clone().unwrap_or_default();
        let chain = err.display_chain();
        let report = err.report();
        let output_file_str = output_file.display().to_string();
        assert!(detail.contains("清理 wpgen 数据失败"));
        assert!(chain.contains("remove wpgen output"));
        assert!(chain.contains(output_file.to_string_lossy().as_ref()));
        assert_eq!(
            first_meta_str(&report, key::CONFIG_KIND),
            Some(ConfigKind::SinkRuntime.as_str())
        );
        assert_eq!(
            first_meta_str(&report, key::COMPONENT_KIND),
            Some(ComponentKind::Generator.as_str())
        );
        assert_eq!(first_meta_str(&report, key::COMPONENT_NAME), Some("wpgen"));
        assert_eq!(
            first_meta_str(&report, key::FILE_PATH),
            Some(output_file_str.as_str())
        );
    }
}
