use super::ConfDelegate;
use crate::facade::config::{ENGINE_CONF_FILE, WPARSE_LOG_PATH};
use crate::orchestrator::config::WPSRC_TOML;
use futures_util::TryFutureExt;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{EnvTomlLoad, TomlIO};
use orion_error::compat_prelude::ErrorOweBase;
use orion_error::conversion::ToStructError;
use orion_error::{ErrorWith, UvsFrom, UvsReason};
use orion_variate::{EnvDict, EnvEvaluable};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use tokio::fs::create_dir_all;
use wp_conf::engine::EngineConfig;
use wp_conf::paths::OUT_FILE_PATH;
use wp_conf::structure::ConfStdOperation;
use wp_conf::utils::{backup_clean, save_conf, save_data};
use wp_error::config_error::ConfResult;
use wp_error::error_handling::target;
use wp_error::run_error::{RunReason, RunResult};

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
            std::fs::create_dir_all(parent)
                .owe(ConfIOReason::Uvs(UvsReason::resource_error()))
                .with_context(parent)?;
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
            .owe(ConfIOReason::Uvs(UvsReason::resource_error()))
            .with_context(&path)?
            .env_eval(dict)
            .conf_absolutize(self.work_root());
        Ok(conf)
    }

    /// 清理工作目录中的配置文件
    pub fn cleanup_work_directory(&self, dict: &EnvDict) -> RunResult<()> {
        let wp_conf = EngineConfig::load_or_init(self.work_root(), dict)
            .map_err(|e| {
                RunReason::from_conf()
                    .to_err()
                    .with_detail(format!("load engine config for cleanup failed: {}", e))
            })?
            .env_eval(dict)
            .conf_absolutize(self.work_root());
        backup_clean(self.config_path_string(ENGINE_CONF_FILE)).map_err(|e| {
            RunReason::from_conf()
                .to_err()
                .with_detail(format!("cleanup engine config backup failed: {}", e))
        })?;
        backup_clean(wp_conf.src_conf_of(WPSRC_TOML)).map_err(|e| {
            RunReason::from_conf()
                .to_err()
                .with_detail(format!("cleanup source config backup failed: {}", e))
        })?;
        // PUBLIC_ADM 废弃：不再清理 public.oml
        // 默认清理 connectors default + models templates（wpsrc）
        {
            // minimal: 清理 connectors source default + src conf
            if let Some(conn_path) =
                connector_template_by_id(&self.work_root().join("connectors/source.d"), "file_src")
            {
                backup_clean(conn_path).map_err(|e| {
                    RunReason::from_conf()
                        .to_err()
                        .with_detail(format!("cleanup source connector backup failed: {}", e))
                })?;
            }
            backup_clean(wp_conf.src_conf_of(WPSRC_TOML)).map_err(|e| {
                RunReason::from_conf()
                    .to_err()
                    .with_detail(format!("cleanup source config backup failed: {}", e))
            })?;
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
    fn load_engine_config_expands_admin_api_token_file_env() {
        let _guard = home_lock().lock().expect("lock HOME override");
        let temp = tempfile::tempdir().expect("temp dir");
        let home = temp.path().join("fake-home");
        std::fs::create_dir_all(&home).expect("create fake home");
        let _home = HomeOverride::new(&home);

        let conf_dir = temp.path().join("conf");
        std::fs::create_dir_all(&conf_dir).expect("create conf dir");
        std::fs::write(
            conf_dir.join(ENGINE_CONF_FILE),
            r#"version = "1.0"

[admin_api]
enabled = false

[admin_api.auth]
mode = "bearer_token"
token_file = "${HOME}/.warp_parse/admin_api.token"
"#,
        )
        .expect("write config");

        let conf = WarpConf::new(temp.path())
            .load_engine_config(&EnvDict::new())
            .expect("load config");

        assert_eq!(
            conf.admin_api().auth.token_file,
            home.join(".warp_parse")
                .join("admin_api.token")
                .display()
                .to_string()
        );
    }
}
