use orion_conf::{EnvTomlLoad, ErrorOwe, ErrorWith, TomlIO, error::OrionConfResult};
use orion_error::OperationContext;
use orion_variate::{EnvDict, EnvEvaluable};
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};
use wp_error::diagnostic_meta::{ConfigKind, OperationContextMetaExt, OperationKind};
use wp_error::error_handling::RobustnessMode;
use wp_log::conf::LogConf;

use crate::stat::StatConf;

impl EngineConfig {}

fn engine_load_context(path: &Path) -> OperationContext {
    OperationContext::new()
        .with_meta_value(ConfigKind::Engine)
        .with_meta_value(OperationKind::LoadConfigFile)
        .with_file_path(path)
}

#[derive(Debug, Default, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ProjectRemoteConf {
    #[serde(default, alias = "enable")]
    pub enabled: bool,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub init_version: String,
}

impl EnvEvaluable<ProjectRemoteConf> for ProjectRemoteConf {
    fn env_eval(mut self, dict: &EnvDict) -> ProjectRemoteConf {
        self.repo = self.repo.env_eval(dict);
        self.init_version = self.init_version.env_eval(dict);
        self
    }
}

#[derive(Debug, Default, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AdminApiTlsConf {
    #[serde(default, alias = "enable")]
    pub enabled: bool,
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
}

