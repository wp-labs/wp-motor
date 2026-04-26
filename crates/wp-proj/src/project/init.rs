use super::warp::{WarpProject, normalize_work_root};
use crate::utils::error_handler::ErrorHandler;
use orion_conf::{EnvTomlLoad, ErrorOwe, ToStructError, TomlIO};
use orion_error::UvsFrom;
use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use wp_conf::paths::{OUT_FILE_PATH, RESCURE_FILE_PATH, SRC_FILE_PATH};
use wp_conf::{engine::EngineConfig, generator::wpgen::WpGenConfig};
use wp_engine::facade::config::WPARSE_LOG_PATH;
use wp_error::{RunError, RunReason};
//use wp_engine::orchestrator::config::models::warp::core::EngineConfig;
//use wp_engine::orchestrator::config::SRC_FILE_PATH;
use wp_error::run_error::RunResult;

const CONF_DIR: &str = "conf";
const CONF_WPARSE_FILE: &str = "conf/wparse.toml";
const CONF_WPGEN_FILE: &str = "conf/wpgen.toml";
const MODELS_WPL_DIR: &str = "models/wpl";
const MODELS_OML_DIR: &str = "models/oml";
const MODELS_KNOWLEDGE_DIR: &str = "models/knowledge";
const MODELS_KNOWLEDGE_EXAMPLE_DIR: &str = "models/knowledge/example";
const SEMANTIC_DICT_FILE: &str = "models/knowledge/semantic_dict.toml";
const TOPOLOGY_SOURCES_DIR: &str = "topology/sources";
const TOPOLOGY_SINKS_DIR: &str = "topology/sinks";
const ADMIN_API_TOKEN_ENV_REF: &str = "${HOME}/.warp_parse/admin_api.token";
const ADMIN_API_TOKEN_FALLBACK_FILE: &str = "runtime/admin_api.token";
const ADMIN_API_TOKEN_HOME_DIR: &str = ".warp_parse";
const ADMIN_API_TOKEN_NAME: &str = "admin_api.token";

fn default_admin_api_block() -> String {
    format!(
        r#"
[admin_api]
enabled = false
bind = "127.0.0.1:19090"
request_timeout_ms = 15000
max_body_bytes = 4096

[admin_api.tls]
enabled = false
cert_file = ""
key_file = ""

[admin_api.auth]
mode = "bearer_token"
token_file = "{ADMIN_API_TOKEN_ENV_REF}"
"#
    )
}

fn resolve_admin_api_token_path(work_root: &Path) -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| {
            home.join(ADMIN_API_TOKEN_HOME_DIR)
                .join(ADMIN_API_TOKEN_NAME)
        })
        .unwrap_or_else(|| work_root.join(ADMIN_API_TOKEN_FALLBACK_FILE))
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum PrjScope {
    Full,
    Normal,
    Model,
    Topology,
    Conf,
    Data,
}
impl PrjScope {
    pub fn enable_connector(&self) -> bool {
        *self == PrjScope::Full
    }
    pub fn enable_model(&self) -> bool {
        //*self == InitMode::Model || *self == InitMode::Full
        matches!(self, PrjScope::Model | PrjScope::Full | PrjScope::Normal)
    }
    pub fn enable_conf(&self) -> bool {
        matches!(self, PrjScope::Conf | PrjScope::Full | PrjScope::Normal)
    }
    pub fn enable_topology(&self) -> bool {
        matches!(self, PrjScope::Topology | PrjScope::Full | PrjScope::Normal)
    }
}

impl FromStr for PrjScope {
    type Err = RunError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s {
            "full" => Self::Full,
            "normal" => Self::Normal,
            "model" => Self::Model,
            "conf" => Self::Conf,
            "topology" => Self::Topology,
            "data" => Self::Data,
            _ => {
                return Err(RunReason::from_validation()
                    .to_err()
                    .with_detail("not init mode"));
            }
        };
        Ok(mode)
    }
}

impl WarpProject {
    // ========== 初始化方法 ==========

