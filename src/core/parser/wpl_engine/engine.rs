//! WplWorkshop 实现

use crate::core::parser::wpl_engine::parser::MultiParser;
use crate::core::parser::{ParseOption, WplPipeline, WplRepository};
use crate::stat::MonSend;
use crate::types::AnyResult;
use getset::Getters;
use wp_connector_api::SourceEvent;
use wpl::WparseResult;

use crate::core::parser::indexing::ResourceIndexer;
use crate::resources::sinks::null_resources::AssignRes;
use crate::resources::utils::code_ins_parse_units;
use crate::sinks::{InfraSinkAgent, SinkGroupAgent, SinkInfraAble};

#[derive(Getters)]
pub struct WplEngine {
    pub pipelines: MultiParser,
    pub infra_agent: InfraSinkAgent,
}

impl WplEngine {
    pub(crate) fn from(pipe_lines: Vec<WplPipeline>, infra: InfraSinkAgent) -> AnyResult<Self> {
        // 规则命中排序
        //let mut pipelines = pipe_lines;
        //pipelines.sort_by(|a, b| b.hit_cnt.cmp(&a.hit_cnt));

        Ok(Self {
            pipelines: MultiParser::new(pipe_lines),
            infra_agent: infra,
        })
    }

    pub fn from_code(wpl_spc: &WplRepository, infra: InfraSinkAgent) -> AnyResult<Self> {
        let mut idx = ResourceIndexer::default();
        let mut pipelines = Vec::new();
        for pkg in &wpl_spc.packages {
            let mut units =
                code_ins_parse_units(AssignRes::use_it(infra.default().clone()), pkg, &mut idx)?;
            pipelines.append(&mut units);
        }
        Self::from(pipelines, infra)
    }
}

impl SinkInfraAble for WplEngine {
    fn miss(&self) -> SinkGroupAgent {
        self.infra_agent.miss.clone()
    }

    fn error(&self) -> SinkGroupAgent {
        self.infra_agent.error.clone()
    }

    fn residue(&self) -> SinkGroupAgent {
        self.infra_agent.residue.clone()
    }
}

impl WplEngine {
    /// 批量处理多个数据包，提高性能
    /// 通过批量聚合数据到相同 sink 来减少网络/IO 开销
    pub async fn proc_batch(
        &mut self,
        batch: Vec<SourceEvent>,
        setting: &ParseOption,
    ) -> WparseResult<()> {
        if batch.is_empty() {
            return Ok(());
        }
        // 第一阶段：解析并分组数据
        let parsed_data = self.batch_parse_package(batch, setting)?;

        // 第二阶段：发送数据（跳过逻辑在 DataSender 内部细分：仅屏蔽业务 sinks，保留 infra 通道）
        self.send_batched_data(parsed_data).await
    }

    /// 处理单个事件（向后兼容接口）
    pub async fn proc_async(&self, event: SourceEvent, setting: &ParseOption) -> WparseResult<()> {
        // 将单个事件包装成批次处理
        let batch = vec![event];
        let mut workshop = WplEngine {
            pipelines: self.pipelines.clone(),
            infra_agent: self.infra_agent.clone(),
        };
        workshop.proc_batch(batch, setting).await
    }
}

impl WplEngine {
    /// 发送统计数据
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        self.pipelines.send_stat(mon_send).await
    }

    pub fn optimized(&mut self, _count: usize) {
        self.pipelines.optimized(_count)
    }
}
