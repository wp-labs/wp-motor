use super::super::prelude::*;
use crate::ast::group::WplGroupType;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::physical::foundation::gen_chars;
use crate::eval::value::parser::{ParserFactory, protocol};
use crate::parser::utils::{quot_r_str, quot_str, take_key, window_path};
use wp_model_core::model::FNameStr;
derive_base_prs!(KeyValP);

// kv解析格式目前不支持：kv(digit)，必须要指定解析原始字段名称，这是为了解析同一类型的日志数据时，可以忽略有时候不存在的字段，减少规则书写条数
// 比如：(kv(@id,time@access_time,ip@sip,ip@sipv6)) 这个规则
// 既可以适配(id="fffff", time ="2022:11:02 23:11:02", sip="1.1.1.1")
// 也可以适配(id="fffff6", time ="2022:11:02 23:11:04", sipv6="::1")
impl PatternParser for KeyValP {
    fn pattern_parse(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        _name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let _ = multispace0.parse_next(data)?;
        let (key, _, _) =
            (take_key, multispace0, alt((literal(":"), literal("=")))).parse_next(data)?;
        match fpu.group_enum {
            WplGroupType::SomeOf(_) => value_take(e_id, fpu, ups_sep, data, key, out),
            _ => protocol::take_sub_tdo(e_id, fpu, ups_sep, data, key, out),
        }
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let key = gen_chars(gnc, 3, false);
        if let Some(conf) = g_conf {
            let meta = DataType::from(&conf.gen_type)?;
            let parser = ParserFactory::create(&meta)?;
            let sep = f_conf.resolve_sep(&WplSep::default());
            let field = parser.generate(gnc, &sep, f_conf, Some(conf))?;
            return Ok(field.data_field);
        }
        let val = gen_chars(gnc, 30, false);
        Ok(DataField::from_chars(key, val))
    }
}

