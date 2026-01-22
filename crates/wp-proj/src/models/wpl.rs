use orion_error::{ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_conf::engine::EngineConfig;
use wp_engine::facade::config::WPARSE_RULE_FILE;
use wp_error::run_error::{RunReason, RunResult};
use wpl::WplCode;

use crate::traits::{Checkable, Component, ComponentBase, ComponentLifecycle, HasExamples};
use crate::types::CheckStatus;
use crate::utils::TemplateInitializer;

#[derive(Clone)]
pub struct Wpl {
    base: ComponentBase,
}

// Deref to ComponentBase for seamless access to base methods
impl std::ops::Deref for Wpl {
    type Target = ComponentBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Wpl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Wpl {
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            base: ComponentBase::new(work_root, eng_conf),
        }
    }

    fn rule_root(&self) -> PathBuf {
        self.resolve_path(self.eng_conf().rule_root())
    }

    /// Initialize WPL with example content for the specified project directory
    pub fn init_with_examples(&self) -> RunResult<()> {
        let work_root = self.work_root();
        // Include example WPL content using include_str!
        let example_wpl_content = include_str!("../example/wpl/nginx/parse.wpl");

        // Parse the example WPL content to validate it
        let code = WplCode::build(
            PathBuf::from("example/nginx/parse.wpl"),
            example_wpl_content,
        )
        .map_err(|e| RunReason::from_conf(format!("build example wpl failed: {}", e)).to_err())?;

        let _pkg = code.parse_pkg().map_err(|e| {
            RunReason::from_conf(format!("parse example wpl failed: {}", e)).to_err()
        })?;

        // Create WPL directory and example files
        self.create_example_files(work_root)?;

        println!("WPL initialized successfully with example content and sample data");
        Ok(())
    }

    /// Create example WPL files in the specified project directory
    fn create_example_files(&self, _work_root: &Path) -> RunResult<()> {
        let wpl_dir = self.rule_root();
        let initializer = TemplateInitializer::new(wpl_dir.clone());

        // Prepare file contents
        let example_wpl_content = include_str!("../example/wpl/nginx/parse.wpl");
        let sample_data = Self::get_sample_data();

        // Write all files using the initializer
        initializer.write_files(&[
            ("parse.wpl", example_wpl_content),
            ("sample.dat", sample_data),
        ])?;

        println!("Created example WPL files:");
        println!("  - {:?}", wpl_dir.join("parse.wpl"));
        println!("  - {:?}", wpl_dir.join("sample.dat"));

        Ok(())
    }

    /// Get the sample data content as a string
    pub fn get_sample_data() -> &'static str {
        include_str!("../example/wpl/nginx/sample.dat")
    }

    pub fn check(&self, _dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        let rule_root = self.rule_root();
        let rules =
            wp_conf::utils::find_conf_files(rule_root.to_string_lossy().as_ref(), WPARSE_RULE_FILE)
                .unwrap_or_default();

        // 如果没有找到规则文件，尝试手动查找 *.wpl 文件
        if rules.is_empty() {
            let absolute_rule_root = self.rule_root();
            let wpl_pattern = format!("{}/*.wpl", absolute_rule_root.display());

            if let Ok(glob_results) = glob::glob(&wpl_pattern) {
                let wpl_files: Vec<_> = glob_results.filter_map(Result::ok).collect();

                if !wpl_files.is_empty() {
                    // 使用找到的 .wpl 文件
                    for fp in wpl_files {
                        let raw = std::fs::read_to_string(&fp).unwrap_or_default();
                        if raw.trim().is_empty() {
                            return Err(RunReason::from_conf(format!(
                                "配置错误: WPL文件为空: {:?}",
                                fp
                            ))
                            .to_err());
                        }
                        let code = WplCode::build(fp.clone(), raw.as_str()).map_err(|e| {
                            RunReason::from_conf(format!("build wpl failed: {:?}: {}", fp, e))
                                .to_err()
                        })?;
                        let _pkg = code.parse_pkg().map_err(|e| {
                            RunReason::from_conf(format!("parse wpl failed: {:?}: {}", fp, e))
                                .to_err()
                        })?;
                    }
                    return Ok(CheckStatus::Suc);
                }
            }
        }

        // 检查是否有任何WPL规则文件存在
        if rules.is_empty() {
            return Ok(CheckStatus::Miss);
        }

        for fp in rules {
            let raw = std::fs::read_to_string(&fp).unwrap_or_default();
            if raw.trim().is_empty() {
                return Err(
                    RunReason::from_conf(format!("配置错误: WPL文件为空: {:?}", fp)).to_err(),
                );
            }
            let code = WplCode::build(fp.clone(), raw.as_str()).map_err(|e| {
                RunReason::from_conf(format!("build wpl failed: {:?}: {}", fp, e)).to_err()
            })?;
            let _pkg = code.parse_pkg().map_err(|e| {
                RunReason::from_conf(format!("parse wpl failed: {:?}: {}", fp, e)).to_err()
            })?;
        }
        Ok(CheckStatus::Suc)
    }
}

// Trait implementations for unified component interface
impl Component for Wpl {
    fn component_name(&self) -> &'static str {
        "WPL"
    }
}

impl Checkable for Wpl {
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        // Delegate to the existing check implementation
        Wpl::check(self, dict)
    }
}

impl HasExamples for Wpl {
    fn init_with_examples(&self) -> RunResult<()> {
        // Delegate to the existing init_with_examples implementation
        Wpl::init_with_examples(self)
    }
}

impl ComponentLifecycle for Wpl {
    fn init(&self, _dict: &EnvDict) -> RunResult<()> {
        // WPL initialization uses examples by default
        self.init_with_examples()
    }
}
