use crate::compat::LegacyOwe;
use crate::core::generator::rules::fetch_oml_data;
use crate::core::parser::WplPipeline;
use crate::core::parser::indexing::ResourceIndexer;
use crate::orchestrator::config::WPARSE_OML_FILE;
use crate::orchestrator::config::WPARSE_RULE_FILE;
use crate::orchestrator::engine::definition::WplCodePKG;
use orion_error::conversion::ToStructError;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::thread;
use wp_conf::engine::EngineConfig;
use wp_error::run_error::{RunReason, RunResult};
use wp_stat::StatReq;
use wpl::AnnotationType;
use wpl::WplEvaluator;
use wpl::util::fetch_wpl_data;
use wpl::{WplCode, WplExpress, WplPackage, WplRule, WplStatementType};

use super::RuleKey;
use super::core::allocator::ParserResAlloc;
use super::core::manager::OmlRepository;

/// 构建全局 `pkg/rule` → parser 映射（跨所有包），用于解析 copy_event_parse 的跨包目标引用。
///
/// key 为 `pkg/rule` 全路径（如 `/fun/raw_event`）。包含 `#[no_match]` rule 的 parser，
/// 供同包或跨包 copy_event_parse 注入。
pub fn build_global_parser_map(
    packages: &[WplPackage],
) -> RunResult<HashMap<SmolStr, WplEvaluator>> {
    let mut map = HashMap::new();
    for pkg in packages {
        for rule in pkg.rules.iter() {
            let parser = build_multi_src_parser_set(rule)?;
            map.insert(rule.path(pkg.name.as_str()).into(), parser);
        }
    }
    Ok(map)
}

/// 判断 rule 是否显式声明不参与 parse_event 自动匹配（`#[no_match]`）。
///
/// `#[no_match]` 是使用者对「仅作 copy_event_parse 等显式调用目标」的 rule 的显式声明；
/// 装配 pipeline 时跳过这类 rule（其 parser 仍由 `build_global_parser_map` 保留供注解注入），
/// 避免 parse_event 直接匹配它们、抢在引用方 rule 之前产出残缺 record。
fn is_no_match(rule: &WplRule) -> bool {
    rule.statement
        .tags()
        .as_ref()
        .map(|t| t.no_match)
        .unwrap_or(false)
}

pub fn multi_code_ins_parse_units(
    alloc: &impl ParserResAlloc,
    lang_pkg: &WplPackage,
    idx: &mut ResourceIndexer,
    stat_reqs: Vec<StatReq>,
    parser_map: &HashMap<SmolStr, WplEvaluator>,
) -> RunResult<Vec<WplPipeline>> {
    let mut items = Vec::new();
    for rule in lang_pkg.rules.iter() {
        let wpl_path = rule.path(lang_pkg.name.as_str());
        let parser = parser_map
            .get(wpl_path.as_str())
            .cloned()
            .expect("parser built in build_global_parser_map");
        let funcs = annotate_funcs(rule, lang_pkg.name.as_str(), parser_map)?;
        let agent = alloc.alloc_parse_res(&RuleKey::from(&wpl_path))?;
        let mut ppu = WplPipeline::new(
            idx.checkin(wpl_path.as_str()),
            wpl_path,
            lang_pkg.name.to_string(),
            rule.name().to_string(),
            funcs,
            parser,
            agent,
            stat_reqs.clone(),
        );
        // #[no_match]：不参与 parse_event 自动匹配，但仍建 pipeline 以提供 sink 路由
        // （copy_event_parse 旁路 record 经其 wpl_key 路由到对应 sink）
        if is_no_match(rule) {
            ppu.auto_match = false;
        }
        items.push(ppu);
    }
    Ok(items)
}

pub fn code_ins_parse_units(
    alloc: impl ParserResAlloc,
    lang_pkg: &WplPackage,
    idx: &mut ResourceIndexer,
    parser_map: &HashMap<SmolStr, WplEvaluator>,
) -> RunResult<Vec<WplPipeline>> {
    debug_ctrl!("thread: {:?}, load rule ", thread::current().id(),);
    let mut items = Vec::new();
    for rule in lang_pkg.rules.iter() {
        let wpl_path = rule.path(lang_pkg.name.as_str());
        let parser = parser_map
            .get(wpl_path.as_str())
            .cloned()
            .expect("parser built in build_global_parser_map");
        let funcs = annotate_funcs(rule, lang_pkg.name.as_str(), parser_map)?;
        let agent = alloc.alloc_parse_res(&RuleKey::from(wpl_path.as_str()))?;
        let mut ppu = WplPipeline::new(
            idx.checkin(wpl_path.as_str()),
            wpl_path,
            lang_pkg.name.to_string(),
            rule.name().to_string(),
            funcs,
            parser,
            agent,
            Vec::new(),
        );
        if is_no_match(rule) {
            ppu.auto_match = false;
        }
        items.push(ppu);
    }
    Ok(items)
}

