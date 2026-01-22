use std::fs::remove_file;
use std::path::Path;

use crate::facade::config::{load_warp_engine_confs, stat_reqs_from};
use crate::resources::ResManager;
use crate::runtime::sink::infrastructure::InfraSinkService;
use orion_error::ErrorOwe;
use orion_error::UvsLogicFrom;
use orion_variate::EnvDict;
use wp_conf::utils::save_data;
use wp_error::RunReason;
use wp_error::run_error::RunResult;
use wp_stat::StatStage;

#[tokio::test(flavor = "multi_thread")]
#[cfg(test)]
async fn test_res() -> RunResult<()> {
    // Ensure built-in sinks are registered for Factory path (file/null/test_rescue)

    use wp_conf::test_support::ForTest;
    crate::sinks::register_builtin_sinks();

    let (conf_manager, main_conf) =
        load_warp_engine_confs("./tests/instance", &EnvDict::test_default())?;

    let dict = EnvDict::new();
    let _data_src = conf_manager.load_source_config(&dict)?;
    let stat_reqs = stat_reqs_from(main_conf.stat_conf());

    let env_dict = EnvDict::new();
    let infra_sinks = InfraSinkService::default_ins(
        main_conf.sinks_root(),
        main_conf.rescue_root(),
        stat_reqs.get_requ_items(StatStage::Sink),
        &env_dict,
    )
    .await?;
    let mut res_center = ResManager::default();
    res_center
        .load_all_wpl_code(&main_conf, infra_sinks.agent().error())
        .await?;
    res_center.load_all_ldm(main_conf.oml_root()).await?;

    res_center
        .load_all_sink(main_conf.sinks_root(), &env_dict)
        .owe_conf()?;

    let res_path = "res.dat";
    if Path::new(res_path).exists() {
        remove_file(res_path).owe_res()?;
    }
    save_data(Some(res_center.to_string()), res_path, true).owe_res()?;
    println!(
        "{:?}",
        res_center
            .wpl_index()
            .as_ref()
            .ok_or(RunReason::from_logic("not init  wpl index".to_string()))?
    );

    //rule_mdl_relation
    println!("{}", res_center.rule_mdl_relation());
    println!("{}", res_center.sink_mdl_relation());
    println!("sink {}, ", res_center.rule_sink_db());
    assert_eq!(
        res_center
            .wpl_index()
            .as_ref()
            .ok_or(RunReason::from_logic("not init  wpl index"))?
            .rule_key()
            .len(),
        3
    );
    assert_eq!(res_center.name_mdl_res().len(), 3);

    Ok(())
}
