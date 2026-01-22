use super::super::prelude::*;
use std::collections::HashMap;
use wp_model_core::model::FNameStr;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::generator::FieldGenConf;
use crate::generator::GenChannel;
use crate::types::AnyResult;
use winnow::ascii::multispace0;
use winnow::{ModalResult, Parser};
use wp_model_core::model::DataField;
use wp_parser::symbol::ctx_desc;

#[derive(Default)]
pub struct SymbolP {}

#[derive(Default)]
pub struct PeekSymbolP {}

impl PatternParser for SymbolP {
    fn pattern_parse(
        &self,
        fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        let buffer = if let Some(content) = &fpu.conf().content {
            content
                .as_str()
                .context(ctx_desc("<symbol>"))
                .parse_next(data)?
        } else {
            "".parse_next(data)?
        };

        out.push(DataField::from_symbol(name, buffer));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let name = "".to_string();
        //let _ = self.base().field_conf.length;

        // 优化: 避免不必要的 double clone
        let dat = f_conf
            .content
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();

        if let Some(conf) = g_conf
            && let Some(fmt) = &conf.gen_fmt
        {
            let mut vars = HashMap::new();
            vars.insert("val".to_string(), dat.clone());
            match strfmt::strfmt(fmt, &vars) {
                Ok(dat) => {
                    return Ok(DataField::from_symbol(name, dat));
                }
                Err(e) => {
                    error!("gen fmt error: {}", e);
                }
            }
        }
        Ok(DataField::from_symbol(name, dat))
    }
}

impl PatternParser for PeekSymbolP {
    fn pattern_parse(
        &self,
        fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let data_1: &str = data;
        let (input, _) = multispace0.parse_peek(data_1)?;
        let (_, buffer) = if let Some(content) = &fpu.conf().content {
            content.as_str().parse_peek(input)?
        } else {
            "".parse_peek(input)?
        };

        out.push(DataField::from_symbol(name, buffer));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("peek_symbol not gen data")
    }
}
#[cfg(test)]
mod tests {
    use crate::ast::WplField;
    use crate::ast::fld_fmt::for_test::fdc3_1;
    use crate::eval::value::test_utils::ParserTUnit;

    use super::*;
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    #[test]
    fn test_symbol() -> AnyResult<()> {
        let mut data = "color=red";
        let conf = fdc3_1("symbol", "color=red", " ")?;
        let res = ParserTUnit::new(SymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "color=red"));

        let mut data = "color=black  ";
        let conf = fdc3_1("symbol", "color=black", " ")?;
        let res = ParserTUnit::new(SymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "color=black"));

        Ok(())
    }

    #[test]
    fn test_peek_symbol() -> AnyResult<()> {
        let mut data = "color=red";
        let conf = fdc3_1("symbol", "color=red", " ")?;
        let res = ParserTUnit::new(PeekSymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "color=red"));

        let mut data = "color=black  ";
        let conf = fdc3_1("symbol", "color=black", " ")?;
        let res = ParserTUnit::new(PeekSymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "color=black"));

        Ok(())
    }

    #[test]
    fn test_symbol2() {
        let mut data = "[color=red] ";
        let conf = WplField::try_parse("symbol([color=red])").assert();
        let res = ParserTUnit::new(SymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "[color=red]"));
    }

    #[test]
    fn test_peek_symbol2() {
        let mut data = "[color=red] ";
        let conf = WplField::try_parse("peek_symbol([color=red])").assert();
        let res = ParserTUnit::new(PeekSymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "[color=red]"));
    }

    #[test]
    fn test_from_case() -> AnyResult<()> {
        let mut data = "  color=red hello";
        let conf = WplField::try_parse("symbol(color=red)").assert();
        let res = ParserTUnit::new(SymbolP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(res[0], DataField::from_symbol("symbol", "color=red",));
        Ok(())
    }

    #[test]
    fn test_gen() -> AnyResult<()> {
        let conf = WplField::try_parse("symbol(color=red)").assert();
        ParserTUnit::new(SymbolP::default(), conf.clone()).verify_gen_parse_suc();
        Ok(())
    }
}
