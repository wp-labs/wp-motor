//! Sources Management Module
//!
//! This module provides comprehensive source management functionality including
//! validation, initialization, and routing operations for data sources
//! in the Warp Flow System.

use crate::compat::{ErrorConv, ErrorOweBase, UvsFrom};
use orion_conf::{EnvTomlLoad, ErrorWith, TomlIO};
use orion_error::conversion::ToStructError;
use orion_variate::EnvDict;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_cli_core::business::connectors::sources as sources_core;
use wp_conf::sources::types::{SourceItem, WarpSources};
use wp_conf::structure::SourceInstanceConf;
use wp_conf::{engine::EngineConfig, sources::build::load_source_instances_from_file};
use wp_engine::facade::config::WPSRC_TOML;
use wp_error::run_error::{RunReason, RunResult};

// Re-export modules and types
pub use super::source_builder::source_builders;

use crate::traits::{Checkable, Component, ComponentBase, ComponentLifecycle, HasStatistics};
use crate::types::CheckStatus;

/// Constants for default source configurations
pub const DEFAULT_FILE_SOURCE_KEY: &str = "file_1";
pub const DEFAULT_FILE_SOURCE_PATH: &str = "gen*.dat";
pub const DEFAULT_SYSLOG_SOURCE_ID: &str = "syslog_1";
pub const DEFAULT_SYSLOG_HOST: &str = "0.0.0.0";
pub const DEFAULT_SYSLOG_PORT: i64 = 1514;

/// Sources management system for data source operations
///
/// The `Sources` struct provides a centralized interface for managing all
/// source-related operations including validation, initialization, and routing
/// of data sources within the project.
#[derive(Clone)]
pub struct Sources {
    base: ComponentBase,
}

// Deref to ComponentBase for seamless access to base methods
impl std::ops::Deref for Sources {
    type Target = ComponentBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Sources {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Sources {
    /// Creates a new Sources instance
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            base: ComponentBase::new(work_root, eng_conf),
        }
    }

    fn sources_root(&self) -> PathBuf {
        self.resolve_path(self.eng_conf().src_root())
    }

    pub fn check(&self, dict: &EnvDict) -> RunResult<CheckStatus> {
        let sources_dir = self.sources_root();

        // Verify directory exists and has configuration (old wpsrc.toml or new .toml files)
        if !sources_dir.exists() {
            return Err(RunReason::from_conf().to_err().with_detail(format!(
                "sources directory not found: {}",
                sources_dir.display()
            )));
        }

        // Parse and validate configuration
        self.validate_wpsrc_config(self.work_root(), &sources_dir, dict)?;

        // Attempt to build specifications to ensure they are valid
        self.build_source_specs(&sources_dir, dict)?;

        eprintln!("✓ Sources configuration validation passed");
        Ok(CheckStatus::Suc)
    }

    pub fn init(&self, dict: &EnvDict) -> RunResult<()> {
        let sources_dir = self.sources_root();
        let wpsrc_path = sources_dir.join(WPSRC_TOML);

        // Ensure parent directory exists
        self.ensure_directory_exists(&sources_dir)?;

        // Backward compat: if wpsrc.toml already exists, use old format
        if wpsrc_path.exists() {
            let mut sources_config = self.load_or_create_config(&wpsrc_path, dict)?;
            self.add_default_sources(&mut sources_config)?;
            sources_config
                .save_toml(&wpsrc_path)
                .owe(RunReason::from_conf())
                .with_context(&wpsrc_path)
                .doing("save sources config")?;
        } else {
            // New format: one .toml per source, no [[sources]] wrapper
            let default_sources = self.default_source_items();
            for source in default_sources {
                let file_path = sources_dir.join(format!("{}.toml", source.key));
                if file_path.exists() {
                    continue; // don't overwrite existing source files
                }
                let content = toml::to_string_pretty(&source)
                    .owe(RunReason::from_conf())
                    .with_context(&file_path)
                    .doing("serialize source config")?;
                fs::write(&file_path, content)
                    .owe(RunReason::from_conf())
                    .with_context(&file_path)
                    .doing("write source config")?;
            }
        }

        println!("✓ Sources initialization completed");
        Ok(())
    }

    fn validate_wpsrc_config(
        &self,
        _work_root: &Path,
        sources_path: &Path,
        dict: &EnvDict,
    ) -> RunResult<()> {
        // Use load_source_instances_from_file which handles both:
        // - old format: wpsrc.toml with [[sources]] array
        // - new format: directory with individual .toml files
        load_source_instances_from_file(sources_path, dict)
            .owe(RunReason::from_conf())
            .with_context(sources_path)
            .doing("validate sources config")?;
        Ok(())
    }

