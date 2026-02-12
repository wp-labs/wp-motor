use crate::language::NestedAccessor;
use crate::language::{MatchCase, MatchCond};
use crate::language::{MatchCondition, MatchSource, PreciseEvaluator};
use crate::language::{MatchFun, MatchOperation};
use crate::parser::collect_prm::oml_aga_collect;
use crate::parser::keyword::{kw_gw_match, kw_in};
use crate::parser::oml_aggregate::oml_crate_calc_ref;
use crate::parser::static_ctx::parse_static_value;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt, peek, repeat};
use winnow::error::{ContextError, StrContext, StrContextValue};
use winnow::stream::Stream;
use winnow::token::take;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;
use wp_parser::symbol::{
    symbol_brace_beg, symbol_brace_end, symbol_comma, symbol_marvel, symbol_match_to,
    symbol_semicolon, symbol_under_line,
};
use wp_parser::utils::get_scope;
use wpl::parser::utils::quot_str;

use super::syntax;
use super::tdc_prm::{oml_aga_tdc, oml_aga_value};

fn match_cond1(data: &mut &str) -> WResult<MatchCond> {
    multispace0.parse_next(data)?;
    // Try cond_fun before cond_in to allow functions like in_range, in_*
    alt((cond_neq, cond_fun, cond_in, cond_eq)).parse_next(data)
}

fn match_cond1_item(data: &mut &str) -> WResult<MatchCase> {
    multispace0.parse_next(data)?;
    let cond = match_cond1.parse_next(data)?;
    let calc = match_calc_target.parse_next(data)?;

    Ok(MatchCase::new(MatchCondition::Single(cond), calc))
}

fn match_cond_default_item(data: &mut &str) -> WResult<MatchCase> {
    multispace0.parse_next(data)?;
    let _ = cond_default.parse_next(data)?;
    let calc = match_calc_target.parse_next(data)?;

    Ok(MatchCase::new(MatchCondition::Default, calc))
}

fn match_cond2_item(data: &mut &str) -> WResult<MatchCase> {
    multispace0.parse_next(data)?;
    let cond = match_cond2
        .context(ctx_desc(">> (<match_value>,<match_value>) "))
        .parse_next(data)?;
    let calc = match_calc_target.parse_next(data)?;

    Ok(MatchCase::new(cond, calc))
}

fn match_cond3_item(data: &mut &str) -> WResult<MatchCase> {
    multispace0.parse_next(data)?;
    let cond = match_cond3
        .context(ctx_desc(">> (<match_value>,<match_value>,<match_value>) "))
        .parse_next(data)?;
    let calc = match_calc_target.parse_next(data)?;

    Ok(MatchCase::new(cond, calc))
}

fn match_cond4_item(data: &mut &str) -> WResult<MatchCase> {
    multispace0.parse_next(data)?;
    let cond = match_cond4
        .context(ctx_desc(
            ">> (<match_value>,<match_value>,<match_value>,<match_value>) ",
        ))
        .parse_next(data)?;
    let calc = match_calc_target.parse_next(data)?;

    Ok(MatchCase::new(cond, calc))
}

fn match_calc_target(data: &mut &str) -> WResult<NestedAccessor> {
    symbol_match_to.parse_next(data)?;
    let gw = alt((
        oml_aga_tdc,
        oml_aga_value,
        oml_aga_collect,
        parse_static_value,
    ))
    .parse_next(data)?;
    opt(symbol_comma).parse_next(data)?;
    opt(symbol_semicolon).parse_next(data)?;
    let sub_gw = match gw {
        PreciseEvaluator::Obj(x) => NestedAccessor::Field(x),
        PreciseEvaluator::Tdc(x) => NestedAccessor::Direct(x),
        PreciseEvaluator::Collect(x) => NestedAccessor::Collect(x),
        PreciseEvaluator::StaticSymbol(sym) => NestedAccessor::StaticSymbol(sym),
        _ => {
            unreachable!("not support to match item")
        }
    };
    Ok(sub_gw)
}

fn match_cond2(data: &mut &str) -> WResult<MatchCondition> {
    multispace0.parse_next(data)?;
    let code = get_scope(data, '(', ')')?;
    let mut code_data: &str = code;

    let fst = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let sec = match_cond1.parse_next(&mut code_data)?;
    Ok(MatchCondition::Double(fst, sec))
}

fn match_cond3(data: &mut &str) -> WResult<MatchCondition> {
    multispace0.parse_next(data)?;
    let code = get_scope(data, '(', ')')?;
    let mut code_data: &str = code;

    let a = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let b = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let c = match_cond1.parse_next(&mut code_data)?;
    Ok(MatchCondition::Triple(a, b, c))
}

fn match_cond4(data: &mut &str) -> WResult<MatchCondition> {
    multispace0.parse_next(data)?;
    let code = get_scope(data, '(', ')')?;
    let mut code_data: &str = code;

    let a = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let b = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let c = match_cond1.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let d = match_cond1.parse_next(&mut code_data)?;
    Ok(MatchCondition::Quadruple(a, b, c, d))
}

