use orion_error::{ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
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

        // Prepare file contents
        let example_oml_content = include_str!("../example/oml/nginx.oml");
        let knowdb_content = r#"# OML Knowledge Database Configuration
# This file defines the OML models available for use

[[models]]
name = "example_oml"
file = "example.oml"
description = "Example OML model for demonstration purposes"
rule = "/example/*"
"#;

        // Write all files using the initializer
        initializer.write_files(&[
            ("example.oml", example_oml_content),
            ("knowdb.toml", knowdb_content),
        ])?;

        println!("Created example OML files:");
        println!("  - {:?}", oml_dir.join("example.oml"));
        println!("  - {:?}", oml_dir.join("knowdb.toml"));

        Ok(())
    }

    pub fn check(&self, _dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        let oml_root = self.oml_root();
        if !oml_root.exists() {
            return Ok(CheckStatus::Miss);
        }
        let root_str = oml_root
            .to_str()
            .ok_or_else(|| RunReason::from_conf("OML文件路径无效").to_err())?;
        let oml_files = find_conf_files(root_str, WPARSE_OML_FILE)
            .map_err(|e| RunReason::from_conf(format!("OML 查找失败: {}", e)).to_err())?;
        if oml_files.is_empty() {
            return Ok(CheckStatus::Miss);
        }
        for f in &oml_files {
            ErrorHandler::check_file_not_empty(f, "OML")?;
        }

        fetch_oml_data(root_str, WPARSE_OML_FILE)
            .map_err(|e| RunReason::from_conf(format!("parse oml failed: {}", e)).to_err())?;
        Ok(CheckStatus::Suc)
    }
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
        assert!(!temp.path().join("models/oml/*.oml").exists());
    }
}
