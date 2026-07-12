use crate::compat::LegacyOwe;
// Loader: aggregate submodules for config loading
mod manager;
mod source;
mod wpgen;

pub use manager::WarpConf;
pub use wp_conf::loader::ConfDelegate;

#[cfg(test)]
mod tests {
    use crate::compat::LegacyOwe;
    use crate::orchestrator::config::WPGEN_TOML;
    use crate::orchestrator::config::loader::WarpConf;
    use crate::test_support::TestCasePath;
    use orion_conf::error::{ConfIOReason, OrionConfResult};
    use orion_variate::EnvDict;
    use std::fs;
    use std::path::Path;
    use wp_conf::test_support::ForTest;
    use wp_log::conf::Output as LogOutput;

    #[test]
    fn test_wpgen_conf_init_clean() -> OrionConfResult<()> {
        use crate::orchestrator::config::models::wpgen::WpGenConfig;
        let tw = TestCasePath::new("wp", "wpgen_conf").owe(ConfIOReason::core_conf())?;
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
    fn test_wpgen_resolved_without_connector() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_no_conn").owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);
        // 写入最小新格式配置（不使用 connectors）
        let toml = r#"
version = "1.0"

[generator]
count = 10
speed = 100
parallel = 2

[output]

[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        assert!(
            cm.load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn test_wpgen_resolved_with_connector_and_override() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_resolved").owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        // 准备 sink 连接器文件（id = file_json_sink）
        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
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
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;
        // 写入新格式配置：引用 file_json_sink，并覆盖 file 名称
        let toml = r#"
version = "1.0"
[generator]
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
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
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
    fn test_wpgen_rejects_output_stream_tag_field() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_meta_params").owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
        let cfile = format!(
            "{}/connectors/sink.d/file_json_sink.toml",
            cm.work_root_path()
        );
        let connectors = r#"
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.dat"
"#;
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;

        let toml = r#"
version = "1.0"
[generator]
count = 5
speed = 50
parallel = 1
[output]
name = "test_out"
connect = "file_json_sink"
params = { file = "over.dat", stream_tag_field = "wp_stream_tag" }
[logging]
level = "info"
output = "file"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("runtime output param 'stream_tag_field' is not supported in wpgen"),
            "msg={}",
            msg
        );
        cm.clear_work_directory();
        Ok(())
    }

    #[test]
    fn test_wpgen_output_connect_reports_missing_sink_detail() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_missing_sink").owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
        let cfile = format!("{}/connectors/sink.d/tcp_sink.toml", cm.work_root_path());
        let connectors = r#"
[[connectors]]
id = "tcp_sink"
type = "tcp"
allow_override = ["addr", "port"]
[connectors.params]
addr = "127.0.0.1"
port = 9000
"#;
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;
        let toml = r#"
version = "1.0"
[generator]
speed = 1
[output]
connect = "tcp_src"
[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("[output].connect 'tcp_src'"), "msg={}", msg);
        assert!(msg.contains("connectors/sink.d"), "msg={}", msg);
        assert!(msg.contains("tcp_sink"), "msg={}", msg);
        assert!(msg.contains("not a source connector id"), "msg={}", msg);
        cm.clear_work_directory();
        Ok(())
    }

    #[test]
    fn test_wpgen_resolved_override_not_allowed() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_resolved_2").owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);
        // 仅允许 base/file
        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
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
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;
        let toml = r#"
version = "1.0"
[generator]
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
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("override 'path' not allowed"), "msg={}", msg);
        cm.clear_work_directory();
        Ok(())
    }

    #[test]
    fn test_wpgen_rejects_stream_tag_field_even_if_connector_allows_it() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_reject_stream_tag_even_allowed")
            .owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
        let cfile = format!(
            "{}/connectors/sink.d/file_json_sink.toml",
            cm.work_root_path()
        );
        let connectors = r#"
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "stream_tag_field"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.dat"
"#;
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;

        let toml = r#"
version = "1.0"
[generator]
speed = 1
[output]
name = "test_out"
connect = "file_json_sink"
params = { file = "over.dat", stream_tag_field = "wp_stream_tag" }
[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("runtime output param 'stream_tag_field' is not supported in wpgen"),
            "msg={}",
            msg
        );
        cm.clear_work_directory();
        Ok(())
    }

    #[test]
    fn test_wpgen_rejects_wp_meta_disable_even_if_connector_allows_it() -> OrionConfResult<()> {
        let tw = TestCasePath::new("wp", "wpgen_reject_wp_meta_disable_even_allowed")
            .owe(ConfIOReason::core_conf())?;
        let path = tw.path_string();
        let cm = WarpConf::new(&path);

        let cdir = format!("{}/connectors/sink.d", cm.work_root_path());
        std::fs::create_dir_all(&cdir).owe(ConfIOReason::core_conf())?;
        let cfile = format!(
            "{}/connectors/sink.d/file_json_sink.toml",
            cm.work_root_path()
        );
        let connectors = r#"
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "wp_meta_disable"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.dat"
"#;
        fs::write(cfile, connectors).owe(ConfIOReason::core_conf())?;

        let toml = r#"
version = "1.0"
[generator]
speed = 1
[output]
name = "test_out"
connect = "file_json_sink"
params = { file = "over.dat", wp_meta_disable = ["wp_oml_name"] }
[logging]
level = "info"
output = "stdout"
"#;
        let p = cm.ensure_config_path_exists(WPGEN_TOML)?;
        fs::write(&p, toml).owe(ConfIOReason::core_conf())?;
        let err = cm
            .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("runtime output param 'wp_meta_disable' is not supported in wpgen"),
            "msg={}",
            msg
        );
        cm.clear_work_directory();
        Ok(())
    }
}
