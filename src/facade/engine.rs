//! wparse 引擎的 Facade 封装：装配/启动/重载/优雅退出 与 PID 管理。

use futures_lite::StreamExt;
use orion_variate::EnvDict;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::timeout;
use wp_knowledge::facade::init_thread_cloned_from_knowdb;

use orion_error::{ErrorConv, ErrorOwe, ErrorWith, OperationContext};
use wp_conf::{RunArgs, RunMode};
use wp_error::run_error::RunResult;
use wp_log::conf::log_init;
// bring logging macros into scope (Rust 2018+ requires explicit import for macro_rules! macros)
use wp_log::{info_ctrl, warn_ctrl};
use wp_stat::{StatRequires, StatStage};

use crate::facade::args::ParseArgs;
use crate::facade::args::resolve_run_work_root;
use crate::orchestrator::config::loader::WarpConf;
use crate::orchestrator::config::models::{load_warp_engine_confs, stat_reqs_from};
use crate::orchestrator::engine::resource::EngineResource;
use crate::orchestrator::engine::resource::WarpResourceBuilder;
use crate::orchestrator::engine::service::start_warp_service;
use crate::resources::core::manager::ResManager;
use crate::runtime::actor::{self, TaskManager};
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::sources::SourceConfigParser;
use crate::utils::process::PidRec;
use wp_conf::constants;
use wp_conf::engine::EngineConfig;
use wp_ctrl_api::CommandType;

/// wparse 应用入口：对外隐藏内部装配细节
pub struct WpApp {
    main_conf: EngineConfig,
    conf_manager: WarpConf,
    stat_reqs: StatRequires,
    run_args: RunArgs,
    #[allow(dead_code)]
    cmd_send: Sender<CommandType>,
    #[allow(dead_code)]
    cmd_recv: Receiver<CommandType>,
    pid_guard: Option<PidRec>,
    bus_enabled: bool,
    env_dict: EnvDict,
}

impl WpApp {
    /// 从 CLI 参数构建应用上下文：加载主配置、完成运行参数、初始化日志/统计
    pub fn try_from(args: ParseArgs, env_dict: EnvDict) -> Result<Self, wp_error::RunError> {
        //let mut args = args;
        let (conf_manager, mut main_conf) =
            load_warp_engine_confs(resolve_run_work_root(&args.work_root)?.as_str(), &env_dict)?;
        // CLI 覆盖：当提供 --wpl-dir 时，优先于 wparse.toml 的 [models].wpl
        if let Some(dir) = &args.wpl_dir {
            main_conf.set_rule_root(dir.clone());
        }
        let run_args = args.completion_from(&main_conf)?;
        let stat_reqs = stat_reqs_from(main_conf.stat_conf());
        log_init(main_conf.log_conf()).err_conv()?;
        info_ctrl!("log conf: {} ", main_conf.log_conf());
        // 初始化引擎侧注册表：注册内置工厂 + 导入 API 已注册工厂 + 打印注册清单
        crate::connectors::startup::init_runtime_registries();
        let (cmd_send, cmd_recv) = tokio::sync::mpsc::channel::<CommandType>(1000);
        Ok(Self {
            main_conf,
            conf_manager,
            stat_reqs,
            run_args,
            cmd_send,
            cmd_recv,
            pid_guard: None,
            bus_enabled: false,
            env_dict,
        })
    }

    /// 若启用企业控制面，连接命令总线以支持热重载
    pub async fn start_cmd_if(&mut self) -> RunResult<()> {
        #[cfg(feature = "enterprise-backend")]
        {
            let enabled = crate::wp_ctrl_enterprise::start(self.cmd_send.clone())
                .await
                .owe_conf()?;
            self.bus_enabled = enabled;
            Ok(())
        }
        #[cfg(not(feature = "enterprise-backend"))]
        {
            info_ctrl!(
                "enterprise control plane disabled (feature 'enterprise-backend' not enabled)"
            );
            Ok(())
        }
    }

    /// 启动服务并返回任务管理器
    pub async fn start_service(
        &mut self,
        run_mode: RunMode,
        env_dict: &EnvDict,
    ) -> RunResult<TaskManager> {
        // 保持 PID 文件在进程生命周期内存在
        self.pid_guard = Some(PidRec::current(
            self.conf_manager.runtime_path("wparse.pid").as_str(),
        )?);
        info_ctrl!(
            "build engine with run_mode={}, parallel={}, line_max={:?}",
            run_mode,
            self.run_args.parallel,
            self.run_args.line_max
        );

        let eng_res = load_engine_res(
            &self.main_conf,
            &self.conf_manager,
            self.stat_reqs.clone(),
            run_mode.clone(),
            env_dict,
        )
        .await?;

        let task_manager = start_warp_service(
            eng_res,
            run_mode,
            self.run_args.clone(),
            self.stat_reqs.clone(),
        )
        .await?;
        Ok(task_manager)
    }

    /// 运行主循环：处理信号与控制面热重载
    async fn engine_working(&mut self, run_mode: RunMode) -> RunResult<()> {
        let mut signals = actor::signal::stop_signals()?;

        let mut task_admin = self
            .start_service(run_mode.clone(), &self.env_dict.clone())
            .await?;
        warn_ctrl!("engine started!");

        if self.bus_enabled {
            loop {
                tokio::select! {
                    /*
                    Some(_) = self.cmd_recv.recv() => {
                        info_ctrl!("wparse engine reloading...");
                        task_admin.all_down_wait_signal_ex().await?;
                        self.start_service(run_mode.clone()).await?;
                        info_ctrl!("wparse engine reload done!");
                    }
                    */
                    stop = async {
                        if let Ok(Some(_)) = timeout(Duration::from_millis(100), signals.next()).await {
                            info_ctrl!("recv signal, stop all routine!");
                            return true;
                        }
                        false
                    } => {
                        if stop {
                            task_admin.all_down_wait_signal_ex().await?;
                            break;
                        }
                    }
                    else => {continue;}
                }
            }
        } else {
            task_admin.all_down_wait_signal().await?;
        }
        Ok(())
    }

