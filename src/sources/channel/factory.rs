use super::{
    register_channel_field_processors,
    source::{ChannelSource, DEFAULT_CHANNEL_BATCH},
};
use anyhow::{bail, ensure};
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use orion_conf::{ToStructError, UvsConfFrom};
use serde_json::json;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::mpsc::Sender;
use wp_conf::connectors::{ConnectorDef, ConnectorScope, ParamMap};
use wp_conf_base::ConfParser;
use wp_connector_api::{
    SourceBuildCtx, SourceDefProvider, SourceFactory, SourceHandle, SourceMeta, SourceReason,
    SourceResult, SourceSpec as ResolvedSourceSpec, SourceSvcIns, Tags,
};

const DEFAULT_CHANNEL_CAPACITY: usize = 1000;

#[derive(Debug, Clone)]
struct ChannelSourceSpec {
    capacity: usize,
    batch_limit: usize,
}

impl ChannelSourceSpec {
    fn from_params(params: &ParamMap) -> anyhow::Result<Self> {
        let capacity = params
            .get("capacity")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_CHANNEL_CAPACITY as i64);
        ensure!(
            capacity > 0,
            "channel.capacity must be > 0 (got {capacity})"
        );
        let batch_limit = params
            .get("batch_lines")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_CHANNEL_BATCH as i64);
        ensure!(
            batch_limit > 0,
            "channel.batch_lines must be > 0 (got {batch_limit})"
        );
        Ok(Self {
            capacity: capacity as usize,
            batch_limit: batch_limit as usize,
        })
    }
}

fn registry() -> &'static RwLock<HashMap<String, Sender<String>>> {
    static REG: OnceCell<RwLock<HashMap<String, Sender<String>>>> = OnceCell::new();
    REG.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn store_sender(name: &str, sender: Sender<String>) {
    if let Ok(mut guard) = registry().write() {
        guard.insert(name.to_string(), sender);
    }
}

/// 根据源名称获取 channel sender，用于向指定 source 注入数据。
pub fn channel_sender(name: &str) -> Option<Sender<String>> {
    registry()
        .read()
        .ok()
        .and_then(|map| map.get(name).cloned())
}

/// 从注册表中移除对应 sender，释放多余引用。
pub fn unregister_channel_sender(name: &str) {
    if let Ok(mut guard) = registry().write() {
        guard.remove(name);
    }
}

pub struct ChannelSourceFactory;

impl Default for ChannelSourceFactory {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl SourceFactory for ChannelSourceFactory {
    fn kind(&self) -> &'static str {
        "channel"
    }

    fn validate_spec(&self, resolved: &ResolvedSourceSpec) -> SourceResult<()> {
        let res: anyhow::Result<()> = (|| {
            if let Err(e) = Tags::validate(&resolved.tags) {
                bail!("Invalid tags: {}", e);
            }
            ChannelSourceSpec::from_params(&resolved.params)?;
            Ok(())
        })();
        res.map_err(|e| SourceReason::from_conf(e.to_string()).to_err())
    }

    async fn build(
        &self,
        resolved: &ResolvedSourceSpec,
        _ctx: &SourceBuildCtx,
    ) -> SourceResult<SourceSvcIns> {
        let spec = ChannelSourceSpec::from_params(&resolved.params)
            .map_err(|e| SourceReason::from_conf(e.to_string()).to_err())?;
        let tags = Tags::from_parse(&resolved.tags);
        let mut source =
            ChannelSource::with_capacity(resolved.name.clone(), tags.clone(), spec.capacity);
        source.set_batch_limit(spec.batch_limit);
        let sender = source.sender();
        store_sender(&resolved.name, sender.clone());
        register_channel_field_processors(&resolved.name);

        let mut meta = SourceMeta::new(resolved.name.clone(), resolved.kind.clone());
        for (k, v) in tags.iter() {
            meta.tags.set(k, v);
        }
        let handle = SourceHandle::new(Box::new(source), meta);
        Ok(SourceSvcIns::new().with_sources(vec![handle]))
    }
}

impl SourceDefProvider for ChannelSourceFactory {
    fn source_def(&self) -> ConnectorDef {
        let mut params = ParamMap::new();
        params.insert("capacity".into(), json!(DEFAULT_CHANNEL_CAPACITY));
        params.insert("batch_lines".into(), json!(DEFAULT_CHANNEL_BATCH));
        ConnectorDef {
            id: "channel_src".into(),
            kind: self.kind().into(),
            scope: ConnectorScope::Source,
            allow_override: vec!["capacity".into(), "batch_lines".into()],
            default_params: params,
            origin: Some("builtin:channel_source".into()),
        }
    }
}

/// 注册 channel 源工厂（供引擎启动时调用）。
pub fn register_channel_factory() {
    crate::connectors::registry::register_source_factory(ChannelSourceFactory);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_parsing_enforces_bounds() {
        let mut params = ParamMap::new();
        params.insert("capacity".into(), json!(2048));
        params.insert("batch_lines".into(), json!(64));
        let conf = ChannelSourceSpec::from_params(&params).expect("spec");
        assert_eq!(conf.capacity, 2048);
        assert_eq!(conf.batch_limit, 64);

        params.insert("capacity".into(), json!(0));
        assert!(ChannelSourceSpec::from_params(&params).is_err());
    }

    #[tokio::test]
    async fn factory_builds_channel_and_registers_sender() {
        let fac = ChannelSourceFactory;
        let spec = ResolvedSourceSpec {
            name: "test_channel".into(),
            kind: "channel".into(),
            connector_id: String::new(),
            params: {
                let mut map = ParamMap::new();
                map.insert("capacity".into(), json!(4));
                map
            },
            tags: vec!["env:test".into()],
        };
        let ctx = SourceBuildCtx::new(std::path::PathBuf::from("."));
        let mut svc = fac.build(&spec, &ctx).await.expect("build channel");
        assert_eq!(svc.sources.len(), 1);
        assert!(channel_sender("test_channel").is_some());

        // Clean up registry to avoid leaking senders across tests
        unregister_channel_sender("test_channel");
        svc.sources.clear();
    }
}
