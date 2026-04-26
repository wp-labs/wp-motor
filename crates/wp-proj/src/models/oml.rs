use orion_conf::ErrorWith;
use orion_error::{ErrorOwe, ToStructError, UvsFrom};
use orion_variate::EnvDict;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_conf::{engine::EngineConfig, utils::find_conf_files};
use wp_engine::facade::config::WPARSE_OML_FILE;
use wp_engine::facade::generator::fetch_oml_data;
use wp_error::run_error::{RunReason, RunResult};

use crate::traits::{Checkable, Component, ComponentBase, ComponentLifecycle, HasExamples};
use crate::types::CheckStatus;
use crate::utils::{TemplateInitializer, error_handler::ErrorHandler};

#[derive(Clone)]
pub struct Oml {
    base: ComponentBase,
}

// Deref to ComponentBase for seamless access to base methods
impl std::ops::Deref for Oml {
    type Target = ComponentBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Oml {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Oml {
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            base: ComponentBase::new(work_root, eng_conf),
        }
    }

    fn oml_root(&self) -> PathBuf {
        self.resolve_path(self.eng_conf().oml_root())
    }

    /// Initialize OML with example content for the specified project directory
    pub fn init_with_examples(&self) -> RunResult<()> {
        let work_root = self.work_root();
        let example_oml_content = include_str!("../example/oml/nginx.oml");
        if !example_oml_content.contains("name") || !example_oml_content.contains("rule") {
            return ErrorHandler::config_error("example OML content is missing essential fields");
        }

        self.create_example_files(work_root)?;

        println!("OML initialized successfully with example content");
        Ok(())
    }

    /// Create example OML files in the specified project directory
    fn create_example_files(&self, _work_root: &Path) -> RunResult<()> {
        let oml_dir = self.oml_root();
        let initializer = TemplateInitializer::new(oml_dir.clone());

        let example_oml_content = include_str!("../example/oml/nginx.oml");
        initializer.write_files(&[("example.oml", example_oml_content)])?;

        println!("Created example OML files:");
        println!("  - {:?}", oml_dir.join("example.oml"));

        Ok(())
    }

    pub fn check(&self, _dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        let oml_root = self.oml_root();
        if !oml_root.exists() {
            return Ok(CheckStatus::Miss);
        }
        let root_str = oml_root.to_str().ok_or_else(|| {
            RunReason::from_conf()
                .to_err()
                .with_detail("oml root path is not valid UTF-8")
        })?;
        let oml_files = find_conf_files(root_str, WPARSE_OML_FILE)
            .owe_conf()
            .with(root_str)
            .want("find oml files")?;
        if oml_files.is_empty() {
            return Ok(CheckStatus::Miss);
        }
        for f in &oml_files {
            ErrorHandler::check_file_not_empty(f, "OML")?;
        }

        // Structural parse via fetch_oml_data (syntax + basic parse)
        fetch_oml_data(root_str, WPARSE_OML_FILE)
            .owe_rule()
            .with(root_str)
            .want("parse oml models")?;

        // Semantic validation: model name uniqueness and rule pattern format
        validate_oml_semantics(&oml_files)?;
        Ok(CheckStatus::Suc)
    }
}

/// Parse OML file head to extract name and rule patterns.
/// Returns (name, rules_vec) from the head section before `---`.
fn parse_oml_head(file_path: &Path) -> Result<(String, Vec<String>), String> {
    let content = std::fs::read_to_string(file_path).map_err(|e| format!("read error: {}", e))?;
    let head: String = content
        .lines()
        .take_while(|line| line.trim() != "---")
        .collect::<Vec<_>>()
        .join("\n");

    let mut name = String::new();
    let mut rules: Vec<String> = Vec::new();

    for line in head.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed
            .strip_prefix("name")
            .and_then(|s| s.trim().strip_prefix(':'))
        {
            name = value.trim().to_string();
        } else if let Some(value) = trimmed
            .strip_prefix("rule")
            .and_then(|s| s.trim().strip_prefix(':'))
        {
            rules.push(value.trim().to_string());
        }
    }

    if name.is_empty() {
        return Err("missing 'name' field in OML head".to_string());
    }
    Ok((name, rules))
}

/// Validate OML semantics: model name uniqueness and rule pattern format.
fn validate_oml_semantics(oml_files: &[PathBuf]) -> RunResult<()> {
    let mut names: BTreeMap<String, String> = BTreeMap::new(); // name -> file

    for fp in oml_files {
        let (name, rules) = match parse_oml_head(fp) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("  ⚠ OML lint skipped for {}: {}", fp.display(), e);
                continue;
            }
        };

        // Check model name uniqueness
        if let Some(prev_file) = names.get(&name) {
            eprintln!(
                "  ⚠ Duplicate OML model name '{}' in {} (previously defined in {})",
                name,
                fp.display(),
                prev_file
            );
        }
        names.insert(name.clone(), fp.display().to_string());

        // Check rule pattern format — only flag truly empty patterns
        for rule in &rules {
            if rule.is_empty() {
                eprintln!(
                    "  ⚠ Empty rule pattern in OML model '{}' in {}",
                    name,
                    fp.display()
                );
            }
        }
    }
    Ok(())
}

// Trait implementations for unified component interface
impl Component for Oml {
    fn component_name(&self) -> &'static str {
        "OML"
    }
}

impl Checkable for Oml {
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        // Delegate to the existing check implementation
        Oml::check(self, dict)
    }
}

impl HasExamples for Oml {
    fn init_with_examples(&self) -> RunResult<()> {
        // Delegate to the existing init_with_examples implementation
        Oml::init_with_examples(self)
    }
}

impl ComponentLifecycle for Oml {
    fn init(&self, _dict: &EnvDict) -> RunResult<()> {
        // OML initialization uses examples by default
        self.init_with_examples()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::temp_workdir;
    use std::sync::Arc;
    use wp_conf::engine::EngineConfig;

    #[test]
    fn initialize_examples_creates_valid_files() {
        let temp = temp_workdir();
        let root = temp.path().to_str().unwrap();
        let eng = Arc::new(EngineConfig::init(root).conf_absolutize(root));
        let oml = Oml::new(root, eng);
        oml.init_with_examples().expect("init examples");

        let example_file = temp.path().join("models/oml/example.oml");
        assert!(example_file.exists());
        assert!(
            !temp.path().join("models/oml/knowdb.toml").exists(),
            "knowdb.toml should not be generated under models/oml"
        );
        assert!(!temp.path().join("models/oml/*.oml").exists());
    }
}
