use std::net::IpAddr;

use smol_str::SmolStr;
use winnow::{
    Parser,
    ascii::{digit1, multispace0},
    combinator::{alt, fail},
    token::literal,
};
use wp_parser::{
    WResult,
    fun::{fun_trait::Fun0Builder, parser::call_fun_args0},
};
use wp_parser::{
    atom::take_string,
    fun::{
        fun_trait::{Fun1Builder, Fun2Builder, ParseNext},
        parser::{call_fun_args1, call_fun_args2, take_arr},
    },
};

use crate::ast::{
    WplFun,
    processor::{
        CharsHas, CharsIn, CharsInArg, CharsNotHas, CharsNotHasArg, CharsValue, DigitHas,
        DigitHasArg, DigitIn, DigitInArg, DigitRange, Has, HasArg, IpIn, IpInArg, RegexMatch,
        ReplaceFunc, SelectLast, TakeField, TargetCharsHas, TargetCharsIn, TargetCharsNotHas,
        TargetDigitHas, TargetDigitIn, TargetHas, TargetIpIn, normalize_target,
    },
};

use super::utils::take_key;

/// 解析带引号的字符串："any string, with special chars"
/// 支持转义字符：\" \\ \n \t
fn take_quoted_string(input: &mut &str) -> WResult<String> {
    literal("\"").parse_next(input)?;

    let mut result = String::new();
    let mut chars = input.chars();

    loop {
        match chars.next() {
            None => {
                return fail.parse_next(input);
            }
            Some('\\') => {
                // 处理转义字符
                match chars.next() {
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    _ => {
                        return fail.parse_next(input);
                    }
                }
            }
            Some('"') => {
                // 结束引号
                let consumed = input.len() - chars.as_str().len();
                *input = &input[consumed..];
                return Ok(result);
            }
            Some(ch) => result.push(ch),
        }
    }
}

/// 解析单引号字符串：'any string, with special chars'
/// 单引号为原始字符串，只支持 \' 转义单引号本身，其他字符按字面意思处理
fn take_single_quoted_string(input: &mut &str) -> WResult<String> {
    literal("'").parse_next(input)?;

    let mut result = String::new();
    let mut chars = input.chars();

    loop {
        match chars.next() {
            None => {
                return fail.parse_next(input);
            }
            Some('\\') => {
                // 单引号字符串只处理 \' 转义
                match chars.as_str().chars().next() {
                    Some('\'') => {
                        result.push('\'');
                        chars.next(); // 消费 '
                    }
                    _ => {
                        // 其他情况，\ 按字面意思处理
                        result.push('\\');
                    }
                }
            }
            Some('\'') => {
                // 结束引号
                let consumed = input.len() - chars.as_str().len();
                *input = &input[consumed..];
                return Ok(result);
            }
            Some(ch) => result.push(ch),
        }
    }
}

/// 解析字符串：支持单引号、双引号（可包含逗号、空格等特殊字符）和不带引号
fn take_string_or_quoted(input: &mut &str) -> WResult<String> {
    multispace0.parse_next(input)?;
    alt((
        take_quoted_string,
        take_single_quoted_string,
        take_string.map(|s: &str| s.to_string()),
    ))
    .parse_next(input)
}

