use orion_variate::EnvDict;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

mod stats_bridge;

pub fn ensure_stats_telemetry_bridge_installed() {
    stats_bridge::ensure_stats_telemetry_bridge_installed();
}

pub fn attach_stats_monitor_sender(mon_send: crate::stat::MonSend) {
    stats_bridge::attach_stats_monitor_sender(mon_send);
}

pub fn log_missing_knowdb_config(prefix: &str, conf: &Path) {
    warn_ctrl!(
        "{}knowdb config not found at {}; skip knowdb init",
        prefix,
        conf.display()
    );
}

pub fn log_knowdb_init_error(prefix: &str, conf: &Path, err: &impl std::fmt::Debug) {
    warn_ctrl!(
        "{}init knowdb skipped ({}): {:#?}",
        prefix,
        conf.display(),
        err
    );
}

#[derive(Clone, Debug)]
pub struct KnowdbHandler {
    root: Arc<PathBuf>,
    conf: Arc<PathBuf>,
    authority_uri: Arc<String>,
    initialized: Arc<AtomicBool>,
    dict: Arc<EnvDict>,
    local_tables: Arc<Vec<String>>,
}

#[derive(Deserialize)]
struct KnowdbTableProbe {
    name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Deserialize)]
struct KnowdbProbe {
    #[serde(default)]
    tables: Vec<KnowdbTableProbe>,
}

const fn default_true() -> bool {
    true
}

fn load_local_table_names(conf: &Path) -> Vec<String> {
    let Ok(body) = std::fs::read_to_string(conf) else {
        return Vec::new();
    };
    let Ok(probe) = toml::from_str::<KnowdbProbe>(&body) else {
        return Vec::new();
    };
    probe
        .tables
        .into_iter()
        .filter(|table| table.enabled)
        .map(|table| table.name)
        .collect()
}

impl KnowdbHandler {
    pub fn new(root: &Path, conf: &Path, authority_uri: &str, dict: &EnvDict) -> Self {
        Self {
            root: Arc::new(root.to_path_buf()),
            conf: Arc::new(conf.to_path_buf()),
            authority_uri: Arc::new(authority_uri.to_string()),
            initialized: Arc::new(AtomicBool::new(false)),
            dict: Arc::new(dict.clone()),
            local_tables: Arc::new(load_local_table_names(conf)),
        }
    }

    pub fn mark_initialized(&self) {
        self.initialized.store(true, Ordering::SeqCst);
    }

    pub fn ensure_thread_ready(&self) {
        if self.initialized.load(Ordering::SeqCst) {
            return;
        }
        match wp_knowledge::facade::init_thread_cloned_from_knowdb(
            &self.root,
            &self.conf,
            &self.authority_uri,
            &self.dict,
        ) {
            Ok(_) => {
                self.initialized.store(true, Ordering::SeqCst);
                info_ctrl!("init thread-cloned knowdb provider success ");
            }
            Err(err) => {
                warn_ctrl!(
                    "init thread-cloned knowdb provider failed (conf={}): {:#?}",
                    self.conf.display(),
                    err
                );
            }
        }
    }

    pub fn authority_uri(&self) -> &str {
        self.authority_uri.as_str()
    }

    pub fn local_tables(&self) -> &[String] {
        self.local_tables.as_slice()
    }

    pub fn has_local_tables(&self) -> bool {
        !self.local_tables.is_empty()
    }
}
