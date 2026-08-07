use std::str::FromStr;

use crate::language::{
    Base64Decode, EncodeType, Get, HtmlEscape, HtmlUnescape, Ip4ToInt, IpToBigUint, JsonEscape,
    JsonUnescape, MapTo, MapValue, Nth, PIPE_BASE64_DECODE, PIPE_GET, PIPE_HTML_ESCAPE,
    PIPE_HTML_UNESCAPE, PIPE_IP_TO_BIGUINT, PIPE_IP4_TO_INT, PIPE_JSON_ESCAPE, PIPE_JSON_UNESCAPE,
    PIPE_MAP_TO, PIPE_NTH, PIPE_PATH, PIPE_SKIP_EMPTY, PIPE_STARTS_WITH, PIPE_STR_ESCAPE,
    PIPE_TIME_FROM_TS, PIPE_TIME_FROM_TS_MS, PIPE_TIME_FROM_TS_US, PIPE_TIME_TO_TS,
    PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_URL,
    PathGet, PathType, PiPeOperation, PipeFun, PipeSource, PreciseEvaluator, SkipEmpty,
    StartsWith, StrEscape, TimeFromTs, TimeFromTsMs, TimeFromTsUs, TimeStampUnit, TimeToTs,
    TimeToTsMs, TimeToTsUs, TimeToTsZone, ToJson, UrlGet, UrlType,
};
use crate::language::{
    Base64Encode, ExtractMainWord, ExtractSubjectObject, IntranetIp, OnFail, PIPE_BASE64_ENCODE,
    PIPE_EXTRACT_MAIN_WORD, PIPE_EXTRACT_SUBJECT_OBJECT, PIPE_INTRANET_IP, PIPE_ON_FAIL,
    PIPE_TO_STR, ToStr,
};
use crate::parser::keyword::kw_gw_pipe;
use crate::parser::oml_aggregate::oml_var_get;
use crate::winnow::error::ParserError;
use winnow::ascii::{alphanumeric0, digit1, multispace0};
use winnow::combinator::{alt, fail, opt, repeat};
use winnow::error::{ContextError, ErrMode, StrContext};
use winnow::stream::Stream; // for checkpoint/reset on &str
use winnow::token::take;
use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::fun::fun_trait::{Fun1Builder, Fun2Builder};
use wp_primitives::fun::parser;
use wp_primitives::symbol::{ctx_desc, symbol_pipe};
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
/// zone 时区偏移解析：可选负号 + 数字；超 i32 范围或超出 FixedOffset 上限时报错（避免静默变 0）
fn parse_zone(data: &mut &str) -> WResult<i32> {
    let sign = opt("-").parse_next(data)?;
    multispace0.parse_next(data)?;
    let zone = digit1.parse_next(data)?;
    let signed = match sign {
        Some(_) => format!("-{}", zone),
        None => zone.to_string(),
    };
    let z = signed.parse::<i32>().map_err(|_| {
        warn_rule!("invalid zone '{}': out of i32 range", signed);
        // Cut：硬错误，避免被 alt 兜底成无参形式后在顶层残留括号报错
        ErrMode::Cut(ContextError::from_input(data))
    })?;
    // FixedOffset 上限 -86400 < secs < 86400，整点 zone 合法范围 -23..=23；
    // 超范围的字面量是模型错误，解析期报错而非运行时透传
    if z.abs() > 23 {
        warn_rule!("invalid zone '{}': 超出 FixedOffset 范围（整点 ±23h）", z);
        return Err(ErrMode::Cut(ContextError::from_input(data)));
    }
    Ok(z)
}

impl Fun2Builder for TimeToTsZone {
    type ARG1 = i32;
    type ARG2 = TimeStampUnit;
    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS_ZONE
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
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
impl Fun1Builder for TimeToTsMs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS_MS
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeToTsMs { zone: Some(args) }
    }
}

