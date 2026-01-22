use orion_variate::EnvDict;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;
use wp_conf::test_support::ForTest;
use wp_engine::facade::test_helpers as fth;

static INIT: Once = Once::new();
fn ensure_runtime_inited() {
    INIT.call_once(|| {
        wp_engine::connectors::startup::init_runtime_registries();
    });
}

#[tokio::test]
async fn test_unified_sources_config_build_file_source() {
    // 1) 准备一个临时文件作为文件源
    let tmp_path = PathBuf::from("/tmp/wparse_unified_sources_test.log");
    {
        let mut f = File::create(&tmp_path).expect("create tmp file failed");
        let _ = writeln!(f, "hello");
    }

    // 2) 集中初始化（注册所有内置工厂）
    ensure_runtime_inited();

    // 3) 构建 v2 connectors 目录，写入文件连接器；构造 v2 [[sources]] 配置
    let work = PathBuf::from("tmp/test_v2");
    let cdir = fth::create_connectors_dir(&work);
    std::fs::write(
        cdir.join("c1.toml"),
        format!(
            r#"[[connectors]]
id = "file_main"
type = "file"
allow_override = ["path","encode"]
[connectors.params]
path = "{}"
encode = "text"
"#,
            tmp_path.display()
        ),
    )
    .unwrap();
    let cfg = r#"
[[sources]]
key = "file_unified"
enable = true
connect = "file_main"
params_override = { }
"#;

    // 4) 解析并构建（work_dir 指向 tmp/test_v2，使解析器能向上寻找 connectors/source.d）
    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let env_dict = EnvDict::new();
    let (inits, _) = parser
        .parse_and_build_from(cfg, &env_dict)
        .await
        .expect("parse_and_build_from_str failed");

    assert_eq!(inits.len(), 1);
    assert_eq!(inits[0].source.identifier(), "file_unified");
}

#[test]
fn test_unified_sources_config_validate_only() {
    // 1) 集中初始化（注册所有内置工厂）
    ensure_runtime_inited();

    // 2) 准备 work 根与 connectors（从 work 起向上查找 connectors/source.d）
    let work = PathBuf::from("tmp/test_v2_val");
    let cdir = fth::create_connectors_dir(&work);
    std::fs::write(
        cdir.join("c1.toml"),
        format!(
            r#"[[connectors]]
id = "file_main"
type = "file"
allow_override = ["path","encode"]
[connectors.params]
path = "{}"
encode = "text"
"#,
            "/tmp/wparse_unified_sources_validate.log"
        ),
    )
    .unwrap();
    let cfg = r#"
[[sources]]
key = "file_unified"
enable = true
connect = "file_main"
params_override = { }
"#;

    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let specs = parser
        .parse_and_validate_only(cfg, &EnvDict::test_default())
        .expect("validate-only failed");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "file_unified");
    // 简化后的 validate-only 不解析 connectors，不填充 kind
    assert!(specs[0].kind.is_empty());
}

#[test]
fn test_validate_only_without_connectors_ok() {
    // 不创建 connectors；validate-only 应该成功返回最小 CoreSourceSpec
    let work = PathBuf::from("tmp/test_v2_val_noconn");
    std::fs::create_dir_all(&work).unwrap();
    let cfg = r#"
[[sources]]
key = "s1"
enable = true
connect = "missing_conn"
tags = ["env:test"]
"#;
    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let specs = parser
        .parse_and_validate_only(cfg, &EnvDict::test_default())
        .expect("validate-only without connectors should succeed");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "s1");
    // 简化校验：kind 为空，params 为空表
    assert!(specs[0].kind.is_empty());
    assert!(specs[0].params.is_empty());
}

#[tokio::test]
async fn test_build_requires_connectors() {
    // 未创建 connectors，parse_and_build_from 应失败
    let work = PathBuf::from("tmp/test_v2_build_noconn");
    std::fs::create_dir_all(&work).unwrap();
    let cfg = r#"
[[sources]]
key = "s1"
enable = true
connect = "file_main"
"#;
    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let env_dict = EnvDict::new();
    let res = parser.parse_and_build_from(cfg, &env_dict).await;
    assert!(res.is_err(), "expect error without connectors");
    let s = format!("{}", res.unwrap_err());
    println!("{}", s);
    assert!(s.contains("connector not found"));
}

#[tokio::test]
async fn test_build_file_source_with_base_file_params() {
    // 1) 准备临时输出文件（作为输入文件路径）
    let tmpdir = std::env::temp_dir();
    let tmpfile = tmpdir.join("wparse_unified_sources_base_file.log");
    std::fs::write(&tmpfile, "hello\n").unwrap();

    // 2) 集中初始化（注册所有内置工厂）
    ensure_runtime_inited();

    // 3) connectors 使用 base+file 形式
    let work = PathBuf::from("tmp/test_v2_base_file");
    let cdir = fth::create_connectors_dir(&work);
    std::fs::create_dir_all(&cdir).unwrap();
    std::fs::write(
        cdir.join("c1.toml"),
        format!(
            r#"[[connectors]]
id = "file_main"
type = "file"
allow_override = ["base","file","encode"]
[connectors.params]
base = "{}"
file = "{}"
encode = "text"
"#,
            tmpdir.display(),
            tmpfile.file_name().unwrap().to_string_lossy()
        ),
    )
    .unwrap();
    let cfg = r#"
[[sources]]
key = "file_unified"
enable = true
connect = "file_main"
params_override = { }
"#;

    // 4) 构建并断言成功
    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let env_dict = EnvDict::new();
    let (inits, _) = parser
        .parse_and_build_from(cfg, &env_dict)
        .await
        .expect("parse_and_build_from (base+file) failed");
    assert_eq!(inits.len(), 1);
    assert_eq!(inits[0].source.identifier(), "file_unified");
}

#[tokio::test]
async fn test_build_override_whitelist_enforced() {
    // connectors 不允许覆写 encode，但配置尝试覆写，应报错
    ensure_runtime_inited();
    let work = PathBuf::from("tmp/test_v2_whitelist");
    let cdir = fth::create_connectors_dir(&work);
    std::fs::create_dir_all(&cdir).unwrap();
    let tmp_path = std::env::temp_dir().join("wparse_unified_sources_wl.log");
    std::fs::write(&tmp_path, "hello\n").unwrap();
    std::fs::write(
        cdir.join("c1.toml"),
        format!(
            r#"[[connectors]]
id = "file_main"
type = "file"
allow_override = ["path"]
[connectors.params]
path = "{}"
encode = "text"
"#,
            tmp_path.display()
        ),
    )
    .unwrap();
    let cfg = r#"
[[sources]]
key = "s1"
enable = true
connect = "file_main"
params_override = { encode = "hex" }
"#;
    let parser = wp_engine::sources::SourceConfigParser::new(work.clone());
    let env_dict = EnvDict::new();
    let res = parser.parse_and_build_from(cfg, &env_dict).await;
    assert!(res.is_err(), "expect error due to override not allowed");
}

// legacy bridge removed in v2; test deleted
