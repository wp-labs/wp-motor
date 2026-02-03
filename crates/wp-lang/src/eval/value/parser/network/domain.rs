use super::super::prelude::*;
use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::generator::GenChannel;
use rand::prelude::IndexedRandom;
use rand::rng;
use winnow::ascii::alpha1;
use winnow::combinator::repeat;
use winnow::token::{literal, take_until, take_while};
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataField, DataType};

// domain格式
// 英文域名：英文字母、数字及" - " ( 即连字符或减号 ) 任意组合而成 , 但开头及结尾均不能含有" - "。 域名中字母不分大小写。域名最长可达 67 个字节 ( 包括后缀 .com 、.top、.tech、.net 、.org 、.biz等 )
// ((?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+)([a-zA-Z]{2,11})
derive_base_prs!(DomainP);

impl PatternParser for DomainP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let start = data.checkpoint();
        let non_root: Vec<&str> = repeat(1.., non_root_domain).parse_next(data)?;
        let non_root = non_root.join(".");
        let root = root_domain(data)?;

        let domain = format!("{}.{}", non_root, root);
        if domain.chars().count().gt(&67) {
            return Err(ErrMode::Backtrack(context_error(
                data,
                &start,
                "domain length need <= 67",
            )));
        }
        out.push(DataField::from_domain(name, domain));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let charset: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            .chars()
            .collect();
        let mut rng = rng();
        let mut domain = String::new();
        for _i in 0..5 {
            if let Some(random_char) = charset.choose(&mut rng) {
                domain.push(*random_char);
            }
        }
        Ok(DataField::from_chars(
            DataType::Domain.to_string(),
            format!("www.{}.com", domain),
        ))
    }
}

// 域名允许字符，目前仅支持中英文域名
#[allow(dead_code)]
fn domain_format(c: char) -> bool {
    c.is_alphanumeric() || c.eq(&'-') || (0x4E00..=0x9FA5).contains(&(c as i32))
}

// 非顶级域名匹配

#[allow(dead_code)]
pub fn non_root_domain<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    let start = input.checkpoint();
    let mut val = take_until(1.., ".").parse_next(input)?;
    if val.strip_prefix("-").is_some() || val.strip_suffix("-").is_some() {
        return Err(ErrMode::Backtrack(context_error(
            input,
            &start,
            "<domain> not starting or ending with a b'-'",
        )));
    }

    let resp = take_while(1.., |c: char| domain_format(c))
        .context(ctx_desc("<domain>::non_root_domain [0-9a-zA-Z-]"))
        .parse_next(&mut val)?;
    if val.is_empty() {
        literal('.')
            .context(ctx_desc("<split> ."))
            .parse_next(input)?;
        return Ok(resp);
    }

    Err(ErrMode::Backtrack(context_error(
        input,
        &start,
        "<domain> inconformity",
    )))
}

// 顶级域名(根域名[a-zA-Z]{2,11})
#[allow(dead_code)]
fn root_domain<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    let start = input.checkpoint();
    let val = alpha1
        .context(ctx_desc("<domain>::root_domain [a-zA-Z]"))
        .parse_next(input)?;

    if val.len().gt(&11) || val.len().lt(&2) {
        return Err(ErrMode::Backtrack(context_error(
            input,
            &start,
            "2 <= top_part len <= 11",
        )));
    }
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    #[test]
    fn test_domain() {
        let mut data = "1-test.warppase.ai";
        let y = ParserTUnit::new(DomainP::default(), WplField::try_parse("domain").assert())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            y.first(),
            Some(&DataField::from_domain("domain", "1-test.warppase.ai"))
        );

        let mut data = "-1-test.warppase.ai";
        let y = ParserTUnit::new(DomainP::default(), WplField::try_parse("domain").assert())
            .verify_parse_suc(&mut data);
        assert!(y.is_err());

        let mut data = "www.s123/df.com";
        let y = ParserTUnit::new(DomainP::default(), WplField::try_parse("domain").assert())
            .verify_parse_suc(&mut data);
        assert!(y.is_err());
    }
}