impl Fun1Builder for TimeFromTsMs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_FROM_TS_MS
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeFromTsMs { zone: Some(args) }
    }
}
impl Fun1Builder for TimeToTs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeToTs { zone: Some(args) }
    }
}
impl Fun1Builder for TimeToTsUs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS_US
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeToTsUs { zone: Some(args) }
    }
}
impl Fun1Builder for TimeFromTs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_FROM_TS
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeFromTs { zone: Some(args) }
    }
}
impl Fun1Builder for TimeFromTsUs {
    type ARG1 = i32;
    fn fun_name() -> &'static str {
        PIPE_TIME_FROM_TS_US
    }
    fn args1(data: &mut &str) -> WResult<i32> {
        parse_zone(data)
    }
    fn build(args: Self::ARG1) -> Self {
        TimeFromTsUs { zone: Some(args) }
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

impl Fun1Builder for OnFail {
    type ARG1 = String;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        use wpl::parser::utils::quot_str;
        multispace0.parse_next(data)?;
        let value = quot_str.parse_next(data)?;
        Ok(value.to_string())
    }

    fn fun_name() -> &'static str {
        PIPE_ON_FAIL
    }

    fn build(args: Self::ARG1) -> Self {
        OnFail { value: args }
    }
}

impl Fun1Builder for StartsWith {
    type ARG1 = String;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        use wpl::parser::utils::quot_str;
        multispace0.parse_next(data)?;
        let prefix = quot_str.parse_next(data)?;
        Ok(prefix.to_string())
    }

    fn fun_name() -> &'static str {
        PIPE_STARTS_WITH
    }

    fn build(args: Self::ARG1) -> Self {
        StartsWith { prefix: args }
    }
}

impl Fun1Builder for MapTo {
    type ARG1 = MapValue;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        use winnow::ascii::float;
        use winnow::token::literal;
        use wpl::parser::utils::quot_str;

        multispace0.parse_next(data)?;

        // 尝试解析布尔值
        if literal::<&str, &str, ContextError>("true")
            .parse_next(data)
            .is_ok()
        {
            return Ok(MapValue::Bool(true));
        }
        if literal::<&str, &str, ContextError>("false")
            .parse_next(data)
            .is_ok()
        {
            return Ok(MapValue::Bool(false));
        }

        // 尝试解析数字：先尝试整数，失败时再尝试浮点数
        let checkpoint = data.checkpoint();

        // Try parsing as integer first (to avoid precision loss for large integers)
        // Use digit1 to capture the numeric string, then parse directly

        // Check for optional negative sign
        let has_minus = literal::<&str, &str, ContextError>("-")
            .parse_next(data)
            .is_ok();

        // Try to parse digits
        if let Ok(digits) = digit1::<&str, ContextError>.parse_next(data) {
            // Peek next char - if it's '.' or 'e'/'E', it's a float
            let peek_checkpoint = data.checkpoint();
            let next_char = opt(take::<usize, &str, ContextError>(1usize)).parse_next(data);
            data.reset(&peek_checkpoint);

            let is_float = next_char
                .ok()
                .flatten()
                .map(|c| c == "." || c == "e" || c == "E")
                .unwrap_or(false);

            if !is_float {
                // It's an integer
                let num_str = if has_minus {
                    format!("-{}", digits)
                } else {
                    digits.to_string()
                };

                if let Ok(i) = num_str.parse::<i64>() {
                    return Ok(MapValue::Digit(i));
                } else {
                    warn_rule!("integer out of range: {}", num_str);
                    // Fall through to try float parsing
                }
            }
        }

        // Reset and try parsing as float
        data.reset(&checkpoint);
        if let Ok(f) = float::<&str, f64, ContextError>.parse_next(data) {
            return Ok(MapValue::Float(f));
        }
        data.reset(&checkpoint);

        // 尝试解析字符串
        if let Ok(s) = quot_str.parse_next(data) {
            return Ok(MapValue::Chars(s.to_string()));
        }

        fail.context(ctx_desc("expected string, number, or boolean"))
            .parse_next(data)
    }

    fn fun_name() -> &'static str {
        PIPE_MAP_TO
    }

    fn build(args: Self::ARG1) -> Self {
        MapTo { value: args }
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
                warn_rule!("unimplemented format {} base64 decode: {}", val, e);
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
                warn_rule!("invalid path arg '{}': {}", val, e);
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
                warn_rule!("invalid url arg '{}': {}", val, e);
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
    let from = alt((
        oml_var_get.map(PipeSource::Accessor),
        crate::parser::access_direct_prm::oml_access_direct.map(PipeSource::AccessDirect),
    ))
    .parse_next(data)?;
    let items = repeat(1.., oml_pipe).parse_next(data)?;
    Ok(PreciseEvaluator::Pipe(PiPeOperation::new(from, items)))
}