pub fn annotate_funcs(
    rule: &WplRule,
    pkg_name: &str,
    parser_map: &HashMap<SmolStr, WplEvaluator>,
) -> RunResult<Vec<AnnotationType>> {
    let mut funcs = AnnotationType::convert(rule.statement.tags())
        .into_iter()
        .filter(|ann| !matches!(ann, AnnotationType::Null(_)))
        .collect::<Vec<_>>();
    // 解析 copy_event_parse 的目标 rule parser：
    //   先按引用字面值（全路径如 /fun/raw_event）查全局 map（跨包引用）；
    //   未命中再按「当前包/裸名」查（同包裸名引用如 raw_event → /pipe_demo/raw_event）。
    //   命中后把 rule_name 规范化为命中的全路径——side-record 按它路由，
    //   send_to_sink_groups 才能按目标 pipeline 的 wpl_key（全路径）匹配到 sink。
    //   引用不存在的 rule 是逻辑错误，直接报错阻止启动。
    for ann in &mut funcs {
        if let AnnotationType::CopyEventParse(c) = ann {
            let resolved = parser_map
                .get(&c.rule_name)
                .map(|p| (p.clone(), c.rule_name.clone()))
                .or_else(|| {
                    let full = format!("{}/{}", pkg_name, c.rule_name);
                    parser_map
                        .get(full.as_str())
                        .map(|p| (p.clone(), SmolStr::from(full)))
                });
            match resolved {
                Some((p, resolved_key)) => {
                    c.target = Some(p);
                    c.rule_name = resolved_key;
                }
                None => {
                    return Err(RunReason::rule_error().to_err().with_detail(format!(
                        "copy_event_parse: target rule \"{}\" not found (referenced by rule \"{}/{}\"); cross-package targets must be referenced as pkg/rule",
                        c.rule_name, pkg_name, rule.name()
                    )));
                }
            }
        }
    }
    Ok(funcs)
}

pub fn build_multi_src_parser_set(rule: &WplRule) -> RunResult<WplEvaluator> {
    let parser = rule_to_parser_ex(rule, None)?;
    Ok(parser)
}

pub fn rule_to_parser_ex(rule: &WplRule, preorder: Option<&WplExpress>) -> RunResult<WplEvaluator> {
    let parser = match &rule.statement {
        WplStatementType::Express(code) => {
            WplEvaluator::from(code, preorder).owe(RunReason::rule_error())?
        }
    };
    Ok(parser)
}

pub fn rule_to_parser(rule: &WplRule) -> RunResult<WplEvaluator> {
    let parser = match &rule.statement {
        WplStatementType::Express(code) => {
            WplEvaluator::from(code, None).owe(RunReason::rule_error())?
        }
    };
    Ok(parser)
}

pub async fn load_oml_code(oml_root: &str) -> RunResult<OmlRepository> {
    fetch_oml_data(oml_root, WPARSE_OML_FILE).owe(RunReason::core_conf())
}

pub async fn load_wpl_code(
    conf: &EngineConfig,
    rule_file: Option<String>,
) -> RunResult<Vec<WplCode>> {
    let rule_path: String = rule_file.clone().unwrap_or(conf.rule_root().to_string());
    fetch_wpl_data(rule_path.as_str(), WPARSE_RULE_FILE).owe(RunReason::core_conf())
}