    /// 初始化 WPL 和 OML 模型（基于示例文件）
    pub fn init_models(&mut self) -> RunResult<()> {
        self.wpl().init_with_examples()?;
        self.oml().init_with_examples()?;
        Ok(())
    }

    /// 完整的项目初始化：包括配置、模型和所有组件
    pub(crate) fn init_components(&mut self, mode: PrjScope) -> RunResult<()> {
        // 1) 先进行基础项目初始化（包括目录创建、配置、连接器、wpgen配置等）
        self.init_basic(mode.clone())?;

        // 2) 知识库目录骨架初始化（仅在模型启用时，因为知识库是模型的一部分）
        if mode.enable_model() {
            self.knowledge().init(self.work_root())?;
            // 语义词典配置初始化（属于知识配置）
            Self::init_semantic_dict_config(self.work_root_path())?;
        }

        // 3) WPL 和 OML 模型初始化（仅在模型启用时）
        if mode.enable_model() {
            self.init_models()?;
        }

        println!("✓ 项目初始化完成");

        Ok(())
    }

    pub(crate) fn load_components(&mut self, mode: PrjScope) -> RunResult<()> {
        if mode.enable_conf() {
            let eng_conf = Self::load_engine_config_only(self.work_root_path(), &self.dict)?;
            self.replace_engine_conf(eng_conf);
            Self::load_wpgen_config_only(self.work_root_path(), &self.dict)?;
        }
        if mode.enable_connector() {
            self.connectors()
                .check(self.work_root(), &self.dict)
                .map(|_| ())?;
        }
        if mode.enable_topology() {
            self.sinks_c().check(&self.dict)?;
            self.sources_c().check(&self.dict)?;
        }
        if mode.enable_model() {
            self.wpl().check(&self.dict)?;
            let _ = self.oml().check(&self.dict)?;
        }
        Ok(())
    }

    /// 仅初始化基础项目结构（不包括模型）
    pub fn init_basic(&mut self, mode: PrjScope) -> RunResult<()> {
        // 1) 基础配置和数据目录初始化
        //let conf_manager = WarpConf::new(self.work_root());
        self.mk_framework_dir(mode.clone())?;

        if mode.enable_conf() {
            // wparse/wpgen 主配置初始化（如不存在则复制示例文件）
            let eng_conf = Self::init_engine_config(self.work_root_path(), &self.dict)?;
            self.replace_engine_conf(eng_conf);
            Self::init_wpgen_config(self.work_root_path())?;
            Self::init_admin_api_token(self.work_root_path())?;
        }

        // 连接器模板初始化
        if mode.enable_connector() {
            self.connectors().init_definition(self.work_root())?;
        }
        if mode.enable_topology() {
            // 输出接收器骨架初始化
            self.sinks_c().init()?;
            // 输入源和连接器补齐
            self.sources_c().init(&self.dict)?;
            // 知识库目录骨架初始化
        }

        if mode.enable_model() {
            self.knowledge().init(self.work_root())?;
        }

        // 模型目录结构已预创建，跳过此步骤

        println!("✓ 基础项目初始化完成");
        Ok(())
    }

    /// 初始化 wpgen 配置文件
    fn init_wpgen_config<P: AsRef<Path>>(work_root: P) -> RunResult<()> {
        let work_root = work_root.as_ref();
        let conf_dir = work_root.join(CONF_DIR);
        ErrorHandler::safe_create_dir(&conf_dir)?;

        let wpgen_config_path = work_root.join(CONF_WPGEN_FILE);
        if !wpgen_config_path.exists() {
            let wpgen_config_content = include_str!("../example/conf/wpgen.toml");
            ErrorHandler::safe_write_file(&wpgen_config_path, wpgen_config_content)?;
        }

        Ok(())
    }

    fn init_admin_api_token<P: AsRef<Path>>(work_root: P) -> RunResult<()> {
        use std::fs;

        let work_root = work_root.as_ref();
        let token_path = resolve_admin_api_token_path(work_root);
        if token_path.exists() {
            return Ok(());
        }
        if let Some(parent) = token_path.parent() {
            ErrorHandler::safe_create_dir(parent)?;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let token = format!(
            "wp-admin-{}-{}-{}\n",
            std::process::id(),
            now.as_secs(),
            now.subsec_nanos()
        );
        fs::write(&token_path, token).owe_conf()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(&token_path).owe_conf()?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&token_path, perms).owe_conf()?;
        }

