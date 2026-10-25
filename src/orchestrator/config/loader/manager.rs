use super::ConfDelegate;
use crate::compat::LegacyOwe;
use crate::facade::config::{ENGINE_CONF_FILE, WPARSE_LOG_PATH};
use crate::orchestrator::config::WPSRC_TOML;
use futures_util::TryFutureExt;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{EnvTomlLoad, TomlIO};
use orion_error::conversion::SourceRawErr;
use orion_error::conversion::ToStructError;
use orion_error::{UnifiedReason, conversion::ErrorWith};
use orion_variate::{EnvDict, EnvEvaluable};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use tokio::fs::create_dir_all;
use wp_conf::engine::EngineConfig;
use wp_conf::paths::OUT_FILE_PATH;
use wp_conf::state::StateDir;
use wp_conf::structure::ConfStdOperation;
use wp_conf::utils::{backup_clean, save_conf, save_data};
use wp_error::config_error::ConfResult;
use wp_error::error_handling::target;
use wp_error::run_error::{RunReason, RunResult};

#[derive(Clone)]
pub struct WarpConf {
    original_root: PathBuf,
    work_root: OnceCell<PathBuf>,
    /// Resolved runtime state directory. `None` keeps the legacy
    /// `<work_root>/.run` layout. Set via the [`WarpConf::with_state_dir`]
    /// builder so clones can never disagree about the state location.
    state_dir: Option<StateDir>,
}

impl WarpConf {
    pub fn new<P: AsRef<Path>>(original_root: P) -> Self {
        Self {
            original_root: PathBuf::from(original_root.as_ref()),
            work_root: OnceCell::new(),
            state_dir: None,
        }
    }

    /// Use an explicitly resolved state directory.
    ///
    /// This is a builder (consuming `self`) on purpose: a setter would let two
    /// clones of the same `WarpConf` write state to different directories.
    pub fn with_state_dir(mut self, state_dir: StateDir) -> Self {
        self.state_dir = Some(state_dir);
        self
    }

    /// Resolved state directory (defaults to `<work_root>/.run`).
    pub fn state_dir(&self) -> StateDir {
        match &self.state_dir {
            Some(dir) => dir.clone(),
            None => StateDir::default_for(self.work_root()),
        }
    }

    pub fn state_dir_path(&self) -> PathBuf {
        self.state_dir().to_path_buf()
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
                .owe(ConfIOReason::resource_error())
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
    ///
    /// 运行时目录由 `[runtime].state_dir` 决定，默认 `<work_root>/.run`。
    pub fn runtime_path(&self, file_name: &str) -> String {
        let state_dir = self.state_dir();
        if let Err(err) = state_dir.ensure() {
            // Do not fail here: callers only need a path, and the actual
            // open/create that follows reports a more actionable error.
            warn_ctrl!("{}; returning path anyway", err);
        }
        state_dir.file(file_name).to_string_lossy().to_string()
    }
    /// 获取工作根目录的路径字符串
    pub fn work_root_path(&self) -> String {
        self.work_root().to_string_lossy().to_string()
    }
    /// 加载引擎配置
    pub fn load_engine_config(&self, dict: &EnvDict) -> OrionConfResult<EngineConfig> {
        let path = self.config_path(ENGINE_CONF_FILE);
        let conf = EngineConfig::env_load_toml(&path, dict)
            .owe(ConfIOReason::resource_error())
            .with_context(&path)?
            .env_eval(dict)
            .conf_absolutize(self.work_root());
        Ok(conf)
    }

    /// 清理工作目录中的配置文件
    pub fn cleanup_work_directory(&self, dict: &EnvDict) -> RunResult<()> {
        let wp_conf = EngineConfig::load_or_init(self.work_root(), dict)
            .map_err(|e| {
                RunReason::core_conf()
                    .to_err()
                    .with_detail(format!("load engine config for cleanup failed: {}", e))
            })?
            .env_eval(dict)
            .conf_absolutize(self.work_root());
        backup_clean(self.config_path_string(ENGINE_CONF_FILE)).map_err(|e| {
            RunReason::core_conf()
                .to_err()
                .with_detail(format!("cleanup engine config backup failed: {}", e))
        })?;
        backup_clean(wp_conf.src_conf_of(WPSRC_TOML)).map_err(|e| {
            RunReason::core_conf()
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
                    RunReason::core_conf()
                        .to_err()
                        .with_detail(format!("cleanup source connector backup failed: {}", e))
                })?;
            }
            backup_clean(wp_conf.src_conf_of(WPSRC_TOML)).map_err(|e| {
                RunReason::core_conf()
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
    fn runtime_path_defaults_to_dot_run() {
        let temp = tempfile::tempdir().expect("temp dir");
        let conf = WarpConf::new(temp.path());

        assert_eq!(
            conf.runtime_path("wparse.pid"),
            temp.path()
                .join(".run/wparse.pid")
                .to_string_lossy()
                .to_string()
        );
        assert!(temp.path().join(".run").is_dir());
    }

    #[test]
    fn runtime_path_uses_configured_state_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let state = StateDir::resolve(temp.path(), "state").expect("resolve state dir");
        let conf = WarpConf::new(temp.path()).with_state_dir(state.clone());

        assert_eq!(
            conf.runtime_path("authority.sqlite"),
            state.file("authority.sqlite").to_string_lossy().to_string()
        );
        assert_eq!(conf.state_dir_path(), state.path());
        assert!(state.path().is_dir());
        assert!(
            !temp.path().join(".run").exists(),
            "configured state dir must not create the legacy .run directory"
        );
    }

    #[test]
    fn state_dir_survives_clone() {
        let temp = tempfile::tempdir().expect("temp dir");
        let state = StateDir::resolve(temp.path(), "state").expect("resolve state dir");
        let conf = WarpConf::new(temp.path())
            .with_state_dir(state.clone())
            .clone();

        assert_eq!(conf.state_dir(), state);
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
