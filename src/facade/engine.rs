//! wparse 引擎的 Facade 封装：装配/启动/重载/优雅退出 与 PID 管理。

use futures_lite::StreamExt;
use orion_variate::EnvDict;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Receiver;
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
use crate::facade::runtime_ctrl::{
    RuntimeCommandReq, RuntimeCommandResp, RuntimeCommandResult, RuntimeControlHandle,
    default_runtime_command_bus,
};
use crate::orchestrator::config::loader::WarpConf;
use crate::orchestrator::config::models::{load_warp_engine_confs, stat_reqs_from};
use crate::orchestrator::engine::resource::EngineResource;
use crate::orchestrator::engine::resource::WarpResourceBuilder;
use crate::orchestrator::engine::service::{
    EngineRuntime, start_processing_tasks, start_warp_service,
};
use crate::resources::core::manager::ResManager;
use crate::runtime::actor::constants::ACTOR_CMD_POLL_TIMEOUT_MS;
use crate::runtime::actor::exit_policy::ExitAction;
use crate::runtime::actor::exit_policy::ExitPhase;
use crate::runtime::actor::{self, ExitPolicyKind};
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use crate::sources::SourceConfigParser;
use crate::utils::process::PidRec;
use wp_conf::constants;
use wp_conf::engine::EngineConfig;
use wp_ctrl_api::CommandType;

#[derive(Clone, Copy)]
struct ProcessingLoadOptions {
    emit_user_notices: bool,
    persist_runtime_state: bool,
}

fn semantic_dict_config_path(main_conf: &EngineConfig, work_root: &Path) -> PathBuf {
    let primary = PathBuf::from(main_conf.knowledge_root()).join("semantic_dict.toml");
    if primary.exists() {
        return primary;
    }

    work_root.join("knowledge/semantic_dict.toml")
}

fn knowdb_config_path(main_conf: &EngineConfig) -> PathBuf {
    PathBuf::from(main_conf.knowledge_root()).join("knowdb.toml")
}

/// wparse 应用入口：对外隐藏内部装配细节
pub struct WpApp {
    main_conf: EngineConfig,
    conf_manager: WarpConf,
    stat_reqs: StatRequires,
    run_args: RunArgs,
    control_handle: RuntimeControlHandle,
    cmd_recv: Receiver<RuntimeCommandReq>,
    pid_guard: Option<PidRec>,
    env_dict: EnvDict,
}

impl WpApp {
    /// 从 CLI 参数构建应用上下文：加载主配置、完成运行参数、初始化日志/统计
    pub fn try_from(args: ParseArgs, env_dict: EnvDict) -> Result<Self, wp_error::RunError> {
        //let mut args = args;
        let (conf_manager, mut main_conf) =
            load_warp_engine_confs(resolve_run_work_root(&args.work_root)?.as_str(), &env_dict)?;
        crate::knowledge::ensure_stats_telemetry_bridge_installed();
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
        let (control_handle, cmd_recv) = default_runtime_command_bus();
        Ok(Self {
            main_conf,
            conf_manager,
            stat_reqs,
            run_args,
            control_handle,
            cmd_recv,
            pid_guard: None,
            env_dict,
        })
    }

    pub fn control_handle(&self) -> RuntimeControlHandle {
        self.control_handle.clone()
    }

    pub async fn validate_load_model(&mut self) -> RunResult<()> {
        let _ = load_processing_res(
            &self.main_conf,
            &self.conf_manager,
            self.stat_reqs.clone(),
            &self.env_dict,
            ProcessingLoadOptions {
                emit_user_notices: false,
                persist_runtime_state: false,
            },
        )
        .await?;
        Ok(())
    }

