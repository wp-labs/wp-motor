use crate::{
    core::parser::wpl_engine, facade::diagnostics::print_run_error,
    orchestrator::config::loader::WarpConf,
};
use orion_conf::ErrorWith;
use orion_error::{conversion::ConvErr, conversion::ToStructError};
use orion_variate::{EnvDict, EnvEvaluable};
use std::env;
use std::path::{Path, PathBuf};
use wp_conf::engine::EngineConfig;
use wp_conf::stat::StatConf;
use wp_conf::state::StateDir;
use wp_error::{
    RunReason,
    run_error::{IntoRunError, RunResult},
};
use wp_knowledge::facade;
use wp_log::conf::LogConf;
use wp_stat::{StatReq, StatRequires, StatStage, StatTarget};

/// Load configuration using a supplied EnvDict for templating overrides
pub fn load_warp_engine_confs(
    work_root: &str,
    dict: &EnvDict,
) -> RunResult<(WarpConf, EngineConfig)> {
    load_warp_engine_confs_with_state(work_root, dict, None)
}

/// Load configuration with an explicit state-directory override.
///
/// `state_dir_override` (CLI `--state-dir`) takes precedence over
/// `[runtime].state_dir`; an empty value is treated as "not provided".
/// The resolved [`StateDir`] is attached to the returned [`WarpConf`] so every
/// runtime path goes through the same directory.
pub fn load_warp_engine_confs_with_state(
    work_root: &str,
    dict: &EnvDict,
    state_dir_override: Option<&str>,
) -> RunResult<(WarpConf, EngineConfig)> {
    let conf_manager = WarpConf::new(work_root);
    let abs_root = conf_manager.work_root().to_path_buf();
    env::set_current_dir(&abs_root).map_err(|err| {
        error_ctrl!(
            "设置工作目录失败: path={}, error={}",
            abs_root.display(),
            &err
        );
        RunReason::core_conf()
            .to_err()
            .with_detail(format!("set work root '{}' failed", abs_root.display()))
            .with_source(err)
    })?;
    let main_conf = EngineConfig::load(&abs_root, dict)
        // 保留配置解析详情（IntoRunError 提取 ConfIOReason::Other 内层 toml 错误）
        .map_err(IntoRunError::into_run_err)
        .doing("load engine config failed")?
        .env_eval(dict)
        .conf_absolutize(&abs_root);

    let state_dir = resolve_state_dir(&abs_root, &main_conf, state_dir_override)?;
    Ok((conf_manager.with_state_dir(state_dir), main_conf))
}

/// Resolve the effective runtime state directory.
///
/// Precedence: explicit override (CLI `--state-dir`) > `[runtime].state_dir` >
/// `<work_root>/.run`. An empty override is ignored so a blank CLI value cannot
/// silently reset a configured directory.
pub fn resolve_state_dir(
    work_root: &Path,
    main_conf: &EngineConfig,
    state_dir_override: Option<&str>,
) -> RunResult<StateDir> {
    let raw = state_dir_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| main_conf.runtime_state_dir().to_string());
    StateDir::resolve(work_root, &raw).map_err(|err| {
        RunReason::core_conf().to_err().with_detail(format!(
            "[runtime].state_dir (or --state-dir) is invalid: {err}"
        ))
    })
}