// 支持省略前缀 `pipe` 的管道表达式：read(...) | func | func ...
pub fn oml_aga_pipe_noprefix(data: &mut &str) -> WResult<PreciseEvaluator> {
    let cp = data.checkpoint();
    let from = oml_var_get.parse_next(data)?;
    match repeat(1.., oml_pipe).parse_next(data) {
        Ok(items) => Ok(PreciseEvaluator::Pipe(PiPeOperation::new(
            PipeSource::Accessor(from),
            items,
        ))),
        Err(_e) => {
            data.reset(&cp);
            fail.parse_next(data)
        }
    }
}

pub fn oml_pipe(data: &mut &str) -> WResult<PipeFun> {
    symbol_pipe.parse_next(data)?;
    multispace0.parse_next(data)?;
    let fun = alt((pipe_fun_with_args, pipe_fun_simple))
        .context(StrContext::Label("pipe fun"))
        .context(ctx_desc("fun not found!"))
        .parse_next(data)?;
    Ok(fun)
}

fn pipe_fun_with_args(data: &mut &str) -> WResult<PipeFun> {
    // 带参管道函数（winnow alt 元组上限 9，超限时拆多层）
    // 注意前缀关系：to_ts_zone/to_ts_ms/to_ts_us 排在 to_ts 前，from_ts_ms/from_ts_us 排在 from_ts 前
    alt((
        alt((
            parser::call_fun_args2::<TimeToTsZone>.map(PipeFun::TimeToTsZone),
            parser::call_fun_args1::<TimeToTsMs>.map(PipeFun::TimeToTsMs),
            parser::call_fun_args1::<TimeToTsUs>.map(PipeFun::TimeToTsUs),
            parser::call_fun_args1::<TimeToTs>.map(PipeFun::TimeToTs),
            parser::call_fun_args1::<TimeFromTsMs>.map(PipeFun::TimeFromTsMs),
            parser::call_fun_args1::<TimeFromTsUs>.map(PipeFun::TimeFromTsUs),
            parser::call_fun_args1::<TimeFromTs>.map(PipeFun::TimeFromTs),
            parser::call_fun_args1::<Nth>.map(PipeFun::Nth),
            parser::call_fun_args1::<Get>.map(PipeFun::Get),
        )),
        alt((
            parser::call_fun_args1::<OnFail>.map(PipeFun::OnFail),
            parser::call_fun_args1::<StartsWith>.map(PipeFun::StartsWith),
            parser::call_fun_args1::<MapTo>.map(PipeFun::MapTo),
            parser::call_fun_args1::<Base64Decode>.map(PipeFun::Base64Decode),
            parser::call_fun_args1::<PathGet>.map(PipeFun::PathGet),
            parser::call_fun_args1::<UrlGet>.map(PipeFun::UrlGet),
        )),
    ))
    .parse_next(data)
}

fn pipe_fun_simple(data: &mut &str) -> WResult<PipeFun> {
    alt((
        pipe_fun_simple_core_a,
        pipe_fun_simple_core_b,
        pipe_fun_simple_extra,
    ))
    .parse_next(data)
}

fn pipe_fun_simple_core_a(data: &mut &str) -> WResult<PipeFun> {
    alt((
        PIPE_HTML_ESCAPE.map(|_| PipeFun::HtmlEscape(HtmlEscape::default())),
        PIPE_HTML_UNESCAPE.map(|_| PipeFun::HtmlUnescape(HtmlUnescape::default())),
        PIPE_STR_ESCAPE.map(|_| PipeFun::StrEscape(StrEscape::default())),
        PIPE_JSON_ESCAPE.map(|_| PipeFun::JsonEscape(JsonEscape::default())),
        PIPE_JSON_UNESCAPE.map(|_| PipeFun::JsonUnescape(JsonUnescape::default())),
    ))
    .parse_next(data)
}

