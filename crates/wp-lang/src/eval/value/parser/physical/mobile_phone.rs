use super::super::prelude::*;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use winnow::ascii::digit1;
use wp_model_core::model::FNameStr;

derive_base_prs!(MobilePhoneP);

impl PatternParser for MobilePhoneP {
    fn pattern_parse(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let start = data.checkpoint();
        let phone = digit1
            .context(ctx_desc("<mobile_phone>"))
            .parse_next(data)?;
        if phone::is_valid_phone(phone) {
            out.push(DataField::from_mobile_phone(name, phone));
            Ok(())
        } else {
            Err(ErrMode::Backtrack(context_error(
                data,
                &start,
                "mobile_phone format not match",
            )))
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("phone generate");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    #[test]
    fn test_parse_mobile() {
        let mut data = "13562479856";
        let y = ParserTUnit::new(
            MobilePhoneP::default(),
            WplField::try_parse("mobile_phone").assert(),
        )
        .verify_parse_suc(&mut data)
        .assert();

        assert_eq!(
            y.first(),
            Some(&DataField::from_mobile_phone("mobile_phone", "13562479856"))
        );
    }
}
