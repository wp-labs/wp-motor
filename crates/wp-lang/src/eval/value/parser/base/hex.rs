use super::super::prelude::*;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;
use crate::generator::{FieldGenConf, GenScopeEnum};
use crate::generator::{GenChannel, ParserValue};
use crate::types::AnyResult;
use rand::Rng;
use winnow::ascii::multispace0;
use winnow::ascii::{Caseless, hex_uint};
use winnow::combinator::{opt, preceded};
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataField, DataType, HexT};
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::ctx_desc;

#[derive(Default)]
pub struct HexDigitP {}

impl ParserValue<HexT> for HexDigitP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<HexT> {
        preceded(
            multispace0,
            preceded(opt(Caseless("0x")), hex_uint::<_, u128, _>),
        )
        .context(ctx_desc("<hex>"))
        .map(HexT)
        .parse_next(data)
    }
}

impl PatternParser for HexDigitP {
    fn pattern_parse(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let obj = Self::parse_value(data)?;
        out.push(DataField::new_opt(DataType::Hex, Some(name), obj.into()));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let range = if let Some(Some(GenScopeEnum::Digit(digit))) = g_conf.map(|c| &c.scope) {
            let beg: u32 = digit.beg as u32;
            let end: u32 = digit.end as u32;
            beg..end
        } else {
            0..2000
        };
        let dat = gnc.rng.random_range(range);
        Ok(DataField::from_hex(f_conf.safe_name(), HexT(dat as u128)))
    }
}

#[cfg(test)]
mod tests {

    use crate::ast::{WplField, WplSep};
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    use super::*;
    use crate::eval::runtime::field::FieldEvalUnit;
    use crate::eval::value::test_utils::ParserTUnit;

    #[test]
    fn test_hex() -> AnyResult<()> {
        let conf = WplField::default();
        let ups_sep = WplSep::default();
        let parser = HexDigitP::default();
        let mut out = Vec::new();
        let fpu = FieldEvalUnit::for_test(parser, conf);
        fpu.parse(&ups_sep, &mut "0x16fe67000", None, &mut out)
            .assert();

        let mut out = Vec::new();
        assert!(fpu.parse(&ups_sep, &mut "16fe70", None, &mut out).is_ok());

        let mut data = "(0x16fe67000)";
        let conf = WplField::try_parse("hex<(,)>").assert();
        let field = ParserTUnit::new(HexDigitP::default(), conf)
            .verify_parse_suc_end(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_hex("hex", HexT(6172340224)));
        Ok(())
    }
}
