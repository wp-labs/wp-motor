use crate::connectors::registry;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{EnvTomlLoad, ErrorOwe, ErrorWith};
use orion_error::{ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use wp_conf::structure::SourceInstanceConf;
use wp_connector_api::{AcceptorHandle, SourceBuildCtx, SourceHandle};
use wp_log::info_ctrl;

use wp_conf::sources::core_to_resolved_with;
/// 统一格式：[[sources]] 列表
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct UnifiedSourcesConfig {
    #[serde(default)]
    pub sources: Vec<wp_specs::CoreSourceSpec>,
}

impl UnifiedSourcesConfig {
    #[allow(clippy::ptr_arg)]
    pub fn from_file(path: &PathBuf, dict: &EnvDict) -> OrionConfResult<Self> {
        let content = std::fs::read_to_string(path)
            .owe_conf()
            .want("load config")
            .with(path)?;
        Self::from_str(&content, dict)
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str, dict: &EnvDict) -> OrionConfResult<Self> {
        Self::env_parse_toml(content, dict).want("to UnifiedSourcesConfig")
    }
}

struct SourceFactoryLookup;
impl wp_conf::sources::SourceFactoryRegistry for SourceFactoryLookup {
    fn get_factory(
        &self,
        kind: &str,
    ) -> Option<std::sync::Arc<dyn wp_connector_api::SourceFactory + 'static>> {
        crate::connectors::registry::get_source_factory(kind)
    }
}

/// 配置解析和构建器
pub struct SourceConfigParser {
    work_dir: PathBuf,
}

impl SourceConfigParser {
    async fn build_from_specs_with_ids(
        &self,
        specs: Vec<SourceInstanceConf>,
    ) -> OrionConfResult<(Vec<SourceHandle>, Vec<AcceptorHandle>)> {
        let ctx = SourceBuildCtx::new(self.work_dir.clone());
        let mut sources = Vec::new();
        let mut acceptors = Vec::new();
        for item in specs {
            let core: wp_specs::CoreSourceSpec = (&item).into();
            let connector_id = item.connector_id.clone().unwrap_or_default();
            let resolved = core_to_resolved_with(&core, connector_id);
            let fac = registry::get_source_factory(&resolved.kind).ok_or_else(|| {
                ConfIOReason::from_validation(format!(
                    "No factory registered for source kind '{}' (source '{}')",
                    resolved.kind, resolved.name
                ))
                .to_err()
            })?;
            let svc = fac.build(&resolved, &ctx).await.map_err(|e| {
                ConfIOReason::from_validation(format!(
                    "Factory build failed for source '{}' of kind '{}': {}",
                    resolved.name, resolved.kind, e
                ))
            })?;
            sources.extend(svc.sources);
            if let Some(acc) = svc.acceptor {
                acceptors.push(acc);
            }
        }
        Ok((sources, acceptors))
    }
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// 解析配置文件（仅支持 [[sources]] + connect/params_override）并构建所有已启用的源
    #[allow(clippy::ptr_arg)]
    pub async fn parse_and_build(
        &self,
        config_path: &PathBuf,
        dict: &EnvDict,
    ) -> OrionConfResult<(Vec<SourceHandle>, Vec<AcceptorHandle>)> {
        // 使用配置层装配：加载 connectors + 合并 + 产出 CoreSpec + connector_id
        let specs = wp_conf::sources::load_source_instances_from_file(config_path, dict)?;
        // 插件校验（类型特有；不触发 I/O）
        struct Lookup;
        impl wp_conf::sources::SourceFactoryRegistry for Lookup {
            fn get_factory(
                &self,
                kind: &str,
            ) -> Option<std::sync::Arc<dyn wp_connector_api::SourceFactory + 'static>> {
                crate::connectors::registry::get_source_factory(kind)
            }
        }
        wp_conf::sources::validate_specs_with_factory(&specs, &Lookup)?;
        self.build_from_specs_with_ids(specs).await
    }

    /// 解析配置字符串（仅支持 [[sources]] + connect/params_override）并构建所有已启用的源
    pub async fn parse_and_build_from(
        &self,
        config_str: &str,
        dict: &EnvDict,
    ) -> OrionConfResult<(Vec<SourceHandle>, Vec<AcceptorHandle>)> {
        // 起点：work_root；由配置层自行解析 modern/legacy（sources/ 或 source/）布局
        let start = self.work_dir.clone();
        let specs = wp_conf::sources::load_source_instances_from_str(config_str, &start, dict)?;
        struct Lookup2;
        impl wp_conf::sources::SourceFactoryRegistry for Lookup2 {
            fn get_factory(
                &self,
                kind: &str,
            ) -> Option<std::sync::Arc<dyn wp_connector_api::SourceFactory + 'static>> {
                crate::connectors::registry::get_source_factory(kind)
            }
        }
        wp_conf::sources::validate_specs_with_factory(&specs, &Lookup2)?;
        self.build_from_specs_with_ids(specs).await
    }

    /// 仅解析并执行最小校验（不进行实际构建，不触发 I/O）
    pub fn parse_and_validate_only(
        &self,
        config_str: &str,
        dict: &EnvDict,
    ) -> OrionConfResult<Vec<wp_specs::CoreSourceSpec>> {
        // 轻量解析：不依赖 connectors、不做 Factory 校验，仅返回最小 CoreSourceSpec
        // 用途：快速检查 [[sources]] 基本结构，供 CLI 展示/索引构建。
        wp_conf::sources::parse_and_validate_only(config_str, dict)
    }
}

impl SourceConfigParser {
    /// 解析并构建（带运行模式过滤）：batch 下忽略 tcp/syslog(tcp) 源
    pub async fn build_source_handles(
        &self,
        wpsrc_path: &Path,
        run_mode: wp_conf::RunMode,
        dict: &EnvDict,
    ) -> OrionConfResult<(Vec<String>, Vec<SourceHandle>, Vec<AcceptorHandle>)> {
        let specs = wp_conf::sources::load_source_instances_from_file(wpsrc_path, dict)?;
        wp_conf::sources::validate_specs_with_factory(&specs, &SourceFactoryLookup)?;

        // Filter specs by run_mode
        let filtered: Vec<SourceInstanceConf> = match run_mode {
            wp_conf::RunMode::Batch => {
                let kept: Vec<SourceInstanceConf> = specs
                    .into_iter()
                    .filter_map(|item| {
                        let kind = item.kind().to_ascii_lowercase();
                        if kind == "tcp" {
                            info_ctrl!("run-mode=batch: ignore: {:#?}", item.core());
                            return None;
                        }
                        if kind == "syslog" {
                            info_ctrl!("run-mode=batch: ignore: {:#?}", item.core());
                            return None;
                        }
                        Some(item)
                    })
                    .collect();
                kept
            }
            _ => specs,
        };

        let keys: Vec<String> = filtered.iter().map(|s| s.name().clone()).collect();
        let (handles, acceptors) = self.build_from_specs_with_ids(filtered).await?;
        Ok((keys, handles, acceptors))
    }
}
