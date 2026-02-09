use crate::core::DataRecordRef;
use wp_model_core::model::data::record::RecordItem;
use crate::core::ExpEvaluator;
use crate::language::{EvalExp, ObjModel, PreciseEvaluator};
use crate::parser::error::OMLCodeErrorTait;
use crate::parser::keyword::{kw_head_sep_line, kw_oml_enable, kw_oml_name, kw_static};
use crate::parser::oml_aggregate::oml_aggregate;
use crate::parser::static_ctx::{clear_symbols, install_symbols};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use winnow::ascii::multispace0;
use winnow::combinator::repeat;
use winnow::error::{ContextError, ErrMode, StrContext, StrContextValue};
use winnow::stream::Stream;
use wp_data_model::cache::FieldQueryCache;
use wp_error::{OMLCodeError, OMLCodeResult};
use wp_model_core::model::{DataField, DataRecord};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::{take_obj_path, take_var_name};
use wp_parser::symbol::symbol_colon;
use wp_parser::utils::get_scope;
use wpl::parser::utils::peek_str;

use super::keyword::kw_oml_rule;

pub fn oml_parse_raw(data: &mut &str) -> WResult<ObjModel> {
    oml_conf_code.parse_next(data)
}
pub fn oml_parse(data: &mut &str, tag: &str) -> OMLCodeResult<ObjModel> {
    match oml_conf_code.parse_next(data) {
        Ok(o) => Ok(o),
        Err(e) => Err(OMLCodeError::from_syntax(e, data, tag)),
    }
}

pub fn oml_conf_code(data: &mut &str) -> WResult<ObjModel> {
    let name = oml_conf_head.parse_next(data)?;
    debug_rule!("obj model: {} begin ", name);
    let mut a_items = ObjModel::new(name);

    // Parse optional config items (enable and rule) in any order
    loop {
        multispace0.parse_next(data)?;
        let ck = data.checkpoint();
        // Try to parse enable first (to avoid 'enable' being consumed as a rule path)
        if oml_conf_enable.parse_next(data).is_ok_and(|en| {
            a_items.set_enable(en);
            true
        }) {
            continue;
        }
        data.reset(&ck);
        // Try to parse rules
        if oml_conf_rules.parse_next(data).is_ok_and(|rules| {
            a_items.bind_rules(Some(rules));
            true
        }) {
            continue;
        }
        data.reset(&ck);
        // Neither enable nor rules found, break
        break;
    }
    debug_rule!("obj model: rules loaded!");

    kw_head_sep_line.parse_next(data)?;

    let static_items = parse_static_blocks(data)?;
    let mut items: Vec<EvalExp> = repeat(1.., oml_aggregate).parse_next(data)?;
    debug_rule!("obj model: aggregate item  loaded!");
    //repeat(1.., terminated(oml_aggregate, symbol_semicolon)).parse_next(data)?;
    a_items.items.append(&mut items);
    clear_symbols();

    finalize_static_blocks(&mut a_items, static_items)?;

    // Check if any field name starts with "__" (temporary field marker)
    let has_temp = check_temp_fields(&a_items.items);
    a_items.set_has_temp_fields(has_temp);

    multispace0.parse_next(data)?;
    if !data.is_empty() {
        if peek_str("---", data).is_ok() {
            kw_head_sep_line.parse_next(data)?;
        } else {
            //探测错误;
            oml_aggregate.parse_next(data)?;
        }
    }
    Ok(a_items)
}

/// Check if any evaluation expression has a target field starting with "__"
fn check_temp_fields(items: &[EvalExp]) -> bool {
    for item in items {
        match item {
            EvalExp::Single(single) => {
                if check_targets_temp(single.target()) {
                    return true;
                }
            }
            EvalExp::Batch(batch) => {
                if check_batch_target_temp(batch.target()) {
                    return true;
                }
            }
        }
    }
    false
}

fn check_targets_temp(targets: &[crate::language::EvaluationTarget]) -> bool {
    targets.iter().any(|t| {
        t.name()
            .as_ref()
            .map(|n| n.starts_with("__"))
            .unwrap_or(false)
    })
}