fn cond_eq(data: &mut &str) -> WResult<MatchCond> {
    multispace0.parse_next(data)?;

    // Try parsing static symbol first
    let cp = data.checkpoint();
    if let Ok(PreciseEvaluator::StaticSymbol(sym)) = parse_static_value(data) {
        return Ok(MatchCond::EqSym(sym));
    }
    data.reset(&cp);

    // Fall back to value expression
    let tdo = syntax::oml_value.parse_next(data)?;
    Ok(MatchCond::Eq(tdo))
}

fn cond_default(data: &mut &str) -> WResult<MatchCond> {
    multispace0.parse_next(data)?;
    symbol_under_line.parse_next(data)?;
    Ok(MatchCond::Default)
}
fn cond_neq(data: &mut &str) -> WResult<MatchCond> {
    symbol_marvel.parse_next(data)?;

    // Try parsing static symbol first
    let cp = data.checkpoint();
    if let Ok(PreciseEvaluator::StaticSymbol(sym)) = parse_static_value(data) {
        return Ok(MatchCond::NeqSym(sym));
    }
    data.reset(&cp);

    // Fall back to value expression
    let tdo = syntax::oml_value.parse_next(data)?;
    Ok(MatchCond::Neq(tdo))
}
fn cond_in(data: &mut &str) -> WResult<MatchCond> {
    let _ = multispace0.parse_next(data)?;
    kw_in.parse_next(data)?;

    // Extract the scope once
    let scope = get_scope(data, '(', ')')?;
    let mut code: &str = scope;

    // Try parsing both as static symbols
    let cp1 = code.checkpoint();
    if let Ok(PreciseEvaluator::StaticSymbol(beg_sym)) = parse_static_value(&mut code)
        && symbol_comma.parse_next(&mut code).is_ok()
    {
        // Try second element as symbol
        let cp2 = code.checkpoint();
        if let Ok(PreciseEvaluator::StaticSymbol(end_sym)) = parse_static_value(&mut code) {
            return Ok(MatchCond::InSym(beg_sym, end_sym));
        }
        code.reset(&cp2);
    }
    code.reset(&cp1);

    // Fall back to value expressions (reuse the same scope)
    let beg_tdo = syntax::oml_value.parse_next(&mut code)?;
    symbol_comma.parse_next(&mut code)?;
    let end_tdo = syntax::oml_value.parse_next(&mut code)?;
    Ok(MatchCond::In(beg_tdo, end_tdo))
}

/// Parse function-based match condition like `starts_with('prefix')`
fn cond_fun(data: &mut &str) -> WResult<MatchCond> {
    use winnow::token::take_while;

    multispace0.parse_next(data)?;

    // Try to parse function name followed by '('
    let cp = data.checkpoint();

    // Parse function name (identifier: letters, digits, underscore)
    // Must start with a letter
    let first_char = peek(take::<usize, &str, ContextError>(1usize)).parse_next(data);
    if !first_char
        .map(|s| s.chars().next().unwrap().is_ascii_alphabetic())
        .unwrap_or(false)
    {
        data.reset(&cp);
        return winnow::combinator::fail.parse_next(data);
    }

    let fun_name: &str =
        take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_').parse_next(data)?;

    // Check if this is a known match function
    // If not, reset and fail so other parsers can try
    const KNOWN_MATCH_FUNCTIONS: &[&str] = &[
        "starts_with",
        "ends_with",
        "contains",
        "regex_match",
        "is_empty",
        "iequals",
        "gt",
        "lt",
        "eq",
        "in_range",
    ];

    if !KNOWN_MATCH_FUNCTIONS.contains(&fun_name) {
        data.reset(&cp);
        return winnow::combinator::fail.parse_next(data);
    }

    multispace0.parse_next(data)?;

    // Must be followed by '(' to be a function call
    let next_char = peek(take::<usize, &str, ContextError>(1usize)).parse_next(data);
    if !next_char.map(|s| s == "(").unwrap_or(false) {
        data.reset(&cp);
        return winnow::combinator::fail.parse_next(data);
    }

    // Parse arguments in parentheses
    let arg_str = get_scope(data, '(', ')')?;

    // Extract argument values (can be multiple, comma-separated)
    let args = if !arg_str.trim().is_empty() {
        let mut args_vec = Vec::new();
        let mut arg_data = arg_str.trim();

        loop {
            multispace0.parse_next(&mut arg_data)?;

            // Try to parse as quoted string first
            if let Ok(quoted) = quot_str.parse_next(&mut arg_data) {
                args_vec.push(quoted.to_string());
            } else {
                // Try to parse as unquoted number or identifier
                use winnow::token::take_while;
                let unquoted: &str = take_while(1.., |c: char| {
                    c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'
                })
                .parse_next(&mut arg_data)?;
                args_vec.push(unquoted.to_string());
            }

            multispace0.parse_next(&mut arg_data)?;

            // Check for comma (more arguments)
            if symbol_comma.parse_next(&mut arg_data).is_ok() {
                continue;
            } else {
                break;
            }
        }

        args_vec
    } else {
        Vec::new()
    };

    Ok(MatchCond::Fun(MatchFun::new_with_args(fun_name, args)))
}
pub fn oml_match(data: &mut &str) -> WResult<MatchOperation> {
    let _ = multispace0.parse_next(data)?;
    kw_gw_match.parse_next(data)?;
    let _ = multispace0.parse_next(data)?;
    let oct = oml_crate_calc_ref.parse_next(data)?;
    let (item, default) = match &oct {
        MatchSource::Single(_) => oml_match1_body
            .context(ctx_desc(">> { *<match_item> }"))
            .parse_next(data)?,
        MatchSource::Double(_, _) => oml_match2_body
            .context(ctx_desc(">> { *<match_item> }"))
            .parse_next(data)?,
        MatchSource::Triple(_, _, _) => oml_match3_body
            .context(ctx_desc(">> { *<match_item> }"))
            .parse_next(data)?,
        MatchSource::Quadruple(_, _, _, _) => oml_match4_body
            .context(ctx_desc(">> { *<match_item> }"))
            .parse_next(data)?,
    };
    Ok(MatchOperation::new(oct, item, default))
}

