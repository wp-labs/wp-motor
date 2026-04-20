use orion_conf::error::OrionConfResult;
use orion_error::{ErrorOweSource, ErrorWith, OperationContext};
use orion_variate::EnvDict;
use std::fs;
use std::path::Path;
use wp_conf::structure::SinkInstanceConf;
use wp_error::diagnostic_meta::{ComponentKind, ConfigKind, OperationContextMetaExt};

#[derive(Debug, Clone)]
pub struct DataCleanItem {
    pub sink: String,
    pub path: Option<String>,
    pub existed: bool,
    pub cleaned: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DataCleanReport {
    pub items: Vec<DataCleanItem>,
}

impl DataCleanReport {
    pub fn cleaned_count(&self) -> usize {
        self.items.iter().filter(|i| i.cleaned).count()
    }
}

/// Clean file-like outputs for all configured sinks under sink_root (business.d/infra.d)
pub fn clean_outputs(sink_root: &Path, env_dict: &EnvDict) -> OrionConfResult<DataCleanReport> {
    let mut rep = DataCleanReport::default();
    if !(sink_root.join("business.d").exists() || sink_root.join("infra.d").exists()) {
        return Ok(rep);
    }
    for conf in
        wp_conf::sinks::load_infra_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)
            .with(sink_root)
            .want("load infra sink routes for clean")?
    {
        for s in conf.sink_group.sinks.iter() {
            append_clean_item(&mut rep, s)?;
        }
    }
    for conf in
        wp_conf::sinks::load_business_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)
            .with(sink_root)
            .want("load business sink routes for clean")?
    {
        for s in conf.sink_group.sinks.iter() {
            append_clean_item(&mut rep, s)?;
        }
    }
    Ok(rep)
}

fn append_clean_item(rep: &mut DataCleanReport, s: &SinkInstanceConf) -> OrionConfResult<()> {
    let path = s.resolve_file_path();
    if let Some(p) = &path {
        let path_ref = Path::new(p);
        let existed = path_ref.exists();
        let mut cleaned = false;
        if existed {
            fs::remove_file(path_ref)
                .owe_conf_source()
                .with(
                    OperationContext::want("clean sink output file")
                        .with_meta_value(ConfigKind::SinkRuntime)
                        .with_meta_value(ComponentKind::Sink)
                        .with_component_name(s.full_name())
                        .with_file_path(path_ref),
                )
                .with(path_ref)
                .want("remove sink output file")?;
            cleaned = true;
        }
        rep.items.push(DataCleanItem {
            sink: s.full_name(),
            path,
            existed,
            cleaned,
        });
    } else {
        rep.items.push(DataCleanItem {
            sink: s.full_name(),
            path: None,
            existed: false,
            cleaned: false,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clean_outputs;
    use orion_variate::EnvDict;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wp_conf::test_support::ForTest;
    use wp_error::diagnostic_meta::{ComponentKind, ConfigKind, MetaValue, first_meta_str, key};

    fn tmp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn clean_outputs_reports_route_config_errors() {
        let root = tmp_dir("wpcli_clean_invalid");
        let sink_root = root.join("sink");
        let biz = sink_root.join("business.d");
        fs::create_dir_all(&biz).unwrap();
        fs::write(
            biz.join("broken.toml"),
            r#"version = "2.0"

[sink_group]
name = "broken"
rule = ["/demo"]
oml = ["demo"]
"#,
        )
        .unwrap();

        let err = clean_outputs(&sink_root, &EnvDict::test_default())
            .expect_err("invalid sink route config should fail");
        let msg = format!("{:#}", err);
        assert!(msg.contains("validation error"));
        assert!(msg.contains("group 'broken' has no sinks"));
    }

    #[test]
    fn clean_outputs_reports_delete_failure_with_metadata() {
        let root = tmp_dir("wpcli_clean_delete_failure");
        let sink_root = root.join("topology").join("sinks");
        let connector_dir = root.join("connectors").join("sink.d");
        let infra = sink_root.join("infra.d");
        fs::create_dir_all(&connector_dir).unwrap();
        fs::create_dir_all(&infra).unwrap();

        fs::write(
            connector_dir.join("file.toml"),
            r#"[[connectors]]
id = "file_sink"
type = "file"
allow_override = ["path"]
"#,
        )
        .unwrap();

        let output_path = root.join("data").join("out").join("blocked.dat");
        fs::create_dir_all(&output_path).unwrap();
        fs::write(
            infra.join("default.toml"),
            format!(
                r#"version = "2.0"

[sink_group]
name = "default"

[[sink_group.sinks]]
name = "blocked"
connect = "file_sink"
params = {{ path = "{}" }}
"#,
                output_path.display()
            ),
        )
        .unwrap();

        let err = clean_outputs(&sink_root, &EnvDict::test_default())
            .expect_err("directory at sink output path should fail remove_file");
        let chain = err.display_chain();
        let report = err.report();
        let output_path_str = output_path.display().to_string();

        assert_eq!(
            first_meta_str(&report, key::CONFIG_KIND),
            Some(ConfigKind::SinkRuntime.as_str())
        );
        assert_eq!(
            first_meta_str(&report, key::COMPONENT_KIND),
            Some(ComponentKind::Sink.as_str())
        );
        assert_eq!(
            first_meta_str(&report, key::COMPONENT_NAME),
            Some("default/blocked")
        );
        assert_eq!(
            first_meta_str(&report, key::FILE_PATH),
            Some(output_path_str.as_str())
        );
        assert!(
            chain.contains("remove sink output file"),
            "unexpected error chain: {chain}"
        );
    }
}
