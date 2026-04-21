use orion_conf::ErrorWith;
use orion_error::{ErrorWrapAs, UvsFrom, WrapStructError};
use orion_variate::EnvDict;
use std::path::Path;
use wp_error::run_error::{RunReason, RunResult};

/// 清理 sinks 输出数据
pub fn clean_outputs(work_root: &str, dict: &EnvDict) -> RunResult<bool> {
    let conf_manager = wp_engine::facade::config::WarpConf::new(work_root);
    let main_path = conf_manager.config_path_string(wp_engine::facade::config::ENGINE_CONF_FILE);

    // 只有当配置文件存在时才进行 sinks 清理
    if !Path::new(&main_path).exists() {
        return Ok(false);
    }

    let (_, main_conf) = wp_engine::facade::config::load_warp_engine_confs(work_root, dict)
        .wrap_as(
            RunReason::from_conf(),
            "load engine config for sink clean failed",
        )
        .with(work_root)
        .doing("load engine config for sink clean")?;

    let sink_root = Path::new(&conf_manager.work_root_path()).join(main_conf.sink_root());

    // 使用现有的 sinks 清理功能
    wp_cli_core::data::clean::clean_outputs(&sink_root, dict)
        .map(|_| {
            println!("✓ Cleaned sink outputs from {}", main_conf.sink_root());
            true
        })
        .map_err(|e| e.wrap(RunReason::from_conf()))
        .with(&sink_root)
        .doing("清理 sinks 输出失败")
}
