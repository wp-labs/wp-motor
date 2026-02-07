use super::super::prelude::*;
use std::fmt::Write;
use wp_model_core::model::FNameStr;

use winnow::stream::AsChar;
use winnow::token::take_while;
use wp_model_core::model::DataField;
use wp_model_core::model::DataType;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::physical::foundation::gen_chars;
use crate::generator::FieldGenConf;
use crate::generator::GenChannel;
use crate::types::AnyResult;

#[derive(Default)]
pub struct SnP {}

impl PatternParser for SnP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let sn = take_while(1.., (AsChar::is_alpha, AsChar::is_dec_digit, '-')).parse_next(data)?;
        out.push(DataField::new(DataType::SN, name, sn));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let one = gen_chars(gnc, 4, true);
        let two = gen_chars(gnc, 2, true);
        let thr = gen_chars(gnc, 4, true);
        let fro = gen_chars(gnc, 4, true);
        let mut buf = String::new();
        write!(buf, "{}-{}-{}-{}", one, two, thr, fro,).expect("write sn error");
        Ok(DataField::from_chars(f_conf.safe_name(), buf))
    }
}

#[cfg(test)]
mod tests {

    use crate::ast::{WplField, WplSep};
    use crate::eval::runtime::field::FieldEvalUnit;
    use crate::eval::value::parser::compute::device::SnP;
    //test sn

    #[test]
    fn test_sn() {
        let conf = WplField::default();
        let parser = SnP::default();
        let ups_sep = WplSep::default();
        let fpu = FieldEvalUnit::for_test(parser, conf);
        let mut out = Vec::new();
        assert!(
            fpu.parse(0, &ups_sep, &mut "KM-KJY-DC-USG12004-B02 ", None, &mut out)
                .is_ok()
        );
    }
}
