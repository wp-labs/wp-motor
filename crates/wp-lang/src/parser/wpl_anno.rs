use crate::ast::AnnEnum;
use crate::ast::AnnFun;
use crate::ast::TagKvs;
use crate::parser::utils;
use smol_str::SmolStr;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, cut_err, delimited, separated};
use winnow::token::literal;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

fn wpl_tags(input: &mut &str) -> WResult<AnnEnum> {
    let tags: Vec<(SmolStr, SmolStr)> = delimited(
        (multispace0, literal("tag"), multispace0, literal('(')),
        cut_err(separated(1.., utils::take_tag_kv, literal(",")))
            .context(ctx_desc("tag(key: \"val\", ... )")),
        (multispace0, literal(')')),
    )
    .parse_next(input)?;
    let mut obj = TagKvs::new();
    for tag in tags {
        obj.insert(tag.0, tag.1);
    }
    Ok(AnnEnum::Tags(obj))
}

fn copy_raw(input: &mut &str) -> WResult<AnnEnum> {
    let obj = delimited(
        (multispace0, literal("copy_raw"), multispace0, literal('(')),
        cut_err(utils::take_tag_kv).context(ctx_desc("copy_raw(name: \"...\")")),
        (multispace0, literal(')')),
    )
    .parse_next(input)?;
    Ok(AnnEnum::Copy(obj))
}

pub fn ann_fun(input: &mut &str) -> WResult<AnnFun> {
    multispace0.parse_next(input)?;
    literal("#[")
        .context(ctx_desc("annotation start"))
        .parse_next(input)?;
    let x: Vec<AnnEnum> =
        separated(0.., alt((wpl_tags, copy_raw)), literal(",")).parse_next(input)?;
    multispace0.parse_next(input)?;
    literal("]")
        .context(ctx_desc("annotation end"))
        .parse_next(input)?;
    multispace0.parse_next(input)?;
    let mut af = AnnFun::default();
    for item in x {
        match item {
            AnnEnum::Copy(v) => {
                af.copy_raw = Some(v);
            }
            AnnEnum::Tags(v) => {
                af.tags = v;
            }
        }
    }
    Ok(af)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::ast::{AnnEnum, AnnFun};
    use crate::parser::utils::take_tag_kv;
    use crate::parser::wpl_anno::{ann_fun, wpl_tags};
    use orion_error::TestAssert;
    use wp_parser::Parser;

    #[test]
    fn test_tag_key() {
        assert_eq!(
            take_tag_kv.parse(r#"tag:"hello""#).assert(),
            ("tag".into(), "hello".into())
        );

        assert_eq!(
            take_tag_kv.parse(r#"tag_1:"hello2""#).assert(),
            ("tag_1".into(), "hello2".into())
        );

        // 放宽：支持空格、中文、转义引号
        assert_eq!(
            take_tag_kv.parse(r#"desc:"hello world""#).assert(),
            ("desc".into(), "hello world".into())
        );
        assert_eq!(
            take_tag_kv.parse(r#"cn:"中文 值""#).assert(),
            ("cn".into(), "中文 值".into())
        );
        assert_eq!(
            take_tag_kv.parse(r#"q:"say \"hi\"""#).assert(),
            ("q".into(), r#"say "hi""#.into())
        );
    }

    #[test]
    fn test_tags() {
        assert_eq!(
            wpl_tags.parse(r#"tag(tag_0:"xyz")"#).assert(),
            AnnEnum::Tags(BTreeMap::from([("tag_0".into(), "xyz".into())]))
        );

        assert_eq!(
            wpl_tags
                .parse(r#"tag( tag:"hello",   tag2:"hello2",    tag3:"hello3")"#)
                .assert(),
            AnnEnum::Tags(BTreeMap::from([
                ("tag".into(), "hello".into()),
                ("tag2".into(), "hello2".into()),
                ("tag3".into(), "hello3".into()),
            ]))
        );

        assert_eq!(
            wpl_tags
                .parse(r#"tag( tag:"hello222",   tag_2:"hello2",    tag3:"hello3")"#)
                .assert(),
            AnnEnum::Tags(BTreeMap::from([
                ("tag".into(), "hello222".into()),
                ("tag_2".into(), "hello2".into()),
                ("tag3".into(), "hello3".into()),
            ]))
        );
    }

    #[test]
    fn test_annotation() {
        assert_eq!(
            ann_fun
                .parse(
                    r#"
            #[tag(tag : "hello", cc_y : "qw_/e" ), copy_raw(name:"tq")]
            "#
                )
                .assert(),
            AnnFun {
                tags: BTreeMap::from([
                    ("tag".into(), "hello".into()),
                    ("cc_y".into(), "qw_/e".into())
                ]),
                copy_raw: Some(("name".into(), "tq".into())),
            }
        );

        assert_eq!(
            ann_fun
                .parse(
                    r#"
            #[tag(tag : "hello", cc_y : "qw_/e" )]
            "#
                )
                .assert(),
            AnnFun {
                tags: BTreeMap::from([
                    ("tag".into(), "hello".into()),
                    ("cc_y".into(), "qw_/e".into())
                ]),
                copy_raw: None,
            }
        );

        assert_eq!(
            ann_fun
                .parse(
                    r#"
            #[copy_raw(name:"tq")]
            "#
                )
                .assert(),
            AnnFun {
                tags: Default::default(),
                copy_raw: Some(("name".into(), "tq".into())),
            }
        );
    }
}
