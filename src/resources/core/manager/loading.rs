use crate::compat::LegacyOwe;
use std::path::{Path, PathBuf};
use wp_specs::WildArray;

use crate::orchestrator::config::build_sinks::SinkRouteTable;
use crate::resources::ModelName;
use crate::resources::utils::{load_engine_code, load_oml_code};
use crate::sinks::SinkGroupAgent;
use oml::core::ConfADMExt;
use oml::language::{DataModel, ObjModel};
use orion_conf::ErrorWith;
use orion_error::{OperationContext, conversion::SourceErr, conversion::ToStructError};
use orion_variate::EnvDict;
use wp_conf::engine::EngineConfig;
use wp_error::RunReason;
use wp_error::run_error::{IntoRunError, RunResult};

use crate::core::parser::{SpaceIndex, WplRepository};

use super::conflicts::{MatcherKind, find_duplicate_group_names, find_duplicate_matchers};
use super::res_manager::ResManager;

/// 一个已加载的 business 路由组：名称、来源文件与匹配器。
///
/// daemon 与离线检查共用：前者用它做启动期重复绑定告警/fail-closed，后者用它出报告。
pub(crate) struct LoadedSinkGroup {
    pub name: String,
    pub origin: Option<PathBuf>,
    pub oml_matchers: WildArray,
    pub rule_matchers: WildArray,
}

pub(crate) struct LoadedSinkRoutes {
    pub table: SinkRouteTable,
    pub groups: Vec<LoadedSinkGroup>,
}

