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

#[derive(Deserialize)]
pub(crate) struct KnowdbProviderProbe {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tables: Vec<String>,
    /// 新版 `[provider.sqldb]` / `[[provider.sqldb]]`（单表或数组都接受）。
    /// 只关心是否存在；字段值由 wp-knowledge 解析时校验。
    #[serde(default, deserialize_with = "deserialize_sqldb_probe")]
    pub sqldb: Option<Vec<serde::de::IgnoredAny>>,
    /// 新版 `[provider.redis]`。
    #[serde(default)]
    pub redis: Option<serde::de::IgnoredAny>,
}

/// 同时接受 `[provider.sqldb]`（单表）与 `[[provider.sqldb]]`（数组）两种 TOML 写法。
fn deserialize_sqldb_probe<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<serde::de::IgnoredAny>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{MapAccess, SeqAccess, Visitor};

    struct SqldbProbeVisitor;

    impl<'de> Visitor<'de> for SqldbProbeVisitor {
        type Value = Option<Vec<serde::de::IgnoredAny>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .write_str("a `[provider.sqldb]` table or a `[[provider.sqldb]]` array of tables")
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let spec = serde::de::IgnoredAny::deserialize(
                serde::de::value::MapAccessDeserializer::new(map),
            )?;
            Ok(Some(vec![spec]))
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let specs = Vec::<serde::de::IgnoredAny>::deserialize(
                serde::de::value::SeqAccessDeserializer::new(seq),
            )?;
            Ok(Some(specs))
        }
    }

    deserializer.deserialize_any(SqldbProbeVisitor)
}

#[derive(Deserialize)]
pub(crate) struct KnowdbTableProbe {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Deserialize)]
pub(crate) struct KnowdbProbe {
    #[serde(default)]
    pub provider: Option<KnowdbProviderProbe>,
    #[serde(default)]
    pub tables: Vec<KnowdbTableProbe>,
}

pub(crate) const fn default_true() -> bool {
    true
}

pub(crate) fn load_knowdb_probe(conf: &Path) -> Option<KnowdbProbe> {
    let Ok(body) = std::fs::read_to_string(conf) else {
        return None;
    };
    toml::from_str::<KnowdbProbe>(&body).ok()
}

pub(crate) fn load_local_table_names(conf: &Path) -> Vec<String> {
    let Some(probe) = load_knowdb_probe(conf) else {
        return Vec::new();
    };
    probe
        .tables
        .into_iter()
        .filter(|table| table.enabled)
        .map(|table| table.name)
        .collect()
}

pub(crate) fn load_provider_table_names(conf: &Path) -> Vec<String> {
    let Some(probe) = load_knowdb_probe(conf) else {
        return Vec::new();
    };
    probe
        .provider
        .map(|provider| provider.tables)
        .unwrap_or_default()
}

pub(crate) fn uses_external_provider_only(conf: &Path) -> bool {
    let Some(probe) = load_knowdb_probe(conf) else {
        return false;
    };
    let has_external = probe.provider.as_ref().is_some_and(|provider| {
        // 旧版 `[provider]` 格式
        let legacy_kind = matches!(
            provider.kind.as_deref(),
            Some("postgres" | "mysql" | "redis")
        );
        // 新版 `[provider.sqldb]` / `[[provider.sqldb]]`
        let new_sqldb = provider
            .sqldb
            .as_ref()
            .is_some_and(|specs| !specs.is_empty());
        // 新版 `[provider.redis]`
        let new_redis = provider.redis.is_some();
        legacy_kind || new_sqldb || new_redis
    });
    has_external && probe.tables.into_iter().all(|table| !table.enabled)
}

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
                warn_ctrl!(
                    "init thread-cloned knowdb provider failed (conf={}): {:#?}",
                    self.conf.display(),
                    err
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uses_external_provider_only_with_provider_only_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider]
kind = "postgres"
connection_uri = "postgres://demo"
            "#,
        )
        .expect("write knowdb");

        assert!(uses_external_provider_only(&conf));
    }

    #[test]
    fn test_uses_external_provider_only_false_when_local_tables_exist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider]
kind = "postgres"
connection_uri = "postgres://demo"

[[tables]]
name = "local_asset_data"
            "#,
        )
        .expect("write knowdb");

        assert!(!uses_external_provider_only(&conf));
        assert_eq!(
            load_local_table_names(&conf),
            vec!["local_asset_data".to_string()]
        );
    }

    #[test]
    fn test_load_provider_table_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider]
kind = "postgres"
connection_uri = "postgres://demo"
tables = ["asset_data", "zone"]
            "#,
        )
        .expect("write knowdb");

        assert_eq!(
            load_provider_table_names(&conf),
            vec!["asset_data".to_string(), "zone".to_string()]
        );
    }

    #[test]
    fn test_uses_external_provider_only_with_new_array_format() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[[provider.sqldb]]
name = "geo"
kind = "postgres"
connection_uri = "postgres://demo@127.0.0.1/geo_db"

[[provider.sqldb]]
name = "asset"
kind = "postgres"
connection_uri = "postgres://demo@127.0.0.1/asset_db"
            "#,
        )
        .expect("write knowdb");

        assert!(uses_external_provider_only(&conf));
    }

    #[test]
    fn test_uses_external_provider_only_with_new_single_format() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider.sqldb]
kind = "postgres"
connection_uri = "postgres://demo@127.0.0.1/demo"
            "#,
        )
        .expect("write knowdb");

        assert!(uses_external_provider_only(&conf));
    }

    #[test]
    fn test_uses_external_provider_only_with_redis_new_format() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider.redis]
connection_uri = "redis://127.0.0.1:6379"
            "#,
        )
        .expect("write knowdb");

        assert!(uses_external_provider_only(&conf));
    }

    #[test]
    fn test_uses_external_provider_only_false_when_new_format_has_local_tables() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conf = dir.path().join("knowdb.toml");
        std::fs::write(
            &conf,
            r#"
version = 2
[provider.sqldb]
kind = "postgres"
connection_uri = "postgres://demo@127.0.0.1/demo"

[[tables]]
name = "local_asset_data"
            "#,
        )
        .expect("write knowdb");

        assert!(!uses_external_provider_only(&conf));
        assert_eq!(
            load_local_table_names(&conf),
            vec!["local_asset_data".to_string()]
        );
    }
}
