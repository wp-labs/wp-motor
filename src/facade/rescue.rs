//! wprescue 的 Facade 封装：仅 batch 模式，负责救援流程的装配与运行。

use std::path::Path;
use std::sync::Arc;

use orion_error::conversion::ConvErr;
use orion_variate::EnvDict;
use serde::Deserialize;
use wp_conf::RunArgs;
use wp_error::run_error::RunResult;
use wp_log::conf::log_init;
use wp_knowledge::loader::build_authority_from_knowdb;
use wp_stat::{StatRequires, StatStage};

use crate::facade::args::{ParseArgs, resolve_run_work_root};
use oml::core::evaluator::query::sql::set_local_sqlite_priority_route;
use crate::orchestrator::config::loader::WarpConf;
use crate::orchestrator::config::models::{load_warp_engine_confs, stat_reqs_from};
use crate::orchestrator::engine::recovery::recover_main;
use crate::resources::core::manager::ResManager;
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::utils::process::PidRec;
use wp_conf::engine::EngineConfig;

#[derive(Deserialize)]
struct KnowdbProviderProbe {
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct KnowdbProbe {
    #[serde(default)]
    provider: Option<KnowdbProviderProbe>,
    #[serde(default)]
    tables: Vec<KnowdbTableProbe>,
}

#[derive(Deserialize)]
struct KnowdbTableProbe {
    name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

const fn default_true() -> bool {
    true
}

fn should_rebuild_local_authority(knowdb_path: &Path) -> bool {
    let Ok(body) = std::fs::read_to_string(knowdb_path) else {
        return true;
    };
    let Ok(probe) = toml::from_str::<KnowdbProbe>(&body) else {
        return true;
    };
    let Some(provider) = probe.provider else {
        return true;
    };
    !matches!(provider.kind.as_deref(), Some("postgres") | Some("mysql"))
}

fn load_local_table_names(knowdb_path: &Path) -> Vec<String> {
    let Ok(body) = std::fs::read_to_string(knowdb_path) else {
        return Vec::new();
    };
    let Ok(probe) = toml::from_str::<KnowdbProbe>(&body) else {
        return Vec::new();
    };
    probe
        .tables
        .into_iter()
        .filter(|table| table.enabled)
        .map(|table| table.name)
        .collect()
}

/// wprescue 应用入口（batch-only）
pub struct WpRescueApp {
    main_conf: EngineConfig,
    conf_manager: WarpConf,
    stat_reqs: StatRequires,
    run_args: RunArgs,
    pid_guard: Option<PidRec>,
    val_dict: EnvDict,
}

impl WpRescueApp {
    /// 从 CLI 参数构建应用上下文
    pub fn try_from(args: ParseArgs, val_dict: EnvDict) -> Result<Self, wp_error::RunError> {
        let (conf_manager, main_conf) =
            load_warp_engine_confs(resolve_run_work_root(&args.work_root)?.as_str(), &val_dict)?;
        crate::knowledge::ensure_stats_telemetry_bridge_installed();
        let run_args = args.completion_from(&main_conf)?;
        let stat_reqs = stat_reqs_from(main_conf.stat_conf());
        log_init(main_conf.log_conf()).conv_err()?;
        Ok(Self {
            main_conf,
            conf_manager,
            stat_reqs,
            run_args,
            pid_guard: None,
            val_dict,
        })
    }

    /// 执行 batch 模式救援流程
    pub async fn run_batch(&mut self) -> RunResult<()> {
        // 知识库（V2，可选）：检测 [models].knowledge/knowdb.toml
        let mut knowdb_handler = None;
        let knowdb_path =
            std::path::PathBuf::from(self.main_conf.knowledge_root()).join("knowdb.toml");
        if knowdb_path.exists() {
            let local_tables = load_local_table_names(&knowdb_path);
            let auth_file =
                std::path::PathBuf::from(self.conf_manager.runtime_path("authority.sqlite"));
            if should_rebuild_local_authority(&knowdb_path) || !local_tables.is_empty() {
                let _ = std::fs::remove_file(&auth_file);
            }
            let authority_uri = format!("file:{}?mode=rwc&uri=true", auth_file.display());
            if !local_tables.is_empty() {
                let _ = build_authority_from_knowdb(
                    Path::new(self.conf_manager.work_root_path().as_str()),
                    &knowdb_path,
                    &authority_uri,
                    &self.val_dict,
                );
                set_local_sqlite_priority_route(authority_uri.clone(), local_tables);
            }
            match wp_knowledge::facade::init_thread_cloned_from_knowdb(
                Path::new(self.conf_manager.work_root_path().as_str()),
                &knowdb_path,
                &authority_uri,
                &self.val_dict,
            ) {
                Ok(_) => {
                    let handler = crate::knowledge::KnowdbHandler::new(
                        Path::new(self.conf_manager.work_root_path().as_str()),
                        &knowdb_path,
                        &authority_uri,
                        &self.val_dict,
                    );
                    handler.mark_initialized();
                    knowdb_handler = Some(handler);
                    info_ctrl!("init knowdb success({}) ", knowdb_path.display(),);
                }
                Err(err) => {
                    crate::knowledge::log_knowdb_init_error("rescue mode: ", &knowdb_path, &err);
                }
            }
        } else {
            crate::knowledge::log_missing_knowdb_config("rescue mode: ", &knowdb_path);
        }

        // PID
        self.pid_guard = Some(PidRec::current(
            self.conf_manager.runtime_path("wprescue.pid").as_str(),
        )?);

        // Infra 与 sinks 装配
        let infra_sinks = InfraSinkService::default_ins(
            self.main_conf.sinks_root(),
            self.main_conf.rescue_root(),
            self.stat_reqs.get_requ_items(StatStage::Sink),
            &self.val_dict,
        )
        .await?;

        // 读取源配置并构建 ResManager
        //let data_src = self.conf_manager.load_source_config()?;
        let res_center = ResManager::build(&self.main_conf, &infra_sinks, &self.val_dict).await?;
        let sink_service = SinkService::async_sinks_spawn(
            self.main_conf.rescue_root().to_string(),
            res_center.must_get_sink_table()?,
            &res_center,
            self.stat_reqs.get_requ_items(StatStage::Sink),
            self.main_conf.speed_limit(),
        )
        .await?;

        // 进入恢复主循环
        recover_main(
            infra_sinks,
            self.run_args.clone(),
            self.main_conf.rescue_root(),
            sink_service,
            self.stat_reqs.clone(),
            knowdb_handler.map(Arc::new),
        )
        .await?;
        Ok(())
    }
}
