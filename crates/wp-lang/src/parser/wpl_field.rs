use super::wpl_fun;
use crate::ast::WplSep;
use crate::ast::fld_fmt::WplFieldFmt;
use crate::ast::{DEFAULT_FIELD_KEY, WplField, WplFieldSet, WplPipe};
use crate::parser::datatype::take_datatype;
use crate::parser::utils::{
    peek_next, peek_str, take_key, take_parentheses, take_ref_path_or_quoted, take_to_end,
    take_var_name,
};
use crate::parser::wpl_group::wpl_group;
use crate::types::WildMap;
use winnow::ascii::{digit0, digit1, multispace0};
use winnow::combinator::{alt, delimited, fail, opt, preceded, repeat};
use winnow::error::{StrContext, StrContextValue};
use winnow::stream::Stream;
use winnow::token::{literal, take, take_till};
// Use workspace-wide parser result alias to decouple from winnow's concrete type
use wp_model_core::model::DataType;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::{ctx_desc, ctx_literal};
use wp_parser::utils::{RestAble, get_scope};

// Removed unused generic helper that depended on winnow's two-parameter ModalResult.
// If needed later, prefer concrete `&str` + `ModalResult` signatures via wp_parser::WResult.

pub fn wpl_end_sep_str(input: &mut &str) -> ModalResult<String> {
    repeat(1.., preceded(literal("\\"), take(1u8)))
        .fold(String::new, |mut acc, c| {
            acc.push_str(c);
            acc
        })
        .map(|x| x)
        .parse_next(input)
}

pub fn wpl_mid_sep_str(input: &mut &str) -> ModalResult<String> {
    repeat(1.., preceded(literal("^"), take(1u8)))
        .fold(String::new, |mut acc, c| {
            acc.push_str(c);
            acc
        })
        .map(|x| x)
        .parse_next(input)
}
pub fn wpl_sep(data: &mut &str) -> ModalResult<Option<WplSep>> {
    if peek_str("\\", data).is_ok() {
        let sep = wpl_end_sep_str.parse_next(data)?;
        if sep.is_empty() {
            fail.context(ctx_desc("end sep less")).parse_next(data)?;
        }
        Ok(Some(WplSep::field_sep(sep)))
    } else {
        Ok(None)
    }
}

// 分隔符对：<abc,xyz>
pub fn delimiter_pair(input: &mut &str) -> ModalResult<(String, String)> {
    multispace0.parse_next(input)?;
    let cp = input.checkpoint();
    let scope_str = get_scope(input, '<', '>').err_reset(input, &cp)?;
    let mut data = scope_str;
    let first = take_till(0.., |c| c == ',').parse_next(&mut data)?;
    literal(",").parse_next(&mut data)?;
    let second = take_to_end.parse_next(&mut data)?;
    if first.is_empty() || second.is_empty() {
        fail.context(ctx_desc("scope beg or end is empty!"))
            .parse_next(input)?;
        //return Err(Backtrack(ParserError::from_error_kind(
        //    &"scope beg or end is empty ",
        //    ErrorKind::Fail,
        //)));
    }
    Ok((first.to_string(), second.to_string()))
}
fn fmt_scope_std(data: &mut &str) -> ModalResult<WplFieldFmt> {
    let (beg, end) = delimiter_pair
        .context(StrContext::Label("scope"))
        .context(StrContext::Expected(StrContextValue::Description(
            "<fmt>:: <...,...>",
        )))
        .parse_next(data)?;
    Ok(WplFieldFmt {
        scope_beg: Some(beg.to_string()),
        scope_end: Some(end.to_string()),
        ..Default::default()
    })
}

