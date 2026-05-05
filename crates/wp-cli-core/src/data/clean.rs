use orion_conf::ErrorWith;
use orion_conf::error::OrionConfResult;
use orion_variate::EnvDict;
use std::fs;
use std::path::Path;
use wp_conf::structure::SinkInstanceConf;

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
            .with_context(sink_root)
            .doing("load infra sink routes for clean")?
    {
        for s in conf.sink_group.sinks.iter() {
            append_clean_item(&mut rep, s)?;
        }
    }
    for conf in
        wp_conf::sinks::load_business_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)
            .with_context(sink_root)
            .doing("load business sink routes for clean")?
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
        let existed = Path::new(p).exists();
        let mut cleaned = false;
        if existed {
            cleaned = fs::remove_file(p).is_ok();
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
}