fn pipe_fun_simple_core_b(data: &mut &str) -> WResult<PipeFun> {
    alt((
        PIPE_BASE64_ENCODE.map(|_| PipeFun::Base64Encode(Base64Encode::default())),
        PIPE_TIME_TO_TS_MS.map(|_| PipeFun::TimeToTsMs(TimeToTsMs::default())),
        PIPE_TIME_TO_TS_US.map(|_| PipeFun::TimeToTsUs(TimeToTsUs::default())),
        PIPE_TIME_TO_TS.map(|_| PipeFun::TimeToTs(TimeToTs::default())),
        PIPE_TIME_FROM_TS_MS.map(|_| PipeFun::TimeFromTsMs(TimeFromTsMs::default())),
        PIPE_TIME_FROM_TS_US.map(|_| PipeFun::TimeFromTsUs(TimeFromTsUs::default())),
        PIPE_TIME_FROM_TS.map(|_| PipeFun::TimeFromTs(TimeFromTs::default())),
        PIPE_TO_JSON.map(|_| PipeFun::ToJson(ToJson::default())),
    ))
    .parse_next(data)
}

fn pipe_fun_simple_extra(data: &mut &str) -> WResult<PipeFun> {
    alt((
        PIPE_TO_STR.map(|_| PipeFun::ToStr(ToStr::default())),
        PIPE_SKIP_EMPTY.map(|_| PipeFun::SkipEmpty(SkipEmpty::default())),
        PIPE_IP4_TO_INT.map(|_| PipeFun::Ip4ToInt(Ip4ToInt::default())),
        PIPE_IP_TO_BIGUINT.map(|_| PipeFun::IpToBigUint(IpToBigUint::default())),
        PIPE_INTRANET_IP.map(|_| PipeFun::IntranetIp(IntranetIp::default())),
        PIPE_EXTRACT_MAIN_WORD.map(|_| PipeFun::ExtractMainWord(ExtractMainWord::default())),
        PIPE_EXTRACT_SUBJECT_OBJECT
            .map(|_| PipeFun::ExtractSubjectObject(ExtractSubjectObject::default())),
    ))
    .parse_next(data)
}

#[cfg(test)]
mod tests {
    use crate::parser::pipe_prm::oml_aga_pipe;
    use crate::parser::utils::for_test::{assert_oml_parse, err_of_oml};
    use wp_error::parse_error::OMLCodeReason;
    use wp_primitives::WResult;

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

        // 时间戳函数支持可选 zone（无参、带正/负 zone 均可解析）
        let mut code = r#" pipe take(ip) | Time::to_ts | Time::to_ts_ms | Time::to_ts_us | Time::from_ts | Time::from_ts_ms | Time::from_ts_us"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | Time::to_ts(0) | Time::to_ts_ms(8) | Time::to_ts_us(-8) | Time::from_ts(8) | Time::from_ts_ms(-5) | Time::from_ts_us(0)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | Time::to_ts_ms(0) | Time::from_ts_ms(0) "#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | skip_empty"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | path(name)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(ip) | url(host)"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        let mut code = r#" pipe take(message) | extract_main_word"#;
        assert_oml_parse(&mut code, oml_aga_pipe);