    /// 若启用企业控制面，连接命令总线以支持热重载
    pub async fn start_cmd_if(&mut self) -> RunResult<()> {
        #[cfg(feature = "enterprise-backend")]
        {
            crate::wp_ctrl_enterprise::start(self.control_handle.command_sender())
                .await
                .owe_conf()?;
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
    ) -> RunResult<EngineRuntime> {
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
        let work_root_str = self.conf_manager.work_root_path();
        let work_root = Path::new(work_root_str.as_str());
        let semantic_dict_path = semantic_dict_config_path(&self.main_conf, work_root);
        oml::set_semantic_dict_config_path(Some(semantic_dict_path));
        match oml::check_semantic_dict_config(None) {
            Ok(Some(msg)) => info_ctrl!("semantic dict: {}", msg),
            Ok(None) => {
                info_ctrl!("semantic dict: use builtin (missing or disabled external config)")
            }
            Err(e) => warn_ctrl!("semantic dict config invalid: {}, fallback to builtin", e),
        }

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

    async fn reload_processing_p0(
        &mut self,
        runtime: &mut EngineRuntime,
        request_id: &str,
    ) -> RunResult<bool> {
        info_ctrl!("runtime reload P0 start request_id={}", request_id);
        let eng_res = load_processing_res(
            &self.main_conf,
            &self.conf_manager,
            self.stat_reqs.clone(),
            &self.env_dict,
            ProcessingLoadOptions {
                emit_user_notices: true,
                persist_runtime_state: true,
            },
        )
        .await?;
        let next_monitor_sink = eng_res.infra.as_ref().map(|i| i.moni_agent());
        let processing = start_processing_tasks(
            &self.run_args,
            eng_res,
            runtime.monitor_sender(),
            &self.stat_reqs,
            runtime.next_processing_epoch(),
        )
        .await?;
        let mut pending_processing = Some(processing);
        let deadline = Instant::now() + runtime.reload_timeout();
        let mut force_replaced = false;

        if let Err(err) = runtime.isolate_picker().await {
            if let Some(processing) = pending_processing.take() {
                let _ = processing.shutdown_unused().await;
            }
            return Err(err);
        }
        runtime.disconnect_parse_router();
        if let Err(err) = runtime.wait_processing_drained(deadline).await {
            warn_ctrl!(
                "runtime reload P0 graceful drain timed out or failed request_id={} fallback_to_force_replace error={}",
                request_id,
                err
            );
            if let Err(force_err) = runtime.force_stop_processing().await {
                if let Some(processing) = pending_processing.take() {
                    let _ = processing.shutdown_unused().await;
                }
                return Err(force_err);
            }
            force_replaced = true;
        }

        runtime.install_processing_tasks(
            pending_processing
                .take()
                .expect("pending processing should exist before install"),
        );
        runtime.replace_monitor_sink(next_monitor_sink);
        runtime.resume_picker().await?;
        info_ctrl!(
            "runtime reload P0 done request_id={} force_replaced={}",
            request_id,
            force_replaced
        );
        Ok(force_replaced)
    }

    async fn handle_runtime_command(
        &mut self,
        runtime: &mut EngineRuntime,
        req: RuntimeCommandReq,
    ) -> RunResult<()> {
        let request_id = req.request_id().to_string();
        let command = format!("{:?}", req.command());
        info_ctrl!(
            "runtime command received request_id={} command={}",
            request_id,
            command
        );
        self.control_handle.mark_running(&request_id, req.command());

        let result = match req.command() {
            CommandType::LoadModel => match self.reload_processing_p0(runtime, &request_id).await {
                Ok(false) => RuntimeCommandResult::ReloadDone,
                Ok(true) => RuntimeCommandResult::ReloadDoneWithForceReplace,
                Err(err) => {
                    warn_ctrl!(
                        "runtime reload P0 failed request_id={} error={}",
                        request_id,
                        err
                    );
                    RuntimeCommandResult::ReloadFailed {
                        reason: err.to_string(),
                    }
                }
            },
        };

        let result_code = match &result {
            RuntimeCommandResult::ReloadDone => "reload_done",
            RuntimeCommandResult::ReloadDoneWithForceReplace => "reload_done_with_force_replace",
            RuntimeCommandResult::ReloadFailed { .. } => "reload_failed",
        };
        self.control_handle.finish(&request_id, result.clone());
        info_ctrl!(
            "runtime command finished request_id={} command={} result={}",
            request_id,
            command,
            result_code
        );
        req.respond(RuntimeCommandResp::accepted(request_id, result));
        Ok(())
    }

    /// 运行主循环：处理信号与控制面热重载
    async fn engine_working(&mut self, run_mode: RunMode) -> RunResult<()> {
        let mut signals = actor::signal::stop_signals()?;
        let mut task_admin = self
            .start_service(run_mode.clone(), &self.env_dict.clone())
            .await?;
        let exit_policy = match run_mode {
            RunMode::Batch => ExitPolicyKind::Batch,
            RunMode::Daemon => ExitPolicyKind::Daemon,
        };
        warn_ctrl!("engine started!");

        if matches!(run_mode, RunMode::Daemon) {
            self.control_handle.activate();
            let mut exit_phase = ExitPhase::Running;
            let mut phase_started_at = Instant::now();
            loop {
                tokio::select! {
                    Some(req) = self.cmd_recv.recv() => {
                        if let Err(err) = self.handle_runtime_command(&mut task_admin, req).await {
                            warn_ctrl!("runtime command loop failed: {}", err);
                        }
                    }
                    stop = async {
                        if let Ok(Some(_)) = timeout(Duration::from_millis(100), signals.next()).await {
                            info_ctrl!("recv signal, stop all routine!");
                            return true;
                        }
                        false
                    } => {
                        let action = task_admin
                            .task_manager_mut()
                            .next_exit_policy_action(exit_policy, exit_phase, phase_started_at, stop)?;
                        if !matches!(action, ExitAction::Stay) {
                            self.control_handle.deactivate();
                        }
                        if task_admin
                            .task_manager_mut()
                            .apply_exit_policy_action(exit_policy, &mut exit_phase, &mut phase_started_at, action, None)
                            .await?
                        {
                            break;
                        }
                    }
                    else => {continue;}
                }
            }
        } else {
            self.control_handle.deactivate();
            let mut exit_phase = ExitPhase::Running;
            let mut phase_started_at = Instant::now();
            loop {
                let stop = if let Ok(Some(_)) = timeout(
                    Duration::from_millis(ACTOR_CMD_POLL_TIMEOUT_MS),
                    signals.next(),
                )
                .await
                {
                    info_ctrl!("recv signal, stop all routine!");
                    true
                } else {
                    false
                };
                let action = task_admin.task_manager_mut().next_exit_policy_action(
                    exit_policy,
                    exit_phase,
                    phase_started_at,
                    stop,
                )?;
                if matches!(action, ExitAction::EnterQuiescing(_)) {
                    task_admin.disconnect_parse_router();
                }
                if task_admin
                    .task_manager_mut()
                    .apply_exit_policy_action(
                        exit_policy,
                        &mut exit_phase,
                        &mut phase_started_at,
                        action,
                        None,
                    )
                    .await?
                {
                    break;
                }
            }
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
    let knowdb_path = knowdb_config_path(main_conf);
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
                crate::knowledge::log_knowdb_init_error("", &knowdb_path, &err);
            }
        }
    } else {
        crate::knowledge::log_missing_knowdb_config("", &knowdb_path);
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

async fn load_processing_res(
    main_conf: &EngineConfig,
    conf_manager: &WarpConf,
    stat_reqs: StatRequires,
    env_dict: &EnvDict,
    options: ProcessingLoadOptions,
) -> RunResult<EngineResource> {
    let mut ctx = OperationContext::want("load-processing-res").with_auto_log();
    let knowdb_path = knowdb_config_path(main_conf);
    let mut knowdb_handler = None;
    if knowdb_path.exists() {
        let authority_uri = processing_authority_uri(conf_manager, options.persist_runtime_state);
        if options.persist_runtime_state
            && let Some(auth_file) = authority_path_from_uri(&authority_uri)
            && Path::new(auth_file).exists()
        {
            let _ = std::fs::remove_file(auth_file);
        }
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
                    authority_uri.as_str(),
                    env_dict,
                );
                handler.mark_initialized();
                knowdb_handler = Some(handler);
            }
            Err(err) => {
                crate::knowledge::log_knowdb_init_error("", &knowdb_path, &err);
            }
        }
    } else {
        crate::knowledge::log_missing_knowdb_config("", &knowdb_path);
    }

