#![allow(dead_code)]
use crate::knowledge::KnowdbHandler;
use crate::resources::ResManager;
use crate::runtime::sink::act_sink::SinkService;
use crate::runtime::sink::infrastructure::InfraSinkService;
use std::sync::Arc;
use wp_connector_api::{AcceptorHandle, DataSource, ServiceAcceptor, SourceHandle};

/// WarpResource 的增强版本，支持新旧架构的桥接
///
/// 这个结构保持向后兼容性，同时支持新的 Box<dyn DataSource> 架构
#[derive(Default)]
pub struct EngineResource {
    pub infra: Option<InfraSinkService>,
    pub resc: Option<ResManager>,
    pub sinks: Option<SinkService>,
    pub sources: Vec<SourceHandle>,
    pub acceptors: Vec<AcceptorHandle>,
    pub knowdb_handler: Option<Arc<KnowdbHandler>>,
}

impl EngineResource {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加新格式的数据源
    pub fn add_source(&mut self, source: SourceHandle) {
        self.sources.push(source);
    }

    /// 批量添加新格式的数据源
    pub fn add_sources(&mut self, sources: Vec<SourceHandle>) {
        self.sources.extend(sources);
    }

    /// 添加接受器集合
    pub fn add_acceptors(&mut self, acceptors: Vec<AcceptorHandle>) {
        self.acceptors.extend(acceptors);
    }

    /// 设置基础设施服务
    pub fn set_infra(&mut self, infra: InfraSinkService) {
        self.infra = Some(infra);
    }

    /// 设置资源管理器
    pub fn set_resource_manager(&mut self, resc: ResManager) {
        self.resc = Some(resc);
    }

    /// 设置 Sink 协调器
    pub fn set_sink_coordinator(&mut self, sinks: SinkService) {
        self.sinks = Some(sinks);
    }

    pub fn set_knowdb_handler(&mut self, handler: Option<KnowdbHandler>) {
        self.knowdb_handler = handler.map(Arc::new);
    }

    /// 获取所有数据源
    pub fn get_all_sources(&mut self) -> Vec<SourceHandle> {
        self.sources.drain(..).collect()
    }

    /// 获取所有接受器（来自源的 acceptors 聚合）
    pub fn get_all_acceptors(&mut self) -> Vec<AcceptorHandle> {
        std::mem::take(&mut self.acceptors)
    }

    /// 检查是否有任何数据源
    pub fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }

    /// 检查是否有任何接受器
    pub fn has_acceptors(&self) -> bool {
        !self.acceptors.is_empty()
    }

    /// 获取数据源的数量
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// 获取接受器的数量
    pub fn acceptor_count(&self) -> usize {
        self.acceptors.len()
    }

    /// 列出所有数据源的标识符
    pub fn list_source_identifiers(&self) -> Vec<String> {
        let mut identifiers = Vec::new();

        // 添加新格式源的标识符
        for source_init in &self.sources {
            identifiers.push(source_init.source.identifier());
        }

        identifiers
    }

    /// 验证资源的完整性
    pub fn validate(&self) -> Result<(), String> {
        if !self.has_sources() {
            return Err("No data sources configured".to_string());
        }

        Ok(())
    }

    /// 简化版本的验证，只检查数据源
    pub fn validate_sources_only(&self) -> Result<(), String> {
        if !self.has_sources() {
            return Err("No data sources configured".to_string());
        }
        Ok(())
    }
}

/// 构建器模式用于创建 EngineResource
pub struct WarpResourceBuilder {
    resource: EngineResource,
}

impl WarpResourceBuilder {
    pub fn new() -> Self {
        Self {
            resource: EngineResource::new(),
        }
    }

    pub fn with_infra(mut self, infra: InfraSinkService) -> Self {
        self.resource.set_infra(infra);
        self
    }

    pub fn with_resource_manager(mut self, resc: ResManager) -> Self {
        self.resource.set_resource_manager(resc);
        self
    }

    pub fn with_sink_coordinator(mut self, sinks: SinkService) -> Self {
        self.resource.set_sink_coordinator(sinks);
        self
    }

    pub fn with_knowdb_handler(mut self, handler: Option<KnowdbHandler>) -> Self {
        self.resource.set_knowdb_handler(handler);
        self
    }

    pub fn with_sources(mut self, sources: Vec<SourceHandle>) -> Self {
        self.resource.sources = sources;
        self
    }

    pub fn with_acceptors(mut self, acceptors: Vec<AcceptorHandle>) -> Self {
        self.resource.acceptors = acceptors;
        self
    }

    pub fn add_source(mut self, source: SourceHandle) -> Self {
        self.resource.add_source(source);
        self
    }

    pub fn build(self) -> Result<EngineResource, String> {
        self.resource.validate()?;
        Ok(self.resource)
    }

    pub fn build_sources_only(self) -> Result<EngineResource, String> {
        self.resource.validate_sources_only()?;
        Ok(self.resource)
    }

    pub fn build_unchecked(self) -> EngineResource {
        self.resource
    }
}

impl Default for WarpResourceBuilder {
    fn default() -> Self {
        Self::new()
    }
}