pub fn oml_match1_body(data: &mut &str) -> WResult<(Vec<MatchCase>, Option<MatchCase>)> {
    let _ = multispace0.parse_next(data)?;
    symbol_brace_beg.parse_next(data)?;
    let item = repeat(1.., match_cond1_item).parse_next(data)?;
    let default = opt(match_cond_default_item).parse_next(data)?;
    symbol_brace_end.parse_next(data)?;
    Ok((item, default))
}

pub fn oml_match2_body(data: &mut &str) -> WResult<(Vec<MatchCase>, Option<MatchCase>)> {
    let _ = multispace0.parse_next(data)?;
    symbol_brace_beg.parse_next(data)?;
    let item = repeat(1.., match_cond2_item).parse_next(data)?;
    let default = opt(match_cond_default_item).parse_next(data)?;
    //.err_reset(data, &cp)?
    symbol_brace_end.parse_next(data)?;
    Ok((item, default))
}

pub fn oml_match3_body(data: &mut &str) -> WResult<(Vec<MatchCase>, Option<MatchCase>)> {
    let _ = multispace0.parse_next(data)?;
    symbol_brace_beg.parse_next(data)?;
    let item = repeat(1.., match_cond3_item).parse_next(data)?;
    let default = opt(match_cond_default_item).parse_next(data)?;
    symbol_brace_end.parse_next(data)?;
    Ok((item, default))
}

pub fn oml_match4_body(data: &mut &str) -> WResult<(Vec<MatchCase>, Option<MatchCase>)> {
    let _ = multispace0.parse_next(data)?;
    symbol_brace_beg.parse_next(data)?;
    let item = repeat(1.., match_cond4_item).parse_next(data)?;
    let default = opt(match_cond_default_item).parse_next(data)?;
    symbol_brace_end.parse_next(data)?;
    Ok((item, default))
}

