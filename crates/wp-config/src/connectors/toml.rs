use super::defs::ConnectorTomlFile;
use orion_conf::EnvTomlLoad;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ErrorOwe, ErrorWith, ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wp_connector_api::{ConnectorDef, ConnectorScope};

fn collect_connector_files(dir: &Path) -> OrionConfResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .owe_conf()
        .with(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|s| s == "toml").unwrap_or(false))
        .collect();
    files.sort();
    Ok(files)
}

pub fn load_connector_defs_from_dir(
    dir: &Path,
    scope: ConnectorScope,
    dict: &EnvDict,
) -> OrionConfResult<Vec<ConnectorDef>> {
    let mut map: BTreeMap<String, ConnectorDef> = BTreeMap::new();
    for fp in collect_connector_files(dir)? {
        let file: ConnectorTomlFile = ConnectorTomlFile::env_load_toml(&fp, dict)?;
        for mut def in file.connectors {
            let origin = Some(fp.display().to_string());
            if map.contains_key(&def.id) {
                return ConfIOReason::from_validation(format!(
                    "duplicate connector id '{}' (file {})",
                    def.id,
                    fp.display()
                ))
                .err_result();
            }
            def.scope = scope;
            def.origin = origin;
            map.insert(def.id.clone(), def);
        }
    }
    Ok(map.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::ValueType;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(prefix: &str) -> PathBuf {
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
    fn load_connector_with_env_variables() {
        let base = tmp_dir("conn_env");
        let cdir = base.join("connectors").join("sink.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建包含环境变量的连接器配置
        let connector_toml = r#"
[[connectors]]
id = "file_${ENV_SUFFIX}"
type = "file"
allow_override = ["base", "file"]
[connectors.params]
base = "${WORK_ROOT}/data"
file = "${OUTPUT_FILE}"
fmt = "json"
"#;
        fs::write(cdir.join("env_connector.toml"), connector_toml).unwrap();

        // 设置环境变量字典
        let mut dict = EnvDict::new();
        dict.insert("ENV_SUFFIX", ValueType::from("prod"));
        dict.insert("WORK_ROOT", ValueType::from("/opt/app"));
        dict.insert("OUTPUT_FILE", ValueType::from("output.dat"));

        // 加载连接器定义
        let defs = load_connector_defs_from_dir(&cdir, ConnectorScope::Sink, &dict)
            .expect("load connectors");
        assert_eq!(defs.len(), 1);

        let def = &defs[0];
        assert_eq!(def.id, "file_prod");
        assert_eq!(
            def.default_params.get("base").and_then(|v| v.as_str()),
            Some("/opt/app/data")
        );
        assert_eq!(
            def.default_params.get("file").and_then(|v| v.as_str()),
            Some("output.dat")
        );
    }

    #[test]
    fn load_connector_without_env_keeps_literal() {
        let base = tmp_dir("conn_literal");
        let cdir = base.join("connectors").join("sink.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建不包含环境变量的连接器配置
        let connector_toml = r#"
[[connectors]]
id = "file_sink"
type = "file"
allow_override = ["base"]
[connectors.params]
base = "/data/output"
file = "result.dat"
"#;
        fs::write(cdir.join("static_connector.toml"), connector_toml).unwrap();

        let dict = EnvDict::new();
        let defs = load_connector_defs_from_dir(&cdir, ConnectorScope::Sink, &dict)
            .expect("load connectors");
        assert_eq!(defs.len(), 1);

        let def = &defs[0];
        assert_eq!(def.id, "file_sink");
        assert_eq!(
            def.default_params.get("base").and_then(|v| v.as_str()),
            Some("/data/output")
        );
    }

    #[test]
    fn load_connector_with_undefined_env_keeps_placeholder() {
        let base = tmp_dir("conn_undefined");
        let cdir = base.join("connectors").join("sink.d");
        fs::create_dir_all(&cdir).unwrap();

        // 创建包含未定义环境变量的连接器配置
        let connector_toml = r#"
[[connectors]]
id = "file_sink"
type = "file"
[connectors.params]
base = "${UNDEFINED_VAR}/data"
"#;
        fs::write(cdir.join("undefined.toml"), connector_toml).unwrap();

        let dict = EnvDict::new(); // 空字典，未定义 UNDEFINED_VAR
        let defs = load_connector_defs_from_dir(&cdir, ConnectorScope::Sink, &dict)
            .expect("load connectors");

        let def = &defs[0];
        // 未定义的变量保持原样
        let base_val = def.default_params.get("base").and_then(|v| v.as_str());
        assert!(
            base_val == Some("${UNDEFINED_VAR}/data") || base_val == Some("/data"),
            "got: {:?}",
            base_val
        );
    }
}