const TOP_N: usize = 20;
pub fn stat_reqs_from(conf: &StatConf) -> StatRequires {
    // 将新结构 [[stat.<stage>]] 映射为运行期 StatReq
    const PICK_DEFAULT_FIELDS: [&str; 2] = ["wp_source_type", "wp_access_ip"];
    const PARSE_DEFAULT_FIELDS: [&str; 2] = ["wp_package_name", "wp_rule_name"];
    const SINK_DEFAULT_FIELDS: [&str; 2] = ["wp_sink_group", "wp_sink_name"];

    fn map_target(t: &str) -> StatTarget {
        match t.trim() {
            "*" => StatTarget::All,
            "ignore" => StatTarget::Ignore,
            other => StatTarget::Item(other.to_string()),
        }
    }

    fn collect_or_default(fields: &[String], defaults: &[&str]) -> Vec<String> {
        // 用户未配置 fields 时，使用阶段默认维度作为起始集合。
        if fields.is_empty() {
            defaults.iter().map(|x| (*x).to_string()).collect()
        } else {
            fields.to_vec()
        }
    }

    fn ensure_required_fields(mut fields: Vec<String>, required_fields: &[&str]) -> Vec<String> {
        // 长期监控依赖的维度无条件补齐（去重），不受用户 fields 覆盖影响。
        for req in required_fields {
            if !fields.iter().any(|f| f == req) {
                fields.push((*req).to_string());
            }
        }
        fields
    }

    fn push_stage_reqs(
        out: &mut Vec<StatReq>,
        stage: StatStage,
        items: &[wp_conf::stat::StatItem],
        stage_fields: &[&str],
    ) {
        for it in items {
            out.push(StatReq {
                stage: stage.clone(),
                name: it.key.clone(),
                target: map_target(it.target.as_str()),
                // 先走“用户配置或默认值”，再强制补齐阶段必需维度。
                collect: ensure_required_fields(
                    collect_or_default(&it.fields, stage_fields),
                    stage_fields,
                ),
                max: it.top_n.unwrap_or(TOP_N),
            });
        }
    }

    let mut requs = Vec::new();
    push_stage_reqs(
        &mut requs,
        StatStage::Pick,
        &conf.pick,
        &PICK_DEFAULT_FIELDS,
        // pick 监控长期依赖 wp_source_type/wp_access_ip 维度：即使用户显式配置 fields，也强制补齐。
    );
    push_stage_reqs(
        &mut requs,
        StatStage::Parse,
        &conf.parse,
        &PARSE_DEFAULT_FIELDS,
        // parse 监控长期依赖 wp_package_name/wp_rule_name 维度：即使用户显式配置 fields，也强制补齐。
    );
    push_stage_reqs(
        &mut requs,
        StatStage::Sink,
        &conf.sink,
        &SINK_DEFAULT_FIELDS,
        // sink 监控长期依赖 wp_sink_group/wp_sink_name 维度：即使用户显式配置 fields，也强制补齐。
    );
    StatRequires::from(requs)
}

#[cfg(test)]
mod state_dir_tests {
    use super::*;

    fn conf_with_state_dir(value: &str) -> EngineConfig {
        let body = format!("version = \"1.0\"\n\n[runtime]\nstate_dir = \"{value}\"\n");
        toml::from_str::<EngineConfig>(&body).expect("parse engine config")
    }

    #[test]
    fn resolve_state_dir_prefers_cli_override() {
        let work = Path::new("/work");
        let conf = conf_with_state_dir("from-conf").conf_absolutize(work);
        let dir = resolve_state_dir(work, &conf, Some("from-cli")).expect("resolve state dir");
        assert_eq!(dir.path(), Path::new("/work/from-cli"));
    }

    #[test]
    fn resolve_state_dir_uses_config_when_override_missing() {
        let work = Path::new("/work");
        let conf = conf_with_state_dir("from-conf").conf_absolutize(work);
        let dir = resolve_state_dir(work, &conf, None).expect("resolve state dir");
        assert_eq!(dir.path(), Path::new("/work/from-conf"));
    }

    #[test]
    fn resolve_state_dir_ignores_blank_override() {
        let work = Path::new("/work");
        let conf = conf_with_state_dir("from-conf").conf_absolutize(work);
        let dir = resolve_state_dir(work, &conf, Some("   ")).expect("resolve state dir");
        assert_eq!(
            dir.path(),
            Path::new("/work/from-conf"),
            "a blank CLI value must not reset the configured state dir"
        );
    }

    #[test]
    fn resolve_state_dir_rejects_work_root_value() {
        let work = Path::new("/work");
        let conf = conf_with_state_dir(".").conf_absolutize(work);
        let err = resolve_state_dir(work, &conf, None).unwrap_err();
        assert!(err.to_string().contains("state_dir"), "{err}");
    }

    #[test]
    fn resolve_state_dir_rejects_invalid_override() {
        let work = Path::new("/work");
        let conf = conf_with_state_dir("state").conf_absolutize(work);
        let err = resolve_state_dir(work, &conf, Some("..")).unwrap_err();
        assert!(err.to_string().contains("state_dir"), "{err}");
    }
}
