use orion_conf::{EnvTomlLoad, ErrorOwe, ErrorWith, TomlIO, error::OrionConfResult};
use orion_variate::{EnvDict, EnvEvaluable};
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};
use wp_error::error_handling::RobustnessMode;
use wp_log::conf::LogConf;

use crate::stat::StatConf;

impl EngineConfig {}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct RescueConf {
    #[serde(default = "default_rescue_path")]
    pub path: String,
}

impl Default for RescueConf {
    fn default() -> Self {
        Self {
            path: default_rescue_path(),
        }
    }
}

impl EnvEvaluable<RescueConf> for RescueConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> RescueConf {
        self.path = self.path.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct ModelsConf {
    #[serde(default = "default_wpl_root")]
    pub wpl: String,
    #[serde(default = "default_oml_root")]
    pub oml: String,
}

impl EnvEvaluable<ModelsConf> for ModelsConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> ModelsConf {
        self.wpl = self.wpl.env_eval(dict);
        self.oml = self.oml.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct TopologyConf {
    #[serde(default = "default_sources_root")]
    pub sources: String,
    #[serde(default = "default_sinks_root")]
    pub sinks: String,
}

impl EnvEvaluable<TopologyConf> for TopologyConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> TopologyConf {
        self.sources = self.sources.env_eval(dict);
        self.sinks = self.sinks.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct PerformanceConf {
    #[serde(default = "default_speed_limit")]
    pub rate_limit_rps: usize,
    #[serde(default = "default_parse_workers")]
    pub parse_workers: usize,
}
impl Default for PerformanceConf {
    fn default() -> Self {
        Self {
            rate_limit_rps: 10000,
            parse_workers: 2,
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct EngineConfig {
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    robust: RobustnessMode,
    #[serde(default = "default_models_conf")]
    models: ModelsConf,
    #[serde(default = "default_topology_conf")]
    topology: TopologyConf,
    #[serde(default)]
    performance: PerformanceConf,
    #[serde(default)]
    rescue: RescueConf,
    #[serde(default)]
    log_conf: LogConf,
    // 新版：将原 [stat_conf] 改名为 [stat]；字段保持内部名 stat_conf 以兼容调用方
    #[serde(default, rename = "stat")]
    stat_conf: StatConf,
    /// 是否跳过 PARSE 阶段（不启动解析/采集任务）
    #[serde(default)]
    skip_parse: bool,
    /// 是否跳过 SINK 阶段（不启动 sink/infra 任务；若未进一步配置为黑洞，将阻塞在下发边界）
    #[serde(default)]
    skip_sink: bool,
}

impl EnvEvaluable<EngineConfig> for EngineConfig {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> EngineConfig {
        self.models = self.models.env_eval(dict);
        self.topology = self.topology.env_eval(dict);
        self.rescue = self.rescue.env_eval(dict);
        self
    }
}

// Default values and helper functions
pub fn default_sources_root() -> String {
    "./topology/sources".to_string()
}

pub fn default_version() -> String {
    "1.0".to_string()
}

pub fn default_wpl_root() -> String {
    "./models/wpl".to_string()
}

pub fn default_oml_root() -> String {
    "./models/oml".to_string()
}

pub fn default_sinks_root() -> String {
    "./topology/sinks".to_string()
}

pub fn default_rescue_path() -> String {
    "./data/rescue".to_string()
}

pub fn default_parse_workers() -> usize {
    2
}

pub fn default_speed_limit() -> usize {
    10000
}

pub fn default_topology_conf() -> TopologyConf {
    TopologyConf {
        sources: default_sources_root(),
        sinks: default_sinks_root(),
    }
}

pub fn default_models_conf() -> ModelsConf {
    ModelsConf {
        wpl: default_wpl_root(),
        oml: default_oml_root(),
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            rescue: RescueConf::default(),
            models: default_models_conf(),
            topology: default_topology_conf(),
            performance: PerformanceConf::default(),
            log_conf: LogConf::default(),
            stat_conf: StatConf::default(),
            robust: RobustnessMode::Normal,
            skip_parse: false,
            skip_sink: false,
        }
    }
}

impl EngineConfig {
    pub fn init<P: AsRef<Path>>(_root: P) -> Self {
        Self {
            version: "1.0".to_string(),
            rescue: RescueConf {
                path: default_rescue_path(),
            },
            models: ModelsConf {
                wpl: default_wpl_root(),
                oml: default_oml_root(),
                // Use pluralized roots for sources/sinks; legacy single forms are no longer default
            },
            topology: TopologyConf {
                sources: default_sources_root(),
                sinks: default_sinks_root(),
            },
            performance: PerformanceConf {
                rate_limit_rps: 10000,
                parse_workers: 2,
            },
            log_conf: LogConf::default(),
            stat_conf: StatConf::default(),
            robust: RobustnessMode::Normal,
            skip_parse: false,
            skip_sink: false,
        }
    }

    // Accessors for config fields (prefer using these over direct fields)
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn src_root(&self) -> &str {
        self.topology.sources.as_str()
    }

    pub fn wpl_root(&self) -> &str {
        self.models.wpl.as_str()
    }

    pub fn oml_root(&self) -> &str {
        self.models.oml.as_str()
    }

    pub fn sinks_root(&self) -> &str {
        self.topology.sinks.as_str()
    }

    pub fn robust(&self) -> &RobustnessMode {
        &self.robust
    }

    pub fn parallel(&self) -> usize {
        self.performance.parse_workers
    }

    pub fn speed_limit(&self) -> usize {
        self.performance.rate_limit_rps
    }

    pub fn stat_conf(&self) -> &StatConf {
        &self.stat_conf
    }

    pub fn rule_root(&self) -> &str {
        self.wpl_root()
    }

    // Additional methods that were in the original EngineConfig
    pub fn rescue_root(&self) -> &str {
        &self.rescue.path
    }

    pub fn log_conf(&self) -> &LogConf {
        &self.log_conf
    }

    // 新增阶段控制开关
    pub fn skip_parse(&self) -> bool {
        self.skip_parse
    }
    pub fn skip_sink(&self) -> bool {
        self.skip_sink
    }

    pub fn src_conf_of(&self, file_name: &str) -> String {
        format!("{}/{}", self.src_root(), file_name)
    }

    pub fn conf_absolutize<P: AsRef<Path>>(mut self, work_root: P) -> Self {
        let abs_work_root = work_root.as_ref();
        self.models.wpl = resolve_engine_path(self.models.wpl.as_str(), abs_work_root);
        self.models.oml = resolve_engine_path(self.models.oml.as_str(), abs_work_root);
        self.topology.sources = resolve_engine_path(self.topology.sources.as_str(), abs_work_root);
        self.topology.sinks = resolve_engine_path(self.topology.sinks.as_str(), abs_work_root);
        self.rescue.path = resolve_engine_path(self.rescue.path.as_str(), abs_work_root);
        self
    }

    pub fn load_or_init<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> OrionConfResult<Self> {
        use crate::constants::ENGINE_CONF_FILE;
        let engine_conf_path = work_root.as_ref().join("conf").join(ENGINE_CONF_FILE);
        if engine_conf_path.exists() {
            EngineConfig::env_load_toml(&engine_conf_path, dict)
        } else {
            if let Some(parent) = engine_conf_path.parent() {
                create_dir_all(parent)
                    .owe_res()
                    .want("create path")
                    .with(parent)?;
            }
            let conf = EngineConfig::init(&work_root);
            conf.save_toml(&engine_conf_path)?;
            Ok(conf)
        }
    }
    pub fn load<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> OrionConfResult<Self> {
        use crate::constants::ENGINE_CONF_FILE;
        let engine_conf_path = work_root.as_ref().join("conf").join(ENGINE_CONF_FILE);
        EngineConfig::env_load_toml(&engine_conf_path, dict)
            .want("load engine config")
            .with(ENGINE_CONF_FILE)
    }

    // Add a gen_default method for StatConf compatibility
    pub fn gen_default(&self) -> StatConf {
        StatConf::default()
    }

    // Backward compatibility method
    pub fn sink_root(&self) -> &str {
        self.sinks_root()
    }

    // Add a setter for rule_root if needed
    pub fn set_rule_root(&mut self, _root: String) {
        // This is a no-op since the rule_root is derived from wpl_root
        // The method is kept for compatibility
    }
}

fn resolve_engine_path(value: &str, abs_work_root: &Path) -> String {
    let path = Path::new(value);
    if path.is_absolute() {
        return value.to_string();
    }

    // 拼接路径并规范化，去掉 ./ 和 ../ 等组件
    let joined = abs_work_root.join(path);
    normalize_path(&joined)
}

/// 规范化路径，去掉 . 和 .. 组件
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
    let mut result = PathBuf::new();
    for component in components {
        result.push(component);
    }
    result.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_removes_current_dir() {
        let path = Path::new("/foo/./bar/./baz");
        assert_eq!(normalize_path(path), "/foo/bar/baz");
    }

    #[test]
    fn test_normalize_path_removes_parent_dir() {
        let path = Path::new("/foo/bar/../baz");
        assert_eq!(normalize_path(path), "/foo/baz");
    }

    #[test]
    fn test_normalize_path_complex() {
        let path = Path::new("/foo/./bar/../baz/./qux/../quux");
        assert_eq!(normalize_path(path), "/foo/baz/quux");
    }

    #[test]
    fn test_normalize_path_multiple_current_dirs() {
        let path = Path::new("/foo/././bar/././baz");
        assert_eq!(normalize_path(path), "/foo/bar/baz");
    }

    #[test]
    fn test_normalize_path_already_normalized() {
        let path = Path::new("/foo/bar/baz");
        assert_eq!(normalize_path(path), "/foo/bar/baz");
    }

    #[test]
    fn test_resolve_engine_path_with_current_dir() {
        let work_root = Path::new("/work");
        let result = resolve_engine_path("./topology/sinks", work_root);
        assert_eq!(result, "/work/topology/sinks");
    }

    #[test]
    fn test_resolve_engine_path_absolute() {
        let work_root = Path::new("/work");
        let result = resolve_engine_path("/absolute/path", work_root);
        assert_eq!(result, "/absolute/path");
    }
}
