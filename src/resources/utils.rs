use crate::core::generator::rules::fetch_oml_data;
use crate::core::parser::WplPipeline;
use crate::core::parser::indexing::ResourceIndexer;
use crate::orchestrator::config::WPARSE_OML_FILE;
use crate::orchestrator::config::WPARSE_RULE_FILE;
use crate::orchestrator::engine::definition::WplCodePKG;
use crate::types::AnyResult;
use orion_error::ErrorOwe;
use std::thread;
use wp_conf::engine::EngineConfig;
use wp_error::run_error::RunResult;
use wp_stat::StatReq;
use wpl::AnnotationType;
use wpl::WplEvaluator;
use wpl::util::fetch_wpl_data;
use wpl::{WplCode, WplExpress, WplPackage, WplRule, WplStatementType};

use super::RuleKey;
use super::core::allocator::ParserResAlloc;
use super::core::manager::OmlRepository;

pub fn multi_code_ins_parse_units(
    alloc: &impl ParserResAlloc,
    lang_pkg: &WplPackage,
    idx: &mut ResourceIndexer,
    stat_reqs: Vec<StatReq>,
) -> RunResult<Vec<WplPipeline>> {
    let mut items = Vec::new();
    for rule in lang_pkg.rules.iter() {
        let parser = build_multi_src_parser_set(rule)?;
        let funcs = annotate_funcs(rule);
        let wpl_path = rule.path(lang_pkg.name.as_str());
        let agent = alloc.alloc_parse_res(&RuleKey::from(&wpl_path))?;
        let ppu = WplPipeline::new(
            idx.checkin(wpl_path.as_str()),
            wpl_path,
            funcs,
            parser,
            agent,
            stat_reqs.clone(),
        );
        items.push(ppu);
    }
    Ok(items)
}

pub fn code_ins_parse_units(
    alloc: impl ParserResAlloc,
    lang_pkg: &WplPackage,
    idx: &mut ResourceIndexer,
) -> AnyResult<Vec<WplPipeline>> {
    debug_ctrl!("thread: {:?}, load rule ", thread::current().id(),);
    let mut items = Vec::new();
    for rule in lang_pkg.rules.iter() {
        let parser = build_multi_src_parser_set(rule)?;
        let funcs = annotate_funcs(rule);
        let wpl_path = rule.path(lang_pkg.name.as_str());
        let agent = alloc.alloc_parse_res(&RuleKey::from(wpl_path.as_str()))?;
        let ppu = WplPipeline::new(
            idx.checkin(wpl_path.as_str()),
            wpl_path,
            funcs,
            parser,
            agent,
            Vec::new(),
        );
        items.push(ppu);
    }
    Ok(items)
}

pub fn annotate_funcs(rule: &WplRule) -> Vec<AnnotationType> {
    AnnotationType::convert(rule.statement.tags())
}

pub fn build_multi_src_parser_set(rule: &WplRule) -> RunResult<WplEvaluator> {
    let parser = rule_to_parser_ex(rule, None)?;
    Ok(parser)
}

pub fn rule_to_parser_ex(rule: &WplRule, preorder: Option<&WplExpress>) -> RunResult<WplEvaluator> {
    let parser = match &rule.statement {
        WplStatementType::Express(code) => WplEvaluator::from(code, preorder).owe_rule()?,
    };
    Ok(parser)
}

pub fn rule_to_parser(rule: &WplRule) -> RunResult<WplEvaluator> {
    let parser = match &rule.statement {
        WplStatementType::Express(code) => WplEvaluator::from(code, None).owe_rule()?,
    };
    Ok(parser)
}

pub async fn load_oml_code(oml_root: &str) -> RunResult<OmlRepository> {
    fetch_oml_data(oml_root, WPARSE_OML_FILE).owe_conf()
}

pub async fn load_wpl_code(
    conf: &EngineConfig,
    rule_file: Option<String>,
) -> RunResult<Vec<WplCode>> {
    let rule_path: String = rule_file.clone().unwrap_or(conf.rule_root().to_string());
    fetch_wpl_data(rule_path.as_str(), WPARSE_RULE_FILE).owe_conf()
}

pub async fn load_engine_code(main_conf: &EngineConfig) -> RunResult<WplCodePKG> {
    let model_wpl = load_wpl_code(main_conf, None).await?;
    Ok(WplCodePKG::from_codes(model_wpl))
}