    let infra_sinks = InfraSinkService::default_ins(
        main_conf.sinks_root(),
        main_conf.rescue_root(),
        stat_reqs.get_requ_items(StatStage::Sink),
        env_dict,
    )
    .await?;

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
        if options.emit_user_notices {
            println!(
                "rule_mapping.dat 生成时未找到任何 WPL 规则，请检查 [models].wpl 或 --wpl-dir 配置 (当前: {})",
                main_conf.rule_root()
            );
        }
    }
    if res_center.name_mdl_res().is_empty() {
        warn_ctrl!(
            "rule_mapping.dat 生成时未加载任何 OML 模型，请检查 [models].oml 配置 (当前: {})",
            main_conf.oml_root()
        );
        if options.emit_user_notices {
            println!(
                "rule_mapping.dat 生成时未加载任何 OML 模型，请检查 [models].oml 配置 (当前: {})",
                main_conf.oml_root()
            );
        }
    }

    if options.persist_runtime_state {
        let res_path = conf_manager.runtime_path("rule_mapping.dat");
        if Path::new(&res_path).exists() {
            std::fs::remove_file(&res_path).owe_res()?;
        }
        wp_conf::utils::save_data(Some(res_center.to_string()), res_path.as_str(), true)
            .owe_res()?;
    }

    let builder = WarpResourceBuilder::new()
        .with_infra(infra_sinks)
        .with_resource_manager(res_center)
        .with_sink_coordinator(sink_service)
        .with_knowdb_handler(knowdb_handler);
    ctx.mark_suc();
    Ok(builder.build_unchecked())
}

