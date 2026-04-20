use super::WarpConf;
use crate::orchestrator::config::WPSRC_TOML;
use crate::orchestrator::config::sources_types::{DataEncoding, FileSourceConf, SourceConfig};
use orion_conf::ErrorWith;
use orion_error::ErrorOwe;
use orion_error::OperationContext;
use orion_variate::{EnvDict, EnvEvaluable};
use std::path::{Path, PathBuf};
use wp_conf::engine::EngineConfig;
use wp_error::diagnostic_meta::{ConfigKind, OperationContextMetaExt, OperationKind, RuntimeStage};
use wp_error::run_error::RunResult;

fn orchestrator_config_context(
    kind: ConfigKind,
    path: &Path,
    operation: OperationKind,
) -> OperationContext {
    OperationContext::new()
        .with_meta_value(RuntimeStage::OrchestratorConfigLoad)
        .with_meta_value(kind)
        .with_meta_value(operation)
        .with_file_path(path)
}

impl WarpConf {
    /// 加载源配置并构建所有已启用的源（仅解析，不连接）
    pub fn load_source_config(&self, dict: &EnvDict) -> RunResult<Vec<SourceConfig>> {
        use crate::sources::config::SourceConfigParser;

        let wp_conf = EngineConfig::load_or_init(self.work_root(), dict)
            .owe_conf()
            .with(orchestrator_config_context(
                ConfigKind::Engine,
                self.work_root(),
                OperationKind::LoadConfigFile,
            ))
            .with(self.work_root())
            .want("load engine config")?
            .env_eval(dict)
            .conf_absolutize(self.work_root());
        let path = PathBuf::from(wp_conf.src_conf_of(WPSRC_TOML));
        let content = std::fs::read_to_string(&path)
            .owe_conf()
            .with(orchestrator_config_context(
                ConfigKind::Wpsrc,
                &path,
                OperationKind::LoadConfigFile,
            ))
            .with(&path)
            .want("read source config file")?;

        // 仅支持统一 [[sources]] 配置；不再回退旧格式
        let parser = SourceConfigParser::new(self.work_root().to_path_buf());
        let specs = parser
            .parse_and_validate_only(&content, dict)
            .owe_conf()
            .with(orchestrator_config_context(
                ConfigKind::Wpsrc,
                &path,
                OperationKind::ParseConfig,
            ))
            .with(&path)
            .want("parse source config")?;
        let mut out = Vec::new();
        for spec in specs.into_iter() {
            let f = FileSourceConf {
                key: spec.name,
                path: String::new(),
                enable: true,
                encode: DataEncoding::Text,
                tags: vec![],
            };
            out.push(SourceConfig::File(f));
        }
        Ok(out)
    }
}