pub fn wpl_fun(input: &mut &str) -> WResult<WplFun> {
    multispace0.parse_next(input)?;
    let fun = alt((
        // Put digit_range first to avoid any prefix matching issues
        call_fun_args2::<DigitRangeArg>.map(|arg| WplFun::DigitRange(DigitRange {
            begin: arg.begin,
            end: arg.end,
        })),
        call_fun_args1::<RegexMatch>.map(WplFun::RegexMatch),
        call_fun_args1::<TakeField>.map(WplFun::SelectTake),
        call_fun_args0::<SelectLast>.map(WplFun::SelectLast),
        call_fun_args2::<TargetCharsHas>.map(WplFun::TargetCharsHas),
        call_fun_args1::<CharsHas>.map(WplFun::CharsHas),
        call_fun_args2::<TargetCharsNotHas>.map(WplFun::TargetCharsNotHas),
        call_fun_args1::<CharsNotHasArg>
            .map(|arg| WplFun::CharsNotHas(CharsNotHas { value: arg.value })),
        call_fun_args2::<TargetCharsIn>.map(WplFun::TargetCharsIn),
        call_fun_args1::<CharsInArg>.map(|arg| WplFun::CharsIn(CharsIn { value: arg.value })),
        call_fun_args2::<TargetDigitHas>.map(WplFun::TargetDigitHas),
        call_fun_args1::<DigitHasArg>.map(|arg| WplFun::DigitHas(DigitHas { value: arg.value })),
        call_fun_args2::<TargetDigitIn>.map(WplFun::TargetDigitIn),
        call_fun_args1::<DigitInArg>.map(|arg| WplFun::DigitIn(DigitIn { value: arg.value })),
        call_fun_args2::<TargetIpIn>.map(WplFun::TargetIpIn),
        call_fun_args1::<IpInArg>.map(|arg| WplFun::IpIn(IpIn { value: arg.value })),
        call_fun_args1::<TargetHas>.map(WplFun::TargetHas),
        call_fun_args0::<HasArg>.map(|_| WplFun::Has(Has)),
        call_fun_args0::<JsonUnescape>.map(WplFun::TransJsonUnescape),
        call_fun_args0::<Base64Decode>.map(WplFun::TransBase64Decode),
        call_fun_args2::<ReplaceFunc>.map(WplFun::TransCharsReplace),
    ))
    .parse_next(input)?;
    Ok(fun)
}

impl Fun2Builder for TargetDigitHas {
    type ARG1 = SmolStr;
    type ARG2 = i64;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }
    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        multispace0.parse_next(data)?;
        let val = digit1.parse_next(data)?;
        Ok(val.parse::<i64>().unwrap_or(0))
    }

    fn fun_name() -> &'static str {
        "f_digit_has"
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: normalize_target(args.0),
            value: args.1,
        }
    }
}

impl Fun1Builder for CharsHas {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_string.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "chars_has"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { value: args }
    }
}

impl Fun1Builder for CharsNotHasArg {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_string.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "chars_not_has"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { value: args }
    }
}

impl Fun1Builder for CharsInArg {
    type ARG1 = Vec<CharsValue>;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        take_arr::<CharsValue>(data)
    }

    fn fun_name() -> &'static str {
        "chars_in"
    }

    fn build(args: Self::ARG1) -> Self {
        let value = args.iter().map(|i| i.0.clone()).collect();
        Self { value }
    }
}

impl Fun1Builder for DigitHasArg {
    type ARG1 = i64;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = digit1.parse_next(data)?;
        Ok(val.parse::<i64>().unwrap_or(0))
    }

    fn fun_name() -> &'static str {
        "digit_has"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { value: args }
    }
}

impl Fun1Builder for DigitInArg {
    type ARG1 = Vec<i64>;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        take_arr::<i64>(data)
    }

    fn fun_name() -> &'static str {
        "digit_in"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { value: args }
    }
}

impl Fun1Builder for IpInArg {
    type ARG1 = Vec<IpAddr>;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        take_arr::<IpAddr>(data)
    }

    fn fun_name() -> &'static str {
        "ip_in"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { value: args }
    }
}

impl Fun0Builder for HasArg {
    fn fun_name() -> &'static str {
        "has"
    }

    fn build() -> Self {
        HasArg
    }
}
impl Fun2Builder for TargetCharsHas {
    type ARG1 = SmolStr;
    type ARG2 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }
    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        multispace0.parse_next(data)?;
        let val = take_string.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "f_chars_has"
    }
    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: normalize_target(args.0),
            value: args.1,
        }
    }
}

impl Fun2Builder for TargetCharsNotHas {
    type ARG1 = SmolStr;
    type ARG2 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }
    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        multispace0.parse_next(data)?;
        let val = take_string.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "f_chars_not_has"
    }
    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: normalize_target(args.0),
            value: args.1,
        }
    }
}

impl ParseNext<CharsValue> for CharsValue {
    fn parse_next(input: &mut &str) -> WResult<CharsValue> {
        let val = take_string.parse_next(input)?;
        Ok(CharsValue(val.into()))
    }
}
impl Fun2Builder for TargetCharsIn {
    type ARG1 = SmolStr;
    type ARG2 = Vec<CharsValue>;
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        take_arr::<CharsValue>(data)
    }

    fn fun_name() -> &'static str {
        "f_chars_in"
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        let value: Vec<SmolStr> = args.1.iter().map(|i| i.0.clone()).collect();
        Self {
            target: normalize_target(args.0),
            value,
        }
    }
}

