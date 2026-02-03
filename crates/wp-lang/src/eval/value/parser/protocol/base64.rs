use super::super::prelude::*;
use crate::ast::WplSep;
use crate::generator::FmtField;
use base64::Engine;
use base64::engine::general_purpose;
use smol_str::SmolStr;
use wp_model_core::model::FNameStr;
use wp_model_core::model::Value;

use crate::ast::WplField;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;

derive_base_prs!(Base64P);

impl FieldParser for Base64P {
    fn parse<'a>(
        &self,
        _e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        f_name: Option<FNameStr>,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let sep = fpu.conf().resolve_sep(ups_sep);
        let take = sep.read_until_sep(data)?;

        match general_purpose::STANDARD.decode(take) {
            Ok(output) => {
                let value = String::from_utf8_lossy(&output).to_string();
                out.push(DataField::new_opt(
                    DataType::Base64,
                    f_name,
                    Value::Chars(SmolStr::from(value)),
                ));
                Ok(())
            }
            Err(_e) => fail
                .context(ctx_desc("base64 decode error"))
                .parse_next(data), //Err(Cut(ParserError::from_error_kind(
                                   //&format!("base64 decode error {}", e).as_str(),
                                   //ErrorKind::Verify,
        }
    }

    fn generate(
        &self,
        _gen: &mut GenChannel,
        _ups_sep: &WplSep,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<FmtField> {
        unimplemented!("base64 generate");
    }
}

#[cfg(test)]
mod tests {
    use orion_error::TestAssert;
    use wp_model_core::model::DataType::Base64;

    use crate::eval::value::test_utils::ParserTUnit;

    use super::*;

    #[test]
    fn test_base64() {
        let mut data = "aGVsbG8=";
        let y = ParserTUnit::new(Base64P::default(), WplField::try_parse("base64").assert())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(data, "");
        assert_eq!(y, vec![DataField::new(Base64, "base64", "hello")]);
    }
}
