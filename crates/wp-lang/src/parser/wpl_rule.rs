use super::wpl_anno::ann_fun;
use crate::ast::{WplField, WplRule, WplStatementType};
use crate::parser::wpl_field::wpl_field;
use crate::parser::{parse_code, utils};
use smol_str::SmolStr;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt, repeat};
use winnow::error::StrContext;
use winnow::token::literal;
use wp_parser::Parser;
use wp_parser::symbol::{ctx_desc, ctx_label, ctx_literal};

/// 预处理规则
/// | base64 |
/// | base64 | zip|
/// | base64|
/// |base64|quote|
/// | hex |
fn take_plg_pipe_step(input: &mut &str) -> wp_parser::WResult<SmolStr> {
    (
        literal("plg_pipe"),
        multispace0,
        alt((
            (literal('/'), multispace0, utils::take_key)
                .map(|x| SmolStr::from(format!("plg_pipe/{}", x.2))),
            (
                literal('('),
                multispace0,
                utils::take_key,
                multispace0,
                literal(')'),
            )
                .map(|x| SmolStr::from(format!("plg_pipe/{}", x.2))),
        )),
    )
        .map(|x| x.2)
        .parse_next(input)
}

pub fn pip_proc(input: &mut &str) -> wp_parser::WResult<Vec<SmolStr>> {
    let x: Vec<_> = repeat(
        1..,
        (
            literal('|'),
            multispace0,
            alt((
                take_plg_pipe_step.context(StrContext::Label("expect plg_pipe/<name>")),
                utils::take_key
                    .context(StrContext::Label("expect [a-z],[A-Z],[/],[_]"))
                    .map(SmolStr::from),
            )),
            multispace0,
        )
            .map(|x| x.2),
    )
    .parse_next(input)?;
    if !x.is_empty() {
        literal("|")
            .context(StrContext::Label("end with '|'"))
            .parse_next(input)?;
    }
    Ok(x)
}

pub fn wpl_rule(input: &mut &str) -> wp_parser::WResult<WplRule> {
    let atags = opt(ann_fun).parse_next(input)?;
    (multispace0, "rule", multispace0)
        .context(ctx_label("wpl keyword"))
        .context(ctx_desc("rule"))
        .parse_next(input)?;
    let rule_name = utils::take_exact_path
        .context(ctx_desc("<<< rule <name>"))
        .parse_next(input)?;
    (multispace0, "{", multispace0).parse_next(input)?;

    let stm = WplStatementType::Express(
        parse_code::wpl_express
            .context(ctx_label("group"))
            .context(ctx_desc("+<group>"))
            .parse_next(input)?,
    );
    (multispace0, "}", multispace0)
        .context(ctx_literal("}"))
        .context(ctx_desc("rule end"))
        .parse_next(input)?;
    let rule = WplRule::new(rule_name.to_string(), stm);
    Ok(rule.add_tags(atags))
}

pub(crate) fn wpl_field_vec(input: &mut &str) -> wp_parser::WResult<Vec<WplField>> {
    let mut field_vec = Vec::new();
    multispace0.parse_next(input)?;
    while utils::peek_next((multispace0, ")"), input).is_err() && !input.is_empty() {
        let field = wpl_field.context(ctx_desc("<field>")).parse_next(input)?;
        field_vec.push(field);
        if utils::peek_next((multispace0, ")"), input).is_err() && !input.is_empty() {
            (multispace0, ",")
                .context(ctx_label("symbol"))
                .context(ctx_literal(","))
                .context(ctx_desc("next field"))
                .parse_next(input)?;
        }
    }
    Ok(field_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::wpl_rule;
    use orion_error::TestAssert;

    #[test]
    fn tes_parse_multi_fields() {
        assert_eq!(
            wpl_field_vec
                .parse(
                    r#"ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_""#
                )
                .assert()
                .len(),
            9
        );
    }
    #[test]
    fn test_rule() {
        let data = "rule wparse_3 { (digit,time,sn,chars,time,kv,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request,http/agent)\\,,\n(digit,time,ip,ip,sn,kv,ip,kv,chars,kv,kv,kv,chars,kv,chars,chars,ip,chars,http/request,http/agent)\\, }";
        let _ = wpl_rule::wpl_rule.parse(data).assert();

        let data = "rule\n sys/name_1 \n { \n (digit,\ntime,\nsn)\\,,\n(digit,\ntime,\nip,\nip\n,sn)\\,\n}";
        let _ = wpl_rule::wpl_rule.parse(data).assert();

        let data = "rule wparse_4 {  (digit,time,sn,chars,time,kv,sn,chars,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent)\\, }\n";
        let _ = wpl_rule::wpl_rule.parse(data).assert();

        let data = r#"rule ip_addr { (chars:first",_,chars:addr",_,_,chars:city_name",_,_,chars:country_cn",_,_,_,_,float:latitude,float:longitude,_,_,_,_,_,_,_,digit:ip_beg,digit:ip_end)\,}"#;
        let conf = wpl_rule::wpl_rule.parse(data).assert();
        assert_eq!(
            conf.statement.first_field().unwrap().name,
            Some("first".into())
        );

        let data = r#"rule ip_addr  { (chars:first", _,chars:addr",_,_,chars:city_name",_,_,chars:country_cn",_,_,_,_,float:latitude,float:longitude,_,_,_,_,_,_,_,digit:ip_beg,digit:ip_end)\,}"#;
        let conf = wpl_rule::wpl_rule.parse(data).assert();
        assert_eq!(
            conf.statement.first_field().unwrap().name,
            Some("first".into())
        );

        let data = r#"
        rule /service/for_test/wplab_1 {
            (digit<<,>>,digit,time_3339:recv_time,5*_),
            (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
}
"#;
        let _ = wpl_rule::wpl_rule.parse(data).assert();
        let data = r#" rule x { (kv(digit@message_type),kv(chars@serial_num))\!\| } "#;
        let _ = wpl_rule::wpl_rule.parse(data).assert();
    }

    #[test]
    fn test_plg_pipe_preproc() {
        let mut input = "| plg_pipe/mock_stage | decode/base64 |";
        let items = pip_proc.parse_next(&mut input).assert();
        assert_eq!(items, vec!["plg_pipe/mock_stage", "decode/base64"]);
    }
}
