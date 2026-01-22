use super::super::prelude::*;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use url::Url;
use winnow::token::take_while;
use wp_model_core::model::FNameStr;

derive_base_prs!(UrlP);

impl PatternParser for UrlP {
    fn pattern_parse(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let start = data.checkpoint();
        multispace0.parse_next(data)?;
        let url = take_while(1.., |c: char| c.ne(&' '))
            .context(ctx_desc("<url>"))
            .parse_next(data)?;
        match Url::parse(url) {
            Ok(val) => {
                out.push(DataField::from_url(name, val.to_string()));
                Ok(())
            }
            Err(_) => Err(ErrMode::Backtrack(context_error(
                data,
                &start,
                "url format not match",
            ))),
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("url generate");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    #[test]
    fn test_url() {
        let mut data = r#"https://github.com/servo/rust-url/blob/main/url/src/parser.rs#L396"#;
        let y = ParserTUnit::new(UrlP::default(), WplField::try_parse("url").assert())
            .verify_parse_suc(&mut data)
            .assert();

        assert_eq!(
            y.first(),
            Some(&DataField::from_url(
                "url",
                "https://github.com/servo/rust-url/blob/main/url/src/parser.rs#L396"
            ))
        );
    }
}
