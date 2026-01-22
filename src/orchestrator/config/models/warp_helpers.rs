use crate::{
    core::parser::wpl_engine, facade::diagnostics::print_run_error,
    orchestrator::config::loader::WarpConf,
};
use anyhow::Result;
use orion_error::{ErrorConv, ToStructError, UvsConfFrom};
use orion_variate::{EnvDict, EnvEvaluable};
use std::{env, path::PathBuf};
use wp_conf::engine::EngineConfig;
use wp_conf::stat::StatConf;
use wp_error::{RunReason, run_error::RunResult};
use wp_knowledge::facade;
use wp_log::conf::LogConf;
use wp_stat::{StatReq, StatRequires, StatStage, StatTarget};

/// Load configuration using a supplied EnvDict for templating overrides
pub fn load_warp_engine_confs(
    work_root: &str,
    dict: &EnvDict,
) -> RunResult<(WarpConf, EngineConfig)> {
    let conf_manager = WarpConf::new(work_root);
    let abs_root = conf_manager.work_root().to_path_buf();
    if let Err(err) = env::set_current_dir(&abs_root) {
        error_ctrl!("设置工作目录失败:, error={}", &err);
        panic!("设置工作目录失败");
    };
    let main_conf = EngineConfig::load(&abs_root, dict)
        .err_conv()?
        .env_eval(dict)
        .conf_absolutize(&abs_root);
    Ok((conf_manager, main_conf))
}

const TOP_N: usize = 20;
pub fn stat_reqs_from(conf: &StatConf) -> StatRequires {
    // 将新结构 [[stat.<stage>]] 映射为运行期 StatReq
    fn map_target(t: &str) -> StatTarget {
        match t.trim() {
            "*" => StatTarget::All,
            "ignore" => StatTarget::Ignore,
            other => StatTarget::Item(other.to_string()),
        }
    }

    let mut requs = Vec::new();
    for it in conf.pick.clone() {
        requs.push(StatReq {
            stage: StatStage::Pick,
            name: it.key,
            target: map_target(it.target.as_str()),
            collect: it.fields,
            max: it.top_n.unwrap_or(TOP_N),
        });
    }
    for it in conf.parse.clone() {
        requs.push(StatReq {
            stage: StatStage::Parse,
            name: it.key,
            target: map_target(it.target.as_str()),
            collect: it.fields,
            max: it.top_n.unwrap_or(TOP_N),
        });
    }
    for it in conf.sink.clone() {
        requs.push(StatReq {
            stage: StatStage::Sink,
            name: it.key,
            target: map_target(it.target.as_str()),
            collect: it.fields,
            max: it.top_n.unwrap_or(TOP_N),
        });
    }
    StatRequires::from(requs)
}
