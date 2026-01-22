use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug)]
pub struct KnowdbHandler {
    root: Arc<PathBuf>,
    conf: Arc<PathBuf>,
    authority_uri: Arc<String>,
    initialized: Arc<AtomicBool>,
    dict: Arc<EnvDict>,
}

impl KnowdbHandler {
    pub fn new(root: &Path, conf: &Path, authority_uri: &str, dict: &EnvDict) -> Self {
        Self {
            root: Arc::new(root.to_path_buf()),
            conf: Arc::new(conf.to_path_buf()),
            authority_uri: Arc::new(authority_uri.to_string()),
            initialized: Arc::new(AtomicBool::new(false)),
            dict: Arc::new(dict.clone()),
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
                warn_ctrl!("init thread-cloned knowdb provider failed: {}", err);
            }
        }
    }
}
