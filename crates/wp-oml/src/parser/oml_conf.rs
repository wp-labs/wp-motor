use crate::core::AsyncExpEvaluator;
use crate::core::DataRecordRef;
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
use wp_error::{OMLCodeError, OMLCodeResult};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, Value};
use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::atom::{take_obj_path, take_var_name};
use wp_primitives::symbol::symbol_colon;
use wp_primitives::utils::get_scope;
use wpl::parser::utils::peek_str;

use super::keyword::kw_oml_rule;

#[cfg(test)]
pub(crate) fn oml_parse_syntax_raw(data: &mut &str) -> WResult<ObjModel> {
    oml_conf_code.parse_next(data)
}

pub async fn oml_parse_raw(data: &mut &str) -> WResult<ObjModel> {
    let mut model = oml_conf_code.parse_next(data)?;
    let static_items = std::mem::take(&mut model.static_items);
    finalize_static_blocks(&mut model, static_items).await?;
    compute_sync_flags(&mut model);
    Ok(model)
}

pub async fn oml_parse(data: &mut &str, tag: &str) -> OMLCodeResult<ObjModel> {
    match oml_parse_raw(data).await {
        Ok(o) => Ok(o),
        Err(e) => Err(OMLCodeError::from_syntax(e, data, tag)),
    }
}

struct StaticSymbolGuard;

impl StaticSymbolGuard {
    fn new() -> Self {
        clear_symbols();
        Self
    }
}

impl Drop for StaticSymbolGuard {
    fn drop(&mut self) {
        clear_symbols();
    }
}

