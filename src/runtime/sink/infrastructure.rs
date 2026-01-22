use crate::facade::test_helpers::SinkTerminal;
use crate::orchestrator::InfraSinkConf;
use crate::orchestrator::config::build_sinks::infra_sink_group;
use crate::runtime::actor::diagnostic::DfxSink;
use crate::sinks::InfraSinkAgent;
use crate::sinks::SinkDispatcher;
use crate::sinks::SinkGroupAgent;
use derive_getters::Getters;
use orion_error::ErrorConv;
use orion_variate::EnvDict;
use orion_variate::EnvEvaluable;
use wp_conf::structure::SinkGroupConf;
use wp_error::run_error::RunResult;
use wp_stat::StatReq;

#[derive(Getters)]
pub struct InfraSinkService {
    pub(crate) default_sink: SinkDispatcher,
    pub(crate) miss_sink: SinkDispatcher,
    pub(crate) residue_sink: SinkDispatcher,
    pub(crate) moni_sink: SinkDispatcher,
    //记录运行规则错误SINK
    pub(crate) err_sink: SinkDispatcher,
}

impl InfraSinkService {
    pub fn new(
        default: SinkDispatcher,
        miss: SinkDispatcher,
        moni: SinkDispatcher,
        residue: SinkDispatcher,
        err_sink: SinkDispatcher,
    ) -> Self {
        Self {
            default_sink: default,
            miss_sink: miss,
            moni_sink: moni,
            residue_sink: residue,
            err_sink,
        }
    }
    pub fn moni_agent(&self) -> SinkTerminal {
        SinkTerminal::Channel(self.moni_sink.get_data_sender())
    }
    pub fn err_agent(&self) -> SinkTerminal {
        SinkTerminal::Channel(self.err_sink.get_data_sender())
    }
    pub fn agent(&self) -> InfraSinkAgent {
        InfraSinkAgent {
            default: SinkGroupAgent::new(
                self.default_sink.conf().clone(),
                SinkTerminal::Channel(self.default_sink.get_data_sender()),
            ),
            miss: SinkGroupAgent::new(
                self.miss_sink.conf().clone(),
                SinkTerminal::Channel(self.miss_sink.get_data_sender()),
            ),
            residue: SinkGroupAgent::new(
                self.residue_sink.conf().clone(),
                SinkTerminal::Channel(self.residue_sink.get_data_sender()),
            ),
            moni: SinkGroupAgent::new(
                self.moni_sink.conf().clone(),
                SinkTerminal::Channel(self.moni_sink.get_data_sender()),
            ),
            error: SinkGroupAgent::new(
                self.err_sink.conf().clone(),
                SinkTerminal::Channel(self.err_sink.get_data_sender()),
            ),
        }
    }
    pub fn dfx_agent(&self) -> DfxSink {
        DfxSink::new(
            SinkGroupAgent::new(
                self.moni_sink.conf().clone(),
                SinkTerminal::Channel(self.moni_sink.get_data_sender()),
            ),
            SinkGroupAgent::new(
                self.err_sink.conf().clone(),
                SinkTerminal::Channel(self.err_sink.get_data_sender()),
            ),
        )
    }
    pub async fn default_ins(
        sink_root: &str,
        rescue: &str,
        stat_reqs: Vec<StatReq>,
        dict: &EnvDict,
    ) -> RunResult<Self> {
        let table_conf = InfraSinkConf::load(sink_root, dict)
            .err_conv()?
            .env_eval(dict);
        //.want("load sink_root")?;
        let default_group = infra_sink_group(
            rescue.to_string(),
            &SinkGroupConf::Fixed(table_conf.default),
            stat_reqs.clone(),
        )
        .await?;
        let residue_group = infra_sink_group(
            rescue.to_string(),
            &SinkGroupConf::Fixed(table_conf.residue),
            stat_reqs.clone(),
        )
        .await?;
        // 拆除 intercept 运行逻辑：不再构建/启动拦截组，保持一个空组占位（避免广泛结构变更）
        // intercept 组彻底移除：不再存在占位
        let miss_group = infra_sink_group(
            rescue.to_string(),
            &SinkGroupConf::Fixed(table_conf.miss),
            stat_reqs.clone(),
        )
        .await?;
        let error_group = infra_sink_group(
            rescue.to_string(),
            &SinkGroupConf::Fixed(table_conf.error),
            stat_reqs.clone(),
        )
        .await?;
        let moni_group = infra_sink_group(
            rescue.to_string(),
            &SinkGroupConf::Flexi(table_conf.monitor),
            stat_reqs.clone(),
        )
        .await?;
        Ok(InfraSinkService::new(
            default_group,
            miss_group,
            moni_group,
            residue_group,
            error_group,
        ))
    }
}