fn processing_authority_uri(conf_manager: &WarpConf, persist_runtime_state: bool) -> String {
    if persist_runtime_state {
        return format!(
            "file:{}?mode=rwc&uri=true",
            conf_manager.runtime_path("authority.sqlite")
        );
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "file:wp-validate-{}-{}?mode=memory&cache=shared",
        std::process::id(),
        now.as_nanos()
    )
}

fn authority_path_from_uri(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("file:")?;
    let (path, query) = rest.split_once('?')?;
    if query.contains("mode=memory") {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::ValueType;
    use wp_conf::test_support::ForTest;
    use wp_proj::project::{WarpProject, init::PrjScope};

    #[tokio::test]
    async fn load_processing_res_without_persist_does_not_write_runtime_state() {
        crate::connectors::startup::init_runtime_registries();
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let mut dict = EnvDict::test_default();
        dict.insert("SEC_PWD", ValueType::from("test-password"));
        WarpProject::init(temp_dir.path(), PrjScope::Full, &dict).expect("init project");

        let conf_manager = WarpConf::new(temp_dir.path());
        let main_conf = conf_manager
            .load_engine_config(&dict)
            .expect("load engine config");
        let stat_reqs = stat_reqs_from(main_conf.stat_conf());

        let _ = load_processing_res(
            &main_conf,
            &conf_manager,
            stat_reqs,
            &dict,
            ProcessingLoadOptions {
                emit_user_notices: false,
                persist_runtime_state: false,
            },
        )
        .await
        .expect("validate processing load");

        assert!(
            !temp_dir.path().join(".run").exists(),
            "validation path should not write work_root runtime state"
        );
    }

    #[test]
    fn validation_authority_uses_in_memory_sqlite_uri() {
        let conf_manager = WarpConf::new("/tmp/wp-engine-validate");
        let uri = processing_authority_uri(&conf_manager, false);
        assert!(
            uri.contains("mode=memory"),
            "validation authority must use in-memory sqlite"
        );
        assert!(
            authority_path_from_uri(&uri).is_none(),
            "in-memory authority uri should not resolve to a filesystem path"
        );
    }
}
