use crate::ast::WplSep;
use crate::ast::group::WplGroup;
use crate::parser::constants::{CTX_EXPECT_GROUP_META, CTX_GROUP_CONTENT, CTX_GROUP_META_HINT};
use crate::parser::utils::peek_str;
use crate::parser::wpl_field::wpl_end_sep_str;
use crate::parser::wpl_rule;
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, delimited, opt};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::{ctx_desc, ctx_literal};

pub fn wpl_group(input: &mut &str) -> WResult<WplGroup> {
    let mut group = WplGroup::default();
    multispace0.parse_next(input)?;

    if peek_str("(", input).is_err() {
        let meta_str = alt(("alt", "opt", "some_of", "seq", "not"))
            .context(ctx_literal(CTX_GROUP_META_HINT))
            .context(ctx_desc(CTX_EXPECT_GROUP_META))
            .parse_next(input)?;
        group.meta_from(Some(meta_str));
    }

    let mut fields = delimited(
        (multispace0, '('),
        wpl_rule::wpl_field_vec,
        (multispace0, ')'),
    )
    .context(ctx_literal("( ... )"))
    .context(ctx_desc(CTX_GROUP_CONTENT))
    .parse_next(input)?;
    let group_len =
        opt(delimited('[', digit1.try_map(str::parse::<usize>), ']')).parse_next(input)?;

    let group_sep = opt(wpl_end_sep_str).parse_next(input)?;

    group.base_group_len = group_len;
    group.base_group_sep = group_sep.map(WplSep::group_sep);

    for field in &mut fields {
        if let Some(len) = group.base_group_len {
            field.length = Some(len);
        }

        //if let Some(sep) = &group.base_group_sep {
        //field.use_sep(sep.infer_clone());
        //}
    }
    group.fields.append(&mut fields);
    Ok(group)
}
// old alternative implementation removed (kept in VCS history)

#[cfg(test)]
mod tests {

    use orion_error::TestAssert;

    use crate::parser::wpl_field::wpl_pipe;

    use super::*;

    #[test]
    fn test_parse_group() {
        //let mut rule_define = STMExpress::default();
        let group = wpl_group
            .parse(
                r#"(ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")"#,
            )
            .assert();
        assert_eq!(group.fields.len(), 9);
    }
    #[test]
    fn test_parse_group_trailing_comma() {
        let group = wpl_group.parse(r#"(ip, digit,)"#).assert();
        assert_eq!(group.fields.len(), 2);
    }
    #[test]
    fn test_parse_group2() {
        let group = wpl_group
            .parse(
                r#"seq(ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")"#,
            )
            .assert();
        assert_eq!(group.fields.len(), 9);
        //assert_eq!(rule_define.group[0].meta , );

        let group = wpl_group
            .parse(
                r#"opt (ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")"#,
            )
            .assert();
        assert_eq!(group.fields.len(), 9);
    }

    #[test]
    fn test_parse_group_empty() {
        let group = wpl_group.parse(r#"()"#).assert();
        assert_eq!(group.fields.len(), 0);
    }

    #[test]
    fn test_parse_group_pipe() {
        //let mut rule_define = STMExpress::default();
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)"#).assert();
        let group = wpl_group.parse(r#"(chars:src_sys" |(time,ip))"#).assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(group.fields[0].pipe[0], pipe_expect);
    }

    #[test]
    fn test_parse_group_pipe1_1() {
        let pipe_expect = wpl_pipe.parse(r#"|(time,ip)\,"#).assert();
        let group = wpl_group.parse(r#"(chars:src_sys" |(time,ip)\,)"#).assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(group.fields[0].pipe[0], pipe_expect);
    }
    #[test]
    fn test_parse_group_pipe1_2() {
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)\!\|"#).assert();
        let group = wpl_group
            .parse(r#"(chars:src_sys" |(time,ip)\!\|)"#)
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(group.fields[0].pipe[0], pipe_expect);
    }
    #[test]
    fn test_parse_group_pipe1_3() {
        let pipe_expect = wpl_pipe.parse(r#"| f_has(src)"#).assert();
        let group = wpl_group.parse(r#"(json | f_has(src))"#).assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(group.fields[0].pipe[0], pipe_expect);
    }
    #[test]
    fn test_parse_group_pipe2() {
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)"#).assert();
        let group = wpl_group
            .parse(r#"(chars:src_sys" |(time,ip) , chars:dst_sys" | (time,ip) )"#)
            .assert();
        assert_eq!(group.fields.len(), 2);
        assert_eq!(group.fields[0].pipe[0], pipe_expect.clone());
        assert_eq!(group.fields[1].pipe[0], pipe_expect.clone());
    }

    #[test]
    fn test_parse_group_pattern_sep_with_pipe() {
        // chars{(command=)}|(kvarr\s) inside a group
        let group = wpl_group
            .parse("(chars{(command=)}|(kvarr\\s))")
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert!(group.fields[0].separator.as_ref().unwrap().is_pattern());
        assert!(!group.fields[0].pipe.is_empty());
    }

    #[test]
    fn test_parse_group_pipe3() {
        let pipe_expect = wpl_pipe.parse(r#"|(time,ip)"#).assert();
        let group = wpl_group
            .parse(r#"( json(chars@src_sys | (time,ip) ) )"#)
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(
            group.fields[0]
                .clone()
                .sub_fields
                .unwrap()
                .get("src_sys")
                .assert()
                .pipe[0],
            pipe_expect.clone()
        );
        //assert_eq!(group.fields[1].pipe, Some(pipe_expect.clone()));
    }

    #[test]
    fn test_parse_group_pipe3_1() {
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)\!"#).assert();
        let group = wpl_group
            .parse(r#"( json(chars@src_sys | (time,ip)\! ) )"#)
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(
            group.fields[0]
                .clone()
                .sub_fields
                .unwrap()
                .get("src_sys")
                .unwrap()
                .pipe[0],
            pipe_expect.clone()
        );
    }

    #[test]
    fn test_parse_group_pipe4_1() {
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)\!"#).assert();
        let group = wpl_group
            .parse(r#"( kv(chars@src_sys | (time,ip)\! ) )"#)
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(
            group.fields[0]
                .clone()
                .sub_fields
                .assert()
                .get("src_sys")
                .assert()
                .pipe[0],
            pipe_expect.clone()
        );
    }
    #[test]
    fn test_parse_group_pipe4_2() {
        let pipe_expect = wpl_pipe.parse(r#"| (time,ip)\!"#).assert();
        let group = wpl_group.parse(r#"( kv(chars| (time,ip)\! ) )"#).assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(
            group.fields[0]
                .clone()
                .sub_fields
                .assert()
                .get("src_sys")
                .assert()
                .pipe[0],
            pipe_expect.clone()
        );
    }
}
