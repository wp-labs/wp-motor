use orion_error::ContextRecord;
use orion_error::ErrorOwe;
use orion_error::OperationContext;
use wp_error::DistFocus;

use crate::resources::SinkResUnit;
use wp_conf::sinks::core_to_resolved;
// file/test_rescue/null now use built-in factories; file helpers kept for fallback logic only
use crate::sinks::SinkDispatcher;
use crate::sinks::SinkRuntime;
// test proxy wrapped via builtin factory when kind == test_rescue; not used here anymore
use crate::sinks::FileSink;
use crate::sinks::FormatAdapter;
use crate::sinks::SinkBackendType;
use crate::types::AnyResult;
use orion_overload::append::Appendable;
use wp_conf::limits::parser_channel_cap;
use wp_conf::structure::FileSinkConf;
use wp_conf::structure::SinkInstanceConf;
use wp_conf::structure::{FlexGroup, SinkGroupConf};
use wp_connector_api::SinkBuildCtx;
use wp_error::run_error::{RunReason, RunResult};
use wp_model_core::model::fmt_def::TextFmt;
use wp_stat::StatReq;

// Shared capacity for data channels. Keep small to avoid peak memory blow-up.
// Parser input uses this via runtime/tasks/parse.rs; sink dispatcher also uses it.
/// 获取解析通道容量（转发自 limits::parser_channel_cap）
pub fn dat_channel_max() -> usize {
    parser_channel_cap()
}
pub const CMD_CHANNEL_MAX: usize = 10000;

// Note: legacy sync entry removed; runtime uses async sinks

pub fn fmt_file(out_file: &FileSinkConf, fmt: TextFmt) -> AnyResult<FormatAdapter<FileSink>> {
    let out_path = out_file.path.clone();
    let mut pipe: FormatAdapter<FileSink> = FormatAdapter::new(fmt);
    pipe.next_pipe(FileSink::new(&out_path)?);
    Ok(pipe)
}

#[allow(dead_code)]
pub(crate) fn fmt_file_by_path(out_file: &str, fmt: TextFmt) -> AnyResult<FormatAdapter<FileSink>> {
    let mut pipe: FormatAdapter<FileSink> = FormatAdapter::new(fmt);
    pipe.next_pipe(FileSink::new(out_file)?);
    Ok(pipe)
}

pub async fn build_sink_target(
    s_conf: &SinkInstanceConf,
    replica_idx: usize,
    replica_cnt: usize,
    rate_limit_rps: usize,
) -> RunResult<SinkBackendType> {
    let mut op = OperationContext::want("build-sink-instance").with_auto_log();
    // External sink builders must be registered by the application before calling this
    // function (apps/wparse or apps/wpsink). Keeping registration out of core avoids
    // feature-coupling core with optional extension crates.
    let kind = s_conf.resolved_kind_str();
    op.record("sink_name", s_conf.name().as_str());
    op.record("sink_kind", kind.as_str());
    // Use CoreSinkSpec (from wp-specs) as a unified bridge to plugin-facing specs
    let core: wp_specs::CoreSinkSpec = s_conf.into();
    let ctx =
        SinkBuildCtx::new_with_replica(std::env::current_dir().unwrap(), replica_idx, replica_cnt)
            .with_limit(rate_limit_rps);
    if let Some(factory) = crate::connectors::registry::get_sink_factory(&kind) {
        // Factory path: use flattened params directly
        op.debug("load factory suc!");
        let spec: wp_connector_api::SinkSpec = core_to_resolved(&core);
        let init = factory
            .build(&spec, &ctx)
            .await
            .owe(RunReason::Dist(DistFocus::SinkError(kind)))?;
        op.mark_suc();
        Ok(SinkBackendType::Proxy(init.sink))
    } else {
        Err(wp_error::run_error::RunError::from(RunReason::Dist(
            DistFocus::SinkError(format!("sink factory not found for kind '{}'", kind)),
        )))
    }
}

#[derive(Default, Clone)]
pub struct SinkRouteTable {
    pub group: Vec<FlexGroup>,
}

impl SinkRouteTable {
    pub fn len(&self) -> usize {
        self.group.len()
    }
    pub fn is_empty(&self) -> bool {
        self.group.is_empty()
    }

    pub fn group_name_vec(&self) -> Vec<String> {
        self.group.iter().map(|x| x.name().to_string()).collect()
    }

    pub(crate) fn add_route(&mut self, conf: FlexGroup) {
        self.group.push(conf);
    }
}

// Legacy helpers kept in orchestrator/config namespace
pub(crate) async fn infra_sink_group(
    rescue: String,
    conf: &SinkGroupConf,
    stat_reqs: Vec<StatReq>,
) -> RunResult<SinkDispatcher> {
    let mut cxt = OperationContext::want("assmeble sink group").with_auto_log();
    cxt.record("gourp_name", conf.name().as_str());
    let mut group = SinkDispatcher::new(conf.clone(), SinkResUnit::use_null());

    // 基础组强制单副本：并行在 infra 无效，且禁用文件副本分片逻辑
    // 注意：配置层已对 parallel 报错（crates/wp-config/.../load_infra_route_confs），
    // 这里再次兜底，避免后续变更遗漏导致误用。
    let (p_cnt, sinks) = match conf {
        SinkGroupConf::Fixed(f) => (1, f.sinks()),
        SinkGroupConf::Flexi(f) => (1, f.sinks()),
    };
    for rep in 0..p_cnt {
        for sc in sinks.iter() {
            let sink = build_sink_target(sc, rep, p_cnt, 0).await?;
            // 与业务组保持一致：使用 group/name 作为运行期名称，便于日志与统计
            let full_name = sc.full_name();
            group.append(SinkRuntime::new(
                rescue.clone(),
                full_name,
                sc.clone(),
                sink,
                None,
                stat_reqs.clone(),
            ));
        }
    }
    cxt.mark_suc();
    Ok(group)
}
