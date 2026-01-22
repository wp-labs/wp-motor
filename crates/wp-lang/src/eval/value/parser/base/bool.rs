use super::super::prelude::*;
use crate::generator::FieldGenConf;
use crate::generator::{GenChannel, ParserValue};
use crate::parser::utils::{quot_r_str, quot_str, take_to_end, window_path};
use crate::types::AnyResult;
use wp_model_core::model::DataField;
use wp_model_core::model::DataType;
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DigitValue, Value};

use winnow::ascii::{dec_int, multispace0};
use winnow::combinator::alt;
use winnow::combinator::preceded;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;

#[derive(Default)]
pub struct BoolP {}

impl ParserValue<DigitValue> for BoolP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<DigitValue> {
        preceded(multispace0, dec_int).parse_next(data)
    }
}

impl PatternParser for BoolP {
    fn pattern_parse<'a>(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        let buffer = alt((quot_r_str, quot_str, window_path, take_to_end)).parse_next(data)?;
        if let Some(b) = str_to_bool(buffer) {
            out.push(DataField::new_opt(
                DataType::Bool,
                Some(name),
                Value::Bool(b),
            ));
        } else {
            fail.context(ctx_desc("str to bool")).parse_next(data)?;
        }
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        Ok(DataField::from_bool(f_conf.safe_name(), false))
    }
}

fn str_to_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::WplField;
    use crate::eval::value::parser::base::CharsP;
    use crate::eval::value::parser::protocol::json::JsonP;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    use super::*;

    #[test]
    fn test_bool() -> AnyResult<()> {
        let mut data = "true";
        let conf = WplField::try_parse("bool").assert();
        let field = ParserTUnit::new(BoolP::default(), conf.clone())
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_bool("bool", true));

        let mut data = "true";
        let conf = WplField::try_parse("chars").assert();
        let field = ParserTUnit::new(CharsP::default(), conf.clone())
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_chars("chars", "true"));

        let mut data = "TRUE";
        let conf = WplField::try_parse("bool").assert();
        let field = ParserTUnit::new(BoolP::default(), conf.clone())
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_bool("bool", true));

        let mut data = "1";
        let conf = WplField::try_parse("bool").assert();
        let field = ParserTUnit::new(BoolP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_bool("bool", true));

        let mut data = r#"{"check":false}"#;
        let conf = WplField::try_parse("json(bool@check)").assert();
        let field = ParserTUnit::new(JsonP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_bool("check", false));

        Ok(())
    }
}
