use super::super::prelude::*;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::generator::ParserValue;
use wp_model_core::model::FNameStr;

use crate::parser::utils::quot_str;

use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{fail, preceded, separated};
use wp_model_core::model::DigitValue;
use wp_parser::symbol::{ctx_desc, symbol_brackets_beg, symbol_brackets_end};

#[derive(Default)]
pub struct ArrayP {}

impl ArrayP {
    pub fn new() -> Self {
        Self {}
    }

    fn parse_array_element(
        next_fpu: &FieldEvalUnit,
        sep: &WplSep,
        name: &FNameStr,
        idx_local: &mut usize,
        out: &mut Vec<DataField>,
        input: &mut &str,
    ) -> ModalResult<()> {
        let idx_str = format!("{}/[{}]", name, idx_local);
        let mut probe = *input;
        match next_fpu.parse(0, sep, &mut probe, Some(idx_str.clone().into()), out) {
            Ok(_) => {
                *idx_local += 1;
                *input = probe;
                Ok(())
            }
            Err(_) => {
                // 尝试去除包裹的引号（常见于 JSON 数组元素）
                let mut quoted_probe = *input;
                if let Ok(inner) = quot_str.parse_next(&mut quoted_probe) {
                    let mut inner_ref = inner;
                    next_fpu.parse(0, sep, &mut inner_ref, Some(idx_str.into()), out)?;
                    *idx_local += 1;
                    *input = quoted_probe;
                    Ok(())
                } else {
                    let mut bad = "";
                    let dyn_label = format!("array/[{}]", idx_local);
                    let _: &str = alt((dyn_label.as_str(), "array element"))
                        .context(ctx_desc("array element failed"))
                        .parse_next(&mut bad)?;
                    unreachable!("array element error should not succeed");
                }
            }
        }
    }
}

impl ParserValue<DigitValue> for ArrayP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<DigitValue> {
        preceded(multispace0, digit1.try_map(str::parse::<DigitValue>)).parse_next(data)
    }
}

impl PatternParser for ArrayP {
    fn pattern_parse<'a>(
        &self,
        _e_id: u64,
        fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mut tdo_arr: Vec<DataField> = Vec::with_capacity(10);
        // element index is generated inside sub parser (idx_local)
        if let Some(next_fpu) = fpu.next() {
            let cur_sep = WplSep::field_sep_until(",", "]", false);
            // 包装子解析器：为每个元素增加错误上下文 array/[index]
            let mut idx_local = 0usize;
            let mut sub_parser = |input: &mut &str| {
                Self::parse_array_element(
                    next_fpu,
                    &cur_sep,
                    &name,
                    &mut idx_local,
                    &mut tdo_arr,
                    input,
                )
            };

            symbol_brackets_beg.parse_next(data)?;
            // 允许空数组与尾随逗号：[1,2,] 或 []
            let _: Vec<()> = separated(0.., sub_parser.by_ref(), ",").parse_next(data)?;
            // 可选的尾随逗号，在收尾 ']' 前允许一次（含空白）
            let _ = preceded(multispace0, opt(literal(","))).parse_next(data)?;
            symbol_brackets_end.parse_next(data)?;
            out.push(DataField::from_arr(name, tdo_arr));
            return Ok(());
        }
        fail.parse_next(data)
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        // 最小实现：根据 array 的子类型生成 2 个样例元素，并包装为数组字段
        use crate::eval::value::parser::ParserFactory;
        // no local imports needed; use ParserFactory directly
        use wp_model_core::model::DataType;

        let name = _f_conf.safe_name();
        // 推断子类型：array/<sub>，未指定则默认为 chars
        let elem_meta = match &_f_conf.meta_type {
            DataType::Array(sub) => DataType::from(sub).unwrap_or(DataType::Chars),
            _ => DataType::Chars,
        };

        // 构造元素字段配置，并用对应解析器生成 2 个元素
        let mut items: Vec<DataField> = Vec::with_capacity(2);
        let elem_conf = WplField::sub_for_arr(elem_meta.to_string().as_str())?;
        let elem_parser = ParserFactory::create(&elem_meta)?;
        let sep = WplSep::default();

        // 元素 0
        let mut f0 = elem_parser
            .generate(_gen, &sep, &elem_conf, _g_conf)
            .map(|fmt| fmt.data_field)?;
        f0.set_name(format!("{}/[0]", name));
        items.push(f0);

        // 元素 1
        let mut f1 = elem_parser
            .generate(_gen, &sep, &elem_conf, _g_conf)
            .map(|fmt| fmt.data_field)?;
        f1.set_name(format!("{}/[1]", name));
        items.push(f1);

