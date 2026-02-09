use super::super::prelude::*;
use crate::ast::{DefaultSep, WplSepT};
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::parser::utils::{decode_escapes, interval_data, quot_str, take_key, take_to_end};
use serde_json::{Number, Value};
use std::collections::HashMap;
use winnow::token::{rest, take_until};
use wp_model_core::model::FNameStr;

#[derive(Default, Clone)]
pub struct KvArrP {}
impl KvArrP {}

impl DefaultSep for KvArrP {
    fn sep_str() -> &'static str {
        ","
    }
}

// Supports lines like:
// a="a", b ="x" , c = 1 , d = 1.2
// a="a" b= "x" c=1 d=1.2
impl PatternParser for KvArrP {
    fn pattern_parse(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        _name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        let mut parsed = 0usize;
        let mut emitted_ranges: Vec<(String, usize, usize)> = Vec::new();
        let cur_sep = WplSepT::<Self>::from(ups_sep);
        loop {
            Self::consume_delimiter(data)?;
            if data.is_empty() {
                break;
            }
            let cp = data.checkpoint();
            match Self::take_pair(data, &cur_sep) {
                Ok((key, value)) => {
                    parsed += 1;
                    let start_idx = out.len();
                    Self::emit_value(e_id, fpu, ups_sep, key.as_str(), value, out)?;
                    let end_idx = out.len();
                    if end_idx > start_idx {
                        emitted_ranges.push((key, start_idx, end_idx));
                    }

                    Self::consume_trailing(data, &cur_sep)?;
                }
                Err(_) => {
                    if parsed == 0 {
                        return fail
                            .context(ctx_desc("kvarr requires key=value entries"))
                            .parse_next(data);
                    } else {
                        data.reset(&cp);
                        break;
                    }
                }
            }
        }
        if parsed == 0 {
            return fail
                .context(ctx_desc("kvarr requires key=value entries"))
                .parse_next(data);
        }
        multispace0.parse_next(data)?;
        if !data.is_empty() {
            return fail
                .context(ctx_desc("kvarr parse trailing characters"))
                .parse_next(data);
        }

        Self::rename_duplicates(out, emitted_ranges);
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("kvarr generate")
    }
}

impl KvArrP {
    fn take_pair(data: &mut &str, sep: &WplSepT<Self>) -> ModalResult<(String, Value)> {
        multispace0.parse_next(data)?;
        let key = take_key.parse_next(data)?;
        multispace0.parse_next(data)?;
        alt((literal('='), literal(':')))
            .context(ctx_desc("kv missing '=' or ':' "))
            .parse_next(data)?;
        let value = Self::take_value(data, sep)?;
        Ok((key.to_string(), value))
    }

    fn take_value(input: &mut &str, sep: &WplSepT<Self>) -> ModalResult<Value> {
        multispace0.parse_next(input)?;
        if input.is_empty() {
            return fail
                .context(ctx_desc("kvarr value missing"))
                .parse_next(input);
        }
        if let Ok(val) = quot_str.parse_next(input) {
            return Ok(Value::String(val.to_string()));
        }
        if let Ok(val) = interval_data.parse_next(input) {
            let normalized = decode_escapes(val);
            return Ok(Value::String(normalized));
        }
        let raw = Self::take_unquoted(input, sep)?;
        Ok(Self::scalar_value(raw.as_str()))
    }

    fn take_unquoted(input: &mut &str, sep: &WplSepT<Self>) -> ModalResult<String> {
        let s = *input;
        if s.is_empty() {
            return fail
                .context(ctx_desc("kvarr value missing"))
                .parse_next(input);
        }
        let data = if sep.is_to_end() {
            take_to_end.parse_next(input)?
        } else {
            let sep = sep.sep_str();
            alt((take_until(0.., sep), rest)).parse_next(input)?
        };
        Ok(data.to_string())
    }

    fn scalar_value(raw: &str) -> Value {
        if raw.eq_ignore_ascii_case("true") {
            Value::Bool(true)
        } else if raw.eq_ignore_ascii_case("false") {
            Value::Bool(false)
        } else if let Ok(i) = raw.parse::<i64>() {
            Value::Number(Number::from(i))
        } else if let Ok(f) = raw.parse::<f64>() {
            if let Some(num) = Number::from_f64(f) {
                Value::Number(num)
            } else {
                Value::String(raw.to_string())
            }
        } else {
            Value::String(raw.to_string())
        }
    }