pub fn oml_conf_code(data: &mut &str) -> WResult<ObjModel> {
    let _static_symbol_guard = StaticSymbolGuard::new();
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
    a_items.static_items = static_items;

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
            install_symbols(symbols.clone());
            let exp = oml_aggregate.parse_next(&mut block_data)?;
            ensure_static_block_expr_supported(&exp)?;
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

fn ensure_static_block_expr_supported(exp: &EvalExp) -> Result<(), ErrMode<ContextError>> {
    match exp {
        EvalExp::Single(single) => ensure_static_precise_supported(single.eval_way()),
        EvalExp::Batch(_) => static_block_err("static block only supports single expressions"),
    }
}

fn static_block_err(reason: &'static str) -> Result<(), ErrMode<ContextError>> {
    let mut err = ContextError::new();
    err.push(StrContext::Label("static block"));
    err.push(StrContext::Label(reason));
    Err(ErrMode::Cut(err))
}

fn ensure_static_precise_supported(eval: &PreciseEvaluator) -> Result<(), ErrMode<ContextError>> {
    match eval {
        PreciseEvaluator::StaticSymbol(_) => {
            static_block_err("static block does not support referencing other static symbols")
        }
        PreciseEvaluator::Obj(_) | PreciseEvaluator::ObjArc(_) | PreciseEvaluator::Val(_) => Ok(()),
        PreciseEvaluator::Calc(op) => ensure_static_calc_expr_supported(op.expr()),
        PreciseEvaluator::Map(op) => {
            for binding in op.subs() {
                ensure_static_nested_accessor_supported(binding.acquirer())?;
            }
            Ok(())
        }
        PreciseEvaluator::ObjArray(op) => {
            for item in op.items() {
                ensure_static_precise_supported(item)?;
            }
            Ok(())
        }
        PreciseEvaluator::Sql(_) => {
            static_block_err("static block does not support SQL queries or external effects")
        }
        PreciseEvaluator::Tdc(_)
        | PreciseEvaluator::Pipe(_)
        | PreciseEvaluator::Collect(_)
        | PreciseEvaluator::AccessDirect(_) => {
            static_block_err("static block does not support read/take-based runtime access")
        }
        PreciseEvaluator::Fun(_) => {
            static_block_err("static block does not support runtime functions")
        }
        PreciseEvaluator::Fmt(_) => {
            static_block_err("static block does not support runtime formatting access")
        }
        PreciseEvaluator::Match(_) | PreciseEvaluator::Lookup(_) => {
            static_block_err("static block only supports pure literal/object expressions")
        }
    }
}

fn ensure_static_calc_expr_supported(
    expr: &crate::language::CalcExpr,
) -> Result<(), ErrMode<ContextError>> {
    match expr {
        crate::language::CalcExpr::Const(_) => Ok(()),
        crate::language::CalcExpr::Accessor(_) => {
            static_block_err("static block does not support read/take access inside calc")
        }
        crate::language::CalcExpr::UnaryNeg(inner) => ensure_static_calc_expr_supported(inner),
        crate::language::CalcExpr::Binary { lhs, rhs, .. } => {
            ensure_static_calc_expr_supported(lhs)?;
            ensure_static_calc_expr_supported(rhs)
        }
        crate::language::CalcExpr::Func { arg, .. } => ensure_static_calc_expr_supported(arg),
    }
}

fn ensure_static_nested_accessor_supported(
    accessor: &crate::language::NestedAccessor,
) -> Result<(), ErrMode<ContextError>> {
    match accessor {
        crate::language::NestedAccessor::Field(_)
        | crate::language::NestedAccessor::FieldArc(_) => Ok(()),
        crate::language::NestedAccessor::StaticSymbol(_) => {
            static_block_err("static block does not support referencing other static symbols")
        }
        crate::language::NestedAccessor::Direct(_)
        | crate::language::NestedAccessor::Collect(_) => {
            static_block_err("static block does not support read/take-based runtime access")
        }
        crate::language::NestedAccessor::Fun(_) => {
            static_block_err("static block does not support runtime functions")
        }
        crate::language::NestedAccessor::Pipe(_) => {
            static_block_err("static block does not support pipe operations")
        }
        crate::language::NestedAccessor::Map(op) => {
            for binding in op.subs() {
                ensure_static_nested_accessor_supported(binding.acquirer())?;
            }
            Ok(())
        }
        crate::language::NestedAccessor::ObjArray(op) => {
            for item in op.items() {
                ensure_static_precise_supported(item)?;
            }
            Ok(())
        }
    }
}

#[allow(dead_code)]
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

async fn finalize_static_blocks(
    model: &mut ObjModel,
    static_items: Vec<EvalExp>,
) -> Result<(), ErrMode<ContextError>> {
    if static_items.is_empty() {
        model.set_static_fields(HashMap::new());
        return Ok(());
    }

    let const_fields = materialize_static_items(&static_items).await?;
    rewrite_static_references(model, &const_fields)?;
    model.set_static_fields(const_fields);
    Ok(())
}

async fn materialize_static_items(
    items: &[EvalExp],
) -> Result<HashMap<String, Arc<DataField>>, ErrMode<ContextError>> {
    let mut cache = FieldQueryCache::default();
    let src = DataRecord::default();
    let mut dst = DataRecord::default();

    for exp in items {
        let mut src_ref = DataRecordRef::from(&src);
        exp.eval_proc_async(&mut src_ref, &mut dst, &mut cache)
            .await;
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

/// 在静态符号重写完成后，为每个单值求值器计算并缓存「是否可同步求值」。
/// 必须在 `rewrite_static_references` 之后调用：`StaticSymbol` 重写为 `ObjArc`
/// 后才是同步可求值的。
fn compute_sync_flags(model: &mut ObjModel) {
    for item in &mut model.items {
        if let EvalExp::Single(single) = item {
            let sync = single.eval_way().support_sync();
            single.set_sync(sync);
        }
    }
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
        PreciseEvaluator::Calc(op) => rewrite_calc_operation(op, const_fields),
        PreciseEvaluator::Match(op) => rewrite_match_operation(op, const_fields),
        PreciseEvaluator::Lookup(op) => rewrite_lookup_operation(op, const_fields),
        PreciseEvaluator::Pipe(pipe) => rewrite_pipe_operation(pipe, const_fields),
        PreciseEvaluator::Fun(fun) => rewrite_fun_operation(fun, const_fields),
        PreciseEvaluator::Map(map) => rewrite_map_operation(map, const_fields),
        PreciseEvaluator::ObjArray(arr) => rewrite_obj_array_operation(arr, const_fields),
        PreciseEvaluator::Fmt(op) => rewrite_fmt_operation(op, const_fields),
        PreciseEvaluator::Tdc(op) => rewrite_record_operation(op, const_fields),
        PreciseEvaluator::Collect(arr) => rewrite_arr_operation(arr, const_fields),
        _ => Ok(()),
    }
}

fn rewrite_calc_operation(
    op: &mut crate::language::CalcOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    rewrite_calc_expr(op.expr_mut(), const_fields)
}

fn rewrite_calc_expr(
    expr: &mut crate::language::CalcExpr,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    use crate::language::CalcExpr;

    match expr {
        CalcExpr::Const(_) => Ok(()),
        CalcExpr::Accessor(accessor) => rewrite_direct_accessor(accessor, const_fields),
        CalcExpr::UnaryNeg(inner) => rewrite_calc_expr(inner.as_mut(), const_fields),
        CalcExpr::Binary { lhs, rhs, .. } => {
            rewrite_calc_expr(lhs.as_mut(), const_fields)?;
            rewrite_calc_expr(rhs.as_mut(), const_fields)
        }
        CalcExpr::Func { arg, .. } => rewrite_calc_expr(arg.as_mut(), const_fields),
    }
}

fn rewrite_lookup_operation(
    op: &mut crate::language::LookupOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    rewrite_precise_evaluator(op.key_mut(), const_fields)?;
    rewrite_precise_evaluator(op.default_mut(), const_fields)?;

    let dict_field = const_fields.get(op.dict_symbol()).ok_or_else(|| {
        let mut err = ContextError::new();
        err.push(StrContext::Label("lookup_nocase"));
        err.push(StrContext::Label("static symbol not found"));
        ErrMode::Cut(err)
    })?;

    let Value::Obj(obj) = dict_field.get_value() else {
        let mut err = ContextError::new();
        err.push(StrContext::Label("lookup_nocase"));
        err.push(StrContext::Label("static symbol must be object"));
        return Err(ErrMode::Cut(err));
    };

    let mut dict = crate::language::LookupDict::new();
    for (key, value) in obj.iter() {
        dict.insert(
            key.as_str().trim().to_lowercase(),
            Arc::new(value.as_field().clone()),
        );
    }
    op.bind_compiled(dict);
    Ok(())
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
    rewrite_direct_accessor(op.dat_get_mut(), const_fields)?;
    if let Some(default) = op.default_val_mut() {
        rewrite_generic_accessor(default.accessor_mut(), const_fields)?;
    }
    Ok(())
}

fn rewrite_arr_operation(
    arr: &mut crate::language::ArrOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    rewrite_direct_accessor(&mut arr.dat_crate, const_fields)
}

fn rewrite_obj_array_operation(
    arr: &mut crate::language::ObjArrayOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    for item in arr.items_mut() {
        rewrite_precise_evaluator(item, const_fields)?;
    }
    Ok(())
}

fn rewrite_pipe_operation(
    op: &mut crate::language::PiPeOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match op.from_mut() {
        crate::language::PipeSource::Accessor(a) => rewrite_direct_accessor(a, const_fields),
        // access_direct 作管道源：static 块不支持（已由 ensure_static_precise_supported 拦截）
        crate::language::PipeSource::AccessDirect(_) => Ok(()),
    }
}

fn rewrite_fun_operation(
    _fun: &mut crate::language::FunOperation,
    _const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    Ok(())
}

fn rewrite_fmt_operation(
    op: &mut crate::language::FmtOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    for sub in op.subs_mut() {
        rewrite_record_operation(sub, const_fields)?;
    }
    Ok(())
}

fn rewrite_match_operation(
    op: &mut crate::language::MatchOperation,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    rewrite_match_source(op.dat_crate_mut(), const_fields)?;

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

fn rewrite_match_source(
    source: &mut crate::language::MatchSource,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match source {
        crate::language::MatchSource::Single(accessor) => {
            rewrite_direct_accessor(accessor, const_fields)
        }
        crate::language::MatchSource::Multi(accessors) => {
            for accessor in accessors.iter_mut() {
                rewrite_direct_accessor(accessor, const_fields)?;
            }
            Ok(())
        }
    }
}

fn rewrite_direct_accessor(
    accessor: &mut crate::language::DirectAccessor,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match accessor {
        crate::language::DirectAccessor::Take(take) => {
            if let Some(default) = take.default_acq.as_mut() {
                rewrite_generic_accessor(default, const_fields)?;
            }
        }
        crate::language::DirectAccessor::Read(read) => {
            if let Some(default) = read.default_acq.as_mut() {
                rewrite_generic_accessor(default, const_fields)?;
            }
        }
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
        MatchCondition::Multi(conds) => {
            for c in conds.iter_mut() {
                rewrite_single_match_cond(c, const_fields)?;
            }
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
        MatchCond::Or(alternatives) => {
            for alt in alternatives.iter_mut() {
                rewrite_single_match_cond(alt, const_fields)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn rewrite_nested_accessor(
    accessor: &mut crate::language::NestedAccessor,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match accessor {
        crate::language::NestedAccessor::StaticSymbol(sym) => {
            let field = const_fields.get(sym).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("static reference"));
                err.push(StrContext::Label("symbol not found"));
                ErrMode::Cut(err)
            })?;
            accessor.replace_with_field_arc(Arc::clone(field));
        }
        crate::language::NestedAccessor::Direct(op) => {
            rewrite_record_operation(op, const_fields)?;
        }
        crate::language::NestedAccessor::Collect(arr) => {
            rewrite_arr_operation(arr, const_fields)?;
        }
        crate::language::NestedAccessor::Map(op) => {
            rewrite_map_operation(op, const_fields)?;
        }
        crate::language::NestedAccessor::ObjArray(op) => {
            rewrite_obj_array_operation(op, const_fields)?;
        }
        crate::language::NestedAccessor::Pipe(op) => {
            rewrite_pipe_operation(op, const_fields)?;
        }
        crate::language::NestedAccessor::Field(_)
        | crate::language::NestedAccessor::FieldArc(_)
        | crate::language::NestedAccessor::Fun(_) => {}
    }
    Ok(())
}

fn rewrite_generic_accessor(
    accessor: &mut crate::language::GenericAccessor,
    const_fields: &HashMap<String, Arc<DataField>>,
) -> Result<(), ErrMode<ContextError>> {
    match accessor {
        crate::language::GenericAccessor::StaticSymbol(sym) => {
            let field = const_fields.get(sym).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("static reference"));
                err.push(StrContext::Label("symbol not found"));
                ErrMode::Cut(err)
            })?;
            accessor.replace_with_field_arc(Arc::clone(field));
        }
        crate::language::GenericAccessor::Field(_)
        | crate::language::GenericAccessor::FieldArc(_)
        | crate::language::GenericAccessor::Fun(_) => {}
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
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_conf::{oml_parse_raw, oml_parse_syntax_raw};
    use crate::parser::utils::for_test::{assert_oml_parse, assert_oml_parse_ext};
    use wp_primitives::Parser;
    use wp_primitives::WResult as ModalResult;
    use wp_primitives::comment::CommentParser;

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_sample() -> ModalResult<()> {
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
        assert_oml_parse(&mut code, oml_parse_syntax_raw);
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
        assert_oml_parse(&mut code, oml_parse_syntax_raw);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_fun() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = Now::time() ;
version      : chars   = Now::time() ;
        "#;
        assert_oml_parse(&mut code, oml_parse_syntax_raw);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_pipe() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = pipe take() | base64_encode  ;
version      : chars   = pipe take(ip) | to_str |  base64_encode ;
        "#;
        assert_oml_parse(&mut code, oml_parse_syntax_raw);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_pipe_optional_keyword() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test pipe without 'pipe' keyword - should parse successfully
        let mut code = r#"
name : test
---
url_secure = take(url) | starts_with('https://') | map_to(true) ;
encoded = read(data) | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).await.assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        // Test mixed usage: with and without 'pipe' keyword
        let mut code = r#"
name : test
---
version1 = pipe take(ip) | to_str | base64_encode ;
version2 = take(ip) | to_str | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).await.assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_block_parsing() -> ModalResult<()> {
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

        let model = oml_parse_raw(&mut code).await?;
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_sync_flag_computed_after_static_rewrite() -> ModalResult<()> {
        use crate::language::EvalExp;

        // 普通 read：同步。
        let mut code = r#"
name : sync_read
---
a = read(__sip) ;
"#;
        let model = oml_parse_raw(&mut code).await?;
        match &model.items[0] {
            EvalExp::Single(single) => assert!(*single.sync()),
            _ => panic!("expected single evaluator"),
        }

        // 静态符号引用会被重写为 ObjArc，重写完成后同样应判定为同步。
        let mut code = r#"
name : sync_static
---
static {
    x = chars(hello);
}
a = x ;
"#;
        let model = oml_parse_raw(&mut code).await?;
        match &model.items[0] {
            EvalExp::Single(single) => assert!(*single.sync()),
            _ => panic!("expected single evaluator"),
        }

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_in_map_binding() -> ModalResult<()> {
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

        let model = oml_parse_raw(&mut code).await?;
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_in_default_binding() -> ModalResult<()> {
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

        let model = oml_parse_raw(&mut code).await?;
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_in_nested_default_binding() -> ModalResult<()> {
        use crate::language::{EvalExp, GenericAccessor, NestedAccessor, PreciseEvaluator};

        let mut code = r#"
name : test
---
static {
    fallback = object {
        id = chars(E1);
    };
}

payload = object {
    value = take(Value) { _ : fallback };
};
        "#;

        let model = oml_parse_raw(&mut code).await?;
        assert_eq!(model.static_fields().len(), 1);
        match &model.items[0] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::Map(op) => match op.subs()[0].acquirer() {
                    NestedAccessor::Direct(record) => {
                        let default = record.default_val().as_ref().expect("default binding");
                        match default.accessor() {
                            GenericAccessor::FieldArc(field) => {
                                assert_eq!(field.get_name(), "fallback");
                            }
                            other => panic!("expected field arc accessor, got {:?}", other),
                        }
                    }
                    other => panic!("expected direct accessor, got {:?}", other),
                },
                other => panic!("unexpected evaluator: {:?}", other),
            },
            _ => panic!("expected single evaluator"),
        }
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_in_match_cases() -> ModalResult<()> {
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

        let model = oml_parse_raw(&mut code).await?;
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_lookup_nocase_requires_static_object() {
        let mut code = r#"
name : test
---
static {
    score = chars(foo);
}

risk_score : float = lookup_nocase(score, read(status), 40.0);
        "#;

        assert!(oml_parse_raw(&mut code).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_block_rejects_static_symbol_reference() {
        let mut code = r#"
name : test
---
static {
    fallback = chars(foo);
    value = fallback;
}

result = read(value);
        "#;

        let err = oml_parse_raw(&mut code)
            .await
            .expect_err("static-in-static should fail");
        let msg = err.to_string();
        assert!(msg.contains("static block"));
        assert!(msg.contains("does not support referencing other static symbols"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_block_rejects_runtime_read_accessor() {
        let mut code = r#"
name : test
---
static {
    fallback = chars(foo);
    value = read(input) { _ : fallback };
}

result = read(value);
        "#;

        let err = oml_parse_raw(&mut code)
            .await
            .expect_err("runtime accessor in static block should fail");
        let msg = err.to_string();
        assert!(msg.contains("static block"));
        assert!(msg.contains("read/take-based runtime access"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_block_rejects_sql_effect() {
        let mut code = r#"
name : test
---
static {
    value = select name from some_table where id = 1;
}

result = read(value);
        "#;

        let err = oml_parse_raw(&mut code)
            .await
            .expect_err("sql in static block should fail");
        let msg = err.to_string();
        assert!(msg.contains("static block"));
        assert!(msg.contains("SQL queries or external effects"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_static_symbol_ctx_cleared_after_parse_failure() {
        let mut bad = r#"
name : bad
---
static {
    fallback = chars(foo);
    value = read(input) { _ : fallback };
}

result = read(value);
        "#;
        assert!(oml_parse_raw(&mut bad).await.is_err());

        let mut next = r#"
name : next
---
value = fallback;
        "#;
        assert!(oml_parse_raw(&mut next).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_fmt() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      :chars   = fmt("_{}*{}",@ip,@sys)  ;
        "#;
        oml_parse_syntax_raw.parse_next(&mut code)?;
        //assert_oml_parse(&mut code, oml_conf);
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn test_conf2() -> ModalResult<()> {
        let mut code = r#"
name : test
---
values : obj = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
citys : array = collect take( keys : [ a,b,c* ] ) ;
        "#;
        let model = oml_parse_syntax_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn test_conf3() -> ModalResult<()> {
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
        let model = oml_parse_syntax_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf4() -> ModalResult<()> {
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
        let model = oml_parse_syntax_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_comment() -> ModalResult<()> {
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
        assert_oml_parse_ext(&mut pure_code, oml_parse_syntax_raw, expect);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_conf_quoted_chars() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test that chars() supports both quoted and unquoted strings
        let mut code1 = r#"
name : test
---
msg1 = chars('hello world');
msg2 = chars("goodbye");
msg3 = chars(unquoted);
        "#;
        let model = oml_parse_raw(&mut code1).await.assert();
        assert_eq!(model.name(), "test");

        // Test with special characters
        let mut code2 = r#"
name : test
---
msg = chars('hello\nworld');
        "#;
        let model2 = oml_parse_raw(&mut code2).await.assert();
        assert_eq!(model2.name(), "test");

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_temp_field_filter() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;
        use wp_knowledge::cache::FieldQueryCache;
        use wp_model_core::model::{DataRecord, DataType};

        // Test that fields starting with "__" are converted to ignore type
        let mut code = r#"
name : test
---
__temp = chars(temporary);
result = chars(final);
__another_temp = chars(also_temp);
        "#;
        let model = oml_parse_raw(&mut code).await.assert();
        assert_eq!(model.name(), "test");

        // Transform with empty input
        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform_async(input, cache).await;

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_temp_field_in_computation() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;
        use wp_knowledge::cache::FieldQueryCache;
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
        let model = oml_parse_raw(&mut code).await.assert();

        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform_async(input, cache).await;

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_temp_field_flag() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test case 1: Model with no temporary fields
        let mut code_no_temp = r#"
name : test
---
normal1 = chars(value1);
normal2 = chars(value2);
        "#;
        let model_no_temp = oml_parse_raw(&mut code_no_temp).await.assert();
        assert!(
            !model_no_temp.has_temp_fields(),
            "Should not have temp fields flag"
        );

        // Test case 2: Model with temporary fields
        let mut code_with_temp = r#"
name : test
---
__temp = chars(temp_value);
normal = chars(normal_value);
        "#;
        let model_with_temp = oml_parse_raw(&mut code_with_temp).await.assert();
        assert!(
            model_with_temp.has_temp_fields(),
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
        let model_multi_temp = oml_parse_raw(&mut code_multi_temp).await.assert();
        assert!(
            model_multi_temp.has_temp_fields(),
            "Should have temp fields flag"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_enable_config_default() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test case 1: Default enable (no enable config)
        let mut code_default = r#"
name : test
---
field = chars(value);
        "#;
        let model_default = oml_parse_raw(&mut code_default).await.assert();
        assert!(*model_default.enable(), "Default enable should be true");

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_enable_config_true() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test case 2: Explicit enable = true
        let mut code_enable_true = r#"
name : test
enable : true
---
field = chars(value);
        "#;
        let model_enable_true = oml_parse_raw(&mut code_enable_true).await.assert();
        assert!(*model_enable_true.enable(), "Explicit enable true");

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_enable_config_false() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test case 3: Explicit enable = false
        let mut code_enable_false = r#"
name : test
enable : false
---
field = chars(value);
        "#;
        let model_enable_false = oml_parse_raw(&mut code_enable_false).await.assert();
        assert!(!*model_enable_false.enable(), "Explicit enable false");

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_enable_config_with_rule() -> ModalResult<()> {
        use orion_error::dev::testing::TestAssert;

        // Test case 4: enable with rule
        let mut code_with_rule = r#"
name : test
rule : /test/*
enable : false
---
field = chars(value);
        "#;
        let model_with_rule = oml_parse_raw(&mut code_with_rule).await.assert();
        assert!(!*model_with_rule.enable(), "enable with rule");

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_enable_explicit() -> ModalResult<()> {
        use crate::parser::oml_conf::oml_conf_enable;

        // Test parsing enable directly
        let mut enable_str = "enable : true ";
        let result = oml_conf_enable(&mut enable_str);
        assert!(result.is_ok(), "Should parse enable: {:?}", result);
        assert!(result.unwrap());

        let mut enable_false = "enable : false ";
        let result = oml_conf_enable(&mut enable_false);
        assert!(result.is_ok());
        assert!(!result.unwrap());

        Ok(())
    }

    // issue #348: 完整文件回归 —— 嵌套 object + pipe 成员，后续兄弟字段全部保留
    #[test]
    fn test_issue_348_nested_object_pipe_tail() -> ModalResult<()> {
        use crate::language::{EvalExp, NestedAccessor, PreciseEvaluator};

        let mut code = r#"
name : nested_object_tail
rule : issue_probe/nested_object_tail
---
input_name = read(name);
parser_bug_probe = object {
    before = chars(before);
    rule = object {
        signature_id = pipe read(missing_rule_ids) | nth(0);
    };
    after = chars(after);
    nested_after = object {
        value = chars(also_after);
    };
};
"#;
        let model = oml_parse_syntax_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2, "input_name + parser_bug_probe");
        match &model.items[1] {
            EvalExp::Single(single) => match single.eval_way() {
                PreciseEvaluator::Map(map) => {
                    let names: Vec<String> =
                        map.subs().iter().map(|b| b.target().safe_name()).collect();
                    assert_eq!(names, vec!["before", "rule", "after", "nested_after"]);
                    match map.subs()[1].acquirer() {
                        NestedAccessor::Map(rule) => {
                            let inner: Vec<String> =
                                rule.subs().iter().map(|b| b.target().safe_name()).collect();
                            assert_eq!(inner, vec!["signature_id"]);
                            assert!(
                                matches!(rule.subs()[0].acquirer(), NestedAccessor::Pipe(_)),
                                "nested pipe member must be a Pipe accessor"
                            );
                        }
                        other => panic!("expected nested Map, got {:?}", other),
                    }
                }
                other => panic!("expected Map, got {:?}", other),
            },
            other => panic!("expected Single, got {:?}", other),
        }
        Ok(())
    }

    // issue #348: 非法 object 成员必须使整个 OML 校验失败，而不是静默丢弃
    #[test]
    fn test_issue_348_invalid_member_rejected() -> ModalResult<()> {
        let mut code = r#"
name : bad
---
probe = object {
    before = chars(before);
    after = ;
};
"#;
        let result = oml_parse_syntax_raw.parse_next(&mut code);
        assert!(
            result.is_err(),
            "invalid object member must fail the whole OML validation"
        );
        println!("err: {}", result.unwrap_err());
        Ok(())
    }

    // issue #348: static 块内嵌套 object 使用 pipe 应被明确拒绝（而非 panic）
    #[test]
    fn test_issue_348_static_block_pipe_rejected() -> ModalResult<()> {
        let mut code = r#"
name : static_pipe
---
static {
    tpl = object {
        sig = pipe read(x) | to_str;
    };
}
result = read(tpl);
"#;
        let result = oml_parse_syntax_raw.parse_next(&mut code);
        assert!(
            result.is_err(),
            "static 块不允许 pipe 操作，应报错而非静默接受/panic"
        );
        Ok(())
    }

    // issue #348: 多目标列表容忍逗号前后空白（`a , b`），完整文件被消费
    #[test]
    fn test_issue_348_multi_target_space_before_comma() -> ModalResult<()> {
        let mut code = r#"
name : multi
---
values : obj = object {
    cpu_free, memory_free , cpu_used : digit = take();
    process, disk_free , disk_used : digit = take();
};
"#;
        oml_parse_syntax_raw.parse_next(&mut code)?;
        assert!(code.trim().is_empty(), "完整文件应被消费");
        Ok(())
    }

    // 同类问题：read()/take() 括号内非法参数此前被静默丢弃，现在必须整体失败
    #[test]
    fn test_issue_348_read_take_args_rejected() -> ModalResult<()> {
        // 顶层 read() 参数含垃圾
        let mut code = r#"
name : t
---
x = read(option : [src_ip] @@garbage@@);
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "read() 非法参数必须使整个 OML 校验失败"
        );

        // object 成员内 take() 参数含垃圾
        let mut code = r#"
name : t
---
probe = object {
    x = take(keys : [a, b] @@garbage@@);
    after = chars(after);
};
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "object 成员内 take() 非法参数必须使整个 OML 校验失败"
        );

        // 合法参数形式不受影响
        let mut code = r#"
name : t
---
probe = object {
    x = read(option : [src_ip]);
    y = take(keys : [a, b], in : [c]);
};
"#;
        oml_parse_syntax_raw.parse_next(&mut code)?;
        assert!(code.trim().is_empty());
        Ok(())
    }

    // read()/take() 参数静默丢弃：垃圾参数位于不同位置、合法边界形式
    #[test]
    fn test_issue_348_read_take_args_more_forms() -> ModalResult<()> {
        // 垃圾在参数开头
        let mut code = r#"
name : t
---
x = read(@@garbage@@ option : [src_ip]);
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "垃圾参数在开头应失败"
        );

        // 垃圾在参数中间
        let mut code = r#"
name : t
---
x = read(option : [src_ip] @@garbage@@ keys : [a]);
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "垃圾参数在中间应失败"
        );

        // take() 全部为垃圾
        let mut code = r#"
name : t
---
x = take(@@);
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "take() 全垃圾应失败"
        );

        // pipe 源内的 read() 参数含垃圾（管道源也走 oml_read 校验）
        let mut code = r#"
name : t
---
x = pipe read(option : [src_ip] @@garbage@@) | to_str;
"#;
        assert!(
            oml_parse_syntax_raw.parse_next(&mut code).is_err(),
            "pipe 源内 read() 非法参数应失败"
        );

        // 合法边界形式：尾逗号、尾空白、多参数
        let mut code = r#"
name : t
---
x = read(option : [src_ip] ,);
y = take(keys : [a, b] );
z = read(keys : [a, b], in : [c]);
w = read(/a/b/[0]/1);
"#;
        oml_parse_syntax_raw.parse_next(&mut code)?;
        assert!(code.trim().is_empty());
        Ok(())
    }
}