        Ok(())
    }
    #[test]
    fn test_pipe_oml_err() {
        let mut code = r#" pipe take(ip) | xyz_get()"#;
        let e = err_of_oml(&mut code, oml_aga_pipe);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("fun not found")));

        let mut code = r#" ipe take(ip) | xyz_get()"#;
        let e = err_of_oml(&mut code, oml_aga_pipe);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(
            matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("need 'pipe' keyword"))
        );
    }

    #[test]
    fn test_pipe_optional_keyword() -> WResult<()> {
        use crate::parser::pipe_prm::oml_aga_pipe_noprefix;
        use wp_primitives::Parser;

        // Test pipe without 'pipe' keyword - should parse successfully
        let mut code = r#" take(ip) | to_str | to_json"#;
        let result = oml_aga_pipe_noprefix.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse without 'pipe' keyword");
        println!("Parsed: {}", result.unwrap());

        let mut code = r#" read(url) | starts_with('https://') | map_to(true)"#;
        let result = oml_aga_pipe_noprefix.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse starts_with and map_to");

        let mut code = r#" take(field) | base64_encode | base64_decode(Utf8)"#;
        let result = oml_aga_pipe_noprefix.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse base64 functions");

        let mut code = r#" read(path) | skip_empty | path(name)"#;
        let result = oml_aga_pipe_noprefix.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse skip_empty and path");

        Ok(())
    }

    #[test]
    fn test_map_to_large_integers() -> WResult<()> {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // Test large integer that would lose precision if parsed as f64 first
        // 9007199254740993 is 2^53 + 1, which cannot be exactly represented in f64
        let mut code = r#" pipe take(field) | map_to(9007199254740993)"#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_ok(), "Should parse large integer");

        let parsed = result.unwrap();
        let output = format!("{}", parsed);
        println!("Parsed large integer: {}", output);

        // Verify it contains the exact integer (not truncated)
        assert!(
            output.contains("9007199254740993"),
            "Large integer should be preserved exactly"
        );

        // Test negative large integer
        let mut code2 = r#" pipe take(field) | map_to(-9007199254740993)"#;
        let result2 = oml_aga_pipe.parse_next(&mut code2);
        assert!(result2.is_ok(), "Should parse negative large integer");

        let parsed2 = result2.unwrap();
        let output2 = format!("{}", parsed2);
        assert!(
            output2.contains("-9007199254740993"),
            "Negative large integer should be preserved exactly"
        );

        Ok(())
    }

    #[test]
    fn test_string_escaping_round_trip() -> WResult<()> {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // Test round-trip parsing: parse -> display -> parse
        // The key is that Display output should be parseable
        let test_cases = vec![
            r#" pipe take(field) | starts_with('O\'Reilly')"#,
            r#" pipe take(field) | starts_with('Bob\'s site')"#,
            r#" pipe take(field) | starts_with('path\\with\\backslash')"#,
            r#" pipe take(field) | starts_with('line1\nline2')"#,
            r#" pipe take(field) | starts_with('tab\there')"#,
            r#" pipe take(field) | map_to('test\'value')"#,
        ];

        for code in test_cases {
            println!("\n=== Testing code: {} ===", code);

            // Parse original code
            let mut code_slice = code;
            let result = oml_aga_pipe.parse_next(&mut code_slice);
            assert!(result.is_ok(), "Should parse code: '{}'", code);

            // Get Display output
            let parsed = result.unwrap();
            let output = format!("{}", parsed);
            println!("Display output: {}", output);

            // Parse the output again (round-trip)
            let mut output_slice = output.as_str();
            let result2 = oml_aga_pipe.parse_next(&mut output_slice);
            if let Err(e) = &result2 {
                println!("Round-trip parse error: {}", e);
            }
            assert!(
                result2.is_ok(),
                "Round-trip parse should succeed for: {}",
                code
            );

            // Verify third round also works
            let parsed2 = result2.unwrap();
            let output2 = format!("{}", parsed2);
            println!("Second display output: {}", output2);

            // Output should stabilize after first round-trip
            assert_eq!(
                output, output2,
                "Display output should be stable after round-trip"
            );
        }

        Ok(())
    }

    #[test]
    fn test_time_ts_round_trip() -> WResult<()> {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // 时间戳函数各形式（无参/正 zone/负 zone/组合）Display 往返稳定
        let cases = vec![
            r#" pipe take(field) | Time::to_ts "#,
            r#" pipe take(field) | Time::to_ts(0) "#,
            r#" pipe take(field) | Time::to_ts(-5) "#,
            r#" pipe take(field) | Time::to_ts_ms "#,
            r#" pipe take(field) | Time::to_ts_ms(8) "#,
            r#" pipe take(field) | Time::to_ts_ms(-5) "#,
            r#" pipe take(field) | Time::to_ts_us "#,
            r#" pipe take(field) | Time::to_ts_us(8) "#,
            r#" pipe take(field) | Time::to_ts_us(-5) "#,
            r#" pipe take(field) | Time::from_ts "#,
            r#" pipe take(field) | Time::from_ts(0) "#,
            r#" pipe take(field) | Time::from_ts(-5) "#,
            r#" pipe take(field) | Time::from_ts_ms "#,
            r#" pipe take(field) | Time::from_ts_ms(0) "#,
            r#" pipe take(field) | Time::from_ts_ms(-5) "#,
            r#" pipe take(field) | Time::from_ts_us "#,
            r#" pipe take(field) | Time::from_ts_us(0) "#,
            r#" pipe take(field) | Time::from_ts_us(-5) "#,
            r#" pipe take(field) | Time::from_ts_ms(0) | Time::to_ts_ms "#,
            r#" pipe take(field) | Time::to_ts(0) | Time::to_ts_us(8) "#,
            r#" pipe take(field) | Time::from_ts(0) | Time::from_ts_us(8) "#,
        ];

        for code in cases {
            println!("\n=== Testing code: {} ===", code);

            let mut code_slice = code;
            let result = oml_aga_pipe.parse_next(&mut code_slice);
            assert!(result.is_ok(), "Should parse code: '{}'", code);

            let parsed = result.unwrap();
            let output = format!("{}", parsed);
            println!("Display output: {}", output);

            let mut output_slice = output.as_str();
            let result2 = oml_aga_pipe.parse_next(&mut output_slice);
            assert!(
                result2.is_ok(),
                "Round-trip parse should succeed: '{}' -> '{}'",
                code,
                output
            );

            let parsed2 = result2.unwrap();
            let output2 = format!("{}", parsed2);
            assert_eq!(
                output, output2,
                "Display output should be stable after round-trip"
            );
        }

        Ok(())
    }

    #[test]
    fn test_time_ts_zone_overflow_rejected() {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // zone 超 i32 范围：应报语法错误，而非静默变 0（UTC）
        let mut code = r#" pipe take(ip) | Time::from_ts_ms(99999999999) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "from_ts_ms 超 i32 zone 应被拒绝");

        let mut code = r#" pipe take(ip) | Time::to_ts_ms(99999999999) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "to_ts_ms 超 i32 zone 应被拒绝");

        let mut code = r#" pipe take(ip) | Time::to_ts_zone(99999999999, ms) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "to_ts_zone 超 i32 zone 应被拒绝");

        let mut code = r#" pipe take(ip) | Time::to_ts(99999999999) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "to_ts 超 i32 zone 应被拒绝");

        let mut code = r#" pipe take(ip) | Time::to_ts_us(99999999999) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "to_ts_us 超 i32 zone 应被拒绝");

        let mut code = r#" pipe take(ip) | Time::from_ts(99999999999) "#;
        let result = oml_aga_pipe.parse_next(&mut code);
        assert!(result.is_err(), "from_ts 超 i32 zone 应被拒绝");
    }

    #[test]
    fn test_time_ts_zone_range_rejected() {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // |zone| >= 24 超出 FixedOffset 上限（-86400 < secs < 86400）：解析期报错
        for code in [
            r#" pipe take(ip) | Time::to_ts(24) "#,
            r#" pipe take(ip) | Time::to_ts_ms(-24) "#,
            r#" pipe take(ip) | Time::to_ts_us(25) "#,
            r#" pipe take(ip) | Time::from_ts(24) "#,
            r#" pipe take(ip) | Time::from_ts_ms(-25) "#,
            r#" pipe take(ip) | Time::from_ts_us(24) "#,
            r#" pipe take(ip) | Time::to_ts_zone(25, ms) "#,
        ] {
            let mut slice = code;
            let result = oml_aga_pipe.parse_next(&mut slice);
            assert!(result.is_err(), "超 FixedOffset 范围应被拒绝: '{}'", code);
        }
    }

    #[test]
    fn test_time_ts_zone_range_accepted() {
        use crate::parser::pipe_prm::oml_aga_pipe;
        use wp_primitives::Parser;

        // 边界 |zone| == 23 合法（FixedOffset 上限前一个整点）
        for code in [
            r#" pipe take(ip) | Time::to_ts(23) "#,
            r#" pipe take(ip) | Time::to_ts_ms(-23) "#,
            r#" pipe take(ip) | Time::from_ts_us(23) "#,
            r#" pipe take(ip) | Time::to_ts_zone(-23, ms) "#,
        ] {
            let mut slice = code;
            let result = oml_aga_pipe.parse_next(&mut slice);
            assert!(result.is_ok(), "|zone| == 23 应能解析: '{}'", code);
        }
    }
}