fn check_batch_target_temp(target: &crate::language::BatchEvalTarget) -> bool {
    target
        .origin()
        .name()
        .as_ref()
        .map(|n| n.starts_with("__"))
        .unwrap_or(false)
}

fn parse_static_blocks(data: &mut &str) -> WResult<Vec<EvalExp>> {
    let mut static_items = Vec::new();
    let mut symbols = Vec::new();
    let mut symbol_set = HashSet::new();
    loop {
        multispace0.parse_next(data)?;
        if peek_str("static", data).is_err() {
            break;
        }
        kw_static.parse_next(data)?;
        multispace0.parse_next(data)?;
        let block = get_scope(data, '{', '}')?;
        let mut block_data: &str = block;
        loop {
            multispace0.parse_next(&mut block_data)?;
            if block_data.is_empty() {
                break;
            }
            let exp = oml_aggregate.parse_next(&mut block_data)?;
            let sym_name = extract_static_target(&exp)?;
            if !symbol_set.insert(sym_name.clone()) {
                let mut err = ContextError::new();
                err.push(StrContext::Label("duplicate static binding"));
                err.push(StrContext::Expected(StrContextValue::Description(
                    "unique symbol",
                )));
                return Err(ErrMode::Cut(err));
            }
            symbols.push(sym_name);
            static_items.push(exp);
        }
    }
    if symbols.is_empty() {
        clear_symbols();
    } else {
        install_symbols(symbols);
    }
    Ok(static_items)
}

fn extract_static_target(exp: &EvalExp) -> Result<String, ErrMode<ContextError>> {
    match exp {
        EvalExp::Single(single) => {
            if let Some(target) = single.target().first() {
                Ok(target.safe_name())
            } else {
                let mut err = ContextError::new();
                err.push(StrContext::Label("static assignment"));
                err.push(StrContext::Expected(StrContextValue::Description(
                    "target required",
                )));
                Err(ErrMode::Cut(err))
            }
        }
        EvalExp::Batch(_) => {
            let mut err = ContextError::new();
            err.push(StrContext::Label("static assignment"));
            err.push(StrContext::Expected(StrContextValue::Description(
                "single evaluator",
            )));
            Err(ErrMode::Cut(err))
        }
    }
}

fn finalize_static_blocks(
    model: &mut ObjModel,
    static_items: Vec<EvalExp>,
) -> Result<(), ErrMode<ContextError>> {
    if static_items.is_empty() {
        model.set_static_fields(HashMap::new());
        return Ok(());
    }

    let const_fields = materialize_static_items(&static_items)?;
    rewrite_static_references(model, &const_fields)?;
    model.set_static_fields(const_fields);
    Ok(())
}

fn materialize_static_items(
    items: &[EvalExp],
) -> Result<HashMap<String, Arc<DataField>>, ErrMode<ContextError>> {
    let mut cache = FieldQueryCache::default();
    let src = DataRecord::default();
    let mut dst = DataRecord::default();

    for exp in items {
        let mut src_ref = DataRecordRef::from(&src);
        exp.eval_proc(&mut src_ref, &mut dst, &mut cache);
    }

    let mut const_map = HashMap::new();
    for field in dst.items.into_iter() {
        const_map.insert(field.get_name().to_string(), Arc::new(field.into_owned()));
    }
    Ok(const_map)
}

fn rewrite_static_references(
    model: &mut ObjModel,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    for item in &mut model.items {
        if let EvalExp::Single(single) = item {
            rewrite_precise_evaluator(single.eval_way_mut(), const_fields)?;
        }
    }
    Ok(())
}

