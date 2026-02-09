use super::super::prelude::*;
use winnow::combinator::delimited;
use winnow::token::{literal, take};
use wp_model_core::model::FNameStr;

use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::protocol::take_sub_tdo;
use crate::parser::utils::take_key;
use wp_parser::scope::ScopeEval;

derive_base_prs!(ProtoTextP);

impl ProtoTextP {
    const MAX_DEPTH: usize = 128;
    #[inline]
    fn max_depth(fpu: &FieldEvalUnit) -> usize {
        if let Some(cnt) = fpu.conf().field_cnt() {
            cnt
        } else if let Some(len) = fpu.conf().length() {
            *len
        } else {
            Self::MAX_DEPTH
        }
    }
}

impl PatternParser for ProtoTextP {
    fn pattern_parse<'a>(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        _name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let key = delimited(multispace0, take_key, multispace0).parse_next(data)?;

        if data.starts_with(':') {
            Ok(parse_proto_value(e_id, fpu, key.to_string(), data, out)?)
        } else if data.starts_with('{') {
            let val_len = ScopeEval::len(data, '{', '}');
            let proto_text = take(val_len).parse_next(data)?;
            let mut proto_text = &proto_text[1..proto_text.len() - 1];
            Self::parse_proto_object(e_id, fpu, key.to_string(), &mut proto_text, out, 1)?;
            Ok(())
        } else {
            // 不允许解析库因数据异常直接崩溃：返回结构化错误，保持与 parse_proto_object 的错误风格一致
            fail.context(ctx_desc("data proto-text format error"))
                .parse_next(data)
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("proto generate")
    }
}

