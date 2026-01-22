use super::ConfDelegate;
use crate::facade::config::{ENGINE_CONF_FILE, WPARSE_LOG_PATH};
use crate::orchestrator::config::WPSRC_TOML;
use crate::types::AnyResult;
use futures_util::TryFutureExt;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{EnvTomlLoad, ErrorOwe, ToStructError, TomlIO};
use orion_error::{ErrorWith, UvsResFrom};
use orion_variate::EnvDict;
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use tokio::fs::create_dir_all;
use wp_conf::engine::EngineConfig;
use wp_conf::paths::OUT_FILE_PATH;
use wp_conf::structure::ConfStdOperation;
use wp_conf::utils::{backup_clean, save_conf, save_data};
use wp_error::config_error::ConfResult;
use wp_error::error_handling::target;

#[derive(Clone)]
pub struct WarpConf {
    original_root: PathBuf,
    work_root: OnceCell<PathBuf>,
}

impl WarpConf {
    pub fn new<P: AsRef<Path>>(original_root: P) -> Self {
        Self {
            original_root: PathBuf::from(original_root.as_ref()),
            work_root: OnceCell::new(),
        }
    }

    pub fn work_root(&self) -> &Path {
        self.work_root
            .get_or_init(|| {
                let original = self.original_root.clone();
                if original.is_absolute() {
                    original
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|err| panic!("获取当前工作目录失败: {}", err))
                        .join(original)
                }
            })
            .as_path()
    }

    fn conf_root_path(&self) -> PathBuf {
        self.work_root().join("conf")
    }

    /// 清理工作目录及其所有配置文件
    pub fn clear_work_directory(&self) {
        let work_root = self.work_root();
        if work_root.exists() {
            std::fs::remove_dir_all(work_root)
                .unwrap_or_else(|_| panic!("remove dir all {}", work_root.display()));
        }
    }

    /// 构建配置文件的完整路径字符串
    pub fn config_path_string(&self, file_name: &str) -> String {
        self.conf_root_path()
            .join(file_name)
            .to_string_lossy()
            .to_string()
    }
    /// 确保配置文件的目录存在
    pub fn ensure_config_path_exists(&self, file_name: &str) -> OrionConfResult<PathBuf> {
        let target = self.conf_root_path().join(file_name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).owe_res().with(parent)?;
        }
        Ok(target)
    }
    /// 构建配置文件的完整路径（PathBuf）
    pub fn config_path(&self, file_name: &str) -> PathBuf {
        self.conf_root_path().join(file_name)
    }
    /// 构建工作目录中文件的完整路径字符串
    pub fn work_path_str(&self, file_name: &str) -> String {
        self.work_root()
            .join(file_name)
            .to_string_lossy()
            .to_string()
    }
    pub fn work_path(&self, file_name: &str) -> PathBuf {
        self.work_root().join(file_name)
    }
    /// 构建运行时目录中文件的完整路径字符串
    pub fn runtime_path(&self, file_name: &str) -> String {
        let run_dir = self.work_root().join(".run");
        std::fs::create_dir_all(&run_dir).ok();
        run_dir.join(file_name).to_string_lossy().to_string()
    }
    /// 获取工作根目录的路径字符串
    pub fn work_root_path(&self) -> String {
        self.work_root().to_string_lossy().to_string()
    }
    /// 加载引擎配置
    pub fn load_engine_config(&self, dict: &EnvDict) -> OrionConfResult<EngineConfig> {
        let path = self.config_path(ENGINE_CONF_FILE);
        let conf = EngineConfig::env_load_toml(&path, dict)
            .owe_res()
            .with(&path)?
            .conf_absolutize(self.work_root());
        Ok(conf)
    }

    /// 清理工作目录中的配置文件
    pub fn cleanup_work_directory(&self, dict: &EnvDict) -> AnyResult<()> {
        let wp_conf =
            EngineConfig::load_or_init(self.work_root(), dict)?.conf_absolutize(self.work_root());
        backup_clean(self.config_path_string(ENGINE_CONF_FILE))?;
        backup_clean(wp_conf.src_conf_of(WPSRC_TOML))?;
        // PUBLIC_ADM 废弃：不再清理 public.oml
        // 默认清理 connectors default + models templates（wpsrc）
        {
            // minimal: 清理 connectors source default + src conf
            if let Some(conn_path) =
                connector_template_by_id(&self.work_root().join("connectors/source.d"), "file_src")
            {
                backup_clean(conn_path)?;
            }
            backup_clean(wp_conf.src_conf_of(WPSRC_TOML))?;
        }
        Ok(())
    }

    /// 创建配置委托对象
    pub fn create_config_delegate<T>(&self, file_name: &str) -> ConfDelegate<T>
    where
        T: ConfStdOperation,
    {
        let path = self.config_path_string(file_name);
        ConfDelegate::<T>::new(path.as_str())
    }
    /// 尝试加载配置文件
    pub fn try_load_config<T>(&self, file_name: &str, dict: &EnvDict) -> Option<T>
    where
        T: ConfStdOperation,
    {
        let path = self.config_path_string(file_name);
        T::try_load(path.as_str(), dict).ok()?
    }
}

fn connector_template_by_id(dir: &Path, id: &str) -> Option<PathBuf> {
    let suffix = format!("-{}.toml", id);
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|name| name.ends_with(&suffix))
                    .unwrap_or(false)
        })
}