fn rewrite_precise_evaluator(
    eval: &mut PreciseEvaluator,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match eval {
        PreciseEvaluator::StaticSymbol(sym) => {
            let field = const_fields.get(sym).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("static reference"));
                err.push(StrContext::Label("symbol not found"));
                ErrMode::Cut(err)
            })?;
            // Use Arc::clone instead of DataField clone for zero-copy sharing
            *eval = PreciseEvaluator::ObjArc(Arc::clone(field));
            Ok(())
        }
        PreciseEvaluator::Match(op) => rewrite_match_operation(op, const_fields),
        PreciseEvaluator::Pipe(pipe) => rewrite_pipe_operation(pipe, const_fields),
        PreciseEvaluator::Fun(fun) => rewrite_fun_operation(fun, const_fields),
        PreciseEvaluator::Map(map) => rewrite_map_operation(map, const_fields),
        PreciseEvaluator::Tdc(op) => rewrite_record_operation(op, const_fields),
        PreciseEvaluator::Collect(arr) => rewrite_arr_operation(arr, const_fields),
        _ => Ok(()),
    }
}

fn rewrite_map_operation(
    op: &mut crate::language::MapOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    for binding in op.subs_mut() {
        rewrite_nested_accessor(binding.acquirer_mut(), const_fields)?;
    }
    Ok(())
}

fn rewrite_record_operation(
    op: &mut crate::language::RecordOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    if let Some(default) = op.default_val_mut() {
        rewrite_generic_accessor(default.accessor_mut(), const_fields)?;
    }
    Ok(())
}

fn rewrite_arr_operation(
    _arr: &mut crate::language::ArrOperation,
    _const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    Ok(())
}

fn rewrite_pipe_operation(
    _op: &mut crate::language::PiPeOperation,
    _const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    Ok(())
}

fn rewrite_fun_operation(
    _fun: &mut crate::language::FunOperation,
    _const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    Ok(())
}

fn rewrite_match_operation(
    op: &mut crate::language::MatchOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    // Rewrite result part (already exists)
    for case in op.items_mut() {
        rewrite_nested_accessor(case.result_mut(), const_fields)?;
    }
    if let Some(default_case) = op.default_mut() {
        rewrite_nested_accessor(default_case.result_mut(), const_fields)?;
    }

    // Rewrite condition part (new)
    for case in op.items_mut() {
        rewrite_match_condition(case.condition_mut(), const_fields)?;
    }

    Ok(())
}

fn rewrite_match_condition(
    cond: &mut crate::language::MatchCondition,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    use crate::language::MatchCondition;
    match cond {
        MatchCondition::Single(c) => rewrite_single_match_cond(c, const_fields)?,
        MatchCondition::Double(c1, c2) => {
            rewrite_single_match_cond(c1, const_fields)?;
            rewrite_single_match_cond(c2, const_fields)?;
        }
        MatchCondition::Default => {}
    }
    Ok(())
}

fn rewrite_single_match_cond(
    cond: &mut crate::language::MatchCond,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    use crate::language::MatchCond;
    match cond {
        MatchCond::EqSym(sym) => {
            let sym_str = sym.clone(); // Clone to avoid borrow conflict
            let field = const_fields.get(&sym_str).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("match condition"));
                err.push(StrContext::Label("static symbol not found"));
                ErrMode::Cut(err)
            })?;
            // Use Arc::clone instead of DataField clone for zero-copy sharing
            *cond = MatchCond::EqArc(Arc::clone(field));
        }
        MatchCond::NeqSym(sym) => {
            let sym_str = sym.clone(); // Clone to avoid borrow conflict
            let field = const_fields.get(&sym_str).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("match condition"));
                err.push(StrContext::Label("static symbol not found"));
                ErrMode::Cut(err)
            })?;
            // Use Arc::clone instead of DataField clone for zero-copy sharing
            *cond = MatchCond::NeqArc(Arc::clone(field));
        }
        MatchCond::InSym(beg_sym, end_sym) => {
            let beg_str = beg_sym.clone(); // Clone to avoid borrow conflict
            let end_str = end_sym.clone(); // Clone to avoid borrow conflict
            let beg_field = const_fields.get(&beg_str).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("match condition"));
                err.push(StrContext::Label("static symbol not found"));
                ErrMode::Cut(err)
            })?;
            let end_field = const_fields.get(&end_str).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("match condition"));
                err.push(StrContext::Label("static symbol not found"));
                ErrMode::Cut(err)
            })?;
            // Use Arc::clone instead of DataField clone for zero-copy sharing
            *cond = MatchCond::InArc(Arc::clone(beg_field), Arc::clone(end_field));
        }
        _ => {}
    }
    Ok(())
}