        Ok(DataField::from_arr(name, items))
    }
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, str::FromStr};

    use crate::eval::runtime::vm_unit::WplEvaluator;
    use wp_model_core::model::DataType;

    use super::*;

    /*
    #[test]
    fn test_json_array_err() {
        let mut data = r#"["10.10.10.10"]"#;
        parse_array_vec(&mut data).assert();

        let mut data = r#"10.10.10.10"#;
        let _ = parse_array_vec(&mut data).is_err();
    }
    */

    #[test]
    fn test_array() -> AnyResult<()> {
        let data = r#"[1, 2, 3]"#;
        let rule = r#" rule x { (array/digit:array_val)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        let expected = vec![
            DataField::new_opt(DataType::Digit, Some("array_val/[0]".into()), 1.into()),
            DataField::new_opt(DataType::Digit, Some("array_val/[1]".into()), 2.into()),
            DataField::new_opt(DataType::Digit, Some("array_val/[2]".into()), 3.into()),
        ];
        let expected = DataField::from_arr("array_val".to_string(), expected);
        assert_eq!(tdc.field("array_val"), Some(&expected));

        let data = r#"["hello", "_F]fe", "!@#$*&^\"123"]"#;
        let rule = r#" rule x { (array/chars:array)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        let expected = vec![
            DataField::new_opt(DataType::Chars, Some("array/[0]".into()), "hello".into()),
            DataField::new_opt(DataType::Chars, Some("array/[1]".into()), "_F]fe".into()),
            DataField::new_opt(
                DataType::Chars,
                Some("array/[2]".into()),
                "!@#$*&^\\\"123".into(),
            ),
        ];
        let expected = DataField::from_arr("array".to_string(), expected);
        assert_eq!(tdc.field("array"), Some(&expected));

        // 尾随逗号
        let data = r#"[1,2,3,]"#;
        let rule = r#" rule x { (array/digit:nums)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        let expected = vec![
            DataField::new_opt(DataType::Digit, Some("nums/[0]".into()), 1.into()),
            DataField::new_opt(DataType::Digit, Some("nums/[1]".into()), 2.into()),
            DataField::new_opt(DataType::Digit, Some("nums/[2]".into()), 3.into()),
        ];
        let expected = DataField::from_arr("nums".to_string(), expected);
        assert_eq!(tdc.field("nums"), Some(&expected));

        // 空数组
        let data = r#"[]"#;
        let rule = r#" rule x { (array/digit:empty)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        let expected: Vec<DataField> = vec![];
        let expected = DataField::from_arr("empty".to_string(), expected);
        assert_eq!(tdc.field("empty"), Some(&expected));
        Ok(())
    }

    #[test]
    fn test_array_ip_with_quotes() -> AnyResult<()> {
        let data = r#"["1.1.1.1","2.2.2.2"]"#;
        let rule = r#" rule x { (array/ip:ips)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        let expected = vec![
            DataField::from_ip("ips/[0]", IpAddr::from_str("1.1.1.1")?),
            DataField::from_ip("ips/[1]", IpAddr::from_str("2.2.2.2")?),
        ];
        let expected = DataField::from_arr("ips".to_string(), expected);
        assert_eq!(tdc.field("ips"), Some(&expected));
        Ok(())
    }

    #[test]
    fn test_arr_arr() -> AnyResult<()> {
        let data = "[[1,2],[3,4]]";
        let rule = r#" rule x { (array/array/digit:array)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);

        let obj = DataField::from_arr(
            "array/[0]",
            vec![
                DataField::new_opt(DataType::Digit, Some("array/[0]/[0]".into()), 1.into()),
                DataField::new_opt(DataType::Digit, Some("array/[0]/[1]".into()), 2.into()),
            ],
        );
        let obj1 = DataField::from_arr(
            "array/[1]",
            vec![
                DataField::new_opt(DataType::Digit, Some("array/[1]/[0]".into()), 3.into()),
                DataField::new_opt(DataType::Digit, Some("array/[1]/[1]".into()), 4.into()),
            ],
        );
        assert_eq!(
            tdc.field("array"),
            Some(&DataField::from_arr("array".to_string(), vec![obj, obj1]))
        );
        Ok(())
    }
    #[test]
    fn test_arr_json() -> AnyResult<()> {
        let data = r#"[{"name":"xxx", "value":"xxx"}, {"name": "xxxx", "value": 85.2}]"#;

        let rule = r#" rule x { (array/json:array)}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        let obj = vec![
            DataField::new_opt(DataType::Chars, Some("array/[0]/name".into()), "xxx".into()),
            DataField::new_opt(
                DataType::Chars,
                Some("array/[0]/value".into()),
                "xxx".into(),
            ),
            DataField::new_opt(
                DataType::Chars,
                Some("array/[1]/name".into()),
                "xxxx".into(),
            ),
            DataField::new_opt(DataType::Float, Some("array/[1]/value".into()), 85.2.into()),
        ];
        //let obj2 = TDOEnum::new_opt(Meta::Obj, None, obj.into());
        let expected = DataField::from_arr("array", obj);
        assert_eq!(tdc.field("array"), Some(&expected));
        Ok(())
    }
    #[test]
    fn test_arr_1() -> AnyResult<()> {
        let data = r#"[1.1.1.1,2.2.2.2]"#;
        let rule = r#" rule x { (array/ip:block_ips)}"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        assert!(tdc.field("block_ips").is_some());
        Ok(())
    }
    #[test]
    fn test_arr_01() -> AnyResult<()> {
        let data = r#"[]"#;
        let rule = r#" rule x { (array/ip:block_ips)}"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        assert!(tdc.field("block_ips").is_some());
        Ok(())
    }

    #[test]
    fn test_arr_00() -> AnyResult<()> {
        let data = r#"[]"#;
        let rule = r#" rule x { (array/chars:block_ips)}"#;
        let pipe = WplEvaluator::from_code(rule)?;

        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        assert!(tdc.field("block_ips").is_some());
        Ok(())
    }
}
