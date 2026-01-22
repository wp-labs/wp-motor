use anyhow::Result;
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
pub fn clean_outputs(sink_root: &Path, env_dict: &EnvDict) -> Result<DataCleanReport> {
    let mut rep = DataCleanReport::default();
    if !(sink_root.join("business.d").exists() || sink_root.join("infra.d").exists()) {
        return Ok(rep);
    }
    for conf in
        wp_conf::sinks::load_infra_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)
            .unwrap_or_default()
    {
        for s in conf.sink_group.sinks.iter() {
            append_clean_item(&mut rep, s)?;
        }
    }
    for conf in
        wp_conf::sinks::load_business_route_confs(sink_root.to_string_lossy().as_ref(), env_dict)
            .unwrap_or_default()
    {
        for s in conf.sink_group.sinks.iter() {
            append_clean_item(&mut rep, s)?;
        }
    }
    Ok(rep)
}

fn append_clean_item(rep: &mut DataCleanReport, s: &SinkInstanceConf) -> Result<()> {
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