pub async fn load_engine_code(main_conf: &EngineConfig) -> RunResult<WplCodePKG> {
    let model_wpl = load_wpl_code(main_conf, None).await?;
    Ok(WplCodePKG::from_codes(model_wpl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::ParseOption;
    use crate::core::parser::wpl_engine::parser::MultiParser;
    use crate::core::parser::wpl_engine::types::ProcessResult;
    use crate::sinks::SinkGroupAgent;
    use std::sync::Arc;
    use wp_connector_api::{SourceEvent, Tags};
    use wp_model_core::raw::RawData;
    use wp_primitives::Parser;
    use wpl::wpl_package;

    /// 测试用分配器：每条 rule 给一个 null sink agent，不接真实 sink。
    struct NullAlloc;
    impl ParserResAlloc for NullAlloc {
        fn alloc_parse_res(&self, _rule_key: &RuleKey) -> RunResult<Vec<SinkGroupAgent>> {
            Ok(vec![SinkGroupAgent::null()])
        }
    }

    fn assert_chars(record: &wp_model_core::model::DataRecord, key: &str, expected: &str) {
        use wp_model_core::model::Value;
        let f = record
            .field(key)
            .unwrap_or_else(|| panic!("missing field {key}"));
        match f.get_value() {
            Value::Chars(actual) => assert_eq!(actual, expected, "field {key}"),
            other => panic!("field {key} expected chars, got {:?}", other),
        }
    }

    /// 端到端：copy_event_parse 注入 + wp_event_md5 盖章的完整链路。
    #[test]
    fn end_to_end_copy_event_parse_with_event_md5() {
        let pkg = pkg_with_copy_event_parse("raw_event");
        let mut idx = ResourceIndexer::default();
        let parser_map = build_global_parser_map(std::slice::from_ref(&pkg)).expect("global map");
        // 经 multi_code_ins_parse_units 装配：annotate_funcs 按 pkg/rule 注入 target
        let pipelines =
            multi_code_ins_parse_units(&NullAlloc, &pkg, &mut idx, Vec::new(), &parser_map)
                .expect("build pipelines");
        let mut parser = MultiParser::new(pipelines);

        // 同一 payload 须同时满足 main(json(chars@data)) 与 raw_event(json(chars@raw))
        let payload = r#"{ "data": "main-data", "raw": "raw-content" }"#;
        let event = SourceEvent::new(
            1,
            "test-src",
            RawData::String(payload.to_string()),
            Arc::new(Tags::new()),
        );
        // gen_msg_id=true（meta 生成）+ gen_event_md5=true（嵌在 meta 块内）
        let setting = ParseOption::new(true, true, Vec::new());

        let (record, side_records) = match parser.parse_event(&event, &setting) {
            ProcessResult::Success {
                record,
                side_records,
                ..
            } => (record, side_records),
            other => panic!("expected Success, got {:?}", other),
        };

        // copy_event_parse：raw_event 解析产出作为独立旁路 record 流出（emit，不 merge）
        // 证明 emit：side_records 非空（merge 行为下 side_records 会是空）。
        assert_eq!(
            side_records.len(),
            1,
            "exactly one side record from copy_event_parse"
        );
        let (side_key, side_rec) = &side_records[0];
        // 裸名引用被规范化为全路径 demo/raw_event（side-record 按全路径路由才能命中目标 pipeline）
        assert_eq!(side_key, "demo/raw_event");
        assert_chars(side_rec, "raw", "raw-content");
        // 旁路 record 也盖了事件 meta
        let expected_md5 = format!("{:x}", md5::compute(payload.as_bytes()));
        assert_chars(side_rec, "wp_event_md5", &expected_md5);
        assert!(
            side_rec.field("wp_event_id").is_some(),
            "side record stamped with wp_event_id"
        );

        // main 自身解析产出 data 字段
        assert_chars(&record, "data", "main-data");
        // 主 record 同样盖了 meta
        assert_chars(&record, "wp_event_md5", &expected_md5);
        assert!(record.field("wp_event_id").is_some(), "wp_event_id stamped");
        assert_chars(&record, "wp_src_key", "test-src");
    }

    /// 构造一个含 copy_event_parse 引用的双规则包：
    ///   main 挂 #[copy_event_parse(rule:"raw_event")]，raw_event 是目标 rule。
    ///   raw_event 显式声明 #[no_match]：不参与自动匹配，仅作 copy_event_parse 目标。
    fn pkg_with_copy_event_parse(target: &str) -> WplPackage {
        let code = format!(
            r#"
package demo {{
  #[copy_event_parse(rule:"{target}")] rule main {{ (json(chars@data)) }}
  #[no_match] rule raw_event {{ (json(chars@raw)) }}
}}"#,
            target = target
        );
        wpl_package.parse(code.as_str()).expect("parse package")
    }

    #[test]
    fn build_parser_map_captures_all_rules() {
        let pkg = pkg_with_copy_event_parse("raw_event");
        let map = build_global_parser_map(std::slice::from_ref(&pkg)).expect("build global map");
        // key 为 pkg/rule 全路径
        assert!(map.contains_key("demo/main"));
        assert!(map.contains_key("demo/raw_event"));
    }

    #[test]
    fn no_match_rule_built_but_excluded_from_auto_match() {
        // #[no_match] rule 仍建 pipeline（供 sink 路由），但 auto_match=false，parse_event 跳过
        let pkg = pkg_with_copy_event_parse("raw_event");
        let mut idx = ResourceIndexer::default();
        let parser_map = build_global_parser_map(std::slice::from_ref(&pkg)).expect("global map");
        let pipelines =
            multi_code_ins_parse_units(&NullAlloc, &pkg, &mut idx, Vec::new(), &parser_map)
                .expect("build pipelines");
        // 包内 2 条 rule 都建了 pipeline
        assert_eq!(pipelines.len(), 2, "both rules get a pipeline");
        let by_name: std::collections::HashMap<&str, &WplPipeline> = pipelines
            .iter()
            .map(|p| (p.rule_name().as_str(), p))
            .collect();
        assert!(
            by_name["main"].auto_match,
            "main participates in auto-match"
        );
        assert!(
            !by_name["raw_event"].auto_match,
            "raw_event is #[no_match], excluded from auto-match"
        );
    }

    #[test]
    fn annotate_funcs_injects_target_for_known_rule() {
        let pkg = pkg_with_copy_event_parse("raw_event");
        let map = build_global_parser_map(std::slice::from_ref(&pkg)).expect("build global map");
        let main_rule = pkg
            .rules
            .iter()
            .find(|r| r.name().as_str() == "main")
            .expect("main rule present");
        let funcs =
            annotate_funcs(main_rule, "demo", &map).expect("annotate_funcs ok for known rule");
        let cp = funcs
            .iter()
            .find_map(|f| match f {
                AnnotationType::CopyEventParse(c) => Some(c),
                _ => None,
            })
            .expect("copy_event_parse annotation present");
        // 裸名 "raw_event" 被规范化为全路径 demo/raw_event
        assert_eq!(cp.rule_name.as_str(), "demo/raw_event");
        assert!(
            cp.target.is_some(),
            "target parser should be injected when rule exists"
        );
    }

    #[test]
    fn annotate_funcs_errors_on_unknown_rule() {
        // 引用不存在的 rule：逻辑错误，应返回 Err 阻止引擎启动
        let pkg = pkg_with_copy_event_parse("nope");
        let map = build_global_parser_map(std::slice::from_ref(&pkg)).expect("build global map");
        let main_rule = pkg
            .rules
            .iter()
            .find(|r| r.name().as_str() == "main")
            .expect("main rule present");
        let result = annotate_funcs(main_rule, "demo", &map);
        assert!(
            result.is_err(),
            "annotate_funcs should return Err when copy_event_parse target rule is missing"
        );
        let err_msg = format!("{}", result.err().unwrap());
        assert!(
            err_msg.contains("nope"),
            "error should mention the missing rule name, got: {err_msg}"
        );
    }

    /// 跨包 copy_event_parse：/pipe_demo 的 rule 引用 /fun 包的 raw_event。
    #[test]
    fn copy_event_parse_resolves_cross_package_target() {
        let pipe_pkg = wpl_package
            .parse(
                r#"
package /pipe_demo {
  #[copy_event_parse(rule:"/fun/raw_event")] rule main { (json(chars@data)) }
}
"#,
            )
            .expect("parse pipe_demo package");
        let fun_pkg = wpl_package
            .parse(
                r#"
package /fun {
  #[no_match] rule raw_event { (json(chars@raw)) }
}
"#,
            )
            .expect("parse fun package");
        let pkgs = vec![pipe_pkg, fun_pkg];
        // build_global_parser_map 跨包收集：/pipe_demo/main 与 /fun/raw_event 都在
        let map = build_global_parser_map(&pkgs).expect("build global map");
        assert!(map.contains_key("/pipe_demo/main"));
        assert!(map.contains_key("/fun/raw_event"));

        let pipe_pkg = &pkgs[0];
        let main_rule = pipe_pkg
            .rules
            .iter()
            .find(|r| r.name().as_str() == "main")
            .expect("main rule");
        let funcs = annotate_funcs(main_rule, "/pipe_demo", &map).expect("annotate_funcs ok");
        let cp = funcs
            .iter()
            .find_map(|f| match f {
                AnnotationType::CopyEventParse(c) => Some(c),
                _ => None,
            })
            .expect("copy_event_parse annotation present");
        assert_eq!(cp.rule_name.as_str(), "/fun/raw_event");
        assert!(
            cp.target.is_some(),
            "cross-package target parser should be injected"
        );
    }
}
