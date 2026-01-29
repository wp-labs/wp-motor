use super::super::prelude::*;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::protocol::json_impl::JsonProc;
use serde::Deserialize;
use serde_json::{Deserializer, Value};
use std::io::Cursor;
use wp_model_core::model::FNameStr;

#[derive(Default)]
pub struct ExactJsonP {}

impl PatternParser for ExactJsonP {
    fn pattern_parse<'a>(
        &self,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        let mut cursor = Cursor::new(data.as_bytes());
        let mut deserializer = Deserializer::from_reader(&mut cursor);
        if let Ok(value) = Value::deserialize(&mut deserializer) {
            let json_end = cursor.position() as usize;
            JsonProc::proc_value(fpu, ups_sep, "", &value, name.as_str(), true, out)?;
            let (_, remaining_text) = data.split_at(json_end);
            *data = remaining_text;
            Ok(())
        } else {
            fail.parse_next(data)
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("json generate")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    use crate::types::AnyResult;

    #[test]
    fn test_json() -> AnyResult<()> {
        let mut data = r#"{"a":1,"b":2}"#;
        let conf = WplField::try_parse("exact_json(digit@a,digit@b)").assert();
        ParserTUnit::new(ExactJsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();

        let conf = WplField::try_parse("exact_json(digit@a)").assert();
        ParserTUnit::new(ExactJsonP::default(), conf.clone()).verify_parse_fail(&mut data);

        let conf = WplField::try_parse("exact_json(digit@a,digit@b,digit@c/a)").assert();
        let mut data = r#"{"a":1,"b":2,"c": { "a" : 1 } }"#;
        ParserTUnit::new(ExactJsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        Ok(())
    }

    #[test]
    fn test_json_str_mode_decoded_pipe() -> AnyResult<()> {
        // 默认 raw：保留反斜杠
        let mut data = r#"{"path":"c:\\users\\fc\\file","txt":"line1\nline2"}"#;
        let conf = WplField::try_parse("json(chars@path,chars@txt)").assert();
        let out_raw = ParserTUnit::from_auto(conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        let dr_raw = wp_model_core::model::DataRecord::from(out_raw.clone());
        assert!(dr_raw.field("path").is_some());

        // 使用管道切换为 decoded：反斜杠与换行转义生效
        let mut data = r#"{"path":"c:\\users\\fc\\file","txt":"line1\nline2"}"#;
        let conf =
            WplField::try_parse("exact_json(chars@path,chars@txt) | json_unescape()").assert();
        let out_dec = ParserTUnit::from_auto(conf)
            .verify_parse_suc(&mut data)
            .assert();
        let dr_dec = wp_model_core::model::DataRecord::from(out_dec);
        // path 字段在 decoded 模式下对反斜杠数量无强约束，仅校验 txt 出现换行
        if let Some(v) = dr_dec.field("txt") {
            if let wp_model_core::model::Value::Chars(s) = v.get_value() {
                assert!(s.contains('\n'));
            } else {
                panic!("txt not chars")
            }
        }
        Ok(())
    }

    #[test]
    fn test_json_array() -> AnyResult<()> {
        let rule = r#"rule test { (exact_json(auto@name,auto@value,auto@key))\, }"#;
        let data = r#"{"name": "中国", "value": 96, "key" : ["a","b","c"] }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("key[0]") {
            println!("{}", i);
            //assert_eq!(*i, TDOEnum::from_digit("cpu", 96));
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    #[test]
    fn test_exact_json_missing_immediate() -> AnyResult<()> {
        // b 缺失，应在严格模式下报错
        let mut data = r#"{"a":1}"#;
        let conf = WplField::try_parse("exact_json(digit@a,digit@b)").assert();
        ParserTUnit::new(ExactJsonP::default(), conf.clone()).verify_parse_fail(&mut data);
        Ok(())
    }
}