    #[inline]
    fn consume_delimiter(data: &mut &str) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        Ok(())
    }

    fn consume_trailing(data: &mut &str, sep: &WplSepT<Self>) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        if !sep.is_to_end() {
            opt(sep.sep_str()).parse_next(data)?;
        }
        Ok(())
    }

    fn emit_value(
        e_id: u64,
        fpu: &FieldEvalUnit,
        upper_sep: &WplSep,
        key: &str,
        value: Value,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        if let Some(sub_fpu) = fpu.get_sub_fpu(key) {
            if let Some(raw) = Self::value_to_raw(&value) {
                let mut sep = sub_fpu.conf().resolve_sep(upper_sep);
                if sep.is_space_sep() {
                    sep.set_current("\\0");
                }
                let run_key = sub_fpu.conf().run_key(key);
                let mut raw_ref = raw.as_str();
                sub_fpu.parse(e_id, &sep, &mut raw_ref, run_key, out)?;
            }
            return Ok(());
        }
        match value {
            Value::Null => Ok(()),
            Value::Bool(b) => {
                out.push(DataField::from_bool(key, b));
                Ok(())
            }
            Value::Number(num) => {
                if let (true, Some(f)) = (num.is_f64(), num.as_f64()) {
                    out.push(DataField::from_float(key, f));
                } else if let Some(i) = num.as_i64() {
                    out.push(DataField::from_digit(key, i));
                } else if let Some(u) = num.as_u64() {
                    if u <= i64::MAX as u64 {
                        out.push(DataField::from_digit(key, u as i64));
                    } else {
                        out.push(DataField::from_chars(key, num.to_string()));
                    }
                } else {
                    out.push(DataField::from_chars(key, num.to_string()));
                }
                Ok(())
            }
            Value::String(s) => {
                out.push(DataField::from_chars(key, s));
                Ok(())
            }
            Value::Array(vals) => {
                for v in vals {
                    Self::emit_value(e_id, fpu, upper_sep, key, v, out)?;
                }
                Ok(())
            }
            Value::Object(obj) => {
                for (sub, v) in obj {
                    let composed = format!("{}/{}", key, sub);
                    Self::emit_value(e_id, fpu, upper_sep, composed.as_str(), v, out)?;
                }
                Ok(())
            }
        }
    }

    fn value_to_raw(value: &Value) -> Option<String> {
        match value {
            Value::Null => None,
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(num) => Some(num.to_string()),
            Value::String(s) => Some(s.clone()),
            Value::Array(arr) => Some(Value::Array(arr.clone()).to_string()),
            Value::Object(obj) => Some(Value::Object(obj.clone()).to_string()),
        }
    }

    fn rename_duplicates(out: &mut [DataField], emitted: Vec<(String, usize, usize)>) {
        let mut dup: HashMap<String, Vec<usize>> = HashMap::new();
        for (key, start, end) in emitted {
            if end == start + 1
                && let Some(field) = out.get(start)
                && field.get_name() == key
            {
                dup.entry(key).or_default().push(start);
            }
        }
        for (key, positions) in dup {
            if positions.len() <= 1 {
                continue;
            }
            for (idx, pos) in positions.into_iter().enumerate() {
                if let Some(field) = out.get_mut(pos) {
                    let new_name = format!("{}[{}]", key, idx);
                    field.set_name(new_name);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, str::FromStr};

    use super::*;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::types::AnyResult;
    use crate::{WplEvaluator, ast::WplField};
    use orion_error::TestAssert;
    use wp_model_core::model::{DataField, DataRecord, data::Field};

    #[test]
    fn test_kvarr_with_commas() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(ip@sip, digit@cnt)").assert();
        let mut data = "sip=\"192.168.1.1\", cnt=42";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_sip = DataField::from_ip("sip", IpAddr::from_str("192.168.1.1").unwrap());
        assert_eq!(
            record.field("sip").map(|s| s.as_field()),
            Some(&expected_sip)
        );
        let expected_cnt = DataField::from_digit("cnt", 42);
        assert_eq!(
            record.field("cnt").map(|s| s.as_field()),
            Some(&expected_cnt)
        );
        Ok(())
    }

    fn take_to_sep<'a>(data: &'a mut &'a str) -> ModalResult<&'a str> {
        alt((take_until(0.., ","), rest)).parse_next(data)
    }
    #[test]
    fn test_kvarr_with_commas2() -> AnyResult<()> {
        let mut data = "msg = hello boy,cnt=42";
        assert!(take_to_sep(&mut data).is_ok());

        let conf = WplField::try_parse("kvarr(digit@cnt, array/ip@c)\\,").assert();
        let mut data = r#"msg = hello boy,cnt=42 ,c=[\"1.1.1.1\",\"2.2.2.2\"]"#;
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_msg = DataField::from_chars("msg", "hello boy");
        assert_eq!(
            record.field("msg").map(|s| s.as_field()),
            Some(&expected_msg)
        );
        let expected_cnt = DataField::from_digit("cnt", 42);
        assert_eq!(
            record.field("cnt").map(|s| s.as_field()),
            Some(&expected_cnt)
        );

        let arr = vec![
            Field::from_ip("c/[0]", IpAddr::from_str("1.1.1.1").unwrap()),
            Field::from_ip("c/[1]", IpAddr::from_str("2.2.2.2").unwrap()),
        ];
        let expected_c = DataField::from_arr("c", arr);
        assert_eq!(record.field("c").map(|s| s.as_field()), Some(&expected_c));

        Ok(())
    }

    #[test]
    fn test_kvarr_whitespace_delimited() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(chars@a, chars@b, digit@c)\\s").assert();
        let mut data = "a=\"foo\" b='bar x' c=1";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_a = DataField::from_chars("a", "foo");
        assert_eq!(record.field("a").map(|s| s.as_field()), Some(&expected_a));
        let expected_b = DataField::from_chars("b", "bar x");
        assert_eq!(record.field("b").map(|s| s.as_field()), Some(&expected_b));
        let expected_c = DataField::from_digit("c", 1);
        assert_eq!(record.field("c").map(|s| s.as_field()), Some(&expected_c));
        Ok(())
    }

    #[test]
    fn test_kvarr_fun() -> AnyResult<()> {
        let rule = r#"rule test { (kvarr(chars@a, chars@b, digit@c)\s | f_chars_has(a,foo) ) }"#;
        let data = "a=\"foo\" b=bar c=1";
        let pipe = WplEvaluator::from_code(rule)?;
        let (record, _) = pipe.proc(0, data, 0)?;
        let expected_a = DataField::from_chars("a", "foo");
        assert_eq!(record.field("a").map(|s| s.as_field()), Some(&expected_a));
        let rule = r#"rule test { (kvarr(chars@a, chars@b, digit@c)\s | f_chars_has(a,foox) ) }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_err());
        Ok(())
    }

    #[test]
    fn test_kvarr_duplicate_keys_to_array() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(chars@tag, digit@count)\\s").assert();
        let mut data = "tag=alpha tag=beta count=3";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_tag0 = DataField::from_chars("tag[0]", "alpha");
        assert_eq!(
            record.field("tag[0]").map(|s| s.as_field()),
            Some(&expected_tag0)
        );
        let expected_tag1 = DataField::from_chars("tag[1]", "beta");
        assert_eq!(
            record.field("tag[1]").map(|s| s.as_field()),
            Some(&expected_tag1)
        );
        let expected_count = DataField::from_digit("count", 3);
        assert_eq!(
            record.field("count").map(|s| s.as_field()),
            Some(&expected_count)
        );
        Ok(())
    }

    #[test]
    fn test_kvarr_repeated_keys_are_indexed() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(chars@tag)\\s").assert();
        let mut data = "tag=alpha tag=beta tag=gamma";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_tag0 = DataField::from_chars("tag[0]", "alpha");
        assert_eq!(
            record.field("tag[0]").map(|s| s.as_field()),
            Some(&expected_tag0)
        );
        let expected_tag1 = DataField::from_chars("tag[1]", "beta");
        assert_eq!(
            record.field("tag[1]").map(|s| s.as_field()),
            Some(&expected_tag1)
        );
        let expected_tag2 = DataField::from_chars("tag[2]", "gamma");
        assert_eq!(
            record.field("tag[2]").map(|s| s.as_field()),
            Some(&expected_tag2)
        );
        Ok(())
    }

    #[test]
    fn test_kvarr_type_inference() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(bool@flag, float@ratio, chars@raw)\\s").assert();
        let mut data = "flag=true ratio=1.25 raw=value";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_flag = DataField::from_bool("flag", true);
        assert_eq!(
            record.field("flag").map(|s| s.as_field()),
            Some(&expected_flag)
        );
        let expected_ratio = DataField::from_float("ratio", 1.25);
        assert_eq!(
            record.field("ratio").map(|s| s.as_field()),
            Some(&expected_ratio)
        );
        let expected_raw = DataField::from_chars("raw", "value");
        assert_eq!(
            record.field("raw").map(|s| s.as_field()),
            Some(&expected_raw)
        );
        Ok(())
    }

    #[test]
    fn test_kvarr_ignore_meta() -> AnyResult<()> {
        let conf = WplField::try_parse("kvarr(_@note, digit@count)\\s").assert();
        let mut data = "note=something count=7";
        let parser = ParserTUnit::new(KvArrP::default(), conf);
        let fields = parser.verify_parse_suc(&mut data).assert();
        let record = DataRecord::from(fields);
        let expected_note = DataField::from_ignore("note");
        assert_eq!(
            record.field("note").map(|s| s.as_field()),
            Some(&expected_note)
        );
        let expected_count = DataField::from_digit("count", 7);
        assert_eq!(
            record.field("count").map(|s| s.as_field()),
            Some(&expected_count)
        );
        Ok(())
    }
}