fn fmt_scope_quk(data: &mut &str) -> ModalResult<WplFieldFmt> {
    let _ = literal('\"')
        .context(StrContext::Label("scope"))
        .context(StrContext::Expected(StrContextValue::Description(r#"\""#)))
        .parse_next(data)?;
    Ok(WplFieldFmt {
        scope_beg: Some("\"".to_string()),
        scope_end: Some("\"".to_string()),
        ..Default::default()
    })
}

fn wpl_field_fmt(input: &mut &str) -> ModalResult<WplFieldFmt> {
    multispace0.parse_next(input)?;
    if peek_str("<", input).is_ok() {
        return fmt_scope_std.parse_next(input);
    }
    if peek_str("\"", input).is_ok() {
        return fmt_scope_quk.parse_next(input);
    }
    Ok(WplFieldFmt::default())
}
fn wpl_opt_meta(input: &mut &str) -> ModalResult<(DataType, bool)> {
    multispace0.parse_next(input)?;
    let mut is_opt = true;
    let meta_key = opt(delimited(
        literal("opt("),
        (multispace0, take_key, multispace0),
        literal(")"),
    ))
    .parse_next(input)?;
    let meta_key = match meta_key {
        None => {
            is_opt = false;
            opt(take_key).parse_next(input)?
        }
        Some((_, v, _)) => Some(v),
    };
    if let Some(mk) = meta_key {
        match DataType::from(mk) {
            Ok(meta) => return Ok((meta, is_opt)),
            Err(_) => {
                fail.context(ctx_desc("bad meta")).parse_next(input)?;
            }
        }
    }
    Ok((DataType::Chars, is_opt))
}

#[allow(clippy::bind_instead_of_map)]
fn wpl_id_field(input: &mut &str) -> ModalResult<(String, WplField)> {
    let before_len = input.len();
    let mut content = None;
    let (meta_type, is_opt) = wpl_opt_meta.parse_next(input)?;

    if meta_type == DataType::Symbol {
        content = opt(take_parentheses)
            .parse_next(input)?
            .and_then(|x| Some(x.to_string()));
    }

    let k = opt((multispace0, literal('@'), take_ref_path_or_quoted).map(|x| x.2))
        .map(|x| x.unwrap_or(DEFAULT_FIELD_KEY.to_string()))
        .parse_next(input)?;

    let f_key = opt((multispace0, literal(':'), multispace0, take_key).map(|x| x.3))
        .map(|x| x.map(|x| x.to_string()))
        .parse_next(input)?;

    let (fmt_conf, sep) =
        delimited(multispace0, (wpl_field_fmt, wpl_sep), multispace0).parse_next(input)?;

    let after_len = input.len();

    if before_len == after_len {
        fail.context(ctx_desc("not found name "))
            .parse_next(input)?;
    }

    let mut conf = WplField {
        name: f_key.map(|s| s.into()),
        meta_name: meta_type.static_name().into(),
        meta_type,
        fmt_conf,
        separator: sep,
        content,
        is_opt,
        ..Default::default()
    };
    conf.pipe = repeat(0.., wpl_pipe).parse_next(input)?;
    conf.setup();

    Ok((k, conf))
}

fn wpl_field_subs(input: &mut &str) -> ModalResult<WplFieldSet> {
    let mut set: WildMap<WplField> = WildMap::new();
    (multispace0, literal('('))
        .context(ctx_desc("sub field(...)"))
        .parse_next(input)?;
    while peek_next((multispace0, literal(')')), input).is_err() {
        let (key, field) = wpl_id_field.parse_next(input)?;
        set.insert(key, field);
        opt(literal(',')).parse_next(input)?;
    }
    (multispace0, literal(')'))
        .context(ctx_desc(") "))
        .parse_next(input)?;
    Ok(WplFieldSet::from(set))

    /*
    let opt_tag = (multispace0, opt(literal(',')), multispace0);
    let conf_set = repeat(1.., terminated(wpl_id_field, opt_tag)).fold(
        WPLFieldSet::default,
        |mut acc, (key, conf)| {
            acc.add(key.to_string(), conf);
            acc
        },
    );

    delimited("(", conf_set, ")").parse_next(input)

     */
}

pub fn wpl_field(data: &mut &str) -> ModalResult<WplField> {
    wpl_field_impl
        .context(ctx_desc("<<< <field>"))
        .parse_next(data)
}

fn wpl_field_impl(input: &mut &str) -> ModalResult<WplField> {
    let mut conf = WplField::default();
    multispace0.parse_next(input)?;

    //if peek_next(digit0 , input).is_ok() {
    if peek_next((digit0, literal("*")), input).is_ok() {
        let (rep_cnt, _) = (digit0, literal("*")).parse_next(input)?;
        conf.continuous = true;
        if rep_cnt.is_empty() {
            conf.continuous_cnt = None
        } else {
            conf.continuous_cnt = Some(rep_cnt.parse::<usize>().unwrap_or(255));
        }
    }
    let main_meta = take_datatype.parse_next(input)?;
    conf.meta_name = main_meta.static_name().into();
    conf.meta_type = main_meta;
    parse_symbol(input, &mut conf)?;
    parse_peek_symbol(input, &mut conf)?;

    multispace0.parse_next(input)?;
    if peek_str("(", input).is_ok() {
        conf.sub_fields = Option::from(
            wpl_field_subs
                .context(ctx_literal(conf.meta_type.static_name()))
                .context(ctx_desc("sub define"))
                .parse_next(input)?,
        );
    }
    if peek_str(":", input).is_ok() {
        (":", multispace0).parse_next(input)?;
        let f_name = take_var_name
            .context(ctx_desc("<meta>:<name>"))
            .parse_next(input)?;
        conf.name = Some(f_name.into());
    }
    if let Some(max_len) =
        opt(delimited("[", digit1.try_map(str::parse::<usize>), "]")).parse_next(input)?
    {
        conf.length = Some(max_len);
    }

    let fmt_conf = wpl_field_fmt.parse_next(input)?;
    let opt_sep = wpl_sep.parse_next(input)?;
    conf.separator = opt_sep;
    conf.pipe = repeat(0.., wpl_pipe).parse_next(input)?;

    conf.fmt_conf = fmt_conf;

    conf.setup();

    Ok(conf)
}

pub fn wpl_pipe(data: &mut &str) -> ModalResult<WplPipe> {
    multispace0.parse_next(data)?;
    literal('|').parse_next(data)?;
    multispace0.parse_next(data)?;
    //let mut pipe_exp= STMExpress::default();
    let pipe = alt((wpl_pipe_fun, wpl_pipe_group)).parse_next(data)?;
    Ok(pipe)
}
fn wpl_pipe_group(data: &mut &str) -> ModalResult<WplPipe> {
    let group = wpl_group.parse_next(data)?;
    Ok(WplPipe::Group(group))
}
fn wpl_pipe_fun(data: &mut &str) -> ModalResult<WplPipe> {
    let fun = wpl_fun::wpl_fun.parse_next(data)?;
    Ok(WplPipe::Fun(fun))
}

fn parse_symbol(input: &mut &str, conf: &mut WplField) -> ModalResult<()> {
    if conf.meta_type == DataType::Symbol {
        //if conf.meta_name == "symbol" {
        let content = opt(take_parentheses).parse_next(input)?;
        conf.content = content.map(|x| x.to_string());
    }
    Ok(())
}
fn parse_peek_symbol(input: &mut &str, conf: &mut WplField) -> ModalResult<()> {
    if conf.meta_type == DataType::PeekSymbol {
        //if conf.meta_name == "peek_symbol" {
        let content = opt(take_parentheses).parse_next(input)?;
        conf.content = content.map(|x| x.to_string());
        conf.meta_name = DataType::Symbol.static_name().into();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_error::TestAssert;

    #[test]
    fn test_separator() {
        assert_eq!(wpl_end_sep_str.parse("\\!"), Ok("!".to_string()));
        assert_eq!(wpl_end_sep_str.parse("\\!\\~"), Ok("!~".to_string()));
        assert_eq!(wpl_end_sep_str.parse("\\!\\\""), Ok("!\"".to_string()));
        assert_eq!(wpl_end_sep_str.parse(r#"\!\""#), Ok("!\"".to_string()));
    }

    #[test]
    fn test_scope() {
        // <[,]>
        assert_eq!(
            delimiter_pair.parse(r#"<[,]>"#),
            Ok(("[".into(), "]".into()))
        );
        // <!,!>
        assert_eq!(
            delimiter_pair.parse(r#"<!,!>"#),
            Ok(("!".into(), "!".into()))
        );
        // <{!,!}>
        assert_eq!(
            delimiter_pair.parse(r#"<{!,!}>"#),
            Ok(("{!".into(), "!}".into()))
        );
        // <{!,}>
        assert_eq!(
            delimiter_pair.parse(r#"<{!,}>"#),
            Ok(("{!".into(), "}".into()))
        );
        // <{!,-}>
        assert_eq!(
            delimiter_pair.parse(r#"<{!,-}>"#),
            Ok(("{!".into(), "-}".into()))
        );
        // <{<,>}>
        assert_eq!(
            delimiter_pair.parse(r#"<{<,>}>"#),
            Ok(("{<".into(), ">}".into()))
        );
        // <<,>>
        assert!(delimiter_pair.parse(r#"<<,>>"#).is_ok());
        // <<,>>
        assert_eq!(
            delimiter_pair.parse(r#"<<,>>"#),
            Ok(("<".into(), ">".into()))
        );

        assert!(delimiter_pair.parse(r#"<[,>"#).is_err());

        assert_eq!(
            delimiter_pair.parse(r#"<<,>>"#),
            Ok(("<".into(), ">".into()))
        );
    }

    #[test]
    fn test_parse_end() {
        let conf = wpl_field_fmt.parse(r#"<"http://,/">"#).assert();

        assert_eq!(conf.scope_beg, Some("\"http://".to_string()));
        assert_eq!(conf.scope_end, Some("/\"".to_string()));

        let conf = wpl_field_fmt.parse(r#"<<,>>"#).assert();
        assert_eq!(conf.scope_beg, Some("<".to_string()));
        assert_eq!(conf.scope_end, Some(">".to_string()));

        assert!(wpl_field_fmt.parse(r#"<<,>"#).is_err());

        let conf = wpl_field_fmt.parse(r#"<|,|>"#).assert();
        assert_eq!(conf.scope_beg, Some("|".to_string()));
        assert_eq!(conf.scope_end, Some("|".to_string()));

        let conf = wpl_field_fmt.parse(r#"<|!,!|>"#).assert();
        assert_eq!(conf.scope_beg, Some("|!".to_string()));
        assert_eq!(conf.scope_end, Some("!|".to_string()));

        let conf = wpl_field_fmt.parse("\"").assert();
        assert_eq!(conf.scope_beg, Some("\"".to_string()));
        assert_eq!(conf.scope_end, Some("\"".to_string()));
    }
    #[test]
    fn test_wpl_sep() {
        let sep = wpl_sep.parse("\\,").assert();
        assert_eq!(sep, Some(WplSep::field_sep(",")));

        let sep = wpl_sep.parse("\\!\\,").assert();
        assert_eq!(sep, Some(WplSep::field_sep("!,")));
    }

    #[test]
    fn test_named_field_error() {
        assert!(wpl_id_field.parse_peek("digit2@src_ip: src-ip ").is_err());
    }

    #[test]
    fn test_named_field() {
        let (key, conf) = wpl_id_field.parse("@src_ip:src-ip").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.name, Some("src-ip".into()));

        let (key, conf) = wpl_id_field.parse("@src_ip:src-ip<[,]>").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.name, Some("src-ip".into()));
        assert_eq!(conf.fmt_conf.scope_beg, Some("[".to_string()));
        assert_eq!(conf.fmt_conf.scope_end, Some("]".to_string()));

        let (key, conf) = wpl_id_field.parse("time\"").assert();
        assert_eq!(key, DEFAULT_FIELD_KEY);
        assert_eq!(conf.name, None);
        assert_eq!(conf.fmt_conf.scope_beg, Some("\"".to_string()));
        assert_eq!(conf.fmt_conf.scope_end, Some("\"".to_string()));

        let (key, conf) = wpl_id_field.parse("@src_ip : src-ip").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name.as_str(), "chars");
        assert_eq!(conf.name, Some("src-ip".into()));

        let (key, conf) = wpl_id_field.parse("digit@src_ip: src-ip").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name, "digit".to_string());
        assert_eq!(conf.name, Some("src-ip".into()));

        let (key, conf) = wpl_id_field.parse("digit@src_ip: src-ip ").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name, "digit".to_string());
        assert_eq!(conf.name, Some("src-ip".into()));
        assert!(!conf.is_opt);

        let (key, conf) = wpl_id_field.parse("opt(digit)@src_ip: src-ip ").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name, "digit".to_string());
        assert_eq!(conf.name, Some("src-ip".into()));
        assert!(conf.is_opt);

        let (key, conf) = wpl_id_field.parse("opt( digit )@src_ip: src-ip ").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name, "digit".to_string());
        assert_eq!(conf.name, Some("src-ip".into()));
        assert!(conf.is_opt);

        let (key, conf) = wpl_id_field.parse("@src_ip ").assert();
        assert_eq!(key, "src_ip");
        assert_eq!(conf.meta_name, "chars".to_string());
    }

    #[test]
    fn test_named_field1() {
        let (key, conf) = wpl_id_field.parse("@process/path").assert();
        assert_eq!(key, "process/path");
        assert_eq!(conf.meta_name, "chars".to_string());

        let (key, conf) = wpl_id_field.parse("@process[0]/path").assert();
        assert_eq!(key, "process[0]/path");
        assert_eq!(conf.meta_name, "chars".to_string());
    }

    #[test]
    fn test_quoted_field_name() {
        // Test single-quoted field names with special characters
        let (key, conf) = wpl_id_field.parse("@'@abc'").assert();
        assert_eq!(key, "@abc");
        assert_eq!(conf.meta_name, "chars".to_string());

        // Test single-quoted field name with spaces
        let (key, conf) = wpl_id_field.parse("@'field with spaces'").assert();
        assert_eq!(key, "field with spaces");
        assert_eq!(conf.meta_name, "chars".to_string());

        // Test single-quoted field name with special characters
        let (key, conf) = wpl_id_field.parse("@'special-@field#123'").assert();
        assert_eq!(key, "special-@field#123");
        assert_eq!(conf.meta_name, "chars".to_string());

        // Test with type prefix
        let (key, conf) = wpl_id_field.parse("digit@'@special':field-name").assert();
        assert_eq!(key, "@special");
        assert_eq!(conf.meta_name, "digit".to_string());
        assert_eq!(conf.name, Some("field-name".into()));

        // Test in field set
        let set = wpl_field_subs
            .parse("(@'@field1':name1, @'field 2':name2)")
            .assert();
        let conf = set.get("@field1").assert();
        assert_eq!(conf.name, Some("name1".into()));
        let conf = set.get("field 2").assert();
        assert_eq!(conf.name, Some("name2".into()));
    }

    #[test]
    fn test_ext_obj() {
        let set = wpl_field_subs.parse("(digit@src_ip: src-ip )").assert();
        let conf = set.get("src_ip").assert();
        assert_eq!(conf.name, Some("src-ip".into()));

        let set = wpl_field_subs
            .parse("(@src_ip:src-ip ,@dst_ip:dst-ip )")
            .assert();
        let conf = set.get("src_ip").assert();
        assert_eq!(conf.name, Some("src-ip".into()));
        let conf = set.get("dst_ip").assert();
        assert_eq!(conf.name, Some("dst-ip".into()));

        let set = wpl_field_subs
            .parse("(digit@src_ip/beijing : src-ip/changsha ,digit@dst_ip : dst-ip )")
            .assert();
        let conf = set.get("src_ip/beijing").unwrap();
        assert_eq!(conf.name, Some("src-ip/changsha".into()));
        let conf = set.get("dst_ip").unwrap();
        assert_eq!(conf.name, Some("dst-ip".into()));

        let set = wpl_field_subs
            .parse("(digit@src_ip : src-ip ,digit@dst_ip : dst-ip )")
            .assert();
        let conf = set.get("src_ip").assert();
        assert_eq!(conf.name, Some("src-ip".into()));
        let conf = set.get("dst_ip").unwrap();
        assert_eq!(conf.name, Some("dst-ip".into()));
    }

    #[test]
    fn test_parse_field_conf() {
        let fmt = WplFieldFmt::default();
        //fmt.patten_first = Some(true);
        assert_eq!(
            wpl_field.parse("ip").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                fmt_conf: fmt.clone(),
                ..Default::default()
            }
            .build()
        );
        assert_eq!(
            wpl_field.parse("ip:ip_v4").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                name: Some("ip_v4".into()),
                fmt_conf: fmt.clone(),
                ..Default::default()
            }
            .build()
        );

        assert_eq!(
            wpl_field.parse("*ip").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                fmt_conf: fmt.clone(),
                ..Default::default()
            }
            .build()
        );

        assert_eq!(
            wpl_field.parse("*ip[10]").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                length: Some(10),
                fmt_conf: fmt.clone(),
                ..Default::default()
            }
            .build()
        );
        assert_eq!(
            wpl_field.parse("*ip[10]\\,").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                length: Some(10),
                fmt_conf: WplFieldFmt {
                    //separator: PrioSep::high(","),
                    //patten_first: Some(true),
                    ..Default::default()
                },
                separator: Some(WplSep::field_sep(",")),
                ..Default::default()
            }
            .build()
        );
        assert_eq!(
            wpl_field.parse("*ip:src[10]\\,").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                length: Some(10),
                name: Some("src".into()),
                fmt_conf: WplFieldFmt {
                    //separator: PrioSep::high(","),
                    //patten_first: Some(true),
                    ..Default::default()
                },
                separator: Some(WplSep::field_sep(",")),
                ..Default::default()
            }
            .build()
        );

        assert_eq!(
            wpl_field.parse("ip\\;\\!").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                fmt_conf: WplFieldFmt {
                    //separator: PrioSep::high(";!"),
                    //patten_first: Some(true),
                    ..Default::default()
                },
                separator: Some(WplSep::field_sep(";!")),
                ..Default::default()
            }
            .build()
        );

        assert_eq!(
            wpl_field.parse("*ip\\;").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                fmt_conf: WplFieldFmt {
                    //separator: PrioSep::high(";"),
                    //patten_first: Some(true),
                    ..Default::default()
                },
                separator: Some(WplSep::field_sep(";")),
                ..Default::default()
            }
            .build()
        );
        assert_eq!(
            wpl_field.parse_peek("ip:src;").assert().1,
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                name: Some("src".into()),
                fmt_conf: fmt.clone(),
                ..Default::default()
            }
            .build()
        );

        let field = WplField {
            meta_type: DataType::Chars,
            meta_name: "chars".into(),
            name: Some("src".into()),
            fmt_conf: WplFieldFmt {
                scope_beg: Some("[".to_string()),
                scope_end: Some("]".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
        .build();
        let p_field = wpl_field.parse("chars:src<[,]>").assert();
        assert_eq!(p_field, field);
        assert_eq!(
            wpl_field.parse("chars:src\"").assert(),
            WplField {
                meta_type: DataType::Chars,
                meta_name: "chars".into(),
                name: Some("src".into()),
                fmt_conf: WplFieldFmt {
                    scope_beg: Some("\"".to_string()),
                    scope_end: Some("\"".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            }
            .build()
        );

        assert_eq!(
            wpl_field.parse("3*ip[10]\\,").assert(),
            WplField {
                meta_type: DataType::IP,
                meta_name: "ip".into(),
                continuous: true,
                continuous_cnt: Some(3),
                length: Some(10),
                fmt_conf: WplFieldFmt {
                    //separator: PrioSep::high(","),
                    //patten_first: Some(true),
                    ..Default::default()
                },
                separator: Some(WplSep::field_sep(",")),
                ..Default::default()
            }
            .build()
        );

        let next = WplField {
            meta_type: DataType::Time,
            meta_name: "time".into(),
            desc: "time\"".to_string(),
            fmt_conf: WplFieldFmt {
                scope_beg: Some("\"".to_string()),
                scope_end: Some("\"".to_string()),
                //separator: PrioSep::infer_low(" "),
                //patten_first: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut confs = WplFieldSet::default();
        confs.add(DEFAULT_FIELD_KEY.to_string(), next);
        let mut conf = WplField {
            meta_type: DataType::KV,
            meta_name: "kv".into(),
            sub_fields: Some(confs),
            fmt_conf: WplFieldFmt {
                //separator: PrioSep::infer_low(" "),
                //patten_first: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        conf.setup();

        assert_eq!(wpl_field.parse("kv(time\")").assert(), conf);

        let code = "kv(time@a:cur_time)";
        let conf = wpl_field.parse(code).assert();
        assert_eq!(code, conf.to_string());
    }
}