        println!("✓ 管理面 token 文件已创建: {}", token_path.display());
        Ok(())
    }

    /// 初始化语义词典配置文件
    fn init_semantic_dict_config<P: AsRef<Path>>(work_root: P) -> RunResult<()> {
        let work_root = work_root.as_ref();
        let knowledge_dir = work_root.join(MODELS_KNOWLEDGE_DIR);
        ErrorHandler::safe_create_dir(&knowledge_dir)?;

        let semantic_dict_config_path = work_root.join(SEMANTIC_DICT_FILE);
        if !semantic_dict_config_path.exists() {
            let config_content = oml::generate_default_semantic_dict_config();
            ErrorHandler::safe_write_file(&semantic_dict_config_path, &config_content)?;
            println!(
                "✓ 语义词典配置文件已创建: {}",
                semantic_dict_config_path.display()
            );
        }

        Ok(())
    }

    /// 初始化 wparse 主配置（wparse.toml）
    fn init_engine_config<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> RunResult<EngineConfig> {
        let work_root = work_root.as_ref();
        let abs_root = normalize_work_root(work_root);
        let conf_dir = abs_root.join(CONF_DIR);
        ErrorHandler::safe_create_dir(&conf_dir)?;

        let engine_config_path = abs_root.join(CONF_WPARSE_FILE);
        if !engine_config_path.exists() {
            let conf = EngineConfig::init(&abs_root);
            if let Some(parent) = engine_config_path.parent() {
                ErrorHandler::safe_create_dir(parent)?;
            }
            conf.save_toml(&engine_config_path).owe_conf()?;
        }
        // `WarpProject::build()` may have already materialized wparse.toml via
        // `EngineConfig::load_or_init`, so the admin block must be enforced
        // regardless of whether the file pre-existed.
        Self::ensure_admin_api_config_block(&engine_config_path)?;
        let conf = EngineConfig::env_load_toml(&engine_config_path, dict)
            .owe_conf()?
            .conf_absolutize(&abs_root);
        Ok(conf)
    }

    fn ensure_admin_api_config_block(config_path: &Path) -> RunResult<()> {
        let mut conf = std::fs::read_to_string(config_path).owe_conf()?;
        if conf.contains("[admin_api]") {
            return Ok(());
        }

        if !conf.ends_with('\n') {
            conf.push('\n');
        }
        conf.push_str(&default_admin_api_block());
        std::fs::write(config_path, conf).owe_conf()?;
        Ok(())
    }

    fn load_engine_config_only<P: AsRef<Path>>(
        work_root: P,
        dict: &EnvDict,
    ) -> RunResult<EngineConfig> {
        let work_root = work_root.as_ref();
        let abs_root = normalize_work_root(work_root);
        let engine_config_path = abs_root.join(CONF_WPARSE_FILE);
        if !engine_config_path.exists() {
            return Err(RunReason::from_conf().to_err().with_detail(format!(
                "engine config not found: {}",
                engine_config_path.display()
            )));
        }
        let conf = EngineConfig::env_load_toml(&engine_config_path, dict)
            .owe_conf()?
            .conf_absolutize(&abs_root);
        Ok(conf)
    }

    fn load_wpgen_config_only<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> RunResult<()> {
        let work_root = work_root.as_ref();
        let abs_root = normalize_work_root(work_root);
        let wpgen_config_path = abs_root.join(CONF_WPGEN_FILE);
        if !wpgen_config_path.exists() {
            return Ok(());
        }
        WpGenConfig::load_from_path(&wpgen_config_path, dict).owe_conf()?;
        Ok(())
    }

    fn mk_framework_dir(&self, mode: PrjScope) -> RunResult<()> {
        let work_root = self.work_root_path();
        if mode.enable_conf() {
            ErrorHandler::safe_create_dir(&work_root.join(CONF_DIR))?;
        }
        if mode.enable_model() {
            ErrorHandler::safe_create_dir(&work_root.join(MODELS_WPL_DIR))?;
            ErrorHandler::safe_create_dir(&work_root.join(MODELS_OML_DIR))?;
            ErrorHandler::safe_create_dir(&work_root.join(MODELS_KNOWLEDGE_DIR))?;
            ErrorHandler::safe_create_dir(&work_root.join(MODELS_KNOWLEDGE_EXAMPLE_DIR))?;
        }
        if mode.enable_topology() {
            ErrorHandler::safe_create_dir(&work_root.join(TOPOLOGY_SOURCES_DIR))?;
            ErrorHandler::safe_create_dir(&work_root.join(TOPOLOGY_SINKS_DIR))?;
        }
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(work_root, SRC_FILE_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(work_root, OUT_FILE_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(work_root, WPARSE_LOG_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(work_root, RESCURE_FILE_PATH))?;
        Ok(())
    }

    pub(crate) fn resolve_with_root(base: &Path, raw: &str) -> PathBuf {
        let trimmed = raw.strip_prefix("./").unwrap_or(raw);
        let candidate = Path::new(trimmed);
        if candidate.is_relative() {
            base.join(candidate)
        } else {
            candidate.to_path_buf()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use wp_conf::test_support::ForTest;
    const CONNECTORS_DIR: &str = "connectors";
    const CONNECTORS_SOURCE_DIR: &str = "connectors/source.d";
    const CONNECTORS_SINK_DIR: &str = "connectors/sink.d";
    const MODELS_DIR: &str = "models";
    const TOPO_SOURCES_DIR: &str = "topology/sources";
    const TOPO_SINKS_DIR: &str = "topology/sinks";
    const MODELS_WPL_PARSE_FILE: &str = "models/wpl/parse.wpl";
    const MODELS_WPL_SAMPLE_FILE: &str = "models/wpl/sample.dat";
    const MODELS_OML_EXAMPLE_FILE: &str = "models/oml/example.oml";
    const MODELS_KNOWLEDGE_KNOWDB_FILE: &str = "models/knowledge/knowdb.toml";
    const TOPOLOGY_WPSRC_FILE: &str = "topology/sources/wpsrc.toml";

    fn admin_api_home_lock() -> &'static Mutex<()> {
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

    fn connector_template_exists<P: AsRef<std::path::Path>>(dir: P, id: &str) -> bool {
        let suffix = format!("-{}.toml", id);
        std::fs::read_dir(dir)
            .ok()
            .and_then(|entries| {
                entries.filter_map(|entry| entry.ok()).find(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .map(|name| name.ends_with(&suffix))
                        .unwrap_or(false)
                })
            })
            .is_some()
    }

    #[test]
    fn test_init_mode_from_str() {
        // 测试有效的模式字符串
        assert_eq!(PrjScope::from_str("full").unwrap(), PrjScope::Full);
        assert_eq!(PrjScope::from_str("model").unwrap(), PrjScope::Model);
        assert_eq!(PrjScope::from_str("conf").unwrap(), PrjScope::Conf);
        assert_eq!(PrjScope::from_str("data").unwrap(), PrjScope::Data);

        // 测试无效的模式字符串
        let result = PrjScope::from_str("invalid");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("not init mode"));
        }
    }

    #[test]
    fn test_init_mode_enable_connector() {
        assert!(PrjScope::Full.enable_connector());
        assert!(!PrjScope::Model.enable_connector());
        assert!(!PrjScope::Conf.enable_connector());
        assert!(!PrjScope::Data.enable_connector());
    }

    #[test]
    fn test_init_mode_enable_model() {
        // Full 和 Normal 应该启用模型
        assert!(PrjScope::Full.enable_model());
        assert!(PrjScope::Model.enable_model());

        // Conf 和 Data 不应该启用模型
        assert!(!PrjScope::Conf.enable_model());
        assert!(!PrjScope::Data.enable_model());
    }

    #[test]
    fn test_init_mode_enable_conf() {
        // 除了 Data，其他模式都应该启用配置
        assert!(PrjScope::Full.enable_conf());
        assert!(PrjScope::Normal.enable_conf());
        assert!(PrjScope::Conf.enable_conf());
        assert!(!PrjScope::Data.enable_conf());
    }

    #[test]
    fn test_init_mode_debug_format() {
        assert_eq!(format!("{:?}", PrjScope::Full), "Full");
        assert_eq!(format!("{:?}", PrjScope::Model), "Model");
        assert_eq!(format!("{:?}", PrjScope::Conf), "Conf");
        assert_eq!(format!("{:?}", PrjScope::Data), "Data");
    }

    #[test]
    fn test_init_mode_equality() {
        assert_eq!(PrjScope::Full, PrjScope::Full);
        assert_eq!(PrjScope::Model, PrjScope::Model);
        assert_ne!(PrjScope::Full, PrjScope::Model);
        assert_ne!(PrjScope::Conf, PrjScope::Data);
    }

    #[test]
    fn test_init_mode_clone() {
        let mode = PrjScope::Full;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);

        let mode = PrjScope::Model;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_warp_project_init_full_mode() {
        use tempfile::TempDir;

        // 创建临时目录
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();
        let _guard = admin_api_home_lock().lock().expect("lock HOME override");
        let _home = HomeOverride::new(work_root);

        // 创建项目并使用 Full 模式初始化
        WarpProject::init(work_root, PrjScope::Full, &EnvDict::test_default())
            .expect("Full mode initialization should succeed");

        // 验证创建的目录和文件
        assert!(
            work_root.join(CONF_DIR).exists(),
            "conf directory should exist"
        );
        assert!(
            work_root.join(CONF_WPARSE_FILE).exists(),
            "wparse.toml should exist"
        );
        assert!(
            work_root.join(CONF_WPGEN_FILE).exists(),
            "wpgen.toml should exist"
        );
        let wparse_conf =
            std::fs::read_to_string(work_root.join(CONF_WPARSE_FILE)).expect("read wparse.toml");
        assert!(wparse_conf.contains("[admin_api]"));
        assert!(wparse_conf.contains("enabled = false"));
        assert!(wparse_conf.contains(&format!("token_file = \"{}\"", ADMIN_API_TOKEN_ENV_REF)));
        let token_path = resolve_admin_api_token_path(work_root);
        assert!(token_path.exists(), "admin API token file should exist");
        let token = std::fs::read_to_string(&token_path).expect("read admin API token");
        assert!(
            !token.trim().is_empty(),
            "admin API token should not be empty"
        );

        assert!(
            work_root.join(CONNECTORS_DIR).exists(),
            "connectors directory should exist"
        );
        assert!(
            work_root.join(CONNECTORS_SOURCE_DIR).exists(),
            "source.d directory should exist"
        );
        assert!(
            work_root.join(CONNECTORS_SINK_DIR).exists(),
            "sink.d directory should exist"
        );

        assert!(
            work_root.join(MODELS_DIR).exists(),
            "models directory should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_DIR).exists(),
            "wpl directory should exist"
        );
        assert!(
            work_root.join(MODELS_OML_DIR).exists(),
            "oml directory should exist"
        );
        assert!(
            work_root.join(TOPOLOGY_SOURCES_DIR).exists(),
            "topology sources directory should exist"
        );
        assert!(
            work_root.join(TOPOLOGY_SINKS_DIR).exists(),
            "topology sinks directory should exist"
        );
        let miss_sink_conf =
            std::fs::read_to_string(work_root.join("topology/sinks/infra.d/miss.toml"))
                .expect("read miss sink config");
        assert!(miss_sink_conf.contains("connect = \"file_raw_sink\""));
        assert!(miss_sink_conf.contains("#connect = \"victorialogs_sink\""));
        let monitor_sink_conf =
            std::fs::read_to_string(work_root.join("topology/sinks/infra.d/monitor.toml"))
                .expect("read monitor sink config");
        assert!(monitor_sink_conf.contains("#connect = \"victoriametrics_sink\""));
        assert!(
            work_root.join(TOPO_SOURCES_DIR).exists(),
            "topology/sources should remain absent; use topology/sources"
        );
        assert!(
            work_root.join(TOPO_SINKS_DIR).exists(),
            "topology/sinks should remain absent; use topology/sinks"
        );
        assert!(
            work_root.join(MODELS_KNOWLEDGE_DIR).exists(),
            "knowledge directory should exist"
        );

        // 验证示例文件
        assert!(
            work_root.join(MODELS_WPL_PARSE_FILE).exists(),
            "parse.wpl should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_SAMPLE_FILE).exists(),
            "sample.dat should exist"
        );
        assert!(
            work_root.join(MODELS_OML_EXAMPLE_FILE).exists(),
            "example.oml should exist"
        );
        assert!(
            work_root.join(MODELS_KNOWLEDGE_KNOWDB_FILE).exists(),
            "knowdb.toml should exist"
        );

        // 验证连接器模板
        assert!(
            connector_template_exists(work_root.join(CONNECTORS_SOURCE_DIR), "file_src"),
            "file source connector should exist"
        );
        assert!(
            connector_template_exists(work_root.join(CONNECTORS_SINK_DIR), "file_json_sink"),
            "file sink connector should exist"
        );
        assert!(
            connector_template_exists(work_root.join(CONNECTORS_SINK_DIR), "file_raw_sink"),
            "file raw sink connector should exist"
        );
        assert!(
            connector_template_exists(work_root.join(CONNECTORS_SINK_DIR), "arrow_file_sink"),
            "arrow file sink connector should exist"
        );
        assert!(
            connector_template_exists(work_root.join(CONNECTORS_SINK_DIR), "arrow_tcp_sink"),
            "arrow tcp sink connector should exist"
        );
    }

    #[test]
    fn test_warp_project_init_normal_mode() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        WarpProject::init(work_root, PrjScope::Normal, &EnvDict::test_default())
            .expect("Normal mode initialization should succeed");

        // 验证配置目录
        assert!(
            work_root.join(CONF_DIR).exists(),
            "conf directory should exist"
        );
        assert!(
            work_root.join(CONF_WPARSE_FILE).exists(),
            "wparse.toml should exist"
        );
        assert!(
            work_root.join(CONF_WPGEN_FILE).exists(),
            "wpgen.toml should exist"
        );

        // 验证模型目录和文件
        assert!(
            work_root.join(MODELS_DIR).exists(),
            "models directory should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_DIR).exists(),
            "wpl directory should exist"
        );
        assert!(
            work_root.join(MODELS_OML_DIR).exists(),
            "oml directory should exist"
        );
        assert!(
            work_root.join(MODELS_DIR).exists(),
            "models directory should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_DIR).exists(),
            "wpl directory should exist"
        );
        assert!(
            work_root.join(MODELS_OML_DIR).exists(),
            "oml directory should exist"
        );
        assert!(
            work_root.join(TOPOLOGY_SOURCES_DIR).exists(),
            "topology sources directory should not exist in Model mode"
        );
        assert!(
            work_root.join(TOPOLOGY_SINKS_DIR).exists(),
            "topology sinks directory should not exist in Model mode"
        );
        assert!(
            work_root.join(MODELS_KNOWLEDGE_DIR).exists(),
            "knowledge directory should exist"
        );

        // 验证示例文件
        assert!(
            work_root.join(MODELS_WPL_PARSE_FILE).exists(),
            "parse.wpl should exist"
        );
        assert!(
            work_root.join(MODELS_OML_EXAMPLE_FILE).exists(),
            "example.oml should exist"
        );

        // Normal 模式不会创建 connectors，只有 Full 模式才会。
        assert!(
            !work_root.join(CONNECTORS_DIR).exists(),
            "connectors directory should not exist in Model mode"
        );
    }

    #[test]
    fn test_warp_project_init_conf_mode() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        WarpProject::init(work_root, PrjScope::Conf, &EnvDict::test_default())
            .expect("Conf mode initialization should succeed");

        // 验证配置目录
        assert!(
            work_root.join(CONF_DIR).exists(),
            "conf directory should exist"
        );
        assert!(
            work_root.join(CONF_WPARSE_FILE).exists(),
            "wparse.toml should exist"
        );
        assert!(
            work_root.join(CONF_WPGEN_FILE).exists(),
            "wpgen.toml should exist"
        );

        // Conf 模式不应该创建连接器（只创建配置）
        assert!(
            !work_root.join(CONNECTORS_DIR).exists(),
            "connectors directory should not exist in Conf mode"
        );
        // Conf 模式不应该创建模型（修复后）
        assert!(
            !work_root.join(MODELS_DIR).exists(),
            "models directory should not exist in Conf mode"
        );
    }

    #[test]
    fn test_warp_project_init_data_mode() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        WarpProject::init(work_root, PrjScope::Data, &EnvDict::test_default())
            .expect("Data mode initialization should succeed");

        // Data 模式会创建基础数据目录以及最小配置（engine 默认）
        assert!(
            work_root.join(CONF_DIR).exists(),
            "conf directory should exist"
        );
        assert!(
            work_root.join(CONF_WPARSE_FILE).exists(),
            "wparse.toml should exist"
        );
        assert!(
            !work_root.join(CONNECTORS_DIR).exists(),
            "connectors directory should not exist in Data mode"
        );

        // Data 模式不应该创建任何 models 相关内容（修复后）
        assert!(
            !work_root.join(MODELS_DIR).exists(),
            "models directory should not exist in Data mode"
        );
        assert!(
            !work_root.join(MODELS_KNOWLEDGE_DIR).exists(),
            "knowledge directory should not exist in Data mode"
        );
    }

    #[test]
    fn test_init_admin_api_token_creates_non_empty_token() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();
        let _guard = admin_api_home_lock().lock().expect("lock HOME override");
        let _home = HomeOverride::new(work_root);

        WarpProject::init_admin_api_token(work_root).expect("create admin API token");

        let token_path = resolve_admin_api_token_path(work_root);
        assert!(token_path.exists());
        let token = std::fs::read_to_string(&token_path).expect("read admin API token");
        assert!(!token.trim().is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&token_path)
                .expect("stat admin API token")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn test_init_admin_api_token_keeps_existing_token() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();
        let _guard = admin_api_home_lock().lock().expect("lock HOME override");
        let _home = HomeOverride::new(work_root);

        let token_path = resolve_admin_api_token_path(work_root);
        if let Some(parent) = token_path.parent() {
            std::fs::create_dir_all(parent).expect("create token parent");
        }
        std::fs::write(&token_path, "existing-token\n").expect("seed existing token");

        WarpProject::init_admin_api_token(work_root).expect("reuse existing admin API token");

        let token = std::fs::read_to_string(&token_path).expect("read admin API token");
        assert_eq!(token, "existing-token\n");
    }

    #[test]
    fn test_init_engine_config_adds_admin_api_block_once() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();
        let conf_dir = work_root.join(CONF_DIR);
        std::fs::create_dir_all(&conf_dir).expect("create conf dir");
        let conf_path = work_root.join(CONF_WPARSE_FILE);
        std::fs::write(&conf_path, "version = \"1.0\"\n").expect("write wparse.toml");

        WarpProject::ensure_admin_api_config_block(&conf_path).expect("append admin_api block");
        WarpProject::ensure_admin_api_config_block(&conf_path)
            .expect("skip duplicate admin_api block");

        let conf = std::fs::read_to_string(conf_path).expect("read wparse.toml");
        assert_eq!(conf.matches("[admin_api]").count(), 1);
        assert!(conf.contains("bind = \"127.0.0.1:19090\""));
        assert!(conf.contains(&format!("token_file = \"{}\"", ADMIN_API_TOKEN_ENV_REF)));
    }

    #[test]
    fn test_warp_project_init_basic() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        let mut project = WarpProject::bare(work_root);

        // 测试 init_basic 方法（等效于 Normal 模式）
        let result = project.init_basic(PrjScope::Normal);
        assert!(result.is_ok(), "Basic initialization should succeed");

        // 验证基础结构
        assert!(
            work_root.join(CONF_DIR).exists(),
            "conf directory should exist"
        );
        assert!(
            work_root.join(CONF_WPARSE_FILE).exists(),
            "wparse.toml should exist"
        );
        assert!(
            work_root.join(CONF_WPGEN_FILE).exists(),
            "wpgen.toml should exist"
        );

        assert!(
            work_root.join(MODELS_DIR).exists(),
            "models directory should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_DIR).exists(),
            "wpl directory should exist"
        );
        assert!(
            work_root.join(MODELS_OML_DIR).exists(),
            "oml directory should exist"
        );
    }

    #[test]
    fn test_warp_project_init_models() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        let mut project = WarpProject::bare(work_root);

        // 首先创建基础结构
        project
            .init_basic(PrjScope::Model)
            .expect("Basic initialization should succeed");

        // 测试 init_models 方法
        let result = project.init_models();
        assert!(result.is_ok(), "Models initialization should succeed");

        // 验证模型文件
        assert!(
            work_root.join(MODELS_WPL_PARSE_FILE).exists(),
            "parse.wpl should exist"
        );
        assert!(
            work_root.join(MODELS_WPL_SAMPLE_FILE).exists(),
            "sample.dat should exist"
        );
        assert!(
            work_root.join(MODELS_OML_EXAMPLE_FILE).exists(),
            "example.oml should exist"
        );
        assert!(
            work_root.join(MODELS_KNOWLEDGE_KNOWDB_FILE).exists(),
            "knowdb.toml should exist"
        );
        assert!(
            !work_root.join(TOPOLOGY_WPSRC_FILE).exists(),
            "wpsrc.toml should not exist in pure model initialization"
        );
        assert!(
            !work_root.join(CONNECTORS_DIR).exists(),
            "connectors directory should not exist in model init"
        );
    }

    #[test]
    fn test_resolve_with_root() {
        use wp_conf::paths::{OUT_FILE_PATH, RESCURE_FILE_PATH, SRC_FILE_PATH};

        let base = Path::new("/work");

        // 测试相对路径
        let relative_path = WarpProject::resolve_with_root(base, "data/file.txt");
        assert_eq!(relative_path, Path::new("/work/data/file.txt"));

        // 测试绝对路径
        let absolute_path = WarpProject::resolve_with_root(base, "/absolute/path");
        assert_eq!(absolute_path, Path::new("/absolute/path"));

        // 测试以 "./" 开头的路径
        let prefixed_path = WarpProject::resolve_with_root(base, "./data/file.txt");
        assert_eq!(prefixed_path, Path::new("/work/data/file.txt"));

        // 测试常量路径
        let out_path = WarpProject::resolve_with_root(base, OUT_FILE_PATH);
        assert!(out_path.starts_with("/work"));

        let src_path = WarpProject::resolve_with_root(base, SRC_FILE_PATH);
        assert!(src_path.starts_with("/work"));

        let rescue_path = WarpProject::resolve_with_root(base, RESCURE_FILE_PATH);
        assert!(rescue_path.starts_with("/work"));
    }

    #[test]
    fn test_init_wpgen_config() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let work_root = temp_dir.path();

        // 测试 init_wpgen_config 方法
        let result = WarpProject::init_wpgen_config(work_root);
        assert!(result.is_ok(), "Wpgen config initialization should succeed");

        // 验证配置文件被创建
        let wpgen_config_path = work_root.join(CONF_WPGEN_FILE);
        assert!(wpgen_config_path.exists(), "wpgen.toml should exist");

        // 验证文件内容
        let content =
            fs::read_to_string(&wpgen_config_path).expect("Should be able to read wpgen.toml");
        assert!(!content.is_empty(), "wpgen.toml should not be empty");
        // 检查配置文件是否包含一些基本配置项
        assert!(
            content.contains("["),
            "wpgen.toml should contain at least one section"
        );

        // 测试重复调用（不应该覆盖现有文件）
        let result = WarpProject::init_wpgen_config(work_root);
        assert!(result.is_ok(), "Second call should also succeed");

        let new_content =
            fs::read_to_string(&wpgen_config_path).expect("Should be able to read wpgen.toml");
        assert_eq!(content, new_content, "File should not be overwritten");
    }
}
