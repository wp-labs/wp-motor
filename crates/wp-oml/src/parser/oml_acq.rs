use super::tdc_prm::{oml_aga_tdc, oml_aga_value, oml_sql_raw};
use crate::language::CondAccessor;
use crate::language::PreciseEvaluator;
use crate::language::{GenericAccessor, NestedAccessor};
use crate::language::{SqlFnArg, SqlFnExpr};
use crate::parser::fun_prm::oml_gw_fun;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt, trace};
use winnow::error::{ContextError, ErrMode};
use winnow::stream::Stream;
use winnow::{ModalResult, Parser};
use wp_parser::atom::take_var_name;
use wp_parser::symbol::symbol_semicolon;

pub fn oml_gens_acq(data: &mut &str) -> ModalResult<GenericAccessor> {
    let gw = alt((
        trace("get take:", oml_aga_tdc),
        trace("get fun:", oml_gw_fun),
        trace("get value:", oml_aga_value),
    ))
    .parse_next(data)?;
    opt(symbol_semicolon).parse_next(data)?;
    let sub_gw = match gw {
        PreciseEvaluator::Obj(x) => GenericAccessor::Field(x),
        PreciseEvaluator::Fun(x) => GenericAccessor::Fun(x),
        _ => {
            unreachable!("not support to gens aggregate")
        }
    };
    Ok(sub_gw)
}

pub fn oml_sub_acq(data: &mut &str) -> ModalResult<NestedAccessor> {
    let gw = alt((
        trace("get take:", oml_aga_tdc),
        trace("get fun:", oml_gw_fun),
        trace("get value:", oml_aga_value),
    ))
    .parse_next(data)?;
    opt(symbol_semicolon).parse_next(data)?;
    let sub_gw = match gw {
        PreciseEvaluator::Obj(x) => NestedAccessor::Field(x),
        PreciseEvaluator::Tdc(x) => NestedAccessor::Direct(x),
        PreciseEvaluator::Fun(x) => NestedAccessor::Fun(x),
        _ => {
            unreachable!("not support to sub aggregate")
        }
    };
    Ok(sub_gw)
}
pub fn oml_cond_acq(data: &mut &str) -> ModalResult<CondAccessor> {
    // Prefer SQL function call; otherwise map known evaluators to CondAccessor
    // Try SQL function first (returns CondAccessor directly)
    if let Ok(acc) = oml_sql_fn.parse_peek(data) {
        let _ = oml_sql_fn.parse_next(data)?;
        return Ok(acc.1);
    }
    if let Ok((_, PreciseEvaluator::Tdc(x))) = oml_aga_tdc.parse_peek(data) {
        let _ = oml_aga_tdc.parse_next(data)?;
        return Ok(CondAccessor::Tdc(x));
    }
    if let Ok((_, PreciseEvaluator::Fun(x))) = oml_gw_fun.parse_peek(data) {
        let _ = oml_gw_fun.parse_next(data)?;
        return Ok(CondAccessor::Fun(x));
    }
    if let Ok((_, PreciseEvaluator::Val(x))) = oml_sql_raw.parse_peek(data) {
        let _ = oml_sql_raw.parse_next(data)?;
        return Ok(CondAccessor::Val(x));
    }
    // fallback
    oml_sql_fn.parse_next(data)
}

/// Parse SQL function call usable in WHERE:
/// name(arg1, arg2, ...)
/// args can be: read(...)/take(...), numeric/'string' literal, or column identifier
fn oml_sql_fn(data: &mut &str) -> ModalResult<CondAccessor> {
    let cp = data.checkpoint();
    // function name
    let name = take_var_name.parse_next(data)?;
    // must be followed by '('
    let _ = multispace0.parse_next(data)?;
    if wp_parser::symbol::symbol_bracket_beg
        .parse_next(data)
        .is_err()
    {
        data.reset(&cp);
        return Err(ErrMode::Backtrack(ContextError::new()));
    }
    // parse args until ')'
    let mut args: Vec<SqlFnArg> = Vec::new();
    loop {
        multispace0.parse_next(data)?;
        // Try read/take first
        if let Ok((_, PreciseEvaluator::Tdc(o))) = oml_aga_tdc.parse_peek(data) {
            let _ = oml_aga_tdc.parse_next(data)?;
            args.push(SqlFnArg::Param(Box::new(CondAccessor::Tdc(o))));
        } else if let Ok((_, PreciseEvaluator::Val(v))) = oml_sql_raw.parse_peek(data) {
            let _ = oml_sql_raw.parse_next(data)?;
            args.push(SqlFnArg::Literal(v));
        } else if let Ok(ident) = take_var_name.parse_next(data) {
            args.push(SqlFnArg::Column(ident.to_string()));
        } else {
            // empty or unexpected
        }
        multispace0.parse_next(data)?;
        // if next is ',', continue; if ')', break
        if wp_parser::symbol::symbol_bracket_end
            .parse_peek(data)
            .is_ok()
        {
            wp_parser::symbol::symbol_bracket_end.parse_next(data)?;
            break;
        } else if wp_parser::symbol::symbol_comma.parse_peek(data).is_ok() {
            wp_parser::symbol::symbol_comma.parse_next(data)?;
            continue;
        } else {
            // tolerate missing commas if immediately before ')'
            wp_parser::symbol::symbol_bracket_end.parse_next(data)?;
            break;
        }
    }
    Ok(CondAccessor::SqlFn(SqlFnExpr {
        name: name.to_string(),
        args,
    }))
}
