use super::WarpConf;
use crate::orchestrator::config::WPSRC_TOML;
use crate::orchestrator::config::sources_types::{DataEncoding, FileSourceConf, SourceConfig};
use orion_conf::ErrorWith;
use orion_error::ErrorOwe;
use orion_variate::EnvDict;
use std::path::PathBuf;
use wp_conf::engine::EngineConfig;
use wp_error::run_error::RunResult;

impl WarpConf {
    /// 加载源配置并构建所有已启用的源（仅解析，不连接）
    pub fn load_source_config(&self, dict: &EnvDict) -> RunResult<Vec<SourceConfig>> {
        use crate::sources::config::SourceConfigParser;

        let wp_conf = EngineConfig::load_or_init(self.work_root(), dict)
            .owe_conf()?
            .conf_absolutize(self.work_root());
        let path = PathBuf::from(wp_conf.src_conf_of(WPSRC_TOML));
        let content = std::fs::read_to_string(&path).owe_conf().with(&path)?;

        // 仅支持统一 [[sources]] 配置；不再回退旧格式
        let parser = SourceConfigParser::new(self.work_root().to_path_buf());
        let specs = parser
            .parse_and_validate_only(&content, dict)
            .map_err(|e| {
                use orion_error::{ToStructError, UvsConfFrom};
                wp_error::run_error::RunReason::from_conf(format!(
                    "Failed to parse unified [[sources]] config: {}\npath: {}",
                    e,
                    path.to_string_lossy()
                ))
                .to_err()
            })?;
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
