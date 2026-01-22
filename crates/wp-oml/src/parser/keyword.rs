use crate::language::{OML_CRATE_IN, OmlKwGet};
use winnow::ascii::Caseless;
use winnow::ascii::multispace0;
use winnow::error::{StrContext, StrContextValue};
use winnow::token::literal;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

pub fn kw_gw_match(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("match")
        .context(StrContext::Label("oml keyword "))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'match' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_head_sep_line(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("---")
        .context(StrContext::Label("oml sepline"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need '---' separation line",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_oml_name(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("name")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'name' ",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_oml_rule(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("rule")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'rule' ",
        )))
        .parse_next(data)?;
    Ok(())
}
pub fn kw_in(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal(OML_CRATE_IN)
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'in' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_keys(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("keys")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'keys' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_option(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("option")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'option' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}
pub fn kw_take(data: &mut &str) -> WResult<OmlKwGet> {
    let _ = multispace0.parse_next(data)?;
    literal("take")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'take' keyword",
        )))
        .parse_next(data)?;
    Ok(OmlKwGet::Take)
}
// 'crate' 关键字已废弃：解析层移除；若仍需兼容，请在上层引入别名解析。

pub fn kw_read(data: &mut &str) -> WResult<OmlKwGet> {
    let _ = multispace0.parse_next(data)?;
    literal("read")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'read' keyword",
        )))
        .parse_next(data)?;
    Ok(OmlKwGet::Read)
}

pub fn kw_crate_symbol(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("@")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need '@' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_fmt(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("fmt")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'fmt' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_gw_query(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("query")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'query' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}
pub fn kw_sql_select(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal(Caseless("select"))
        .context(StrContext::Label("sql keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'select' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}
pub fn kw_sql_where(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal(Caseless("where"))
        .context(StrContext::Label("sql keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'where' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_gw_pipe(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("pipe")
        .context(StrContext::Label("oml keyword"))
        .context(ctx_desc("need 'pipe' keyword"))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_gw_collect(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("collect")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'collect' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_gw_get(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("get")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'get' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_object(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("object")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'object' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}

pub fn kw_lib(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("lib")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'lib' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}
pub fn kw_where(data: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(data)?;
    literal("where")
        .context(StrContext::Label("oml keyword"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need 'where' keyword",
        )))
        .parse_next(data)?;
    Ok(())
}