impl Fun2Builder for TargetDigitIn {
    type ARG1 = SmolStr;
    type ARG2 = Vec<i64>;

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        take_arr::<i64>(data)
    }
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "f_digit_in"
    }
    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: normalize_target(args.0),
            value: args.1,
        }
    }
}
impl Fun1Builder for TargetHas {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "f_has"
    }

    fn build(args: Self::ARG1) -> Self {
        Self {
            target: normalize_target(args),
        }
    }
}

impl Fun2Builder for TargetIpIn {
    type ARG1 = SmolStr;
    type ARG2 = Vec<IpAddr>;

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        take_arr::<IpAddr>(data)
    }
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "f_ip_in"
    }
    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: normalize_target(args.0),
            value: args.1,
        }
    }
}

// ---------------- String Mode ----------------
use crate::ast::processor::JsonUnescape;

impl Fun0Builder for JsonUnescape {
    fn fun_name() -> &'static str {
        "json_unescape"
    }

    fn build() -> Self {
        JsonUnescape {}
    }
}

use crate::ast::processor::Base64Decode;
impl Fun0Builder for Base64Decode {
    fn fun_name() -> &'static str {
        "base64_decode"
    }

    fn build() -> Self {
        Base64Decode {}
    }
}

impl Fun2Builder for ReplaceFunc {
    type ARG1 = SmolStr;
    type ARG2 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_string_or_quoted.parse_next(data)?;
        Ok(val.into())
    }

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        multispace0.parse_next(data)?;
        let val = take_string_or_quoted.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "chars_replace"
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            target: args.0,
            value: args.1,
        }
    }
}

/// Parser argument for `digit_range(begin, end)` - converted to DigitRange
#[derive(Clone, Debug, PartialEq)]
pub struct DigitRangeArg {
    pub(crate) begin: i64,
    pub(crate) end: i64,
}

impl Fun2Builder for DigitRangeArg {
    type ARG1 = i64;
    type ARG2 = i64;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = digit1.parse_next(data)?;
        Ok(val.parse::<i64>().unwrap_or(0))
    }

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        multispace0.parse_next(data)?;
        let val = digit1.parse_next(data)?;
        Ok(val.parse::<i64>().unwrap_or(0))
    }

    fn fun_name() -> &'static str {
        "digit_range"
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        Self {
            begin: args.0,
            end: args.1,
        }
    }
}

impl Fun1Builder for RegexMatch {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_string_or_quoted.parse_next(data)?;
        Ok(val.into())
    }

    fn fun_name() -> &'static str {
        "regex_match"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { pattern: args }
    }
}

impl Fun1Builder for TakeField {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = alt((
            take_quoted_string.map(SmolStr::from),
            take_single_quoted_string.map(SmolStr::from),
            take_key.map(SmolStr::from),
        ))
        .parse_next(data)?;
        Ok(val)
    }

    fn fun_name() -> &'static str {
        "take"
    }

    fn build(args: Self::ARG1) -> Self {
        Self { target: args }
    }
}

