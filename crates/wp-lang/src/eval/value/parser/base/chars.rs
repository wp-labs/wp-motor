use super::super::prelude::*;
use std::collections::HashMap;
use wp_model_core::model::FNameStr;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;
use crate::eval::value::parser::physical::foundation::gen_chars;
use crate::generator::{FieldGenConf, GenScopeEnum};
use crate::generator::{GenChannel, ParserValue};
use crate::parser::utils::{quot_r_str, quot_str, take_to_end, window_path};
use crate::types::AnyResult;
use rand::Rng;
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, preceded};
use wp_model_core::model::{DataField, Value};
use wp_model_core::model::{DataType, DigitValue};
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

#[derive(Default)]
pub struct CharsP {}

impl ParserValue<DigitValue> for CharsP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<DigitValue> {
        preceded(multispace0, digit1.try_map(str::parse::<DigitValue>)).parse_next(data)
    }
}

impl PatternParser for CharsP {
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
        out.push(DataField::new_opt(
            DataType::Chars,
            Some(name),
            //Value::Chars(buffer.trim().to_string()),
            Value::Chars(buffer.trim().into()),
        ));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let name = f_conf.safe_name();
        let len = f_conf.length.unwrap_or(20);
        let mut dat = gen_chars(gnc, len, false);
        if let Some(conf) = g_conf {
            match &conf.scope {
                Some(GenScopeEnum::Digit(digit)) => {
                    dat = gnc.rng.random_range(digit.beg..digit.end).to_string();
                }
                Some(GenScopeEnum::Chars(values)) => {
                    let val = gnc.rng.random_range(0..values.len());
                    dat = values[val].to_string();
                }
                _ => {}
            }

            if let Some(fmt) = &conf.gen_fmt {
                let mut vars = HashMap::new();
                vars.insert("val".to_string(), dat.clone());
                match strfmt::strfmt(fmt, &vars) {
                    Ok(dat) => {
                        return Ok(DataField::from_chars(name.to_string(), dat));
                    }
                    Err(e) => {
                        error!("gen fmt error: {}", e);
                    }
                }
            }
        }
        Ok(DataField::from_chars(name.to_string(), dat))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::WplField;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    use super::*;

    #[test]
    fn test_char1() {
        let mut data = "aGVsbG8=";
        let res = ParserTUnit::new(CharsP::default(), WplField::try_parse("chars").assert())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_chars("chars", "aGVsbG8="));

        let mut data = "aGVsbG8= ";
        let res = ParserTUnit::new(CharsP::default(), WplField::try_parse("chars").assert())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_chars("chars", "aGVsbG8="));

        let mut data = "[abc,efg,hij]";
        let conf = WplField::try_parse("chars<[,]>").assert();

        let res = ParserTUnit::new(CharsP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_chars("chars", "abc,efg,hij"));
    }

    #[test]
    fn test_char4() {
        let mut data = r#""abc efg hij""#;
        let conf = WplField::try_parse(r#"chars:name<",">"#).assert();

        let res = ParserTUnit::new(CharsP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_chars("name", "abc efg hij"));
    }

    #[test]
    fn test_char3() {
        let mut data = "{abc efg hij}  xxx yyy";
        let conf = WplField::try_parse("chars<{,}>").assert();
        let res = ParserTUnit::new(CharsP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_chars("chars", "abc efg hij"));
    }

    #[test]
    fn test_from_case() -> AnyResult<()> {
        let mut data = "  -[UMSyncService fetchPersona:forPid:completionHandler:]_block_invoke:";
        let conf = WplField::try_parse("chars<-[,]>").assert();
        let res = ParserTUnit::new(CharsP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            res[0],
            DataField::from_chars(
                "chars",
                "UMSyncService fetchPersona:forPid:completionHandler:",
            )
        );
        assert_eq!(data, "_block_invoke:");
        Ok(())
    }

    #[test]
    fn test_gen() -> AnyResult<()> {
        let conf = WplField::try_parse("chars<[,]>").assert();

        ParserTUnit::new(CharsP::default(), conf).verify_gen_parse_suc();

        let conf = WplField::try_parse("chars").assert();
        ParserTUnit::new(CharsP::default(), conf).verify_gen_parse_suc();

        let conf = WplField::try_parse("chars\\,").assert();
        ParserTUnit::new(CharsP::default(), conf).verify_gen_parse_suc();
        Ok(())
    }
}
