use std::str::FromStr;

use crate::language::{
    Base64Decode, EncodeType, Get, HtmlEscape, HtmlUnescape, JsonEscape, JsonUnescape, Nth,
    PIPE_BASE64_DECODE, PIPE_GET, PIPE_HTML_ESCAPE, PIPE_HTML_UNESCAPE, PIPE_JSON_ESCAPE,
    PIPE_JSON_UNESCAPE, PIPE_NTH, PIPE_PATH, PIPE_SKIP_EMPTY, PIPE_STR_ESCAPE, PIPE_TIME_TO_TS,
    PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_URL, PathGet,
    PathType, PreciseEvaluator, SkipEmpty, StrEscape, TimeStampUnit, TimeToTs, TimeToTsMs,
    TimeToTsUs, TimeToTsZone, ToJson, UrlGet, UrlType,
};
use crate::language::{Base64Encode, PIPE_BASE64_ENCODE, PIPE_TO_STR, ToStr};
use crate::language::{Ip4ToInt, PIPE_IP4_TO_INT, PiPeOperation, PipeFun};
use crate::parser::keyword::kw_gw_pipe;
use crate::parser::oml_aggregate::oml_var_get;
use crate::winnow::error::ParserError;
use winnow::ascii::{alphanumeric0, digit1, multispace0};
use winnow::combinator::{alt, fail, opt, repeat};
use winnow::error::{ContextError, ErrMode, StrContext};
use winnow::stream::Stream; // for checkpoint/reset on &str
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::fun::fun_trait::{Fun1Builder, Fun2Builder};
use wp_parser::fun::parser;
use wp_parser::symbol::{ctx_desc, symbol_pipe};
use wpl::parser::utils::take_key;

impl Fun1Builder for Nth {
    type ARG1 = usize;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let index = digit1.parse_next(data)?;
        let i: usize = index.parse::<usize>().unwrap_or(0);
        Ok(i)
    }

    fn fun_name() -> &'static str {
        PIPE_NTH
    }

    fn build(args: Self::ARG1) -> Self {
        Nth { index: args }
    }
}
impl Fun2Builder for TimeToTsZone {
    type ARG1 = i32;
    type ARG2 = TimeStampUnit;
    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS_ZONE
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        let sign = opt("-").parse_next(data)?;
        multispace0.parse_next(data)?;
        let zone = digit1.parse_next(data)?;
        let i: i32 = zone.parse::<i32>().unwrap_or(0);
        if sign.is_some() { Ok(-i) } else { Ok(i) }
    }
    fn args2(data: &mut &str) -> WResult<TimeStampUnit> {
        let unit = alt((
            "ms".map(|_| TimeStampUnit::MS),
            "us".map(|_| TimeStampUnit::US),
            "ss".map(|_| TimeStampUnit::SS),
            "s".map(|_| TimeStampUnit::SS),
        ))
        .parse_next(data)?;
        Ok(unit)
    }
    fn build(args: (i32, TimeStampUnit)) -> TimeToTsZone {
        TimeToTsZone {
            zone: args.0,
            unit: args.1,
        }
    }
}
impl Fun1Builder for Get {
    type ARG1 = String;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let name = take_key(data)?;
        Ok(name.to_string())
    }

    fn fun_name() -> &'static str {
        PIPE_GET
    }

    fn build(args: Self::ARG1) -> Self {
        Get { name: args }
    }
}
impl Fun1Builder for Base64Decode {
    type ARG1 = EncodeType;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val: &str = alphanumeric0::<&str, ErrMode<ContextError>>
            .parse_next(data)
            .unwrap();
        if val.is_empty() {
            Ok(EncodeType::Utf8)
        } else {
            Ok(EncodeType::from_str(val).map_err(|e| {
                warn_data!("unimplemented format {} base64 decode: {}", val, e);
                ErrMode::<ContextError>::from_input(data)
            })?)
        }
    }

    fn fun_name() -> &'static str {
        PIPE_BASE64_DECODE
    }

    fn build(args: Self::ARG1) -> Self {
        Base64Decode { encode: args }
    }
}
impl Fun1Builder for PathGet {
    type ARG1 = PathType;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val: &str = alphanumeric0::<&str, ErrMode<ContextError>>
            .parse_next(data)
            .unwrap();

        if val.is_empty() {
            Ok(PathType::Default)
        } else {
            Ok(PathType::from_str(val).map_err(|e| {
                warn_data!("invalid path arg '{}': {}", val, e);
                ErrMode::<ContextError>::from_input(data)
            })?)
        }
    }

    fn fun_name() -> &'static str {
        PIPE_PATH
    }

    fn build(args: Self::ARG1) -> Self {
        PathGet { key: args }
    }
}
impl Fun1Builder for UrlGet {
    type ARG1 = UrlType;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val: &str = alphanumeric0::<&str, ErrMode<ContextError>>
            .parse_next(data)
            .unwrap();