pub fn oml_aga_match(data: &mut &str) -> WResult<PreciseEvaluator> {
    let obj = oml_match
        .context(StrContext::Label("method"))
        .context(StrContext::Expected(StrContextValue::StringLiteral(
            ">> match <crate> {...}",
        )))
        .parse_next(data)?;
    Ok(PreciseEvaluator::Match(obj))
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_error::TestAssert;
    use wp_model_core::model::{DataField, FieldStorage};

    use wp_parser::WResult as ModalResult;

    use crate::language::MatchCase;
    use crate::parser::match_prm::{
        match_cond1_item, match_cond2_item, match_cond3_item, match_cond4_item, oml_aga_match,
    };
    use crate::parser::utils::for_test::assert_oml_parse;
    use crate::types::AnyResult;

    #[test]
    fn test_match_item() -> AnyResult<()> {
        let mut code = r#"chars(3) => chars(高危(漏洞));"#;
        let x = match_cond1_item(&mut code).assert();
        println!("{:?}", x);
        assert_eq!(x, MatchCase::eq_const("chars", "3", "高危(漏洞)")?);

        let mut code = r#"chars(A) => chars(5),"#;
        let x = match_cond1_item(&mut code).assert();
        println!("{:?}", x);
        assert_eq!(x, MatchCase::eq_const("chars", "A", "5")?);

        let mut code = r#"ip(127.0.0.1) => ip(10.0.0.1)"#;
        let x = match_cond1_item(&mut code).assert();
        println!("{:?}", x);
        assert_eq!(x, MatchCase::eq_const("ip", "127.0.0.1", "10.0.0.1")?);

        let mut code = r#"(ip(127.0.0.1),ip(127.0.0.100)) => ip(10.0.0.1),"#;
        let x = match_cond2_item(&mut code).assert();
        println!("{:?}", x);
        assert_eq!(
            x,
            MatchCase::eq2_const("ip", "127.0.0.1", "127.0.0.100", "10.0.0.1")?
        );

        let mut code = r#"in (ip(127.0.0.1),ip(127.0.0.100)) => ip(10.0.0.1),"#;
        let x = match_cond1_item(&mut code).assert();
        println!("{:?}", x);
        assert_eq!(
            x,
            MatchCase::in_const("ip", "127.0.0.1", "127.0.0.100", "10.0.0.1")?
        );
        Ok(())
    }

    #[test]
    fn test_match_err() -> AnyResult<()> {
        let mut code = r#"chas(A) => chars(5),"#;
        disp_err(code, match_cond1_item(&mut code));
        let mut code = r#"chars(A) > chars(5),"#;
        disp_err(code, match_cond1_item(&mut code));
        let mut code = r#"chars( ) > chars(5),"#;
        disp_err(code, match_cond1_item(&mut code));
        Ok(())
    }

    fn disp_err<T>(code: &str, result: ModalResult<T>) {
        if let Err(x) = result {
            println!("{}", x);
            println!("{}", code);
        }
    }

    #[test]
    fn test_match() {
        let mut code = r#" match read(city)  {
        chars(A) => chars(bj),
        } "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_2() {
        let mut code = r#" match read(city)   {
        chars(A) => chars(bj),
        chars(B) => chars(cs),
        _ => read(src_city),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_3() {
        let mut code = r#" match read(city)  {
        in (ip(127.0.0.1),   ip(127.0.0.100)) => chars(bj),
        in (ip(127.0.0.100), ip(127.0.0.200)) => chars(bj),
        in (ip(127.0.0.200),  ip(127.0.0.255)) => chars(cs),
        _ => chars(sz),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_4() {
        let mut code = r#" match ( read(city1), read(city2) ) {
        (ip(127.0.0.1),   ip(127.0.0.100)) => chars(bj),
        (ip(127.0.0.100), ip(127.0.0.200)) => chars(bj),
        (ip(127.0.0.200),  ip(127.0.0.255)) => chars(cs),
        _ => chars(sz),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_with_function() {
        // Test function-based matching with starts_with
        let mut code = r#" match read(Content) {
        starts_with('jk2_init()') => chars(E1),
        starts_with('WARNING') => chars(E2),
        _ => chars(E3),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_with_ends_with() {
        // Test ends_with function
        let mut code = r#" match read(filename) {
        ends_with('.log') => chars(log_file),
        ends_with('.txt') => chars(text_file),
        ends_with('.json') => chars(json_file),
        _ => chars(unknown),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_with_contains() {
        // Test contains function
        let mut code = r#" match read(message) {
        contains('error') => chars(error_msg),
        contains('warning') => chars(warn_msg),
        contains('timeout') => chars(timeout_msg),
        _ => chars(normal_msg),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_mixed_functions() {
        // Test mixing different functions
        let mut code = r#" match read(log_line) {
        starts_with('[ERROR]') => chars(error),
        starts_with('[WARN]') => chars(warning),
        contains('exception') => chars(exception),
        ends_with('failed') => chars(failure),
        _ => chars(other),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_oml_parse_basic() {
        use crate::parser::oml_parse_raw;

        let mut conf = r#"name : test
---
A = read(field);
"#;
        let result = oml_parse_raw(&mut conf);
        assert!(result.is_ok(), "Basic OML parse should succeed");
    }

    #[test]
    fn test_oml_parse_with_simple_match() {
        use crate::parser::oml_parse_raw;

        let mut conf = r#"name : test
---
A = match read(field) {
    chars(A) => chars(B),
    _ => chars(C),
};
"#;
        let result = oml_parse_raw(&mut conf);
        assert!(
            result.is_ok(),
            "Match OML parse should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_oml_parse_with_function_match() {
        use crate::parser::oml_parse_raw;

        let mut conf = r#"name : test
---
A = match read(field) {
    starts_with('test') => chars(ok),
    _ => chars(fail),
};
"#;
        let result = oml_parse_raw(&mut conf);
        assert!(
            result.is_ok(),
            "Function match parse should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_match_function_execution() {
        use crate::core::DataTransformer;
        use crate::parser::oml_parse_raw;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::{DataField, DataRecord};

        // Test starts_with
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "Content",
            "[ERROR] System failure",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"name : test
---
EventType = match read(Content) {
    starts_with('[ERROR]') => chars(error),
    _ => chars(other),
};
"#;
        let model = oml_parse_raw(&mut conf).expect("Failed to parse starts_with");
        let target = model.transform(src, cache);
        let expect = DataField::from_chars("EventType".to_string(), "error".to_string());
        assert_eq!(
            target.field("EventType").map(|s| s.as_field()),
            Some(&expect)
        );

        // Test ends_with
        let mut conf2 = r#"name : test
---
FileType = match read(filename) {
    ends_with('.json') => chars(json),
    _ => chars(other),
};
"#;
        let model2 = oml_parse_raw(&mut conf2).expect("Failed to parse ends_with");
        let cache2 = &mut FieldQueryCache::default();
        let data2 = vec![FieldStorage::from_owned(DataField::from_chars(
            "filename",
            "config.json",
        ))];
        let src2 = DataRecord::from(data2);
        let target2 = model2.transform(src2, cache2);
        let expect2 = DataField::from_chars("FileType".to_string(), "json".to_string());
        assert_eq!(
            target2.field("FileType").map(|s| s.as_field()),
            Some(&expect2)
        );

        // Test contains
        let mut conf3 = r#"name : test
---
ErrorType = match read(message) {
    contains('timeout') => chars(timeout),
    _ => chars(other),
};
"#;
        let model3 = oml_parse_raw(&mut conf3).expect("Failed to parse contains");
        let cache3 = &mut FieldQueryCache::default();
        let data3 = vec![FieldStorage::from_owned(DataField::from_chars(
            "message",
            "Connection timeout occurred",
        ))];
        let src3 = DataRecord::from(data3);
        let target3 = model3.transform(src3, cache3);
        let expect3 = DataField::from_chars("ErrorType".to_string(), "timeout".to_string());
        assert_eq!(
            target3.field("ErrorType").map(|s| s.as_field()),
            Some(&expect3)
        );

        // Test regex_match
        let mut conf4 = r#"name : test
---
LogLevel = match read(log_line) {
    regex_match('^\[ERROR\]') => chars(error),
    _ => chars(other),
};
"#;
        let model4 = oml_parse_raw(&mut conf4).expect("Failed to parse regex_match");
        let cache4 = &mut FieldQueryCache::default();
        let data4 = vec![FieldStorage::from_owned(DataField::from_chars(
            "log_line",
            "[ERROR] Failed",
        ))];
        let src4 = DataRecord::from(data4);
        let target4 = model4.transform(src4, cache4);
        let expect4 = DataField::from_chars("LogLevel".to_string(), "error".to_string());
        assert_eq!(
            target4.field("LogLevel").map(|s| s.as_field()),
            Some(&expect4)
        );

        // Test is_empty - empty case
        let mut conf5 = r#"name : test
---
Status = match read(field) {
    is_empty() => chars(empty),
    _ => chars(not_empty),
};
"#;
        let model5 = oml_parse_raw(&mut conf5).expect("Failed to parse is_empty");
        let cache5 = &mut FieldQueryCache::default();
        let data5 = vec![FieldStorage::from_owned(DataField::from_chars("field", ""))];
        let src5 = DataRecord::from(data5);
        let target5 = model5.transform(src5, cache5);
        let expect5 = DataField::from_chars("Status".to_string(), "empty".to_string());
        assert_eq!(
            target5.field("Status").map(|s| s.as_field()),
            Some(&expect5)
        );

        // Test iequals (moved to end, after gt/lt/eq/in_range)
        // Already tested in conf11 below

        // Test gt
        let mut conf6 = r#"name : test
---
Level = match read(count) {
    gt(100) => chars(high),
    _ => chars(low),
};
"#;
        let model6 = oml_parse_raw(&mut conf6).expect("Failed to parse gt");
        let cache6 = &mut FieldQueryCache::default();
        let data6 = vec![FieldStorage::from_owned(DataField::from_digit(
            "count", 150,
        ))];
        let src6 = DataRecord::from(data6);
        let target6 = model6.transform(src6, cache6);
        let expect6 = DataField::from_chars("Level".to_string(), "high".to_string());
        assert_eq!(target6.field("Level").map(|s| s.as_field()), Some(&expect6));

        // Test lt
        let mut conf7 = r#"name : test
---
Grade = match read(score) {
    lt(60) => chars(fail),
    _ => chars(pass),
};
"#;
        let model7 = oml_parse_raw(&mut conf7).expect("Failed to parse lt");
        let cache7 = &mut FieldQueryCache::default();
        let data7 = vec![FieldStorage::from_owned(DataField::from_digit("score", 45))];
        let src7 = DataRecord::from(data7);
        let target7 = model7.transform(src7, cache7);
        let expect7 = DataField::from_chars("Grade".to_string(), "fail".to_string());
        assert_eq!(target7.field("Grade").map(|s| s.as_field()), Some(&expect7));

        // Test eq
        let mut conf8 = r#"name : test
---
Status = match read(level) {
    eq(5) => chars(max),
    _ => chars(normal),
};
"#;
        let model8 = oml_parse_raw(&mut conf8).expect("Failed to parse eq");
        let cache8 = &mut FieldQueryCache::default();
        let data8 = vec![FieldStorage::from_owned(DataField::from_digit("level", 5))];
        let src8 = DataRecord::from(data8);
        let target8 = model8.transform(src8, cache8);
        let expect8 = DataField::from_chars("Status".to_string(), "max".to_string());
        assert_eq!(
            target8.field("Status").map(|s| s.as_field()),
            Some(&expect8)
        );

        // Test in_range
        let mut conf9 = r#"name : test
---
TempZone = match read(temperature) {
    in_range(20, 30) => chars(comfortable),
    _ => chars(other),
};
"#;
        let model9 = oml_parse_raw(&mut conf9).expect("Failed to parse in_range");
        let cache9 = &mut FieldQueryCache::default();
        let data9 = vec![FieldStorage::from_owned(DataField::from_digit(
            "temperature",
            25,
        ))];
        let src9 = DataRecord::from(data9);
        let target9 = model9.transform(src9, cache9);
        let expect9 = DataField::from_chars("TempZone".to_string(), "comfortable".to_string());
        assert_eq!(
            target9.field("TempZone").map(|s| s.as_field()),
            Some(&expect9)
        );

        // Test iequals
        let mut conf10 = r#"name : test
---
Result = match read(status) {
    iequals('success') => chars(ok),
    _ => chars(other),
};
"#;
        let model10 = oml_parse_raw(&mut conf10).expect("Failed to parse iequals");
        let cache10 = &mut FieldQueryCache::default();
        let data10 = vec![FieldStorage::from_owned(DataField::from_chars(
            "status", "SUCCESS",
        ))];
        let src10 = DataRecord::from(data10);
        let target10 = model10.transform(src10, cache10);
        let expect10 = DataField::from_chars("Result".to_string(), "ok".to_string());
        assert_eq!(
            target10.field("Result").map(|s| s.as_field()),
            Some(&expect10)
        );
    }

    #[test]
    fn test_match_with_quoted_strings() {
        use crate::core::DataTransformer;
        use orion_error::TestAssert;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::DataRecord;

        // Test match with quoted strings in results
        let mut code = r#"
name : test
---
Result = match read(status) {
    chars(A) => chars('success message'),
    chars(B) => chars("failure message"),
    _ => chars('default message'),
};
"#;
        let model = crate::parser::oml_conf::oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");

        // Test execution: verify that quoted strings are parsed correctly
        let cache = &mut FieldQueryCache::default();

        // Test case 1: status = A
        let data1 = vec![FieldStorage::from_owned(DataField::from_chars(
            "status", "A",
        ))];
        let src1 = DataRecord::from(data1);
        let target1 = model.transform(src1, cache);
        let expect1 = DataField::from_chars("Result".to_string(), "success message".to_string());
        assert_eq!(
            target1.field("Result").map(|s| s.as_field()),
            Some(&expect1)
        ); // Verify the value contains space

        // Test case 2: status = B
        let data2 = vec![FieldStorage::from_owned(DataField::from_chars(
            "status", "B",
        ))];
        let src2 = DataRecord::from(data2);
        let target2 = model.transform(src2, cache);
        let expect2 = DataField::from_chars("Result".to_string(), "failure message".to_string());
        assert_eq!(
            target2.field("Result").map(|s| s.as_field()),
            Some(&expect2)
        );

        // Test case 3: status = C (default)
        let data3 = vec![FieldStorage::from_owned(DataField::from_chars(
            "status", "C",
        ))];
        let src3 = DataRecord::from(data3);
        let target3 = model.transform(src3, cache);
        let expect3 = DataField::from_chars("Result".to_string(), "default message".to_string());
        assert_eq!(
            target3.field("Result").map(|s| s.as_field()),
            Some(&expect3)
        );
    }

    #[test]
    fn test_cond_fun_parsing() {
        // Test parsing just the condition
        let mut code = r#"starts_with('test')"#;
        let result = cond_fun(&mut code);
        println!("Result: {:?}", result);
        match result {
            Ok(MatchCond::Fun(fun)) => {
                assert_eq!(fun.name, "starts_with");
                assert_eq!(fun.args, vec!["test".to_string()]);
            }
            other => panic!("Expected Fun condition, got: {:?}", other),
        }

        // Test is_empty with no arguments
        let mut code2 = r#"is_empty()"#;
        let result2 = cond_fun(&mut code2);
        match result2 {
            Ok(MatchCond::Fun(fun)) => {
                assert_eq!(fun.name, "is_empty");
                assert!(fun.args.is_empty());
            }
            other => panic!("Expected Fun condition for is_empty, got: {:?}", other),
        }
    }

    #[test]
    fn test_match_with_regex() {
        use wp_parser::Parser;

        // Test regex matching - verify round-trip parsing works
        let mut code = r#" match read(log_line) {
        regex_match('^\[ERROR\].*') => chars(error),
        regex_match('^\[WARN\].*') => chars(warning),
        regex_match('^\d{4}-\d{2}-\d{2}') => chars(dated),
        _ => chars(other),
        }
       "#;
        let result = oml_aga_match.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse regex match");

        // Verify Display output and round-trip parsing
        let parsed = result.unwrap();
        let output = format!("{}", parsed);
        println!("Original regex match Display output:\n{}", output);

        // The output should preserve the original escape sequences
        // quot_str returns raw content, so backslashes are preserved as-is
        assert!(output.contains(r#"regex_match('^\[ERROR\].*')"#));
        assert!(output.contains(r#"regex_match('^\[WARN\].*')"#));
        assert!(output.contains(r#"regex_match('^\d{4}-\d{2}-\d{2}')"#));

        // Verify round-trip: parse the Display output
        let mut output_slice = output.as_str();
        let result2 = oml_aga_match.parse_next(&mut output_slice);
        assert!(result2.is_ok(), "Round-trip parse should succeed");

        // Verify output is stable after round-trip
        let parsed2 = result2.unwrap();
        let output2 = format!("{}", parsed2);
        assert_eq!(
            output.replace(" ", "").replace("\n", ""),
            output2.replace(" ", "").replace("\n", ""),
            "Output should be stable after round-trip"
        );
    }

    #[test]
    fn test_match_with_is_empty() {
        // Test is_empty function
        let mut code = r#" match read(field) {
        is_empty() => chars(empty),
        _ => chars(not_empty),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_with_iequals() {
        // Test case-insensitive matching
        let mut code = r#" match read(status) {
        iequals('success') => chars(ok),
        iequals('error') => chars(fail),
        iequals('warning') => chars(warn),
        _ => chars(unknown),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_with_numeric_comparison() {
        // Test gt
        let mut code = r#" match read(count) {
        gt(100) => chars(high),
        _ => chars(low),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);

        // Test lt
        let mut code2 = r#" match read(score) {
        lt(60) => chars(fail),
        _ => chars(pass),
        }
       "#;
        assert_oml_parse(&mut code2, oml_aga_match);

        // Test eq
        let mut code3 = r#" match read(level) {
        eq(5) => chars(max),
        _ => chars(other),
        }
       "#;
        assert_oml_parse(&mut code3, oml_aga_match);

        // Test in_range
        let mut code4 = r#" match read(temperature) {
        in_range(20, 30) => chars(comfortable),
        in_range(10, 20) => chars(cool),
        in_range(30, 40) => chars(warm),
        _ => chars(extreme),
        }
       "#;
        assert_oml_parse(&mut code4, oml_aga_match);
    }

    #[test]
    fn test_match_function_escaping_round_trip() {
        use wp_parser::Parser;

        // Test strings with special characters in match functions
        let test_cases = vec![
            (
                r#" match read(Content) {
        starts_with('O\'Reilly') => chars(ok),
        _ => chars(fail),
        }"#,
                "O'Reilly",
            ),
            (
                r#" match read(path) {
        contains('error\\path') => chars(error),
        _ => chars(normal),
        }"#,
                r"error\path",
            ),
        ];

        for (code, expected_content) in test_cases {
            let mut code_slice = code;
            let result = oml_aga_match.parse_next(&mut code_slice);
            assert!(
                result.is_ok(),
                "Should parse match with escaped string: {}",
                expected_content
            );

            // Check round-trip: Display output should be parseable
            let parsed = result.unwrap();
            let output = format!("{}", parsed);
            println!("Round-trip output:\n{}", output);

            // Verify escaping is present in output
            let mut output_slice = output.as_str();
            let result2 = oml_aga_match.parse_next(&mut output_slice);
            assert!(
                result2.is_ok(),
                "Round-trip parse should succeed for: {}",
                expected_content
            );
        }
    }

    #[test]
    fn test_match_cond3_item_parse() {
        // Test parsing a triple condition item
        let mut code = r#"(chars(A), chars(B), chars(C)) => chars(result),"#;
        let x = match_cond3_item(&mut code);
        assert!(x.is_ok(), "Should parse triple condition item: {:?}", x);
    }

    #[test]
    fn test_match_cond4_item_parse() {
        // Test parsing a quadruple condition item
        let mut code = r#"(chars(A), chars(B), chars(C), chars(D)) => chars(result),"#;
        let x = match_cond4_item(&mut code);
        assert!(
            x.is_ok(),
            "Should parse quadruple condition item: {:?}",
            x
        );
    }

    #[test]
    fn test_match_triple_source() {
        // Test match with three sources
        let mut code = r#" match ( read(city), read(region), read(country) ) {
        (chars(bj), chars(north), chars(cn)) => chars(result1),
        (chars(sh), chars(east), chars(cn)) => chars(result2),
        _ => chars(default),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_triple_source_with_mixed_cond() {
        // Test match with three sources using different condition types
        let mut code = r#" match ( read(ip_field), read(level), read(zone) ) {
        (in (ip(10.0.0.1), ip(10.0.0.100)), chars(high), chars(east)) => chars(block),
        (ip(192.168.0.1), chars(low), chars(west)) => chars(allow),
        _ => chars(unknown),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_quadruple_source() {
        // Test match with four sources
        let mut code = r#" match ( read(a), read(b), read(c), read(d) ) {
        (chars(1), chars(2), chars(3), chars(4)) => chars(match1),
        (chars(A), chars(B), chars(C), chars(D)) => chars(match2),
        _ => chars(no_match),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_quadruple_source_with_mixed_cond() {
        // Test match with four sources using mixed condition types
        let mut code = r#" match ( read(src_ip), read(dst_ip), read(proto), read(action) ) {
        (in (ip(10.0.0.1), ip(10.0.0.255)), ip(192.168.1.1), chars(tcp), chars(allow)) => chars(rule1),
        (ip(172.16.0.1), in (ip(10.0.0.1), ip(10.0.0.255)), chars(udp), chars(deny)) => chars(rule2),
        _ => chars(default_rule),
        }
       "#;
        assert_oml_parse(&mut code, oml_aga_match);
    }

    #[test]
    fn test_match_triple_round_trip() {
        use wp_parser::Parser;

        let mut code = r#" match ( read(a), read(b), read(c) ) {
        (chars(x), chars(y), chars(z)) => chars(ok),
        _ => chars(fail),
        }
       "#;
        let result = oml_aga_match.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse triple match");

        let parsed = result.unwrap();
        let output = format!("{}", parsed);
        println!("Triple match Display output:\n{}", output);

        // Verify round-trip
        let mut output_slice = output.as_str();
        let result2 = oml_aga_match.parse_next(&mut output_slice);
        assert!(result2.is_ok(), "Round-trip parse should succeed");
    }

    #[test]
    fn test_match_quadruple_round_trip() {
        use wp_parser::Parser;

        let mut code = r#" match ( read(a), read(b), read(c), read(d) ) {
        (chars(1), chars(2), chars(3), chars(4)) => chars(ok),
        _ => chars(fail),
        }
       "#;
        let result = oml_aga_match.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse quadruple match");

        let parsed = result.unwrap();
        let output = format!("{}", parsed);
        println!("Quadruple match Display output:\n{}", output);

        // Verify round-trip
        let mut output_slice = output.as_str();
        let result2 = oml_aga_match.parse_next(&mut output_slice);
        assert!(result2.is_ok(), "Round-trip parse should succeed");
    }

    #[test]
    fn test_oml_parse_with_triple_match() {
        use crate::parser::oml_parse_raw;

        let mut conf = r#"name : test
---
A = match (read(f1), read(f2), read(f3)) {
    (chars(a), chars(b), chars(c)) => chars(ok),
    _ => chars(fail),
};
"#;
        let result = oml_parse_raw(&mut conf);
        assert!(
            result.is_ok(),
            "Triple match OML parse should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_oml_parse_with_quadruple_match() {
        use crate::parser::oml_parse_raw;

        let mut conf = r#"name : test
---
A = match (read(f1), read(f2), read(f3), read(f4)) {
    (chars(a), chars(b), chars(c), chars(d)) => chars(ok),
    _ => chars(fail),
};
"#;
        let result = oml_parse_raw(&mut conf);
        assert!(
            result.is_ok(),
            "Quadruple match OML parse should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_match_triple_execution() {
        use crate::core::DataTransformer;
        use crate::parser::oml_parse_raw;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::DataRecord;

        let cache = &mut FieldQueryCache::default();
        let mut conf = r#"name : test
---
Result = match (read(city), read(level), read(zone)) {
    (chars(bj), chars(high), chars(north)) => chars(matched),
    _ => chars(default),
};
"#;
        let model = oml_parse_raw(&mut conf).expect("Failed to parse triple match");

        // Test case 1: all three match
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("city", "bj")),
            FieldStorage::from_owned(DataField::from_chars("level", "high")),
            FieldStorage::from_owned(DataField::from_chars("zone", "north")),
        ];
        let src = DataRecord::from(data);
        let target = model.transform(src, cache);
        let expect = DataField::from_chars("Result".to_string(), "matched".to_string());
        assert_eq!(
            target.field("Result").map(|s| s.as_field()),
            Some(&expect)
        );

        // Test case 2: partial match falls to default
        let data2 = vec![
            FieldStorage::from_owned(DataField::from_chars("city", "bj")),
            FieldStorage::from_owned(DataField::from_chars("level", "low")),
            FieldStorage::from_owned(DataField::from_chars("zone", "north")),
        ];
        let src2 = DataRecord::from(data2);
        let target2 = model.transform(src2, cache);
        let expect2 = DataField::from_chars("Result".to_string(), "default".to_string());
        assert_eq!(
            target2.field("Result").map(|s| s.as_field()),
            Some(&expect2)
        );
    }

    #[test]
    fn test_match_quadruple_execution() {
        use crate::core::DataTransformer;
        use crate::parser::oml_parse_raw;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::DataRecord;

        let cache = &mut FieldQueryCache::default();
        let mut conf = r#"name : test
---
Result = match (read(a), read(b), read(c), read(d)) {
    (chars(x), chars(y), chars(z), chars(w)) => chars(all_match),
    (chars(x), chars(y), chars(z), chars(other)) => chars(partial),
    _ => chars(default),
};
"#;
        let model = oml_parse_raw(&mut conf).expect("Failed to parse quadruple match");

        // Test case 1: first arm matches
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("a", "x")),
            FieldStorage::from_owned(DataField::from_chars("b", "y")),
            FieldStorage::from_owned(DataField::from_chars("c", "z")),
            FieldStorage::from_owned(DataField::from_chars("d", "w")),
        ];
        let src = DataRecord::from(data);
        let target = model.transform(src, cache);
        let expect = DataField::from_chars("Result".to_string(), "all_match".to_string());
        assert_eq!(
            target.field("Result").map(|s| s.as_field()),
            Some(&expect)
        );

        // Test case 2: second arm matches
        let data2 = vec![
            FieldStorage::from_owned(DataField::from_chars("a", "x")),
            FieldStorage::from_owned(DataField::from_chars("b", "y")),
            FieldStorage::from_owned(DataField::from_chars("c", "z")),
            FieldStorage::from_owned(DataField::from_chars("d", "other")),
        ];
        let src2 = DataRecord::from(data2);
        let target2 = model.transform(src2, cache);
        let expect2 = DataField::from_chars("Result".to_string(), "partial".to_string());
        assert_eq!(
            target2.field("Result").map(|s| s.as_field()),
            Some(&expect2)
        );

        // Test case 3: default
        let data3 = vec![
            FieldStorage::from_owned(DataField::from_chars("a", "no")),
            FieldStorage::from_owned(DataField::from_chars("b", "match")),
            FieldStorage::from_owned(DataField::from_chars("c", "here")),
            FieldStorage::from_owned(DataField::from_chars("d", "at_all")),
        ];
        let src3 = DataRecord::from(data3);
        let target3 = model.transform(src3, cache);
        let expect3 = DataField::from_chars("Result".to_string(), "default".to_string());
        assert_eq!(
            target3.field("Result").map(|s| s.as_field()),
            Some(&expect3)
        );
    }
}