fn parse_proto_value(
    e_id: u64,
    fpu: &FieldEvalUnit,
    key: String,
    data: &mut &str,
    out: &mut Vec<DataField>,
) -> ModalResult<()> {
    let _ = (literal(":"), multispace0).parse_next(data)?;
    let sep = WplSep::default();
    take_sub_tdo(e_id, fpu, &sep, data, key.as_str(), out)
}
impl ProtoTextP {
    pub fn parse_proto_object(
        e_id: u64,
        fpu: &FieldEvalUnit,
        root_key: String,
        data: &mut &str,
        out: &mut Vec<DataField>,
        depth: usize,
    ) -> ModalResult<()> {
        if depth > Self::max_depth(fpu) {
            return fail
                .context(ctx_desc("proto-text nested too deep"))
                .parse_next(&mut "");
        }
        while data.len().ne(&0) {
            // 更轻量的键解析：手动跳过空白 + 读取 key + 跳过空白
            multispace0.parse_next(data)?;
            let key = take_key.parse_next(data)?;
            multispace0.parse_next(data)?;
            // 预分配并拼接，避免 format! 开销
            let mut key_buf = String::with_capacity(root_key.len() + 1 + key.len());
            key_buf.push_str(&root_key);
            key_buf.push('/');
            key_buf.push_str(key);
            let key = key_buf;
            if data.starts_with(':') {
                parse_proto_value(e_id, fpu, key, data, out)?;
            } else if data.starts_with('{') {
                let val_len = ScopeEval::len(data, '{', '}');
                let proto_text = take(val_len).parse_next(data)?;
                let mut proto_text = &proto_text[1..proto_text.len() - 1];
                Self::parse_proto_object(e_id, fpu, key, &mut proto_text, out, depth + 1)?;
            } else {
                return fail
                    .context(ctx_desc("data proto-text format error"))
                    .parse_next(data);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::WplStatementType;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::parser::parse_code::wpl_express;
    use crate::parser::wpl_rule::wpl_rule;
    use crate::types::AnyResult;
    use orion_error::TestAssert;
    use wp_model_core::model::{DataRecord, DataType};
    use wp_model_core::model::data::record::RecordItem;
    use wp_parser::Parser;

    #[test]
    fn test_parse_proto_text_1() {
        let mut data = r#" obj  {serial_num: "cc38f5254b86b145e36805689f09a829" access_time: "2023-09-20 18:56:28.605" sip: "192.168.23.100" sport: 48625 dip: "6.6.6.6" dport: 53 dns_type: 0 host: "ck2aapvgwp2ro9vu7c.org" vendor_id: "warppase.ai" device_ip: "10.48.56.215"}"#;
        let conf = WplField::try_parse("proto_text").assert();
        let obj = ParserTUnit::from_auto(conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        println!("{}", DataRecord::from(obj));
    }
    #[test]
    fn test_parse_proto_text_2() {
        let mut data = r#"message_type: 5"#;
        let conf = WplField::try_parse("proto_text(digit@message_type)").assert();
        let obj = ParserTUnit::from_auto(conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        println!("{}", DataRecord::from(obj));
    }
    #[test]
    fn test_parse_proto_text_3() {
        let  conf = wpl_rule.parse(
            r#"rule test {(proto_text(digit@message_type), proto_text(@skyeye_dns/serial_num, chars@skyeye_dns/access_time, ip@skyeye_dns/sip, digit@skyeye_dns/sport, digit@skyeye_dns/dport, digit@skyeye_dns/dns_type))}"#,
        ).assert();
        let mut values = r#"message_type: 5 skyeye_dns {serial_num: "cc38f5254b86b145e36805689f09a829" access_time: "2023-09-20 18:56:28.605" sip: "192.168.23.100" sport: 48625 dip: "6.6.6.6" dport: 53 dns_type: 0 host: "ck2aapvgwp2ro9vu7c.org" vendor_id: "warppase.ai" device_ip: "10.48.56.215"}"#;
        let mut result = Vec::new();
        let sep = WplSep::default();
        let WplStatementType::Express(rule) = conf.statement;
        for f_conf in rule.group[0].fields.iter() {
            let fpu = FieldEvalUnit::for_test(ProtoTextP::default(), f_conf.clone());
            fpu.parse(0, &sep, &mut values, None, &mut result).assert();
            //result.append(&mut resp);
        }

        assert_eq!(result.len(), 11);
    }

    #[test]
    fn test_parse_proto_text_4() -> AnyResult<()> {
        let express = wpl_express.parse(
            r#"(proto_text(digit@message_type), proto_text(@skyeye_login/serial_num, chars@skyeye_login/access_time, ip@skyeye_login/sip, ip@skyeye_login/dip, digit@skyeye_login/sport, digit@skyeye_login/dport, _@skyeye_login/user_define/*))"#,
        ).assert();
        let mut values = r#"message_type: 7 skyeye_login {serial_num: "654613123_login" access_time: "2020-10-10 12:00:00" sip: "10.2.3.2" sport: 22 dip: "1.0.2.3" dport: 5432 proto: "proto" passwd: "46464864" info: "info" user: "admin" db_type: "alert" vendor_id: "1345456464" device_ip: "10.3.2.1" user_define {name: "user_name" type: "string" value: "qqqqq"}}"#;

        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut values).assert();
        println!("{}", result);
        assert_eq!(result.items.len(), 17);

        let mut ignore = 0;
        for i in result.items {
            if &DataType::Ignore == i.get_meta() {
                ignore += 1;
            }
        }
        assert_eq!(ignore, 3);
        Ok(())
    }

    #[test]
    fn test_parse_proto_text_invalid_not_panic() {
        // 非法输入：key 后既不是 ':' 也不是 '{'，应返回错误而非 panic
        let mut data = "message_type 5";
        let conf = WplField::try_parse("proto_text").assert();
        ParserTUnit::from_auto(conf.clone()).verify_parse_fail(&mut data);
    }

    #[test]
    fn test_parse_proto_text_too_deep() {
        // 构造超过最大深度的嵌套对象：a{b{c{...}}}
        let mut s = String::new();
        let depth = ProtoTextP::MAX_DEPTH + 2;
        for i in 0..depth {
            let ch = ((b'a' + (i % 26) as u8) as char).to_string();
            s.push_str(&format!("{} {{", ch));
        }
        for _ in 0..depth {
            s.push('}');
        }

        let conf = WplField::try_parse("proto_text").assert();
        ParserTUnit::from_auto(conf).verify_parse_fail(&mut s.as_str());
    }
}
