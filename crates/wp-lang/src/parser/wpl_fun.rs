use std::net::IpAddr;

use smol_str::SmolStr;
use winnow::{
    Parser,
    ascii::{digit1, multispace0},
    combinator::alt,
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
        DigitHasArg, DigitIn, DigitInArg, Has, HasArg, IpIn, IpInArg, SelectLast, TakeField,
        TargetCharsHas, TargetCharsIn, TargetCharsNotHas, TargetDigitHas, TargetDigitIn, TargetHas,
        TargetIpIn, normalize_target,
    },
};

use super::utils::take_key;

pub fn wpl_fun(input: &mut &str) -> WResult<WplFun> {
    multispace0.parse_next(input)?;
    let fun = alt((
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

impl Fun1Builder for TakeField {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
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

    use crate::ast::processor::{Has, JsonUnescape, SelectLast, TakeField};

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
    }
}
