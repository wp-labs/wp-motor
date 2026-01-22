//! wprescue 的 Facade 封装：仅 batch 模式，负责救援流程的装配与运行。

use std::path::Path;
use std::sync::Arc;

use orion_error::ErrorConv;
use orion_variate::EnvDict;
use wp_conf::RunArgs;
use wp_error::run_error::RunResult;
use wp_log::conf::log_init;
use wp_stat::{StatRequires, StatStage};

use crate::facade::args::{ParseArgs, resolve_run_work_root};
use crate::orchestrator::config::loader::WarpConf;
use crate::orchestrator::config::models::{load_warp_engine_confs, stat_reqs_from};
use crate::orchestrator::engine::recovery::recover_main;
use crate::resources::core::manager::ResManager;
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::utils::process::PidRec;
use wp_conf::engine::EngineConfig;

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
        let run_args = args.completion_from(&main_conf)?;
        let stat_reqs = stat_reqs_from(main_conf.stat_conf());
        log_init(main_conf.log_conf()).err_conv()?;
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
        // 知识库（V2，可选）：检测 ./models/knowledge/knowdb.toml
        let mut knowdb_handler = None;
        let knowdb_path = std::path::Path::new(self.conf_manager.work_root_path().as_str())
            .join("models/knowledge/knowdb.toml");
        if knowdb_path.exists() {
            let auth_file =
                std::path::PathBuf::from(self.conf_manager.runtime_path("authority.sqlite"));
            let _ = std::fs::remove_file(&auth_file);
            let authority_uri = format!("file:{}?mode=rwc&uri=true", auth_file.display());
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
                    warn_ctrl!(
                        "init knowdb skipped in rescue ({}): {}",
                        knowdb_path.display(),
                        err
                    );
                }
            }
        } else {
            warn_ctrl!(
                "rescue mode: models/knowledge/knowdb.toml not found under {}; skip knowdb init",
                self.conf_manager.work_root_path()
            );
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