fn rewrite_nested_accessor(
    accessor: &mut crate::language::NestedAccessor,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    if let Some(sym) = accessor.as_static_symbol() {
        let field = const_fields.get(sym).ok_or_else(|| {
            let mut err = ContextError::new();
            err.push(StrContext::Label("static reference"));
            err.push(StrContext::Label("symbol not found"));
            ErrMode::Cut(err)
        })?;
        // Use Arc::clone instead of DataField clone for zero-copy sharing
        accessor.replace_with_field_arc(Arc::clone(field));
    }
    Ok(())
}

fn rewrite_generic_accessor(
    accessor: &mut crate::language::GenericAccessor,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    if let Some(sym) = accessor.as_static_symbol() {
        let field = const_fields.get(sym).ok_or_else(|| {
            let mut err = ContextError::new();
            err.push(StrContext::Label("static reference"));
            err.push(StrContext::Label("symbol not found"));
            ErrMode::Cut(err)
        })?;
        // Use Arc::clone instead of DataField clone for zero-copy sharing
        accessor.replace_with_field_arc(Arc::clone(field));
    }
    Ok(())
}

pub fn oml_conf_head(data: &mut &str) -> WResult<String> {
    multispace0.parse_next(data)?;
    let (_, _, name) = (
        kw_oml_name,
        symbol_colon,
        take_obj_path.context(StrContext::Label("oml name")),
    )
        .parse_next(data)?;
    Ok(name.to_string())
}
pub fn oml_conf_rules(data: &mut &str) -> WResult<Vec<String>> {
    multispace0.parse_next(data)?;
    let (_, _) = (kw_oml_rule, symbol_colon).parse_next(data)?;
    // Use custom path parser that stops before reserved keywords
    let rules: Vec<&str> = repeat(0.., oml_rule_path).parse_next(data)?;
    Ok(rules.into_iter().map(|s| s.to_string()).collect())
}

/// Custom rule path parser that checks for reserved keywords
fn oml_rule_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    use winnow::ascii::multispace1;
    use winnow::token::take_while;

    multispace0.parse_next(input)?;
    // Check if it's a reserved keyword before parsing
    let trimmed = input.trim_start();
    if trimmed.starts_with("enable") || trimmed.starts_with("---") {
        // Return backtrack error to stop repeat
        return Err(winnow::error::ErrMode::Backtrack(ContextError::new()));
    }
    let key =
        take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '/', '*'])).parse_next(input)?;
    multispace1.parse_next(input)?;
    Ok(key)
}