    /// 以 daemon 模式运行
    pub async fn run_daemon(&mut self) -> RunResult<()> {
        self.start_cmd_if().await?;
        self.engine_working(RunMode::Daemon).await
    }

    /// 以 batch 模式运行
    pub async fn run_batch(&mut self) -> RunResult<()> {
        self.start_cmd_if().await?;
        self.engine_working(RunMode::Batch).await
    }
}

/// 引擎资源装配（从 apps/wparse/work/loader 迁移并私有化）
async fn load_engine_res(
    main_conf: &EngineConfig,
    conf_manager: &WarpConf,
    stat_reqs: StatRequires,
    run_mode: RunMode,
    env_dict: &EnvDict,
) -> RunResult<EngineResource> {
    let mut ctx = OperationContext::want("load-engine-res").with_auto_log();
    let knowdb_path =
        Path::new(conf_manager.work_root_path().as_str()).join("models/knowledge/knowdb.toml");
    let mut knowdb_handler = None;
    if knowdb_path.exists() {
        let auth_file = PathBuf::from(conf_manager.runtime_path("authority.sqlite"));
        let _ = std::fs::remove_file(&auth_file);
        let authority_uri = format!("file:{}?mode=rwc&uri=true", auth_file.display());
        match init_thread_cloned_from_knowdb(
            Path::new(conf_manager.work_root_path().as_str()),
            &knowdb_path,
            &authority_uri,
            env_dict,
        ) {
            Ok(_) => {
                let handler = crate::knowledge::KnowdbHandler::new(
                    Path::new(conf_manager.work_root_path().as_str()),
                    &knowdb_path,
                    &authority_uri,
                    env_dict,
                );
                handler.mark_initialized();
                knowdb_handler = Some(handler);
            }
            Err(err) => {
                warn_ctrl!("init knowdb skipped ({}): {}", knowdb_path.display(), err);
            }
        }
    } else {
        warn_ctrl!(
            "models/knowledge/knowdb.toml not found under {}; skip knowdb init",
            conf_manager.work_root_path()
        );
    }

    let infra_sinks = InfraSinkService::default_ins(
        main_conf.sinks_root(),
        main_conf.rescue_root(),
        stat_reqs.get_requ_items(StatStage::Sink),
        env_dict,
    )
    .await?;

    // 源配置：解析 wpsrc.toml（统一 [[sources]] + connectors）
    let parser = SourceConfigParser::new(PathBuf::from(conf_manager.work_root_path()));
    let wpsrc_path = PathBuf::from(main_conf.src_conf_of(constants::WPSRC_TOML));
    let (_src_keys, source_inits, acceptor_inits) = parser
        .build_source_handles(&wpsrc_path, run_mode, env_dict)
        .await
        .err_conv()
        .want("parse/build sources")?;

    let mut res_center = ResManager::build(main_conf, &infra_sinks, env_dict).await?;
    let sink_service = SinkService::async_sinks_spawn(
        main_conf.rescue_root().to_string(),
        res_center.must_get_sink_table()?,
        &res_center,
        stat_reqs.get_requ_items(StatStage::Sink),
        main_conf.speed_limit(),
    )
    .await?;

    res_center.ins_engine_res(
        sink_service.agent(),
        stat_reqs.get_requ_items(StatStage::Parse),
    )?;

    // 输出 rule_mapping.dat 至工作目录 .run/rule_mapping.dat
    let res_path = conf_manager.runtime_path("rule_mapping.dat");
    if Path::new(&res_path).exists() {
        std::fs::remove_file(&res_path).owe_res()?;
    }

    // 若未加载到任何 WPL/OML 资源，提前给出提醒，便于定位空映射问题
    let wpl_rule_cnt = res_center
        .wpl_index()
        .as_ref()
        .map(|idx| idx.rule_key().len())
        .unwrap_or(0);
    if wpl_rule_cnt == 0 {
        warn_ctrl!(
            "rule_mapping.dat 生成时未找到任何 WPL 规则，请检查 [models].wpl 或 --wpl-dir 配置 (当前: {})",
            main_conf.rule_root()
        );
        println!(
            "rule_mapping.dat 生成时未找到任何 WPL 规则，请检查 [models].wpl 或 --wpl-dir 配置 (当前: {})",
            main_conf.rule_root()
        );
    }
    if res_center.name_mdl_res().is_empty() {
        warn_ctrl!(
            "rule_mapping.dat 生成时未加载任何 OML 模型，请检查 [models].oml 配置 (当前: {})",
            main_conf.oml_root()
        );
        println!(
            "rule_mapping.dat 生成时未加载任何 OML 模型，请检查 [models].oml 配置 (当前: {})",
            main_conf.oml_root()
        );
    }

    wp_conf::utils::save_data(Some(res_center.to_string()), res_path.as_str(), true).owe_res()?;

    let builder = WarpResourceBuilder::new()
        .with_infra(infra_sinks)
        .with_resource_manager(res_center)
        .with_sink_coordinator(sink_service)
        .with_acceptors(acceptor_inits)
        .with_sources(source_inits)
        .with_knowdb_handler(knowdb_handler);
    ctx.mark_suc();
    Ok(builder.build_unchecked())
}