fn format_groups(groups: &[(String, Option<PathBuf>)]) -> String {
    groups
        .iter()
        .map(|(name, origin)| match origin {
            Some(path) => format!("{name} ({})", path.display()),
            None => name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl ResManager {
    pub async fn load_all_wpl_code(
        &mut self,
        main_conf: &EngineConfig,
        error_sink: &SinkGroupAgent,
    ) -> RunResult<()> {
        info_ctrl!("load all wpl code beg...");
        let wpl_code = load_engine_code(main_conf).await?;
        let wpl_space = WplRepository::from_wpl_tolerant(wpl_code, error_sink.end_point())
            .owe(RunReason::rule_error())?;
        self.wpl_index = Some(SpaceIndex::from(&wpl_space));
        self.wpl_space = Some(wpl_space);
        info_ctrl!("load all wpl code end");
        Ok(())
    }

    pub async fn load_all_ldm(&mut self, oml_root: &str) -> RunResult<()> {
        info_ctrl!("load all oml model");
        let oml_spc = load_oml_code(oml_root).await?;
        let wpl_index = self
            .wpl_index
            .clone()
            .ok_or(RunReason::validation_error())?;
        for (path, _code) in oml_spc.items {
            if std::path::Path::new(path.as_str()).exists() && path.ends_with(".oml") {
                let mdl = ObjModel::load(path.as_str())
                    .await
                    // 保留 OML 解析详情（IntoRunError 提取 Syntax/NotFound 内层消息为 detail）
                    .map_err(IntoRunError::into_run_err)
                    .doing("load oml")
                    .with_context(path.as_str())?;
                // Skip disabled models
                if !mdl.enable() {
                    info_data!("oml disabled, skip: {} ", path);
                    continue;
                }
                info_data!("oml load success, from {} ", path);
                for w_rule in mdl.rules().as_ref() {
                    for r_path in wpl_index.rule_key().iter() {
                        if w_rule.matches(r_path.as_str()) {
                            self.rule_mdl_relation.update(
                                r_path,
                                mdl.name().as_str(),
                                w_rule.to_string().as_str(),
                            );
                        }
                    }
                }
                let key = ModelName::from(mdl.name().as_str());
                let odm = DataModel::Object(mdl);
                self.name_mdl_res.insert(key, odm);
            } else {
                warn_data!("{} not exists", path)
            }
        }
        Ok(())
    }

    /// 与 [`ResManager::load_all_sink`] 共用同一条加载/索引路径，额外返回每个路由组的
    /// 来源文件与匹配器；daemon 用它做启动期重复绑定检测，离线检查用它出报告。
    ///
    /// Duplicate matchers are always logged as errors. When `deny_duplicate_oml`
    /// is set, a duplicate `oml` binding aborts the load (fail closed) instead
    /// of only logging.
    pub(crate) fn load_all_sink_with_origins(
        &mut self,
        sink_root: &str,
        dict: &EnvDict,
        deny_duplicate_oml: bool,
    ) -> RunResult<LoadedSinkRoutes> {
        let mut op = OperationContext::doing("load all sink").with_auto_log();
        let wpl_index = self
            .wpl_index
            .clone()
            .ok_or(RunReason::validation_error())?;

        let mut sink_route = SinkRouteTable::default();
        let busin_d = Path::new(sink_root).join("business.d");
        let infra_d = Path::new(sink_root).join("infra.d");
        if busin_d.exists() || infra_d.exists() {
            let confs =
                wp_conf::sinks::load_business_route_confs_with_origins(sink_root, &Lookup, dict)
                    .source_err(RunReason::core_conf(), "load sink route confs")
                    .with_context(sink_root)?;
            let groups: Vec<LoadedSinkGroup> = confs
                .iter()
                .map(|loaded| LoadedSinkGroup {
                    name: loaded.conf.sink_group.name.clone(),
                    origin: loaded.origin.clone(),
                    oml_matchers: loaded.conf.sink_group.oml().clone(),
                    rule_matchers: loaded.conf.sink_group.rule().clone(),
                })
                .collect();

            // Detect before mutating any index so a fail-closed abort leaves
            // no partially built state behind.
            for dup in find_duplicate_group_names(&groups) {
                let files = dup
                    .origins
                    .iter()
                    .map(|origin| {
                        origin
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                error_ctrl!(
                    "sink_group name '{}' is declared by {} route files [{}]; group names are SinkIDs and will collide",
                    dup.name,
                    dup.origins.len(),
                    files
                );
            }

            let duplicates = find_duplicate_matchers(&groups);
            for dup in &duplicates {
                error_ctrl!(
                    "duplicate {} matcher '{}' bound by sink_groups [{}]; the rule and model indexes may diverge and silently disable the OML stage",
                    dup.kind.as_str(),
                    dup.matcher,
                    format_groups(&dup.groups)
                );
            }
            if deny_duplicate_oml
                && let Some(dup) = duplicates.iter().find(|dup| dup.kind == MatcherKind::Oml)
            {
                return Err(RunReason::core_conf().to_err().with_detail(format!(
                    "duplicate oml matcher '{}' bound by sink_groups [{}]; fix the config or disable [validate].deny_duplicate_oml",
                    dup.matcher,
                    format_groups(&dup.groups)
                )));
            }

            for loaded in confs {
                let mut conf = loaded.conf;
                // 现有的方法正确处理 FlexGroup rule 和 oml 字段
                self.update_sink_rule_index(&wpl_index, &mut conf);
                self.update_sink_mdl_index(&conf);
                sink_route.add_route(conf.sink_group);
            }
            self.sink_table = Some(sink_route.clone());
            op.mark_suc();
            Ok(LoadedSinkRoutes {
                table: sink_route,
                groups,
            })
        } else {
            Err(RunReason::core_conf()
                .to_err()
                .with_detail(format!("sink route dirs not found under '{}'", sink_root)))
        }
    }

    pub(crate) fn load_all_sink(
        &mut self,
        sink_root: &str,
        dict: &EnvDict,
        deny_duplicate_oml: bool,
    ) -> RunResult<SinkRouteTable> {
        self.load_all_sink_with_origins(sink_root, dict, deny_duplicate_oml)
            .map(|loaded| loaded.table)
    }
}
struct Lookup;
impl wp_conf::sinks::SinkFactoryLookup for Lookup {
    fn get(
        &self,
        kind: &str,
    ) -> Option<std::sync::Arc<dyn wp_connector_api::SinkFactory + 'static>> {
        wp_core_connectors::registry::get_sink_factory(kind)
    }
}

#[cfg(test)]
mod fail_closed_tests {
    use super::*;
    use orion_variate::EnvDict;
    use std::fs;
    use wp_conf::test_support::ForTest;

    const CONNECTOR: &str = r#"[[connectors]]
allow_override = ["base", "file", "sync"]
id = "file_json_sink"
type = "file"

[connectors.params]
base = "./data/out_dat"
file = "default.json"
fmt = "json"
sync = false
"#;

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(path, body).expect("write fixture");
    }

    fn route(root: &Path, file: &str, group: &str, oml: &str) {
        write_route(root, file, group, &format!("oml = [\"{oml}\"]"));
    }

    fn route_rule(root: &Path, file: &str, group: &str, rule: &str) {
        write_route(root, file, group, &format!("rule = [\"{rule}\"]"));
    }

    fn write_route(root: &Path, file: &str, group: &str, matcher_line: &str) {
        write(
            root,
            &format!("topology/sinks/business.d/{file}"),
            &format!(
                r#"version = "2.0"

[sink_group]
name = "{group}"
{matcher_line}

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"

[sink_group.sinks.params]
file = "{group}.jsonl"
"#
            ),
        );
    }

    fn load(dir: &Path, deny_duplicate_oml: bool) -> RunResult<LoadedSinkRoutes> {
        write(dir, "connectors/sink.d/00-file_json_sink.toml", CONNECTOR);
        let mut res = ResManager {
            wpl_index: Some(SpaceIndex::default()),
            ..Default::default()
        };
        let dict = EnvDict::test_default();
        let sink_root = dir.join("topology/sinks");
        res.load_all_sink_with_origins(sink_root.to_str().expect("utf8"), &dict, deny_duplicate_oml)
    }

    #[test]
    fn deny_duplicate_oml_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        route(dir.path(), "a.toml", "a", "nginx");
        route(dir.path(), "b.toml", "b", "nginx");

        let err = match load(dir.path(), true) {
            Ok(_) => panic!("duplicate oml must fail closed"),
            Err(err) => err,
        };
        let detail = err.display_chain();
        assert!(detail.contains("duplicate oml matcher"), "{detail}");
        assert!(detail.contains("a.toml"), "{detail}");
        assert!(detail.contains("b.toml"), "{detail}");
    }

    #[test]
    fn duplicate_oml_is_tolerated_when_not_denied() {
        let dir = tempfile::tempdir().expect("tempdir");
        route(dir.path(), "a.toml", "a", "nginx");
        route(dir.path(), "b.toml", "b", "nginx");

        let loaded = load(dir.path(), false).expect("duplicates are tolerated by default");
        assert_eq!(loaded.groups.len(), 2);
    }

    #[test]
    fn deny_duplicate_oml_allows_star_plus_concrete() {
        let dir = tempfile::tempdir().expect("tempdir");
        route(dir.path(), "a.toml", "a", "*");
        route(dir.path(), "b.toml", "b", "nginx");

        let loaded = load(dir.path(), true).expect("star + concrete is not a duplicate");
        assert_eq!(loaded.groups.len(), 2);
    }

    #[test]
    fn deny_duplicate_oml_does_not_block_duplicate_rule_matchers() {
        let dir = tempfile::tempdir().expect("tempdir");
        route_rule(dir.path(), "a.toml", "a", "/nginx/*");
        route_rule(dir.path(), "b.toml", "b", "/nginx/*");

        let loaded = load(dir.path(), true).expect("rule duplicates are log-only");
        assert_eq!(loaded.groups.len(), 2);
    }
}
