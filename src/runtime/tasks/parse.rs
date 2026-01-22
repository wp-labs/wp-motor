use crate::core::parser::{ParseOption, WplPipeline};
use crate::orchestrator::config::build_sinks::dat_channel_max;
use crate::orchestrator::engine::resource::EngineResource;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::parser::act_parser::ActParser;
use crate::runtime::parser::workflow::{ActorWork, ParseWorkerSender};
use crate::sinks::{InfraSinkAgent, SinkRouteAgent};
use crate::stat::MonSend;
use crate::types::EventBatchRecv;
use crate::types::EventBatchSend;
use std::sync::Arc;
use wp_conf::RunArgs;
use wp_error::run_error::RunResult;
use wp_stat::StatRequires;
use wp_stat::StatStage;

/// 启动解析任务
/// 使用 SourceFrame 通道启动解析任务（prehook 在解析线程执行）
pub async fn start_parser_tasks_frames(
    args: &RunArgs,
    resource: &EngineResource,
    mon_send: MonSend,
    stat_reqs: &StatRequires,
) -> RunResult<(Vec<ParseWorkerSender>, TaskGroup)> {
    let mut parser_group = TaskGroup::new("wpl-parse", ShutdownCmd::Timeout(200));
    let (infra, resc, sinks) = match (&resource.infra, &resource.resc, &resource.sinks) {
        (Some(infra), Some(resc), Some(sinks)) => (infra, resc, sinks),
        _ => {
            info_ctrl!("解析任务启动跳过：缺少 infra/resc/sinks 组件");
            return Ok((Vec::new(), parser_group));
        }
    };
    info_ctrl!("start {} parallel to parse (frames)", args.parallel);
    let mut hold_channel =
        crate::runtime::actor::TaskChannel::<wp_connector_api::SourceBatch>::default();
    let mut sub_channel = Vec::new();
    let parser_factory = Arc::new(ActParserFactory::new(
        Arc::new(resc.get_parse_units().clone()),
        sinks.agent(),
        infra.agent(),
    ));

    for _ in 0..args.parallel {
        let (dat_s, dat_r): (EventBatchSend, EventBatchRecv) =
            hold_channel.channel(dat_channel_max());
        let actuator = parser_factory.build().await?;
        // 使用通用的 ActorWork（定义在 runtime/parser/workflow.rs）
        // 代替在函数内部临时定义的 ActorFrameWork，避免重复与每轮循环重新定义类型。
        let mut worker = ActorWork::new(
            "wparse-parse",
            dat_r,
            parser_group.subscribe(),
            mon_send.clone(),
            actuator,
        );
        let reqs = stat_reqs.get_requ_items(StatStage::Parse);
        let setting = ParseOption::new(true, reqs);
        parser_group.append(tokio::spawn(async move {
            if let Err(e) = worker.proc(setting).await {
                error_ctrl!("parse routine error: {}", e);
            }
        }));
        sub_channel.push(ParseWorkerSender::new(dat_s.clone()));
    }
    Ok((sub_channel, parser_group))
}

struct ActParserFactory {
    pipelines: Arc<Vec<WplPipeline>>,
    sinks: SinkRouteAgent,
    infra: InfraSinkAgent,
}

impl ActParserFactory {
    fn new(pipelines: Arc<Vec<WplPipeline>>, sinks: SinkRouteAgent, infra: InfraSinkAgent) -> Self {
        Self {
            pipelines,
            sinks,
            infra,
        }
    }

    async fn build(&self) -> RunResult<ActParser> {
        // 仍需为每个 worker 生成独立的解析管线，但共享 blueprint/agent 句柄以减少资源构造
        let pipelines = self.pipelines.as_ref().clone();
        ActParser::from_all_model(pipelines, self.sinks.clone(), self.infra.clone()).await
    }
}
