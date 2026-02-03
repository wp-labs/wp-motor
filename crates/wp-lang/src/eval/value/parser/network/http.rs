use super::super::prelude::*;
use wp_model_core::model::FNameStr;

use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;
use crate::parser::utils::take_to_end;
use smol_str::SmolStr;
use winnow::ascii::multispace1;
use winnow::combinator::terminated;
use winnow::error::ErrMode;
use winnow::token::{take_until, take_while};
use wp_model_core::model::Value;
use wp_parser::utils::context_error;

derive_base_prs!(RequestP);

derive_base_prs!(StatusP);
derive_base_prs!(AgentP);
derive_base_prs!(MethodP);

impl PatternParser for MethodP {
    fn pattern_parse<'a>(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        // 扫描一次方法 token，然后集合校验，提高匹配效率
        let method = take_while(1.., |c: char| c.is_ascii_uppercase()).parse_next(data)?;
        match method {
            "GET" | "PUT" | "POST" | "DELETE" | "HEAD" | "OPTIONS" | "PATCH" | "TRACE"
            | "CONNECT" => {}
            _ => {
                return fail.context(ctx_desc("http method")).parse_next(data);
            }
        }
        out.push(DataField::new(DataType::HttpMethod, name, method));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        use smol_str::SmolStr;
        Ok(DataField::from_chars(
            f_conf.safe_name(),
            SmolStr::from("GET"),
        ))
    }
}

impl PatternParser for RequestP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        // 方法扫描 + 校验
        let start = data.checkpoint();
        multispace0.parse_next(data)?;
        let method = take_while(1.., |c: char| c.is_ascii_uppercase()).parse_next(data)?;
        match method {
            "GET" | "PUT" | "POST" | "DELETE" | "HEAD" | "OPTIONS" | "PATCH" | "TRACE"
            | "CONNECT" => {}
            _ => {
                return Err(ErrMode::Backtrack(context_error(
                    data,
                    &start,
                    "http method",
                )));
            }
        }

        // URI 直到下一个空格
        let uri = preceded(multispace1, take_until(1.., " ")).parse_next(data)?;
        // 版本：匹配空格 + HTTP/ + 版本 + 可选空白
        let (protocol, version) = (multispace1, literal("HTTP/"), ver_parse, multispace0)
            .map(|x| (x.1, x.2))
            .parse_next(data)?;

        // 预估容量，减少 format! 临时分配
        let mut req = String::with_capacity(method.len() + 1 + uri.len() + 1 + protocol.len() + 8);
        req.push_str(method);
        req.push(' ');
        req.push_str(uri);
        req.push(' ');
        req.push_str(protocol);
        req.push_str(&version);

        out.push(DataField::new_opt(
            DataType::HttpRequest,
            Some(name),
            Value::Chars(SmolStr::from(req)),
        ));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        use smol_str::SmolStr;
        let data = "GET /index  HTTP/1.1 ";
        Ok(DataField::from_chars(
            f_conf.safe_name(),
            SmolStr::from(data),
        ))
    }
}

impl PatternParser for StatusP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let status = delimited(multispace0, digit1.try_map(str::parse::<u32>), multispace0)
            .parse_next(data)?;
        if (100..1000).contains(&status) {
            out.push(DataField::new_opt(
                DataType::HttpStatus,
                Some(name),
                Value::Digit(status as i64),
            ));

            return Ok(());
        }
        fail.context(ctx_desc("status parse fail")).parse_next(data)
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        Ok(DataField::from_digit(f_conf.safe_name(), 200))
    }
}

fn ver_parse(data: &mut &str) -> ModalResult<String> {
    let one = (literal('.'), digit1);
    let two = (literal('.'), digit1, opt(one));
    let three = (literal('.'), digit1, opt(two));
    let (v1, oth) = (digit1, opt(three)).parse_next(data)?;
    if let Some((_, v2, oth)) = oth {
        if let Some((_, v3, oth)) = oth {
            if let Some((_, v4)) = oth {
                return Ok(format!("{}.{}.{}.{}", v1, v2, v3, v4));
            }
            return Ok(format!("{}.{}.{}", v1, v2, v3));
        }
        return Ok(format!("{}.{}", v1, v2));
    }
    Ok(v1.to_string())
}