        if val.is_empty() {
            Ok(UrlType::Default)
        } else {
            Ok(UrlType::from_str(val).map_err(|e| {
                warn_data!("invalid url arg '{}': {}", val, e);
                ErrMode::<ContextError>::from_input(data)
            })?)
        }
    }

    fn fun_name() -> &'static str {
        PIPE_URL
    }

    fn build(args: Self::ARG1) -> Self {
        UrlGet { key: args }
    }
}
pub fn oml_aga_pipe(data: &mut &str) -> WResult<PreciseEvaluator> {
    kw_gw_pipe.parse_next(data)?;
    let from = oml_var_get.parse_next(data)?;
    let items = repeat(1.., oml_pipe).parse_next(data)?;
    Ok(PreciseEvaluator::Pipe(PiPeOperation::new(from, items)))
}

// 支持省略前缀 `pipe` 的管道表达式：read(...) | func | func ...
pub fn oml_aga_pipe_noprefix(data: &mut &str) -> WResult<PreciseEvaluator> {
    let cp = data.checkpoint();
    let from = oml_var_get.parse_next(data)?;
    match repeat(1.., oml_pipe).parse_next(data) {
        Ok(items) => Ok(PreciseEvaluator::Pipe(PiPeOperation::new(from, items))),
        Err(_e) => {
            data.reset(&cp);
            fail.parse_next(data)
        }
    }
}

pub fn oml_pipe(data: &mut &str) -> WResult<PipeFun> {
    symbol_pipe.parse_next(data)?;
    multispace0.parse_next(data)?;
    let fun = alt((
        parser::call_fun_args2::<TimeToTsZone>.map(PipeFun::TimeToTsZone),
        parser::call_fun_args1::<Nth>.map(PipeFun::Nth),
        parser::call_fun_args1::<Get>.map(PipeFun::Get),
        parser::call_fun_args1::<Base64Decode>.map(PipeFun::Base64Decode),
        parser::call_fun_args1::<PathGet>.map(PipeFun::PathGet),
        parser::call_fun_args1::<UrlGet>.map(PipeFun::UrlGet),
        PIPE_HTML_ESCAPE.map(|_| PipeFun::HtmlEscape(HtmlEscape::default())),
        PIPE_HTML_UNESCAPE.map(|_| PipeFun::HtmlUnescape(HtmlUnescape::default())),
        PIPE_STR_ESCAPE.map(|_| PipeFun::StrEscape(StrEscape::default())),
        PIPE_JSON_ESCAPE.map(|_| PipeFun::JsonEscape(JsonEscape::default())),
        PIPE_JSON_UNESCAPE.map(|_| PipeFun::JsonUnescape(JsonUnescape::default())),
        PIPE_BASE64_ENCODE.map(|_| PipeFun::Base64Encode(Base64Encode::default())),
        PIPE_TIME_TO_TS_MS.map(|_| PipeFun::TimeToTsMs(TimeToTsMs::default())),
        PIPE_TIME_TO_TS_US.map(|_| PipeFun::TimeToTsUs(TimeToTsUs::default())),
        PIPE_TIME_TO_TS.map(|_| PipeFun::TimeToTs(TimeToTs::default())),
        PIPE_TO_JSON.map(|_| PipeFun::ToJson(ToJson::default())),
        PIPE_TO_STR.map(|_| PipeFun::ToStr(ToStr::default())),
        PIPE_SKIP_EMPTY.map(|_| PipeFun::SkipEmpty(SkipEmpty::default())),
        PIPE_IP4_TO_INT.map(|_| PipeFun::Ip4ToInt(Ip4ToInt::default())),
    ))
    .context(StrContext::Label("pipe fun"))
    .context(ctx_desc("fun not found!"))
    .parse_next(data)?;
    Ok(fun)
}

#[cfg(test)]
mod tests {
    use crate::parser::pipe_prm::oml_aga_pipe;
    use crate::parser::utils::for_test::{assert_oml_parse, err_of_oml};
    use wp_parser::WResult;

    #[test]
    fn test_oml_crate_lib() -> WResult<()> {
        let mut code = r#" pipe take(ip) | to_str | to_json | base64_encode | base64_decode(Utf8)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | to_str | html_escape | html_unescape | str_escape"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | to_str | json_escape | json_unescape"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | Time::to_ts | Time::to_ts_ms | Time::to_ts_us"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | Time::to_ts_zone(8,ms) | Time::to_ts_zone(-8,ss)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | skip_empty"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | path(name)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | url(host)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);
        Ok(())
    }
    #[test]
    fn test_pipe_oml_err() {
        let mut code = r#" pipe take(ip) | xyz_get()"#;
        let e = err_of_oml(&mut code, oml_aga_pipe);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(e.to_string().contains("fun not found"));

        let mut code = r#" ipe take(ip) | xyz_get()"#;
        let e = err_of_oml(&mut code, oml_aga_pipe);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(e.to_string().contains("need 'pipe' keyword"));
    }
}