impl EnvEvaluable<AdminApiTlsConf> for AdminApiTlsConf {
    fn env_eval(mut self, dict: &EnvDict) -> AdminApiTlsConf {
        self.cert_file = self.cert_file.env_eval(dict);
        self.key_file = self.key_file.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AdminApiAuthConf {
    #[serde(default = "default_admin_api_auth_mode")]
    pub mode: String,
    #[serde(default = "default_admin_api_token_file")]
    pub token_file: String,
}

impl Default for AdminApiAuthConf {
    fn default() -> Self {
        Self {
            mode: default_admin_api_auth_mode(),
            token_file: default_admin_api_token_file(),
        }
    }
}

impl EnvEvaluable<AdminApiAuthConf> for AdminApiAuthConf {
    fn env_eval(mut self, dict: &EnvDict) -> AdminApiAuthConf {
        self.mode = self.mode.env_eval(dict);
        self.token_file = self.token_file.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AdminApiConf {
    #[serde(default, alias = "enable")]
    pub enabled: bool,
    #[serde(default = "default_admin_api_bind")]
    pub bind: String,
    #[serde(default = "default_admin_api_request_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default = "default_admin_api_max_body_bytes")]
    pub max_body_bytes: usize,
    #[serde(default)]
    pub tls: AdminApiTlsConf,
    #[serde(default)]
    pub auth: AdminApiAuthConf,
}

impl Default for AdminApiConf {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_admin_api_bind(),
            request_timeout_ms: default_admin_api_request_timeout_ms(),
            max_body_bytes: default_admin_api_max_body_bytes(),
            tls: AdminApiTlsConf::default(),
            auth: AdminApiAuthConf::default(),
        }
    }
}

impl EnvEvaluable<AdminApiConf> for AdminApiConf {
    fn env_eval(mut self, dict: &EnvDict) -> AdminApiConf {
        self.bind = self.bind.env_eval(dict);
        self.tls = self.tls.env_eval(dict);
        self.auth = self.auth.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ModelsConf {
    #[serde(default = "default_wpl_root")]
    pub wpl: String,
    #[serde(default = "default_oml_root")]
    pub oml: String,
    #[serde(default = "default_knowledge_root")]
    pub knowledge: String,
}

impl EnvEvaluable<ModelsConf> for ModelsConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> ModelsConf {
        self.wpl = self.wpl.env_eval(dict);
        self.oml = self.oml.env_eval(dict);
        self.knowledge = self.knowledge.env_eval(dict);
        self
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct PerformanceConf {
    #[serde(default = "default_speed_limit")]
    pub rate_limit_rps: usize,
    #[serde(default = "default_parse_workers")]
    pub parse_workers: usize,
    #[serde(default = "default_reload_timeout_ms")]
    pub reload_timeout_ms: u64,
}
impl Default for PerformanceConf {
    fn default() -> Self {
        Self {
            rate_limit_rps: 10000,
            parse_workers: 2,
            reload_timeout_ms: default_reload_timeout_ms(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SemanticConf {
    #[serde(default, alias = "enable")]
    pub enabled: bool,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
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
    /// 语义分析功能开关（默认关闭，启用后加载 jieba 分词器和语义词典）
    #[serde(default)]
    semantic: SemanticConf,
    /// 远程规则版本更新配置（默认关闭）
    #[serde(default)]
    project_remote: ProjectRemoteConf,
    #[serde(default)]
    admin_api: AdminApiConf,
    /// Legacy sections accepted for compatibility with older project files.
    /// They are intentionally ignored by the current runtime schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    general: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    log: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rule: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sink: Option<toml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oml: Option<toml::Value>,
}

impl EnvEvaluable<EngineConfig> for EngineConfig {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> EngineConfig {
        self.models = self.models.env_eval(dict);
        self.topology = self.topology.env_eval(dict);
        self.rescue = self.rescue.env_eval(dict);
        self.project_remote = self.project_remote.env_eval(dict);
        self.admin_api = self.admin_api.env_eval(dict);
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

pub fn default_knowledge_root() -> String {
    "./models/knowledge".to_string()
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

pub fn default_reload_timeout_ms() -> u64 {
    10_000
}

pub fn default_admin_api_bind() -> String {
    "127.0.0.1:19090".to_string()
}

pub fn default_admin_api_request_timeout_ms() -> u64 {
    15_000
}

pub fn default_admin_api_max_body_bytes() -> usize {
    4096
}

pub fn default_admin_api_auth_mode() -> String {
    "bearer_token".to_string()
}

pub fn default_admin_api_token_file() -> String {
    "${HOME}/.warp_parse/admin_api.token".to_string()
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
        knowledge: default_knowledge_root(),
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
            semantic: SemanticConf::default(),
            project_remote: ProjectRemoteConf::default(),
            admin_api: AdminApiConf::default(),
            general: None,
            log: None,
            rule: None,
            source: None,
            sink: None,
            oml: None,
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
                knowledge: default_knowledge_root(),
                // Use pluralized roots for sources/sinks; legacy single forms are no longer default
            },
            topology: TopologyConf {
                sources: default_sources_root(),
                sinks: default_sinks_root(),
            },
            performance: PerformanceConf {
                rate_limit_rps: 10000,
                parse_workers: 2,
                reload_timeout_ms: default_reload_timeout_ms(),
            },
            log_conf: LogConf::default(),
            stat_conf: StatConf::default(),
            robust: RobustnessMode::Normal,
            skip_parse: false,
            skip_sink: false,
            semantic: SemanticConf::default(),
            project_remote: ProjectRemoteConf::default(),
            admin_api: AdminApiConf::default(),
            general: None,
            log: None,
            rule: None,
            source: None,
            sink: None,
            oml: None,
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

    pub fn knowledge_root(&self) -> &str {
        self.models.knowledge.as_str()
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

    pub fn reload_timeout_ms(&self) -> u64 {
        self.performance.reload_timeout_ms
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

    pub fn semantic(&self) -> &SemanticConf {
        &self.semantic
    }

    pub fn project_remote(&self) -> &ProjectRemoteConf {
        &self.project_remote
    }

    pub fn admin_api(&self) -> &AdminApiConf {
        &self.admin_api
    }

    pub fn src_conf_of(&self, file_name: &str) -> String {
        format!("{}/{}", self.src_root(), file_name)
    }

    pub fn conf_absolutize<P: AsRef<Path>>(mut self, work_root: P) -> Self {
        let abs_work_root = work_root.as_ref();
        self.models.wpl = resolve_engine_path(self.models.wpl.as_str(), abs_work_root);
        self.models.oml = resolve_engine_path(self.models.oml.as_str(), abs_work_root);
        self.models.knowledge = resolve_engine_path(self.models.knowledge.as_str(), abs_work_root);
        self.topology.sources = resolve_engine_path(self.topology.sources.as_str(), abs_work_root);
        self.topology.sinks = resolve_engine_path(self.topology.sinks.as_str(), abs_work_root);
        self.rescue.path = resolve_engine_path(self.rescue.path.as_str(), abs_work_root);
        if !self.admin_api.tls.cert_file.is_empty() {
            self.admin_api.tls.cert_file =
                resolve_engine_path(self.admin_api.tls.cert_file.as_str(), abs_work_root);
        }
        if !self.admin_api.tls.key_file.is_empty() {
            self.admin_api.tls.key_file =
                resolve_engine_path(self.admin_api.tls.key_file.as_str(), abs_work_root);
        }
        if !self.admin_api.auth.token_file.is_empty() {
            self.admin_api.auth.token_file =
                resolve_engine_path(self.admin_api.auth.token_file.as_str(), abs_work_root);
        }
        self
    }

    pub fn load_or_init<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> OrionConfResult<Self> {
        use crate::constants::ENGINE_CONF_FILE;
        let engine_conf_path = work_root.as_ref().join("conf").join(ENGINE_CONF_FILE);
        if engine_conf_path.exists() {
            EngineConfig::env_load_toml(&engine_conf_path, dict)
                .with(engine_load_context(&engine_conf_path))
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
            .with(engine_load_context(&engine_conf_path))
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
    use std::sync::{Mutex, OnceLock};

    fn home_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct HomeOverride {
        original: Option<std::ffi::OsString>,
    }

    impl HomeOverride {
        fn new(home: &Path) -> Self {
            let original = std::env::var_os("HOME");
            unsafe {
                std::env::set_var("HOME", home);
            }
            Self { original }
        }
    }

    impl Drop for HomeOverride {
        fn drop(&mut self) {
            match &self.original {
                Some(home) => unsafe {
                    std::env::set_var("HOME", home);
                },
                None => unsafe {
                    std::env::remove_var("HOME");
                },
            }
        }
    }

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

    #[test]
    fn test_semantic_conf_accepts_enabled_key() {
        let conf: EngineConfig = toml::from_str(
            r#"
            [semantic]
            enabled = true
            "#,
        )
        .expect("parse config with semantic.enabled");
        assert!(conf.semantic().enabled);
    }

    #[test]
    fn test_semantic_conf_accepts_legacy_enable_key() {
        let conf: EngineConfig = toml::from_str(
            r#"
            [semantic]
            enable = true
            "#,
        )
        .expect("parse config with semantic.enable");
        assert!(conf.semantic().enabled);
    }

    #[test]
    fn test_engine_config_rejects_unknown_top_level_field() {
        let err = toml::from_str::<EngineConfig>(
            r#"
            versoin = "1.0"
            "#,
        )
        .expect_err("unknown top-level fields should fail")
        .to_string();
        assert!(err.contains("unknown field"));
        assert!(err.contains("versoin"));
    }

    #[test]
    fn test_engine_config_accepts_legacy_sections() {
        let conf = toml::from_str::<EngineConfig>(
            r#"
            [general]
            work_root = "."

            [log]
            level = "info"

            [rule]
            root = "./models/wpl"

            [source]
            root = "./topology/sources"

            [sink]
            root = "./models/sinks"

            [oml]
            root = "./models/oml"
            "#,
        )
        .expect("known legacy sections should remain loadable");
        assert_eq!(conf.sink_root(), default_sinks_root());
    }

    #[test]
    fn test_engine_config_rejects_unknown_nested_field() {
        let err = toml::from_str::<EngineConfig>(
            r#"
            [performance]
            parse_worker = 4
            "#,
        )
        .expect_err("unknown nested fields should fail")
        .to_string();
        assert!(err.contains("unknown field"));
        assert!(err.contains("parse_worker"));
    }

    #[test]
    fn test_models_conf_accepts_knowledge_root() {
        let conf: EngineConfig = toml::from_str(
            r#"
            [models]
            knowledge = "${CASE_PATH}/models/knowledge"
            "#,
        )
        .expect("parse config with models.knowledge");
        assert_eq!(conf.knowledge_root(), "${CASE_PATH}/models/knowledge");
    }

    #[test]
    fn test_conf_absolutize_resolves_knowledge_root() {
        let conf: EngineConfig = toml::from_str::<EngineConfig>(
            r#"
            [models]
            knowledge = "./custom/knowledge"
            "#,
        )
        .expect("parse config with relative knowledge root")
        .conf_absolutize("/work");
        assert_eq!(conf.knowledge_root(), "/work/custom/knowledge");
    }

    #[test]
    fn test_project_remote_conf_accepts_fields() {
        let conf: EngineConfig = toml::from_str(
            r#"
            [project_remote]
            enabled = true
            repo = "ssh://git@github.com/acme/wp-project.git"
            init_version = "1.4.2"
            "#,
        )
        .expect("parse config with project_remote");
        assert!(conf.project_remote().enabled);
        assert_eq!(
            conf.project_remote().repo,
            "ssh://git@github.com/acme/wp-project.git"
        );
        assert_eq!(conf.project_remote().init_version, "1.4.2");
    }

    #[test]
    fn test_project_remote_conf_accepts_enable_key() {
        let conf: EngineConfig = toml::from_str(
            r#"
            [project_remote]
            enable = true
            "#,
        )
        .expect("parse config with project_remote.enable");
        assert!(conf.project_remote().enabled);
    }

    #[test]
    fn test_engine_config_serializes_project_remote_as_snake_case() {
        let toml = toml::to_string(&EngineConfig::default()).expect("serialize engine config");
        assert!(
            toml.contains("[project_remote]"),
            "serialized config should use snake_case section name"
        );
        assert!(
            !toml.contains("[project-remote]"),
            "serialized config should not use kebab-case section name"
        );
    }

    #[test]
    fn test_admin_api_token_file_env_eval_uses_home() {
        let _guard = home_lock().lock().expect("lock HOME override");
        let temp = tempfile::tempdir().expect("create temp home");
        let _home = HomeOverride::new(temp.path());

        let conf = EngineConfig::default()
            .env_eval(&orion_variate::EnvDict::new())
            .conf_absolutize("/work");

        assert_eq!(
            conf.admin_api().auth.token_file,
            temp.path()
                .join(".warp_parse")
                .join("admin_api.token")
                .display()
                .to_string()
        );
    }

    #[test]
    fn test_conf_absolutize_resolves_admin_api_relative_paths() {
        let conf = EngineConfig {
            admin_api: AdminApiConf {
                enabled: true,
                bind: default_admin_api_bind(),
                request_timeout_ms: default_admin_api_request_timeout_ms(),
                max_body_bytes: default_admin_api_max_body_bytes(),
                tls: AdminApiTlsConf {
                    enabled: true,
                    cert_file: "./tls/server.crt".to_string(),
                    key_file: "./tls/server.key".to_string(),
                },
                auth: AdminApiAuthConf {
                    mode: default_admin_api_auth_mode(),
                    token_file: "runtime/admin_api.token".to_string(),
                },
            },
            ..EngineConfig::default()
        }
        .conf_absolutize("/work");

        assert_eq!(conf.admin_api().tls.cert_file, "/work/tls/server.crt");
        assert_eq!(conf.admin_api().tls.key_file, "/work/tls/server.key");
        assert_eq!(
            conf.admin_api().auth.token_file,
            "/work/runtime/admin_api.token"
        );
    }
}
