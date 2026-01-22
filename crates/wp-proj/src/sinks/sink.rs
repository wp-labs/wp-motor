use orion_error::ErrorConv;
use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_cli_core::business::connectors::sinks as sinks_core;
use wp_conf::connectors::param_map_to_table;
use wp_conf::engine::EngineConfig;
use wp_conf::sinks::{
    build_route_conf_from,
    io::{business_dir, infra_dir, load_connectors_for, load_route_files_from, load_sink_defaults},
};
use wp_conf::structure::SinkInstanceConf;
use wp_connector_api::ParamMap;
use wp_error::run_error::RunResult;

use crate::traits::{Checkable, Component, ComponentBase, ComponentLifecycle, HasStatistics};
use crate::types::CheckStatus;
use crate::utils::config_path::ConfigPathResolver;

#[derive(Clone)]
pub struct Sinks {
    base: ComponentBase,
}

// Deref to ComponentBase for seamless access to base methods
impl std::ops::Deref for Sinks {
    type Target = ComponentBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Sinks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Sinks {
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            base: ComponentBase::new(work_root, eng_conf),
        }
    }

    fn sink_root(&self) -> PathBuf {
        self.resolve_path(self.eng_conf().sink_root())
    }

    // 校验路由（严格）
    pub fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        sinks_core::validate_routes(self.work_root().to_string_lossy().as_ref(), dict)
            .err_conv()?;
        Ok(CheckStatus::Suc)
        //.map_err(|e| RunReason::from_conf(e.to_string()).to_err())
    }

    pub fn route_rows(
        &self,
        group_filters: &[String],
        sink_filters: &[String],
        dict: &EnvDict,
    ) -> RunResult<Vec<sinks_core::RouteRow>> {
        fn matched(name: &str, filters: &[String]) -> bool {
            filters.is_empty() || filters.iter().any(|f| name.contains(f))
        }

        let sink_root = self.sink_root();
        let defaults = load_sink_defaults(&sink_root, dict).err_conv()?;
        let conn_map =
            load_connectors_for(sink_root.to_string_lossy().as_ref(), dict).err_conv()?;
        let mut rows = Vec::new();

        for (scope, dir) in [
            ("biz", business_dir(&sink_root)),
            ("infra", infra_dir(&sink_root)),
        ] {
            let route_files = load_route_files_from(&dir, dict).err_conv()?;
            for rf in route_files {
                let conf = build_route_conf_from(&rf, defaults.as_ref(), &conn_map).err_conv()?;
                let group = conf.sink_group;
                if !matched(group.name(), group_filters) {
                    continue;
                }
                let rules: Vec<String> =
                    group.rule.as_ref().iter().map(|m| m.to_string()).collect();
                let oml_patterns: Vec<String> =
                    group.oml().as_ref().iter().map(|m| m.to_string()).collect();

                for sink in group.sinks.iter() {
                    if !matched(sink.name(), sink_filters) {
                        continue;
                    }
                    let (target, detail) = target_detail_of(sink);
                    rows.push(sinks_core::RouteRow {
                        scope: scope.to_string(),
                        group: group.name().to_string(),
                        full_name: sink.full_name(),
                        name: sink.name().to_string(),
                        connector: sink.connector_id.clone().unwrap_or_else(|| "-".to_string()),
                        target,
                        fmt: sink.fmt().to_string(),
                        detail,
                        rules: rules.clone(),
                        oml: oml_patterns.clone(),
                    });
                }
            }
        }

        rows.sort_by(|a, b| a.scope.cmp(&b.scope).then(a.full_name.cmp(&b.full_name)));
        Ok(rows)
    }

    // 展平成路由表（biz+infra），带过滤
    // 初始化 sinks 骨架（写入配置指定的sink目录，如果配置不存在则使用默认路径）
    pub fn init(&self) -> RunResult<()> {
        let sink_root = self.sink_root();

        Self::ensure_defaults_file(&sink_root)?;
        Self::ensure_business_demo(&sink_root)?;
        Self::ensure_infra_defaults(&sink_root.join("infra.d"))?;
        Ok(())
    }

    fn ensure_defaults_file(sink_root: &std::path::Path) -> RunResult<()> {
        let p = sink_root.join("defaults.toml");
        let should_write = if p.exists() {
            match std::fs::read_to_string(&p) {
                Ok(body) => body.trim().is_empty(),
                Err(_) => true,
            }
        } else {
            true
        };
        if should_write {
            let body = include_str!("../example/topology/sinks/defaults.toml");
            ConfigPathResolver::write_file_with_dir(&p, body)?;
        }
        Ok(())
    }

    fn ensure_business_demo(sink_root: &std::path::Path) -> RunResult<()> {
        let biz = sink_root.join("business.d");
        ConfigPathResolver::ensure_dir_exists(&biz)?;
        let demo = biz.join("demo.toml");
        if !demo.exists() {
            let demo_content = include_str!("../example/topology/sinks/business.d/demo.toml");
            ConfigPathResolver::write_file_with_dir(&demo, demo_content)?;
        }
        Ok(())
    }

    fn ensure_infra_defaults(dir: &std::path::Path) -> RunResult<()> {
        ConfigPathResolver::ensure_dir_exists(dir)?;

        for (name, body) in [
            (
                "default.toml",
                include_str!("../example/topology/sinks/infra.d/default.toml"),
            ),
            (
                "miss.toml",
                include_str!("../example/topology/sinks/infra.d/miss.toml"),
            ),
            (
                "residue.toml",
                include_str!("../example/topology/sinks/infra.d/residue.toml"),
            ),
            (
                "error.toml",
                include_str!("../example/topology/sinks/infra.d/error.toml"),
            ),
            (
                "monitor.toml",
                include_str!("../example/topology/sinks/infra.d/monitor.toml"),
            ),
        ] {
            let path = dir.join(name);
            if !path.exists() {
                ConfigPathResolver::write_file_with_dir(&path, body)?;
            }
        }

        Ok(())
    }
}

