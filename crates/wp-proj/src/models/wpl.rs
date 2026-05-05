use crate::compat::{ErrorOweBase, UvsFrom};
use orion_conf::ErrorWith;
use orion_error::conversion::ToStructError;
use orion_variate::EnvDict;
use std::collections::{BTreeMap, BTreeSet};
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
        .owe(RunReason::from_conf())
        .with_context("example/nginx/parse.wpl")
        .doing("build example wpl")?;

        let _pkg = code
            .parse_pkg()
            .owe(RunReason::from_conf())
            .with_context("example/nginx/parse.wpl")
            .doing("parse example wpl")?;

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
                    let (_pkg_names, _rule_names) = parse_and_collect_wpl_files(&wpl_files)?;
                    return Ok(CheckStatus::Suc);
                }
            }
        }

        // 检查是否有任何WPL规则文件存在
        if rules.is_empty() {
            return Ok(CheckStatus::Miss);
        }

        let (_pkg_names, _rule_names) = parse_and_collect_wpl_files(&rules)?;
        Ok(CheckStatus::Suc)
    }
}

/// Parse WPL files and collect package names and fully-qualified rule names.
/// Returns (package_names, rule_names) where rule_names are "pkg::rule".
fn parse_and_collect_wpl_files(
    files: &[PathBuf],
) -> RunResult<(BTreeMap<String, String>, BTreeSet<String>)> {
    let mut pkg_names: BTreeMap<String, String> = BTreeMap::new(); // package_name -> file
    let mut rule_names: BTreeSet<String> = BTreeSet::new();

    for fp in files {
        let raw = match std::fs::read_to_string(fp) {
            Ok(s) => s,
            Err(e) => {
                return Err(RunReason::from_conf().to_err().with_detail(format!(
                    "wpl file read error: {}: {}",
                    fp.display(),
                    e
                )));
            }
        };
        if raw.trim().is_empty() {
            return Err(RunReason::from_conf()
                .to_err()
                .with_detail(format!("wpl file is empty: {}", fp.display())));
        }
        let code = WplCode::build(fp.clone(), raw.as_str())
            .owe(RunReason::from_conf())
            .with_context(fp)
            .doing("build wpl code")?;
        let pkg = code
            .parse_pkg()
            .owe(RunReason::from_conf())
            .with_context(fp)
            .doing("parse wpl package")?;

        // Check for empty package
        if pkg.rules.is_empty() {
            eprintln!(
                "  ⚠ WPL package '{}' has no rules in {}",
                pkg.name,
                fp.display()
            );
        }

        // Check for rules with empty field groups (parse nothing)
        for rule in &pkg.rules {
            if rule.statement.first_field().is_none() {
                eprintln!(
                    "  ⚠ WPL rule '{}::{}' has no fields to parse in {}",
                    pkg.name,
                    rule.name,
                    fp.display()
                );
            }
        }

        // Register package name; detect duplicates
        let pkg_name = pkg.name.to_string();
        if let Some(prev_file) = pkg_names.get(&pkg_name) {
            eprintln!(
                "  ⚠ Duplicate WPL package '{}' in {} (previously defined in {})",
                pkg_name,
                fp.display(),
                prev_file
            );
        }
        pkg_names.insert(pkg_name, fp.display().to_string());

        // Register rule names with package prefix
        for rule in &pkg.rules {
            let fq_name = format!("{}::{}", pkg.name, rule.name);
            if !rule_names.insert(fq_name.clone()) {
                eprintln!(
                    "  ⚠ Duplicate WPL rule '{}' in {} (previously defined elsewhere)",
                    fq_name,
                    fp.display()
                );
            }
        }
    }
    Ok((pkg_names, rule_names))
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