impl PatternParser for AgentP {
    //Mozilla/5.0
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mozilla = preceded(multispace0, literal("Mozilla/")).parse_next(data)?;
        let ver = terminated(ver_parse, multispace0).parse_next(data)?;

        //let other = CharsP::new(conf).parse_chars().parse_next(input)?;
        let other = take_to_end.parse_next(data)?;
        // 预分配并拼接，避免 format! 额外分配
        let mut agent = String::with_capacity(mozilla.len() + ver.len() + other.len() + 1);
        agent.push_str(mozilla);
        agent.push_str(&ver);
        agent.push_str(other);
        agent.push(' ');
        out.push(DataField::new_opt(
            DataType::HttpAgent,
            Some(name),
            Value::Chars(SmolStr::from(agent)),
        ));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        use smol_str::SmolStr;
        let agent = r#"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.77 Safari/537.36"#;
        Ok(DataField::from_chars(
            f_conf.safe_name(),
            SmolStr::from(agent),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::fld_fmt::for_test::fdc2;
    use crate::eval::value::test_utils::{ParserTUnit, ParserTestEnv, verify_gen_parse};
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    use super::*;

    #[test]
    fn test_request() -> AnyResult<()> {
        let mut data = "GET /hello.png HTTP/1.1 ";
        let _ = ParserTUnit::new(
            RequestP::default(),
            WplField::try_parse("http/request").assert(),
        )
        .verify_parse_suc(&mut data)
        .assert();
        ParserTUnit::new(
            RequestP::default(),
            WplField::try_parse("http/request").assert(),
        )
        .verify_parse_fail(&mut "GETX /hello.png HTTP/1.1 ");
        Ok(())
    }

    #[test]
    fn test_http_methods_extended() -> AnyResult<()> {
        let conf = WplField::try_parse("http/method").assert();
        for mut data in [
            "HEAD", "OPTIONS", "PATCH", "TRACE", "CONNECT", "POST", "PUT", "GET",
        ] {
            ParserTUnit::new(MethodP::default(), conf.clone())
                .verify_parse_suc(&mut data)
                .assert();
        }
        let mut bad = "UNKNOWN";
        ParserTUnit::new(MethodP::default(), conf.clone()).verify_parse_fail(&mut bad);
        Ok(())
    }

    #[test]
    fn test_ver() {
        let mut data = "1.1";
        let ver = ver_parse(&mut data).assert();
        assert_eq!(ver, "1.1");
        assert!(data.is_empty());

        let mut data = "1.1.2";
        let ver = ver_parse(&mut data).assert();
        assert_eq!(ver, "1.1.2");
        assert!(data.is_empty());

        let mut data = "1.1.2.1";
        let ver = ver_parse(&mut data).assert();
        assert_eq!(ver, "1.1.2.1");
        assert!(data.is_empty());

        let mut data = "1.1.2.1.3";
        let ver = ver_parse(&mut data).assert();
        assert_eq!(ver, "1.1.2.1");
        assert_eq!(data, ".3");
    }

    #[test]
    fn test_agent() -> AnyResult<()> {
        let mut data = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/70.0.3538.77 Safari/537.36";
        ParserTUnit::new(
            AgentP::default(),
            WplField::try_parse("http/agent").assert(),
        )
        .verify_parse_suc(&mut data)
        .assert();
        Ok(())
    }

    #[test]
    fn test_http_gen() -> AnyResult<()> {
        let mut env = ParserTestEnv::new();
        let conf = fdc2("http/request", ",")?;
        //shm.end_conf.scope_beg = Some("\"".into());
        //shm.end_conf.scope_end = Some("\"".into());
        let parser = RequestP::default();
        let fpu = FieldEvalUnit::for_test(parser, conf.clone());
        verify_gen_parse(&mut env, &fpu, &conf);

        let conf = fdc2("http/agent", ",")?;
        //shm.end_conf.scope_beg = Some("\"".into());
        //shm.end_conf.scope_end = Some("\"".into());
        let parser = AgentP::default();
        let fpu = FieldEvalUnit::for_test(parser, conf.clone());
        verify_gen_parse(&mut env, &fpu, &conf);
        let conf = fdc2("http/method", ",")?;
        let parser = MethodP::default();
        let fpu = FieldEvalUnit::for_test(parser, conf.clone());
        verify_gen_parse(&mut env, &fpu, &conf);
        Ok(())
    }
}
