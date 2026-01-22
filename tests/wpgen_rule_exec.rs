//! Integration tests for wpgen rule generation pipeline.
//!
//! These tests exercise the end-to-end path that `wpgen rule` relies on:
//! - load_wpgen_resolved (new-format wpgen.toml)
//! - load_gen_confs (discover gen_rule.wpl + gen_field.toml under rule_root)
//! - rule_gen_run (spawn generator -> sink pipeline)
//!
//! They help guard against regressions where `wpgen rule` produces no data.

use std::fs;
use std::path::{Path, PathBuf};

use orion_variate::EnvDict;
use wp_conf::test_support::ForTest;
use wp_engine::facade::config::{WPGEN_TOML, WarpConf, build_sink_target};
use wp_engine::facade::generator::{GenGRA, RuleGRA, load_gen_confs, rule_gen_run};

fn unique_tmp(prefix: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let p = Path::new("./tmp").join(format!("{}_{}", prefix, now));
    fs::create_dir_all(&p).ok();
    p
}

fn write_file(p: impl AsRef<Path>, content: &str) {
    if let Some(parent) = p.as_ref().parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn wpgen_rule_from_files_produces_data() {
    // Ensure built-in sink factories (file/null) are registered for tests
    wp_engine::sinks::register_builtin_sinks();
    // Arrange: isolated work root and minimal new-format wpgen.toml
    let work = unique_tmp("wpgen_rule_smoke");
    let cm = WarpConf::new(&work);

    // Resolve output to a per-test file under the temp work root
    let _out_dir = work.join("data/out_dat");

    // Prepare a minimal file sink connector and reference it via output.connect
    let cdir = work.join("connectors/sink.d");
    fs::create_dir_all(&cdir).unwrap();
    let connectors = r#"
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "path", "fmt"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "out.dat"
"#;
    write_file(cdir.join("file_json_sink.toml"), connectors);

    // New-format config: rule mode; explicitly set output.connect to file_json_sink
    let wpgen_toml = r#"version = "1.0"

[generator]
mode = "rule"
count = 8
duration_secs = 0
speed = 100
parallel = 2

[output]
connect = "file_json_sink"

[logging]
level = "info"
output = "stdout"
"#
    .to_string();
    let wpgen_path = cm.config_path(WPGEN_TOML);
    write_file(&wpgen_path, &wpgen_toml);

    // Prepare generator rule files in a dedicated rule_root
    let rule_root = work.join("models/wpl/smoke");
    let gen_rule = r#"package /smoke { rule r1 { (digit,ip,chars) } }"#;
    write_file(rule_root.join("gen_rule.wpl"), gen_rule);
    // optional: empty field mapping is acceptable for this rule
    write_file(rule_root.join("gen_field.toml"), "items = {}\n");

    // Act: assemble runtime target and run the generator
    let rt = cm
        .load_wpgen_config(WPGEN_TOML, &EnvDict::test_default())
        .expect("load wpgen resolved");
    let out_target = build_sink_target(&rt.out_sink, 0, 1, 0)
        .await
        .expect("build sink target");
    let rules = load_gen_confs(rule_root.to_str().unwrap(), &EnvDict::test_default())
        .expect("load gen_rule.wpl under rule_root");
    assert!(!rules.is_empty(), "rules should not be empty");

    let gnc = RuleGRA {
        gen_conf: GenGRA {
            total_line: Some(8),
            gen_speed: 10_000,
            parallel: 2,
            stat_sec: 1,
            stat_print: false,
            rescue: work.join("data/rescue").display().to_string(),
            speed_profile: None,
        },
    };

    rule_gen_run(gnc, rules, out_target)
        .await
        .expect("generator should succeed");

    // Assert: output file exists and non-empty (default fallback path)
    let out_path = rt.out_sink.resolve_file_path().expect("resolved file path");
    // The default fallback stores a relative path (./data/out_dat/out.dat). Normalize to abs and verify content.
    let out_abs = if Path::new(&out_path).is_absolute() {
        PathBuf::from(&out_path)
    } else {
        std::env::current_dir().unwrap().join(out_path)
    };
    let meta = fs::metadata(&out_abs).expect("stat out file");
    assert!(meta.len() > 0, "generated output should be non-empty");
}

#[test]
fn wpgen_rule_missing_rule_files_returns_error() {
    // Arrange: empty rule_root directory
    let work = unique_tmp("wpgen_rule_empty");
    let rule_root = work.join("models/wpl/empty");
    fs::create_dir_all(&rule_root).unwrap();

    // Act: attempt to discover generator configs
    let res = load_gen_confs(rule_root.to_str().unwrap(), &EnvDict::test_default());
    assert!(res.is_err(), "expected error when rule files are missing");
    // Assert: clear diagnostic for empty rule set
    let msg = format!("{}", res.err().unwrap());
    assert!(msg.contains("gen rule conf file is empty"), "msg={}", msg);
}