/*
pub fn esc_normal_str(s: &str) -> IResult<&str, &str> {
    take_escaped(none_of(['\\', '"']), '\\', one_of(['"', 'n', '\\'])).parse_peek(s)
}
*/
fn value_take(
    e_id: u64,
    fpu: &FieldEvalUnit,
    upper_sep: &WplSep,
    data: &mut &str,
    key: &str,
    out: &mut Vec<DataField>,
) -> ModalResult<()> {
    multispace0.parse_next(data)?;
    let has_sub = fpu.get_sub_fpu(key).is_some();
    // 预先构造优先级分隔符，KV 的值默认读到行尾
    let mut p_sep = fpu.conf().resolve_sep(upper_sep);
    p_sep.apply_default(WplSep::inherited_sep("\\0"));

    // 优先尝试读取引号字符串或窗口路径（轻量分支）
    let str_val_r = alt((quot_r_str, quot_str, window_path)).parse_next(data);

    if has_sub {
        // 子解析器路径（保持原有语义）
        if let Some(sub_fpu) = fpu.get_sub_fpu(key) {
            match str_val_r {
                Ok(mut str_val) => {
                    let run_key = sub_fpu.conf().run_key(key);
                    let sep = fpu.conf().resolve_sep(&p_sep);
                    return sub_fpu.parse(e_id, &sep, &mut str_val, run_key, out);
                }
                Err(_) => {
                    let sep = fpu.conf().resolve_sep(&p_sep);
                    let run_key = sub_fpu.conf().run_key(key);
                    return sub_fpu.parse(e_id, &sep, data, run_key, out);
                }
            }
        }
    } else {
        // 无子配置快路径：直接产出原始值为 chars，避免子解析检索与额外分支
        match str_val_r {
            Ok(str_val) => {
                out.push(DataField::from_chars(key, str_val));
                return Ok(());
            }
            Err(_) => {
                let sep = p_sep; // 已设置到行尾
                let val = sep.read_until_sep(data)?;
                let trim_val = val.trim();
                out.push(DataField::from_chars(key, trim_val));
                return Ok(());
            }
        }
    }

    // 理论不可达：has_sub 为真但未取到 sub_fpu
    fail.context(ctx_desc("not found sub fpu "))
        .parse_next(data)
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use wp_parse_api::RawData;

    use crate::ast::WplField;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::types::AnyResult;
    use orion_error::TestAssert;
    use wp_model_core::model::DataRecord;

    use super::*;

    /*
    #[test]
    fn test_escaped() {
        assert_eq!(esc_normal_str(r#"1\"23;"#), Ok(("", r#"1\"23;"#)));
    }
    */

    #[test]
    fn test_key_name() -> AnyResult<()> {
        let mut data = r#"destination-zone="tr\"ust""#;
        let conf = WplField::try_parse(r#"kv(@destination-zone)"#).assert();
        let field = ParserTUnit::from_auto(conf).verify_parse_suc_meta(&mut data, DataType::Chars);
        assert_eq!(
            field[0],
            DataField::from_chars("destination-zone".to_string(), r#"tr\"ust"#)
        );

        let mut data = r#"time="2023-05-15 09:22:44" "#;
        let conf = WplField::try_parse(r#"kv(time@time\")"#).assert();
        let field =
            ParserTUnit::from_auto(conf).verify_parse_suc_end_meta(&mut data, DataType::Time);
        assert_eq!(
            field,
            vec![DataField::from_time("time".to_string(), {
                let d = NaiveDate::from_ymd_opt(2023, 5, 15).assert();
                let t = NaiveTime::from_hms_milli_opt(9, 22, 44, 0).unwrap();

                NaiveDateTime::new(d, t)
            }),]
        );

        Ok(())
    }
    #[test]
    fn test_key_2() -> AnyResult<()> {
        let mut data = "sys_name : 幻云 , msg_type:attack_log";
        let conf = WplField::try_parse(r#"kv(@sys_name)\,"#).assert();
        let field = ParserTUnit::new(KeyValP::default(), conf)
            .verify_parse_suc_meta(&mut data, DataType::Chars);
        assert_eq!(
            field,
            vec![DataField::from_chars(
                "sys_name".to_string(),
                "幻云".to_string()
            ),]
        );
        Ok(())
    }
    #[test]
    fn test_key_3() -> AnyResult<()> {
        let conf = WplField::try_parse(r#"kv(@time)"#).assert();
        let mut data = r#"time="2023-05-15 09:22:44" "#;
        let field = ParserTUnit::new(KeyValP::default(), conf)
            .verify_parse_suc_end_meta(&mut data, DataType::Chars);

        assert_eq!(
            field,
            vec![DataField::from_chars(
                "time".to_string(),
                "2023-05-15 09:22:44".to_string()
            ),]
        );
        Ok(())
    }
    #[test]
    fn test_key_4() -> AnyResult<()> {
        let mut data = r#"   pid:666, asid:100028"#;
        let conf = WplField::try_parse(r#"kv(@pid)\,,kv(@asid)\,"#).assert();
        let field = ParserTUnit::new(KeyValP::default(), conf)
            .verify_parse_suc_meta(&mut data, DataType::Chars);
        assert_eq!(
            field,
            vec![DataField::from_chars("pid".to_string(), "666".to_string()),]
        );
        Ok(())
    }
    #[test]
    fn test_kv_arr1() -> AnyResult<()> {
        let data = r#"dip=["1.1.1.1","2.2.2.2"]"#;
        let rule = r#" rule x { (kv(array/chars@dip))}"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);

        Ok(())
    }
    #[test]
    fn test_kv_arr2() -> AnyResult<()> {
        let data = r#"dip=[1.1.1.1,2.2.2.2]"#;
        let rule = r#" rule x { (kv(array/ip@dip))}"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        Ok(())
    }

    #[test]
    fn test_kv_arr3() -> AnyResult<()> {
        let data = r#"dip=[]"#;
        let rule = r#" rule x { (kv(array/ip@dip))}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        assert!(tdc.field("dip").is_some());

        let rule = r#" rule x { (kv(array/ip@dip))}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        assert!(tdc.field("dip").is_some());
        Ok(())
    }

    #[test]
    fn test_kv_arr4() -> AnyResult<()> {
        let data = r#"d=["1","1"]|e=["2","2"]|a= []|b=["3","3"]"#;
        let rule = r#" rule x { some_of(kv(array/chars))\| }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        Ok(())
    }
    /*
    #[test]
    fn test_key_5() -> AnyResult<()> {
        let rule = r#"rule x {(kv(digit@message_type),chars<skyeye_login {,>,kv, chars\} ) } "#;
        let pipe = LangPipe::from_code(rule)?;
        Ok(())
    }

     */
    #[test]
    fn test_key_point() -> AnyResult<()> {
        let mut data =
            r#"detail.sha256="2e7d8e43f518d2f2e54676069510bf48aa2289ca19c0f4165a1b6d4c18351ac9""#;
        let conf = WplField::try_parse(r#"kv"#).assert();
        let field = ParserTUnit::new(KeyValP::default(), conf)
            .verify_parse_suc_meta(&mut data, DataType::Chars);
        assert_eq!(
            field,
            vec![DataField::from_chars(
                "detail.sha256".to_string(),
                "2e7d8e43f518d2f2e54676069510bf48aa2289ca19c0f4165a1b6d4c18351ac9".to_string()
            ),]
        );
        let mut data =
            r#"detail.sha256="2e7d8e43f518d2f2e54676069510bf48aa2289ca19c0f4165a1b6d4c18351ac9""#;
        let conf = WplField::try_parse(r#"kv(@detail.sha256)"#).assert();
        let field = ParserTUnit::new(KeyValP::default(), conf)
            .verify_parse_suc_meta(&mut data, DataType::Chars);
        assert_eq!(
            field,
            vec![DataField::from_chars(
                "detail.sha256".to_string(),
                "2e7d8e43f518d2f2e54676069510bf48aa2289ca19c0f4165a1b6d4c18351ac9".to_string()
            ),]
        );
        Ok(())
    }

    #[test]
    fn test_key_diy_sep() -> AnyResult<()> {
        let data = r#"x.a="hello"!|x.b="18"!|x.c=20"#;
        let rule = r#" rule x { (kv(chars@x.a:y.a),kv(chars@x.b),kv(digit@x.c))\!\|} "#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        let expected = vec![
            DataField::from_chars("y.a".to_string(), "hello".to_string()),
            DataField::from_chars("x.b".to_string(), "18".to_string()),
            DataField::from_digit("x.c".to_string(), 20),
        ];
        assert_eq!(
            tdc.items.iter().map(|s| s.as_field()).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        Ok(())
    }
    #[test]
    fn test_kv_chars2() -> AnyResult<()> {
        let rule = r#" rule x {(chars:content), some_of( kv(chars<",">@event_content) ) } "#;
        let data = r#""主机172.16.12.20存在可疑进程参数问题，进程fscan_amd64的启动参数为./fscan_amd64 -h 172.16.12.0/24，符合可疑进程参数的特性。" event_content="主机172.16.12.20存在可疑进程参数问题，进程fscan_amd64的启动参数为./fscan_amd64 -h 172.16.12.0/24，符合可疑进程参数的特性。"
"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        assert_eq!(
            tdc.get_field("content"),
            Some(&DataField::from_chars(
                "content",
                r#"主机172.16.12.20存在可疑进程参数问题，进程fscan_amd64的启动参数为./fscan_amd64 -h 172.16.12.0/24，符合可疑进程参数的特性。"#
            ))
        );
        assert_eq!(
            tdc.get_field("event_content"),
            Some(&DataField::from_chars(
                "event_content",
                r#"主机172.16.12.20存在可疑进程参数问题，进程fscan_amd64的启动参数为./fscan_amd64 -h 172.16.12.0/24，符合可疑进程参数的特性。"#
            ))
        );

        Ok(())
    }

    #[test]
    fn test_kv_chars1() -> AnyResult<()> {
        let rule = r#" rule x {(chars:a\\\s), some_of( kv(chars@b), ), (json()) } "#;
        //let rule = r#" rule x {(chars:a, kv(chars@b),  json) } "#;
        //let rule = r#" rule x {(chars:a, kv,  json) } "#;
        let data = r#"sddD:\招标项目\6-MSS\mss日志映射表 b="sddD:\招标项目\6-MSS\mss日志映射表" {"c":"sddD:\\招标项目\\6-MSS\\mss日志映射表"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        println!("{}", tdc);
        assert_eq!(
            tdc.get_field("b"),
            Some(&DataField::from_chars(
                "b",
                r#"sddD:\招标项目\6-MSS\mss日志映射表"#
            ))
        );
        assert_eq!(
            tdc.get_field("c"),
            Some(&DataField::from_chars(
                "c",
                r#"sddD:\\招标项目\\6-MSS\\mss日志映射表"#
            ))
        );
        Ok(())
    }

    #[test]
    fn test_kv_multi_keys() -> AnyResult<()> {
        let rule = r#" rule x { (kv,kv(digit),kv(digit@x.c, digit@x.c1:x.c))\!\|} "#;
        let data = r#"x.a="hello"!|x.b=18!|x.c=20"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        asert_kv_x_obj(tdc);
        let data = r#"x.a="hello"!|x.b=18!|x.c1=20"#;
        let (tdc, _) = pipe.proc(0, RawData::from_string(data.to_string()), 0)?;
        asert_kv_x_obj(tdc);
        Ok(())
    }

    fn asert_kv_x_obj(tdc: DataRecord) {
        let expected = vec![
            DataField::from_chars("x.a".to_string(), "hello".to_string()),
            DataField::from_digit("x.b", 18,),
            DataField::from_digit("x.c".to_string(), 20),
        ];
        assert_eq!(
            tdc.items.iter().map(|s| s.as_field()).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
    }
}