// Trait implementations for unified component interface
impl Component for Sinks {
    fn component_name(&self) -> &'static str {
        "Sinks"
    }
}

impl Checkable for Sinks {
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        // Delegate to the existing check implementation
        Sinks::check(self, dict)
    }
}

impl HasStatistics for Sinks {
    fn has_statistics(&self) -> bool {
        // Sinks has statistics capabilities via the stat module
        self.sink_root().exists()
    }
}

impl ComponentLifecycle for Sinks {
    fn init(&self, _dict: &EnvDict) -> RunResult<()> {
        // Delegate to the existing init implementation
        Sinks::init(self)
    }
}

fn target_detail_of(s: &SinkInstanceConf) -> (String, String) {
    let kind = s.resolved_kind_str();
    let params = s.resolved_params_table();
    let target = if kind == "syslog" {
        let proto = params
            .get("protocol")
            .and_then(|v| v.as_str())
            .unwrap_or("udp");
        format!("syslog/{}", proto)
    } else {
        kind.clone()
    };
    let detail = params_one_line(&params);
    (target, detail)
}

fn params_one_line(params: &ParamMap) -> String {
    let table = param_map_to_table(params);
    match toml::to_string(&table) {
        Ok(s) => s.replace(['\n', '\t'], " ").trim().to_string(),
        Err(_) => format!("{:?}", params),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{temp_workdir, write_basic_wparse_config};

    #[test]
    fn init_populates_sink_templates() {
        let temp = temp_workdir();
        write_basic_wparse_config(temp.path());
        let eng = Arc::new(EngineConfig::init(temp.path()).conf_absolutize(temp.path()));
        let sinks = Sinks::new(temp.path(), eng);
        sinks.init().expect("init sinks");

        let sink_root = temp.path().join("topology/sinks");
        assert!(sink_root.join("defaults.toml").exists());
        assert!(sink_root.join("business.d/demo.toml").exists());
        assert!(sink_root.join("infra.d/default.toml").exists());
    }
}
