use super::defaults::{ConnectorTemplate, registered_templates};
use orion_conf::{ErrorOwe, ErrorWith};
use std::fs;
use std::path::Path;
use toml::Value;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, param_map_to_table};
use wp_error::run_error::RunResult;

pub fn init_definitions<P: AsRef<Path>>(work_root: P) -> RunResult<()> {
    for template in registered_templates() {
        write_template_if_absent(work_root.as_ref(), &template)?;
    }
    Ok(())
}

fn write_template_if_absent(work_root: &Path, template: &ConnectorTemplate) -> RunResult<()> {
    let dir = match template.scope {
        ConnectorScope::Source => work_root.join("connectors/source.d"),
        ConnectorScope::Sink => work_root.join("connectors/sink.d"),
    };
    fs::create_dir_all(&dir)
        .owe_res()
        .want("create connector template dir")
        .with(&dir)?;
    let path = dir.join(&template.file_name);
    if path.exists() {
        return Ok(());
    }
    let body = render_connector_file(&template.connectors)?;
    fs::write(&path, body.as_bytes())
        .owe_res()
        .want("write connector template")
        .with(&path)?;
    Ok(())
}

fn render_connector_file(connectors: &[ConnectorDef]) -> RunResult<String> {
    let mut entries = Vec::new();
    for def in connectors {
        entries.push(connector_to_value(def));
    }
    let mut root = toml::value::Table::new();
    root.insert("connectors".to_string(), Value::Array(entries));
    toml::to_string(&Value::Table(root))
        .owe_res()
        .want("serialize connector template")
}

fn connector_to_value(def: &ConnectorDef) -> Value {
    let mut entry = toml::value::Table::new();
    entry.insert("id".into(), Value::String(def.id.clone()));
    entry.insert("type".into(), Value::String(def.kind.clone()));
    if !def.allow_override.is_empty() {
        let arr = def
            .allow_override
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        entry.insert("allow_override".into(), Value::Array(arr));
    }
    if !def.default_params.is_empty() {
        entry.insert(
            "params".into(),
            Value::Table(param_map_to_table(&def.default_params)),
        );
    }
    Value::Table(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::temp_workdir;

    #[test]
    fn init_templates_creates_expected_files() {
        let temp = temp_workdir();
        init_definitions(temp.path()).expect("init templates");
        let templates = registered_templates();
        let first_source = templates
            .iter()
            .find(|t| t.scope == ConnectorScope::Source)
            .expect("source template");
        assert!(
            temp.path()
                .join(format!("connectors/source.d/{}", first_source.file_name))
                .exists()
        );
        let first_sink = templates
            .iter()
            .find(|t| t.scope == ConnectorScope::Sink)
            .expect("sink template");
        assert!(
            temp.path()
                .join(format!("connectors/sink.d/{}", first_sink.file_name))
                .exists()
        );
    }

    #[test]
    fn init_templates_does_not_overwrite_existing_file() {
        let temp = temp_workdir();
        let templates = registered_templates();
        let sample = templates
            .iter()
            .find(|t| t.scope == ConnectorScope::Source)
            .expect("source template");
        let custom = temp
            .path()
            .join(format!("connectors/source.d/{}", sample.file_name));
        fs::create_dir_all(custom.parent().unwrap()).unwrap();
        fs::write(&custom, "[[connectors]]\ncustom = true\n").unwrap();

        init_definitions(temp.path()).expect("init templates");

        let body = std::fs::read_to_string(&custom).unwrap();
        assert!(body.contains("custom"));
    }

    #[test]
    fn render_connector_file_matches_expected_keys() {
        let temp_def = ConnectorDef {
            id: "demo".into(),
            kind: "file".into(),
            scope: ConnectorScope::Sink,
            allow_override: vec!["base".into()],
            default_params: {
                let mut p = wp_connector_api::ParamMap::new();
                p.insert("base".into(), serde_json::Value::String("./out".into()));
                p
            },
            origin: None,
        };
        let body = render_connector_file(&[temp_def]).expect("render");
        assert!(body.contains("[[connectors]]"));
        assert!(body.contains("id = \"demo\""));
        assert!(body.contains("allow_override"));
    }
}
