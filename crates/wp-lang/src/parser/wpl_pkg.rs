use super::wpl_anno::ann_fun;
use crate::ast::{WplPackage, WplRule};
use crate::parser::{MergeTags, utils, wpl_rule};
use smol_str::SmolStr;
use winnow::ascii::{multispace0, multispace1};
use winnow::combinator::{alt, cut_err, delimited, opt, repeat};
use winnow::error::{ContextError, StrContext};
use winnow::token::literal;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::{ctx_desc, ctx_label, ctx_literal};

pub fn wpl_pkg_body2(input: &mut &str) -> WResult<Vec<WplRule>> {
    let mut rules = Vec::new();
    loop {
        wpl_rule::wpl_rule
            .context(StrContext::Expected("rule <name> {...}".into()))
            .map(|x| rules.push(x))
            .parse_next(input)?;
        if !utils::is_next(alt(("rule", "#[")), input) {
            break;
        }
    }
    Ok(rules)
}

pub fn wpl_pkg_body<'a, 'b>(
    package: &'b mut WplPackage,
) -> impl Parser<&'a str, (), ContextError> + 'b {
    move |input: &mut &'a str| {
        use winnow::error::{ContextError, ErrMode};
        delimited(
            multispace0,
            repeat(
                1..,
                wpl_rule::wpl_rule
                    .context(StrContext::Expected("rule <name> {...}".into()))
                    .map(|x| package.rules.push_back(x)),
            ),
            multispace0,
        )
        .parse_next(input)
        .map_err(|e| match e {
            ErrMode::Backtrack(e) | ErrMode::Cut(e) => e,
            ErrMode::Incomplete(_) => ContextError::default(),
        })
    }
}

#[allow(clippy::field_reassign_with_default)]
pub fn wpl_package(input: &mut &str) -> WResult<WplPackage> {
    let mut package = WplPackage::default();
    opt(ann_fun).map(|t| package.tags = t).parse_next(input)?;
    package.name = (
        multispace0,
        literal("package"),
        multispace1,
        utils::take_key,
    )
        .context(ctx_label("wpl keyword"))
        .context(ctx_literal("package <name> "))
        .context(ctx_desc("<<< package <pkg_name> {...}"))
        .map(|x| SmolStr::from(x.3))
        .parse_next(input)?;

    let rules = delimited(
        (multispace0, literal("{"), multispace0),
        cut_err(wpl_pkg_body2).context(ctx_desc("{ rule ... }")),
        (multispace0, literal("}"), multispace0),
    )
    .parse_next(input)?;

    package.append(rules);
    package.merge_tags(&None);
    Ok(package)
}