    /// Builds source specifications for validation
    fn build_source_specs(&self, sources_path: &Path, dict: &EnvDict) -> RunResult<()> {
        let specs = load_source_instances_from_file(sources_path, dict)
            .owe(RunReason::from_conf())
            .with_context(sources_path)
            .doing("build source instances")?;
        self.validate_source_file_paths(&specs)?;
        Ok(())
    }

    /// 校验 file 类型 source 的文件路径存在性，以及 syslog/tcp 的端口范围
    fn validate_source_file_paths(&self, specs: &[SourceInstanceConf]) -> RunResult<()> {
        let work_root = self.work_root().to_path_buf();

        for spec in specs {
            let kind = spec.core.kind.as_str();
            let name = spec.core.name.as_str();

            match kind {
                "file" => {
                    let base = spec
                        .core
                        .params
                        .get("base")
                        .and_then(|v| v.as_str())
                        .unwrap_or("./data/in_dat");
                    let file = spec.core.params.get("file").and_then(|v| v.as_str());

                    let base_path = if Path::new(base).is_absolute() {
                        PathBuf::from(base)
                    } else {
                        work_root.join(base)
                    };

                    if !base_path.exists() {
                        eprintln!(
                            "  ⚠ source '{}' (file): base directory '{}' does not exist",
                            name,
                            base_path.display()
                        );
                        continue;
                    }

                    match file {
                        Some(file_val) => {
                            if has_glob_pattern(file_val) {
                                let pattern_str =
                                    base_path.join(file_val).to_string_lossy().to_string();
                                match glob::glob(&pattern_str) {
                                    Ok(mut paths) => {
                                        let has_match = paths.any(|r| {
                                            r.as_ref().map(|p| p.is_file()).unwrap_or(false)
                                        });
                                        if !has_match {
                                            eprintln!(
                                                "  ⚠ source '{}' (file): no files matched glob pattern '{}'",
                                                name, pattern_str
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        return Err(RunReason::from_conf().to_err().with_detail(
                                            format!(
                                                "source '{}' (file): invalid glob pattern '{}': {}",
                                                name, pattern_str, e
                                            ),
                                        ));
                                    }
                                }
                            } else {
                                let file_path = base_path.join(file_val);
                                if !file_path.exists() || !file_path.is_file() {
                                    eprintln!(
                                        "  ⚠ source '{}' (file): file '{}' does not exist",
                                        name,
                                        file_path.display()
                                    );
                                }
                            }
                        }
                        None => {
                            return Err(RunReason::from_conf().to_err().with_detail(format!(
                                "source '{}' (file): missing required 'file' parameter",
                                name
                            )));
                        }
                    }
                }
                "syslog" | "tcp" => {
                    if let Some(port) = spec.core.params.get("port").and_then(|v| v.as_i64())
                        && !(1..=65535).contains(&port)
                    {
                        return Err(RunReason::from_conf().to_err().with_detail(format!(
                            "source '{}' ({}): port {} is out of valid range [1, 65535]",
                            name, kind, port
                        )));
                    }
                    if let Some(proto) = spec.core.params.get("protocol").and_then(|v| v.as_str()) {
                        match proto.to_lowercase().as_str() {
                            "tcp" | "udp" => {}
                            _ => {
                                return Err(RunReason::from_conf()
                                    .to_err()
                                    .with_detail(format!(
                                        "source '{}' ({}): unsupported protocol '{}'; expected tcp or udp",
                                        name, kind, proto
                                    )));
                            }
                        }
                    }
                }
                _ => {} // 其他 source 类型由其 factory 校验
            }
        }
        Ok(())
    }

    /// Loads existing configuration or creates new empty one
    fn load_or_create_config(&self, config_path: &Path, dict: &EnvDict) -> RunResult<WarpSources> {
        if config_path.exists() {
            WarpSources::env_load_toml(config_path, dict)
                .owe(RunReason::from_conf())
                .with_context(config_path)
                .doing("load sources config")
        } else {
            Ok(WarpSources { sources: vec![] })
        }
    }

    fn add_default_sources(&self, config: &mut WarpSources) -> RunResult<()> {
        for source in self.default_source_items() {
            Self::ensure_source_exists(config, source);
        }
        Ok(())
    }

    fn default_source_items(&self) -> Vec<SourceItem> {
        vec![
            source_builders::file_source(DEFAULT_FILE_SOURCE_KEY, DEFAULT_FILE_SOURCE_PATH),
            source_builders::syslog_tcp_source(
                DEFAULT_SYSLOG_SOURCE_ID,
                DEFAULT_SYSLOG_HOST,
                DEFAULT_SYSLOG_PORT,
            )
            .with_enable(Some(false)),
        ]
    }

    /// Adds a new source only if an entry with the same key is not present
    fn ensure_source_exists(config: &mut WarpSources, source_item: SourceItem) {
        if config.sources.iter().any(|s| s.key == source_item.key) {
            return;
        }
        config.sources.push(source_item);
    }

    // =================== PROJECT MANAGEMENT ===================

    /// Ensures parent directory exists for configuration file
    fn ensure_directory_exists(&self, config_path: &Path) -> RunResult<()> {
        let dir = if config_path.is_dir() {
            config_path.to_path_buf()
        } else if let Some(parent) = config_path.parent() {
            parent.to_path_buf()
        } else {
            config_path.to_path_buf()
        };
        fs::create_dir_all(&dir)
            .owe(RunReason::from_conf())
            .with_context(&dir)
            .doing("create sources config directory")?;
        Ok(())
    }

    // =================== DISPLAY METHODS ===================

    /// Displays sources information in JSON format
    pub fn display_as_json(&self, rows: &[sources_core::RouteRow]) {
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "key": r.key,
                    "kind": r.kind,
                    "enabled": r.enabled,
                    "detail": r.detail
                })
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
    }

