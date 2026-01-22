#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const BASIC_WPARSE_CONFIG: &str = r#"[general]
work_root = "."
log_root = "./data/log"

[log]
level = "info"

[log_conf]
level = "info"
output = "File"

[log_conf.file]
path = "./data/logs"

[rule]
root = "./models/wpl"

[source]
root = "./topology/sources"
wpsrc = "wpsrc.toml"

[sink]
root = "./models/sinks"
business = "business.d"
infra = "infra.d"

[oml]
root = "./models/oml"
repo = "knowdb.toml"
"#;

pub fn temp_workdir() -> TempDir {
    TempDir::new().expect("temp dir")
}

pub fn write_basic_wparse_config(root: &Path) {
    let conf_dir = root.join("conf");
    fs::create_dir_all(&conf_dir).expect("conf dir");
    fs::write(conf_dir.join("wparse.toml"), BASIC_WPARSE_CONFIG).expect("wparse config");
}

pub fn ensure_dir(root: &Path, rel: &str) -> PathBuf {
    let path = root.join(rel);
    fs::create_dir_all(&path).expect("dir");
    path
}

pub fn write_file(root: &Path, rel: &str, body: &str) -> PathBuf {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(&path, body).expect("write file");
    path
}