pub fn oml_conf_enable(data: &mut &str) -> WResult<bool> {
    multispace0.parse_next(data)?;
    let (_, _) = (kw_oml_enable, symbol_colon).parse_next(data)?;
    let value: &str = take_var_name
        .context(StrContext::Label("enable value"))
        .parse_next(data)?;
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            let mut err = ContextError::new();
            err.push(StrContext::Label("enable value"));
            err.push(StrContext::Expected(StrContextValue::Description(
                "true or false",
            )));
            Err(ErrMode::Cut(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::oml_conf::oml_parse_raw;
    use crate::parser::utils::for_test::{assert_oml_parse, assert_oml_parse_ext};
    use wp_model_core::model::data::record::RecordItem;
    use wp_parser::Parser;
    use wp_parser::WResult as ModalResult;
    use wp_parser::comment::CommentParser;

    #[test]
    fn test_conf_sample() -> ModalResult<()> {
        let mut code = r#"
name : test
rule :
    wpx/abc
    wpx/efg
---
version      :chars   = chars(1.0.0) ;
pos_sn       :chars   = take() ;
aler*        :auto   = take() ;
src_ip       :auto   = take();
update_time  :time    = take() { _ :  time(2020-10-01 12:30:30) };

        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        let mut code = r#"
name : test
rule :
    wpx/abc   wpx/efg
---
version      :chars   = chars(1.0.0) ;
pos_sn       :chars   = take() ;
aler*        : auto   = take() ;
update_time  :time    = take() { _ :  time(2020-10-01 12:30:30) };
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_fun() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = Now::time() ;
version      : chars   = Now::time() ;
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_pipe() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = pipe take() | base64_encode  ;
version      : chars   = pipe take(ip) | to_str |  base64_encode ;
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_pipe_optional_keyword() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test pipe without 'pipe' keyword - should parse successfully
        let mut code = r#"
name : test
---
url_secure = take(url) | starts_with('https://') | map_to(true) ;
encoded = read(data) | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        // Test mixed usage: with and without 'pipe' keyword
        let mut code = r#"
name : test
---
version1 = pipe take(ip) | to_str | base64_encode ;
version2 = take(ip) | to_str | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        Ok(())
    }

    #[test]
    fn test_static_block_parsing() -> ModalResult<()> {
        use crate::language::{EvalExp, PreciseEvaluator};

        let mut code = r#"
name : test
---
static {
    template = object {
        id = chars(E1);
    };
}

target_template = template;
        "#;

        let model = oml_parse_raw(&mut code)?;
        assert_eq!(model.static_fields().len(), 1);
        assert_eq!(model.items.len(), 1);
        match &model.items[0] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::ObjArc(field) => {
                    assert_eq!(field.get_name(), "template");
                }
                other => panic!("unexpected evaluator: {:?}", other),
            },
            _ => panic!("expected single evaluator"),
        }
        Ok(())
    }

    #[test]
    fn test_static_in_map_binding() -> ModalResult<()> {
        use crate::language::{EvalExp, NestedAccessor, PreciseEvaluator};

        let mut code = r#"
name : test
---
static {
    tpl = object {
        id = chars(E1);
        tpl = chars(foo)
    };
}

result = object {
    clone = tpl;
};
        "#;

        let model = oml_parse_raw(&mut code)?;
        assert_eq!(model.static_fields().len(), 1);
        match &model.items[0] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::Map(map) => {
                    let subs = map.subs();
                    assert_eq!(subs.len(), 1);
                    match subs[0].acquirer() {
                        NestedAccessor::FieldArc(_) => {}
                        other => panic!("expected field arc accessor, got {:?}", other),
                    }
                }
                other => panic!("unexpected evaluator: {:?}", other),
            },
            _ => panic!("expected single evaluator"),
        }
        Ok(())
    }

    #[test]
    fn test_static_in_default_binding() -> ModalResult<()> {
        use crate::language::{EvalExp, GenericAccessor, PreciseEvaluator};

        let mut code = r#"
name : test
---
static {
    fallback = object {
        id = chars(E1);
        tpl = chars(bar)
    };
}

value = take(Value) { _ : fallback };
        "#;

        let model = oml_parse_raw(&mut code)?;
        assert_eq!(model.static_fields().len(), 1);
        match &model.items[0] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::Tdc(op) => {
                    let default = op.default_val().as_ref().expect("default binding");
                    match default.accessor() {
                        GenericAccessor::FieldArc(field) => {
                            assert_eq!(field.get_name(), "fallback");
                        }
                        other => panic!("expected field arc accessor, got {:?}", other),
                    }
                }
                other => panic!("unexpected evaluator: {:?}", other),
            },
            _ => panic!("expected single evaluator"),
        }
        Ok(())
    }

    #[test]
    fn test_static_in_match_cases() -> ModalResult<()> {
        use crate::language::{EvalExp, NestedAccessor, PreciseEvaluator};

        let mut code = r#"
name : test
---
static {
    tpl = object {
        id = chars(E1);
        tpl = chars(foo)
    };
}

target = match read(Content) {
    starts_with('foo') => tpl;
    _ => tpl;
};
        "#;

        let model = oml_parse_raw(&mut code)?;
        assert_eq!(model.static_fields().len(), 1);
        match &model.items[0] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::Match(op) => {
                    for case in op.items() {
                        match case.result() {
                            NestedAccessor::FieldArc(field) => {
                                assert_eq!(field.get_name(), "tpl");
                            }
                            other => panic!("expected field arc accessor, got {:?}", other),
                        }
                    }
                    if let Some(default_case) = op.default() {
                        match default_case.result() {
                            NestedAccessor::FieldArc(field) => {
                                assert_eq!(field.get_name(), "tpl");
                            }
                            other => panic!("expected field arc accessor, got {:?}", other),
                        }
                    } else {
                        panic!("match should have default case");
                    }
                }
                other => panic!("unexpected evaluator: {:?}", other),
            },
            _ => panic!("expected single evaluator"),
        }
        Ok(())
    }
    #[test]
    fn test_conf_fmt() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      :chars   = fmt("_{}*{}",@ip,@sys)  ;
        "#;
        oml_parse_raw.parse_next(&mut code)?;
        //assert_oml_parse(&mut code, oml_conf);
        Ok(())
    }
    #[test]
    fn test_conf2() -> ModalResult<()> {
        let mut code = r#"
name : test
---
values : obj = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
citys : array = collect take( keys : [ a,b,c* ] ) ;
        "#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[test]
    fn test_conf3() -> ModalResult<()> {
        let mut code = r#"
name : test
---
src_city: chars = match take( x_type ) {
            chars(A) => chars(bj),
            chars(B) => chars(cs),
            _ => take(src_city)
};
values : obj = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
        "#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }

    #[test]
    fn test_conf4() -> ModalResult<()> {
        let mut code = r#"
name : test
---

src_city  = match take( x_type ) {
            chars(A) => chars(bj),
            chars(B) => chars(cs),
            _ => take(src_city)
};
values  = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
"#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[test]
    fn test_conf_comment() -> ModalResult<()> {
        let mut raw_code = r#"
name : test
---
// this is ok;
version      = chars(1.0.0) ;
pos_sn       = take () ;
update_time  = take () { _ :  time(2020-10-01 12:30:30) };
        "#;

        let expect = r#"
name : test
---
version      : auto = chars(1.0.0) ;
pos_sn       : auto = take () ;
update_time  : auto = take () { _ :  time(2020-10-01 12:30:30) };
        "#;

        let code = CommentParser::ignore_comment(&mut raw_code)?;
        let mut pure_code = code.as_str();
        assert_oml_parse_ext(&mut pure_code, oml_parse_raw, expect);
        Ok(())
    }

    #[test]
    fn test_conf_quoted_chars() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test that chars() supports both quoted and unquoted strings
        let mut code1 = r#"
name : test
---
msg1 = chars('hello world');
msg2 = chars("goodbye");
msg3 = chars(unquoted);
        "#;
        let model = oml_parse_raw(&mut code1).assert();
        assert_eq!(model.name(), "test");

        // Test with special characters
        let mut code2 = r#"
name : test
---
msg = chars('hello\nworld');
        "#;
        let model2 = oml_parse_raw(&mut code2).assert();
        assert_eq!(model2.name(), "test");

        Ok(())
    }

    #[test]
    fn test_temp_field_filter() -> ModalResult<()> {
        use crate::core::DataTransformer;
        use orion_error::TestAssert;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::{DataRecord, DataType};

        // Test that fields starting with "__" are converted to ignore type
        let mut code = r#"
name : test
---
__temp = chars(temporary);
result = chars(final);
__another_temp = chars(also_temp);
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");

        // Transform with empty input
        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform(input, cache);

        // Check that normal fields are preserved
        let result_field = output.field("result");
        assert!(result_field.is_some(), "Normal field 'result' should exist");
        assert_eq!(result_field.unwrap().get_meta(), &DataType::Chars);
        assert_eq!(result_field.unwrap().get_value().to_string(), "final");

        // Check that temporary fields are converted to ignore type
        let temp_field = output.field("__temp");
        assert!(
            temp_field.is_some(),
            "Temporary field '__temp' should exist"
        );
        assert_eq!(
            temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "Temporary field should be Ignore type"
        );

        let another_temp_field = output.field("__another_temp");
        assert!(
            another_temp_field.is_some(),
            "Temporary field '__another_temp' should exist"
        );
        assert_eq!(
            another_temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "Temporary field should be Ignore type"
        );

        Ok(())
    }

    #[test]
    fn test_temp_field_in_computation() -> ModalResult<()> {
        use crate::core::DataTransformer;
        use orion_error::TestAssert;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::{DataRecord, DataType};

        // Test that temporary fields can be used in intermediate computation
        // Simple test: use temp field in match expression
        let mut code = r#"
name : test
---
__temp_type = chars(error);
result = match read(__temp_type) {
    chars(error) => chars(failed),
    _ => chars(ok),
};
        "#;
        let model = oml_parse_raw(&mut code).assert();

        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform(input, cache);

        // Debug: print all fields
        println!("Output fields:");
        for field in &output.items {
            println!(
                "  {}: {} = {:?}",
                field.get_name(),
                field.get_meta(),
                field.get_value()
            );
        }

        // Check that the final result field exists
        let result = output.field("result");
        assert!(result.is_some(), "Result field should exist");

        // Check that temporary field is converted to ignore type
        let temp_field = output.field("__temp_type");
        assert!(temp_field.is_some());
        assert_eq!(
            temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "__temp_type should be Ignore type"
        );

        Ok(())
    }

    #[test]
    fn test_temp_field_flag() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 1: Model with no temporary fields
        let mut code_no_temp = r#"
name : test
---
normal1 = chars(value1);
normal2 = chars(value2);
        "#;
        let model_no_temp = oml_parse_raw(&mut code_no_temp).assert();
        assert_eq!(
            model_no_temp.has_temp_fields(),
            false,
            "Should not have temp fields flag"
        );

        // Test case 2: Model with temporary fields
        let mut code_with_temp = r#"
name : test
---
__temp = chars(temp_value);
normal = chars(normal_value);
        "#;
        let model_with_temp = oml_parse_raw(&mut code_with_temp).assert();
        assert_eq!(
            model_with_temp.has_temp_fields(),
            true,
            "Should have temp fields flag"
        );

        // Test case 3: Multiple temporary fields
        let mut code_multi_temp = r#"
name : test
---
__temp1 = chars(value1);
normal = chars(value2);
__temp2 = chars(value3);
        "#;
        let model_multi_temp = oml_parse_raw(&mut code_multi_temp).assert();
        assert_eq!(
            model_multi_temp.has_temp_fields(),
            true,
            "Should have temp fields flag"
        );

        Ok(())
    }

    #[test]
    fn test_enable_config_default() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 1: Default enable (no enable config)
        let mut code_default = r#"
name : test
---
field = chars(value);
        "#;
        let model_default = oml_parse_raw(&mut code_default).assert();
        assert_eq!(
            *model_default.enable(),
            true,
            "Default enable should be true"
        );

        Ok(())
    }

    #[test]
    fn test_enable_config_true() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 2: Explicit enable = true
        let mut code_enable_true = r#"
