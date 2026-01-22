use super::super::prelude::*;
use crate::ast::group::WplGroupType;
use crate::generator::{FieldGenConf, GenScopeEnum};
use crate::generator::{GenChannel, ParserValue};
use crate::types::AnyResult;
use rand::Rng;
use winnow::ascii::{dec_int, float, multispace0};
use winnow::combinator::preceded;
use wp_model_core::model::DataField;
use wp_model_core::model::DataType;
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DigitValue, FloatValue};
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::ctx_desc;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
#[derive(Default)]
pub struct DigitP {}
#[derive(Default)]
pub struct FloatP {}

impl ParserValue<DigitValue> for DigitP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<DigitValue> {
        preceded(multispace0, dec_int).parse_next(data)
    }
}

impl PatternParser for DigitP {
    fn pattern_parse<'a>(
        &self,
        fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        match fpu.group_enum() {
            WplGroupType::Seq(_) => {
                let obj = Self::parse_value
                    .context(ctx_desc("<digit>"))
                    .parse_next(data)?;
                out.push(DataField::new_opt(DataType::Digit, Some(name), obj.into()));
            }
            _ => {
                let obj = Self::parse_value
                    .context(ctx_desc("<digit>"))
                    .parse_next(data)?;
                out.push(DataField::new_opt(DataType::Digit, Some(name), obj.into()));
            }
        }
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let range = if let Some(Some(GenScopeEnum::Digit(digit))) = g_conf.map(|c| &c.scope) {
            let beg: i64 = digit.beg;
            let end: i64 = digit.end;
            beg..end
        } else {
            0..2000
        };
        let dat = gnc.rng.random_range(range);
        Ok(DataField::from_digit(f_conf.safe_name(), dat))
    }
}

impl ParserValue<FloatValue> for FloatP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<FloatValue> {
        preceded(multispace0, float).parse_next(data)
    }
}

impl PatternParser for FloatP {
    fn pattern_parse<'a>(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let obj = Self::parse_value.parse_next(data)?;
        out.push(DataField::new_opt(DataType::Float, Some(name), obj.into()));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let fst = gnc.rng.random_range(0..2000);
        let sec = gnc.rng.random_range(100..999);

        Ok(DataField::from_float(
            f_conf.safe_name(),
            (fst + sec / 1000) as f64,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::WplField;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::parser::protocol::json::JsonP;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::parser::parse_code::wpl_express;
    use crate::types::AnyResult;
    use orion_error::TestAssert;
    use wp_model_core::model::{DataRecord, Value};
    use wp_parse_api::RawData;

    use super::*;
    #[allow(clippy::approx_constant)]
    const PI: f64 = 3.14;

    #[test]
    fn test_digit() -> AnyResult<()> {
        let mut data = "-99";
        let conf = WplField::try_parse("digit").assert();
        let field = ParserTUnit::new(DigitP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_digit("digit", -99));

        let mut data = r#"{"num":-99}"#;
        let conf = WplField::try_parse("json(digit@num)").assert();
        let field = ParserTUnit::new(JsonP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_digit("num", -99));

        let mut data = "[3.14]";
        let conf = WplField::try_parse("float<[,]>").assert();
        let field = ParserTUnit::new(FloatP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_float("float", PI));

        let mut data = "3.14,";
        let conf = WplField::try_parse(r#"float\,"#).assert();
        let _ = ParserTUnit::new(FloatP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        //assert_eq!(field, "3.14");
        let mut data = "<188>May";
        let conf = WplField::try_parse(r#"digit<<,>>"#).assert();
        let field = ParserTUnit::new(DigitP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        match field[0].get_value() {
            Value::Digit(digit) => {
                assert_eq!(*digit, 188);
            }
            _ => panic!("not digit"),
        }
        Ok(())
    }

    #[test]
    fn test_float() -> AnyResult<()> {
        let mut data = "[ 3.14]";
        let conf = WplField::try_parse("float<[,]>").assert();
        let field = ParserTUnit::new(FloatP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_float("float", PI));

        let mut data = "  3.14,";
        let conf = WplField::try_parse(r#"float\,"#).assert();
        let field = ParserTUnit::new(FloatP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_float("float", PI));
        Ok(())
    }

    #[test]
    fn test_digit_gen() -> AnyResult<()> {
        let conf = WplField::try_parse("digit<[,]>").assert();
        ParserTUnit::new(DigitP::default(), conf).verify_gen_parse_suc();

        let conf = WplField::try_parse("digit").assert();
        ParserTUnit::new(DigitP::default(), conf).verify_gen_parse_suc();

        let conf = WplField::try_parse("digit\\,").assert();
        ParserTUnit::new(DigitP::default(), conf).verify_gen_parse_suc();
        Ok(())
    }

    #[test]
    fn test_digit_empty() -> AnyResult<()> {
        let data = r#"1||3"#;
        let wpl = r#"(digit)\|,alt(digit:x,chars:x)\|,(digit)"#;
        let express = wpl_express.parse(wpl).assert();
        let lpp = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let (tdc, _) = lpp.proc(raw, 0)?;
        let tdc_assert = DataRecord::from(vec![
            DataField {
                meta: DataType::Digit,
                name: "digit".into(),
                value: Value::Digit(1.into()),
            },
            DataField {
                meta: DataType::Chars,
                name: "x".into(),
                value: Value::Chars("".into()),
            },
            DataField {
                meta: DataType::Digit,
                name: "digit".into(),
                value: Value::Digit(3.into()),
            },
        ]);
        assert_eq!(tdc, tdc_assert);
        Ok(())
    }
    #[test]
    fn test_digit_opt() -> AnyResult<()> {
        let data = r#"1|2|3"#;
        let wpl = r#"(digit)\|,opt(digit)\|,(digit)"#;
        let express = wpl_express.parse(wpl).assert();
        let lpp = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let (tdc, _) = lpp.proc(raw, 0)?;
        let tdc_assert = DataRecord::from(vec![
            DataField {
                meta: DataType::Digit,
                name: "digit".into(),
                value: Value::Digit(1.into()),
            },
            DataField {
                meta: DataType::Digit,
                name: "digit".into(),
                value: Value::Digit(2.into()),
            },
            DataField {
                meta: DataType::Digit,
                name: "digit".into(),
                value: Value::Digit(3.into()),
            },
        ]);
        assert_eq!(tdc, tdc_assert);
        Ok(())
    }
}
