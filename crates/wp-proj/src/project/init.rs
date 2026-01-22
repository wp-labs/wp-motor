use super::warp::{WarpProject, normalize_work_root};
use crate::utils::error_handler::ErrorHandler;
use orion_conf::{EnvTomlLoad, ErrorOwe, ToStructError, TomlIO};
use orion_error::{UvsConfFrom, UvsValidationFrom};
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
const TOPOLOGY_SOURCES_DIR: &str = "topology/sources";
const TOPOLOGY_SINKS_DIR: &str = "topology/sinks";

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
            _ => return RunReason::from_validation("not init mode").err_result(),
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
        use std::fs;

        let work_root = work_root.as_ref();
        let conf_dir = work_root.join(CONF_DIR);
        if let Err(_) = fs::create_dir_all(&conf_dir) {
            // 如果创建目录失败，记录警告但继续
            eprintln!("Warning: Failed to create conf directory");
        }

        let wpgen_config_path = work_root.join(CONF_WPGEN_FILE);
        if !wpgen_config_path.exists() {
            // 使用 include_str! 读取示例配置文件
            let wpgen_config_content = include_str!("../example/conf/wpgen.toml");
            if let Err(_) = fs::write(&wpgen_config_path, wpgen_config_content) {
                // 如果写入失败，记录警告但继续
                eprintln!("Warning: Failed to write wpgen.toml");
            }
        }

        Ok(())
    }

    /// 初始化 wparse 主配置（wparse.toml）
    fn init_engine_config<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> RunResult<EngineConfig> {
        use std::fs;

        let work_root = work_root.as_ref();
        let abs_root = normalize_work_root(work_root);
        let conf_dir = abs_root.join(CONF_DIR);
        if let Err(_) = fs::create_dir_all(&conf_dir) {
            eprintln!("Warning: Failed to create conf directory");
        }

        let engine_config_path = abs_root.join(CONF_WPARSE_FILE);
        if !engine_config_path.exists() {
            // 使用 EngineConfig::init() 生成配置并保存
            let conf = EngineConfig::init(&abs_root);
            conf.save_toml(&engine_config_path).owe_conf()?;
        }
        let conf = EngineConfig::env_load_toml(&engine_config_path, dict)
            .owe_conf()?
            .conf_absolutize(&abs_root);
        Ok(conf)
    }

    fn load_engine_config_only<P: AsRef<Path>>(
        work_root: P,
        dict: &EnvDict,
    ) -> RunResult<EngineConfig> {
        let work_root = work_root.as_ref();
        let abs_root = normalize_work_root(work_root);
        let engine_config_path = abs_root.join(CONF_WPARSE_FILE);
        if !engine_config_path.exists() {
            return RunReason::from_conf(format!(
                "wparse config missing: {}",
                engine_config_path.display()
            ))
            .err_result();
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
            return RunReason::from_conf(format!(
                "wpgen config missing: {}",
                wpgen_config_path.display()
            ))
            .err_result();
        }
        WpGenConfig::env_load_toml(&wpgen_config_path, dict).owe_conf()?;
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
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(&work_root, SRC_FILE_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(&work_root, OUT_FILE_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(&work_root, WPARSE_LOG_PATH))?;
        ErrorHandler::safe_create_dir(&Self::resolve_with_root(&work_root, RESCURE_FILE_PATH))?;
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
    const MODELS_OML_KNOWDB_FILE: &str = "models/oml/knowdb.toml";
    const TOPOLOGY_WPSRC_FILE: &str = "topology/sources/wpsrc.toml";

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
            work_root.join(MODELS_OML_KNOWDB_FILE).exists(),
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
            work_root.join(MODELS_OML_KNOWDB_FILE).exists(),
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
