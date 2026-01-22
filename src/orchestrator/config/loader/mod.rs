// Loader: aggregate submodules for config loading
mod manager;
mod source;
mod wpgen;

pub use manager::WarpConf;
pub use wp_conf::loader::ConfDelegate;

#[cfg(test)]
mod tests {
    use crate::orchestrator::config::WPGEN_TOML;
    use crate::orchestrator::config::loader::WarpConf;
    use crate::test_support::TestCasePath;
    use crate::types::AnyResult;
    use orion_conf::ErrorOwe;
    use orion_conf::error::OrionConfResult;
    use orion_variate::EnvDict;
    use std::fs;
    use std::path::Path;
    use wp_conf::test_support::ForTest;
    use wp_log::conf::Output as LogOutput;

    #[test]
    fn test_wpgen_conf_init_clean() -> OrionConfResult<()> {
        use crate::orchestrator::config::models::wpgen::WpGenConfig;
        let tw = TestCasePath::new("wp", "wpgen_conf").owe_conf()?;
        let path = tw.path_string();
        let conf_manager = WarpConf::new(&path);
        conf_manager.clear_work_directory();
        let delegate = conf_manager.create_config_delegate::<WpGenConfig>(WPGEN_TOML);
        delegate.init()?;
        delegate.load(&EnvDict::test_default())?;
        delegate.safe_clean()?;
        Ok(())
    }

    #[test]
    fn test_wpgen_resolved_without_connector() -> AnyResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_no_conn")?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);
        // 写入最小新格式配置（不使用 connectors）
        let toml = r#"
version = "1.0"

[generator]
mode = "rule"
count = 10
duration_secs = 0
speed = 100
parallel = 2

[output]

[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml)?;
        assert!(
            cm.load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn test_wpgen_resolved_with_connector_and_override() -> AnyResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_resolved")?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        // 准备 sink 连接器文件（id = file_json_sink）
        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir)?;
        let cfile = format!(
            "{}/connectors/sink.d/file_json_sink.toml",
            cm.work_root_path()
        );
        let connectors = r#"
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "path", "fmt"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.dat"
"#;
        fs::write(cfile, connectors)?;
        // 写入新格式配置：引用 file_json_sink，并覆盖 file 名称
        let toml = r#"
version = "1.0"
[generator]
mode = "rule"
count = 5
speed = 50
parallel = 1
[output]
name = "test_out"
connect = "file_json_sink"
params = { file = "over.dat" }
[logging]
level = "info"
output = "file"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml)?;
        let rt = cm.load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())?;
        assert_eq!(rt.out_sink.resolved_kind_str(), "file");
        // 覆盖应生效，最终路径包含 over.dat
        let fpath = rt.out_sink.resolve_file_path().expect("file path");
        assert!(fpath.ends_with("over.dat"), "got path={}", fpath);
        // connector id 应保留
        assert_eq!(rt.out_sink.connector_id.as_deref(), Some("file_json_sink"));
        cm.clear_work_directory();
        Ok(())
    }

    #[test]
    fn test_wpgen_resolved_override_not_allowed() -> AnyResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_resolved_2")?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);
        // 仅允许 base/file
        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir)?;
        let cfile = format!("{}/connectors/sink.d/01-file-raw.toml", cm.work_root_path());
        let connectors = r#"
[[connectors]]
id = "file_raw"
type = "file"
allow_override = ["base", "file"]
[connectors.params]
fmt = "raw"
base = "./data/out_dat"
file = "a.dat"
"#;
        fs::write(cfile, connectors)?;
        let toml = r#"
version = "1.0"
[generator]
mode = "rule"
speed = 1
[output]
name = "test_out"
connect = "file_raw"
params = { path = "xxx" } # 不在白名单
[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml)?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("override 'path' not allowed"), "msg={}", msg);
        cm.clear_work_directory();
        Ok(())
    }
}