impl Fun0Builder for SelectLast {
    fn fun_name() -> &'static str {
        "last"
    }

    fn build() -> Self {
        SelectLast {}
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use orion_error::TestAssert;

    use crate::ast::processor::{Has, JsonUnescape, ReplaceFunc, SelectLast, TakeField};

    use super::*;

    #[test]
    fn test_parse_fun() {
        let fun = wpl_fun.parse(r#"f_has(src)"#).assert();
        assert_eq!(
            fun,
            WplFun::TargetHas(TargetHas {
                target: Some("src".into())
            })
        );

        let fun = wpl_fun.parse("has()").assert();
        assert_eq!(fun, WplFun::Has(Has));

        let fun = wpl_fun.parse(r#"f_digit_in(src, [1,2,3])"#).assert();
        assert_eq!(
            fun,
            WplFun::TargetDigitIn(TargetDigitIn {
                target: Some("src".into()),
                value: vec![1, 2, 3]
            })
        );

        let fun = wpl_fun.parse("digit_has(42)").assert();
        assert_eq!(fun, WplFun::DigitHas(DigitHas { value: 42 }));

        let fun = wpl_fun.parse("digit_in([4,5])").assert();
        assert_eq!(fun, WplFun::DigitIn(DigitIn { value: vec![4, 5] }));

        let fun = wpl_fun
            .parse(r#"f_ip_in(src, [127.0.0.1, 127.0.0.2])"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TargetIpIn(TargetIpIn {
                target: Some("src".into()),
                value: vec![
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2))
                ]
            })
        );

        let fun = wpl_fun.parse(r#"ip_in([127.0.0.1,::1])"#).assert();
        assert_eq!(
            fun,
            WplFun::IpIn(IpIn {
                value: vec![
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    IpAddr::V6(Ipv6Addr::LOCALHOST),
                ],
            })
        );

        // IPv6 裸字面量与混合示例
        let fun = wpl_fun
            .parse(r#"f_ip_in(src, [::1, 2001:db8::1])"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TargetIpIn(TargetIpIn {
                target: Some("src".into()),
                value: vec![
                    IpAddr::V6(Ipv6Addr::LOCALHOST),
                    IpAddr::V6("2001:db8::1".parse().unwrap()),
                ]
            })
        );

        let fun = wpl_fun.parse("json_unescape()").assert();
        assert_eq!(fun, WplFun::TransJsonUnescape(JsonUnescape {}));

        assert!(wpl_fun.parse("json_unescape(decoded)").is_err());

        let fun = wpl_fun.parse("take(src)").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "src".into(),
            })
        );

        // Test take with double quotes
        let fun = wpl_fun.parse(r#"take("@key")"#).assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "@key".into(),
            })
        );

        // Test take with single quotes
        let fun = wpl_fun.parse("take('@field')").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "@field".into(),
            })
        );

        // Test take with special characters in double quotes
        let fun = wpl_fun.parse(r#"take("field with spaces")"#).assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "field with spaces".into(),
            })
        );

        // Test take with special characters in single quotes
        let fun = wpl_fun.parse("take('field,with,commas')").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "field,with,commas".into(),
            })
        );

        // Test take with escaped quote in double quotes
        let fun = wpl_fun.parse(r#"take("field\"name")"#).assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "field\"name".into(),
            })
        );

        // Test take with escaped quote in single quotes
        let fun = wpl_fun.parse("take('field\\'name')").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "field'name".into(),
            })
        );

        // Test single quotes are raw strings - no escape for \n, \t, etc
        let fun = wpl_fun.parse(r"take('raw\nstring')").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: r"raw\nstring".into(),
            })
        );

        let fun = wpl_fun.parse(r"take('path\to\file')").assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: r"path\to\file".into(),
            })
        );

        // Test double quotes still support escapes
        let fun = wpl_fun.parse(r#"take("line\nbreak")"#).assert();
        assert_eq!(
            fun,
            WplFun::SelectTake(TakeField {
                target: "line\nbreak".into(),
            })
        );

        let fun = wpl_fun.parse("last()").assert();
        assert_eq!(fun, WplFun::SelectLast(SelectLast {}));

        let fun = wpl_fun.parse("f_chars_has(_, foo)").assert();
        assert_eq!(
            fun,
            WplFun::TargetCharsHas(TargetCharsHas {
                target: None,
                value: "foo".into(),
            })
        );

        let fun = wpl_fun.parse("chars_has(bar)").assert();
        assert_eq!(
            fun,
            WplFun::CharsHas(CharsHas {
                value: "bar".into(),
            })
        );

        let fun = wpl_fun.parse("chars_has(中文)").assert();
        assert_eq!(
            fun,
            WplFun::CharsHas(CharsHas {
                value: "中文".into(),
            })
        );

        let fun = wpl_fun.parse("chars_not_has(baz)").assert();
        assert_eq!(
            fun,
            WplFun::CharsNotHas(CharsNotHas {
                value: "baz".into(),
            })
        );

        let fun = wpl_fun.parse("chars_in([foo,bar])").assert();
        assert_eq!(
            fun,
            WplFun::CharsIn(CharsIn {
                value: vec!["foo".into(), "bar".into()],
            })
        );

        let fun = wpl_fun.parse("base64_decode()").assert();
        assert_eq!(fun, WplFun::TransBase64Decode(Base64Decode {}));
        assert!(wpl_fun.parse("base64_decode(decoded)").is_err());

        // chars_replace tests
        let fun = wpl_fun.parse(r#"chars_replace(hello, hi)"#).assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "hello".into(),
                value: "hi".into(),
            })
        );

        let fun = wpl_fun
            .parse(r#"chars_replace(old_value, new_value)"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "old_value".into(),
                value: "new_value".into(),
            })
        );

        // chars_replace with Chinese characters
        let fun = wpl_fun.parse(r#"chars_replace(旧值, 新值)"#).assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "旧值".into(),
                value: "新值".into(),
            })
        );

        // chars_replace with special characters
        let fun = wpl_fun
            .parse(r#"chars_replace(test-old, test-new)"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "test-old".into(),
                value: "test-new".into(),
            })
        );

        // chars_replace with underscores
        let fun = wpl_fun
            .parse(r#"chars_replace(error_code, status_code)"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "error_code".into(),
                value: "status_code".into(),
            })
        );

        // chars_replace with quoted strings (supports commas and spaces)
        let fun = wpl_fun
            .parse(r#"chars_replace("test,old", "test,new")"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "test,old".into(),
                value: "test,new".into(),
            })
        );

        // chars_replace with quoted strings containing spaces
        let fun = wpl_fun
            .parse(r#"chars_replace("hello world", "goodbye world")"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "hello world".into(),
                value: "goodbye world".into(),
            })
        );

        // chars_replace mixing quoted and unquoted
        let fun = wpl_fun
            .parse(r#"chars_replace("test,old", new_value)"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "test,old".into(),
                value: "new_value".into(),
            })
        );

        // chars_replace with empty quoted string
        let fun = wpl_fun.parse(r#"chars_replace(test, "")"#).assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "test".into(),
                value: "".into(),
            })
        );

        // chars_replace with escaped quotes
        let fun = wpl_fun
            .parse(r#"chars_replace("test,old", "\"test,new\"")"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "test,old".into(),
                value: "\"test,new\"".into(),
            })
        );

        // chars_replace with escaped backslash
        let fun = wpl_fun
            .parse(r#"chars_replace("path\\to\\file", "new\\path")"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "path\\to\\file".into(),
                value: "new\\path".into(),
            })
        );

        // chars_replace with newline and tab
        let fun = wpl_fun
            .parse(r#"chars_replace("line1\nline2", "tab\there")"#)
            .assert();
        assert_eq!(
            fun,
            WplFun::TransCharsReplace(ReplaceFunc {
                target: "line1\nline2".into(),
                value: "tab\there".into(),
            })
        );
    }

    #[test]
    fn test_parse_digit_range() {
        use winnow::Parser;
        use wp_parser::fun::parser::call_fun_args2;

        // Direct test of DigitRangeArg parser - simple case
        let mut input = "digit_range(1, 10)";
        let result = call_fun_args2::<DigitRangeArg>.parse_next(&mut input);
        assert!(result.is_ok(), "Simple case should parse successfully: {:?}", result);
        let arg = result.unwrap();
        assert_eq!(arg.begin, 1);
        assert_eq!(arg.end, 10);

        // Direct test with different values
        let mut input2 = "digit_range(100, 200)";
        let result2 = call_fun_args2::<DigitRangeArg>.parse_next(&mut input2);
        assert!(result2.is_ok(), "Different values should parse: {:?}", result2);
        let arg2 = result2.unwrap();
        assert_eq!(arg2.begin, 100);
        assert_eq!(arg2.end, 200);
    }

    #[test]
    fn test_parse_regex_match() {
        let mut wpl_fun = wpl_fun;

        // regex_match with simple pattern (use single quotes for raw string)
        let fun = wpl_fun.parse(r"regex_match('^\d+$')").assert();
        assert_eq!(
            fun,
            WplFun::RegexMatch(RegexMatch {
                pattern: r"^\d+$".into(),
            })
        );

        // regex_match with complex pattern
        let fun = wpl_fun.parse(r"regex_match('^\w+@\w+\.\w+$')").assert();
        assert_eq!(
            fun,
            WplFun::RegexMatch(RegexMatch {
                pattern: r"^\w+@\w+\.\w+$".into(),
            })
        );

        // regex_match with alternation
        let fun = wpl_fun.parse(r"regex_match('^(GET|POST|PUT)$')").assert();
        assert_eq!(
            fun,
            WplFun::RegexMatch(RegexMatch {
                pattern: r"^(GET|POST|PUT)$".into(),
            })
        );
    }
}
