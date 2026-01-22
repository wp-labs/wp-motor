use super::super::prelude::*;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::generator::GenChannel;
use rand::prelude::IndexedRandom;
use rand::rng;
use winnow::token::take_while;
use wp_model_core::model::FNameStr;

derive_base_prs!(EmailP);

impl PatternParser for EmailP {
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
        // 放宽字符集：本地段允许 RFC 5322 常见字符；域名段允许 '-' 和 '.'
        let email = take_while(1.., |c: char| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '@' | '.'
                        | '-'
                        | '_'
                        | '+'
                        | '%'
                        | '!'
                        | '#'
                        | '$'
                        | '&'
                        | '\''
                        | '*'
                        | '/'
                        | '='
                        | '?'
                        | '^'
                        | '`'
                        | '{'
                        | '}'
                        | '|'
                        | '~'
                )
        })
        .context(ctx_desc("<email>"))
        .parse_next(data)?;
        if mailchecker::is_valid(email) {
            out.push(DataField::from_email(name, email));
            Ok(())
        } else {
            Err(ErrMode::Backtrack(context_error(
                data,
                &start,
                "email format not match",
            )))
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let charset: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            .chars()
            .collect();
        let mut rng = rng();
        let mut email = String::new();
        for _i in 0..5 {
            if let Some(random_char) = charset.choose(&mut rng) {
                email.push(*random_char);
            }
        }
        Ok(DataField::from_email(
            f_conf.safe_name(),
            format!("{}@example.com", email),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    #[test]
    fn test_email() {
        let mut data = "johnjoke@example.com";
        let y = ParserTUnit::new(EmailP::default(), WplField::try_parse("email").assert())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            y.first(),
            Some(&DataField::from_email("email", "johnjoke@example.com")),
        );
    }

    #[test]
    fn test_email_plus_underscore_hyphen() {
        for mut data in [
            "user+tag@example-domain.com",
            "first_last@example.com",
            "foo-bar@example.co",
        ] {
            ParserTUnit::new(EmailP::default(), WplField::try_parse("email").assert())
                .verify_parse_suc(&mut data)
                .assert();
        }
    }
}