    /// Displays sources information in table format
    pub fn display_as_table(&self, rows: &[sources_core::RouteRow]) {
        use comfy_table::{Cell as TCell, ContentArrangement, Table};

        let mut table = Table::new();
        table.load_preset(comfy_table::presets::UTF8_FULL);
        table.set_content_arrangement(ContentArrangement::Dynamic);
        table.set_width(120);
        table.set_header(vec![
            TCell::new("key"),
            TCell::new("kind"),
            TCell::new("on"),
            TCell::new("detail"),
        ]);

        for row in rows {
            table.add_row(vec![
                TCell::new(&row.key),
                TCell::new(&row.kind),
                TCell::new(if row.enabled { "on" } else { "off" }),
                TCell::new(&row.detail),
            ]);
        }

        println!("{}", table);
        println!("total: {}", rows.len());
    }
}

// Trait implementations for unified component interface
impl Component for Sources {
    fn component_name(&self) -> &'static str {
        "Sources"
    }
}

impl Checkable for Sources {
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        // Delegate to the existing check implementation
        Sources::check(self, dict)
    }
}

impl HasStatistics for Sources {
    fn has_statistics(&self) -> bool {
        let dir = self.sources_root();
        dir.exists()
            && dir
                .read_dir()
                .map(|mut d| d.next().is_some())
                .unwrap_or(false)
    }
}

impl ComponentLifecycle for Sources {
    fn init(&self, dict: &EnvDict) -> RunResult<()> {
        // Delegate to the existing init implementation
        Sources::init(self, dict)
    }
}

/// 判断字符串是否包含 glob 通配符
fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

// =================== TESTS ===================

#[cfg(test)]
mod tests {

    use crate::test_utils::temp_workdir;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_sources_creation() {
        let temp = temp_workdir();
        let eng = std::sync::Arc::new(EngineConfig::init(temp.path()).conf_absolutize(temp.path()));
        let _sources = Sources::new(temp.path(), eng);
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_FILE_SOURCE_KEY, "file_1");
        assert_eq!(DEFAULT_SYSLOG_SOURCE_ID, "syslog_1");
        assert_eq!(DEFAULT_SYSLOG_HOST, "0.0.0.0");
        assert_eq!(DEFAULT_SYSLOG_PORT, 1514);
    }

    #[test]
    fn add_default_sources_skips_existing_entries() {
        let mut config = WarpSources {
            sources: Vec::new(),
        };
        // first insert default file source manually with custom param
        let mut custom = source_builders::file_source(DEFAULT_FILE_SOURCE_KEY, "custom.dat");
        custom.params.insert("base".into(), json!("custom_base"));
        config.sources.push(custom);

        Sources::ensure_source_exists(
            &mut config,
            source_builders::file_source(DEFAULT_FILE_SOURCE_KEY, DEFAULT_FILE_SOURCE_PATH),
        );

        let stored = config
            .sources
            .iter()
            .find(|s| s.key == DEFAULT_FILE_SOURCE_KEY)
            .unwrap();
        assert_eq!(
            stored.params.get("base").and_then(|v| v.as_str()),
            Some("custom_base")
        );
    }
}