name : test
enable : true
---
field = chars(value);
        "#;
        let model_enable_true = oml_parse_raw(&mut code_enable_true).assert();
        assert_eq!(*model_enable_true.enable(), true, "Explicit enable true");

        Ok(())
    }

    #[test]
    fn test_enable_config_false() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 3: Explicit enable = false
        let mut code_enable_false = r#"
name : test
enable : false
---
field = chars(value);
        "#;
        let model_enable_false = oml_parse_raw(&mut code_enable_false).assert();
        assert_eq!(*model_enable_false.enable(), false, "Explicit enable false");

        Ok(())
    }

    #[test]
    fn test_enable_config_with_rule() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 4: enable with rule
        let mut code_with_rule = r#"
name : test
rule : /test/*
enable : false
---
field = chars(value);
        "#;
        let model_with_rule = oml_parse_raw(&mut code_with_rule).assert();
        assert_eq!(*model_with_rule.enable(), false, "enable with rule");

        Ok(())
    }

    #[test]
    fn test_enable_explicit() -> ModalResult<()> {
        use crate::parser::oml_conf::oml_conf_enable;

        // Test parsing enable directly
        let mut enable_str = "enable : true ";
        let result = oml_conf_enable(&mut enable_str);
        assert!(result.is_ok(), "Should parse enable: {:?}", result);
        assert_eq!(result.unwrap(), true);

        let mut enable_false = "enable : false ";
        let result = oml_conf_enable(&mut enable_false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);

        Ok(())
    }
}
