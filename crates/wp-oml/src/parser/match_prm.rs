use crate::language::MatchOperation;
use crate::language::NestedAccessor;
use crate::language::{MatchCase, MatchCond};
use crate::language::{MatchCondition, MatchSource, PreciseEvaluator};
use crate::parser::collect_prm::oml_aga_collect;
use crate::parser::keyword::{kw_gw_match, kw_in};
use crate::parser::oml_aggregate::oml_crate_calc_ref;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt, peek, repeat};
use winnow::error::{StrContext, StrContextValue};
use winnow::token::take;
use wp_model_core::model::DataField;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;
use wp_parser::symbol::{
    symbol_brace_beg, symbol_brace_end, symbol_comma, symbol_marvel, symbol_match_to,
    symbol_semicolon, symbol_under_line,
};
use wp_parser::utils::get_scope;

use super::syntax;
use super::tdc_prm::{oml_aga_tdc, oml_aga_value};

fn match_cond1(data: &mut &str) -> WResult<MatchCond> {
    multispace0.parse_next(data)?;
    let mut cond_exp = match peek(take(1usize)).parse_next(data)? {
        "!" => cond_neq,
        "i" => {
            if peek(take(2usize)).parse_next(data)? == "in" {
                cond_in
            } else {
                cond_eq
            }
        }
        _ => cond_eq,
    };
    cond_exp.parse_next(data)
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

fn match_calc_target(data: &mut &str) -> WResult<NestedAccessor> {
    symbol_match_to.parse_next(data)?;
    let gw = alt((oml_aga_tdc, oml_aga_value, oml_aga_collect)).parse_next(data)?;
    opt(symbol_comma).parse_next(data)?;
    opt(symbol_semicolon).parse_next(data)?;
    let sub_gw = match gw {
        PreciseEvaluator::Obj(x) => NestedAccessor::Field(x),
        PreciseEvaluator::Tdc(x) => NestedAccessor::Direct(x),
        PreciseEvaluator::Collect(x) => NestedAccessor::Collect(x),
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

fn tdo_val_scope(data: &mut &str) -> WResult<(DataField, DataField)> {
    let scope = get_scope(data, '(', ')')?;
    let mut code: &str = scope;
    let beg_tdo = syntax::oml_value.parse_next(&mut code)?;
    symbol_comma.parse_next(&mut code)?;
    let end_tdo = syntax::oml_value.parse_next(&mut code)?;
    Ok((beg_tdo, end_tdo))
}
fn cond_eq(data: &mut &str) -> WResult<MatchCond> {
    multispace0.parse_next(data)?;
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
    let tdo = syntax::oml_value.parse_next(data)?;
    Ok(MatchCond::Neq(tdo))
}
fn cond_in(data: &mut &str) -> WResult<MatchCond> {
    let _ = multispace0.parse_next(data)?;
    kw_in.parse_next(data)?;
    let (beg, end) = tdo_val_scope.parse_next(data)?;
    Ok(MatchCond::In(beg, end))
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
    use orion_error::TestAssert;
    use wp_parser::WResult as ModalResult;

    use crate::language::MatchCase;
    use crate::parser::match_prm::{match_cond1_item, match_cond2_item, oml_aga_match};
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
}
