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
        Base64Decode, CharsHas, CharsIn, CharsInArg, CharsNotHas, CharsNotHasArg, CharsValue,
        DigitHas, DigitHasArg, DigitIn, DigitInArg, ExtPassFunc, Has, HasArg, IpIn, IpInArg,
        JsonUnescape, SelectLast, SplitInnerSrcFunc, TakeField, TargetCharsHas, TargetCharsIn,
        TargetCharsNotHas, TargetDigitHas, TargetDigitIn, TargetHas, TargetIpIn, VecToSrcFunc,
        normalize_target,
    },
};
use crate::traits::FiledExtendType;

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
        call_fun_args0::<VecToSrcFunc>.map(WplFun::VecToSrc),
        call_fun_args0::<ExtPassFunc>.map(WplFun::TransExtPass),
        call_fun_args1::<SplitInnerSrcFunc>.map(WplFun::SplitToSrc),
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

impl Fun0Builder for ExtPassFunc {
    fn fun_name() -> &'static str {
        "to_ext_pass"
    }

    fn build() -> Self {
        ExtPassFunc::from_registry(FiledExtendType::MemChannel)
            .expect("to_ext_pass processor not registered")
    }
}

impl Fun0Builder for VecToSrcFunc {
    fn fun_name() -> &'static str {
        "vec_to_src"
    }

    fn build() -> Self {
        VecToSrcFunc::from_registry(FiledExtendType::InnerSource)
            .expect("vec_to_src processor not registered")
    }
}

impl Fun1Builder for SplitInnerSrcFunc {
    type ARG1 = SmolStr;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        if let Ok(val) = take_string.parse_next(data) {
            return Ok(val.into());
        }
        // 允许使用特殊符号作为分隔符，必要时剥离成对引号
        let raw =
            winnow::token::take_while(1.., |c: char| !c.is_whitespace() && c != ')' && c != ',')
                .parse_next(data)?;
        let trimmed = raw.trim();
        let normalized = if trimmed.len() >= 2
            && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        Ok(normalized.into())
    }

    fn fun_name() -> &'static str {
        "split_to_src"
    }

    fn build(args: Self::ARG1) -> Self {
        SplitInnerSrcFunc::from_registry(args).expect("split_to_src processor not registered")
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

impl Fun0Builder for JsonUnescape {
    fn fun_name() -> &'static str {
        "json_unescape"
    }

    fn build() -> Self {
        JsonUnescape {}
    }
}
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

    use once_cell::sync::Lazy;
    use orion_error::TestAssert;
    use std::sync::Mutex;
    use wp_model_core::model::DataField;

    use crate::ast::processor::{Has, JsonUnescape, SelectLast, TakeField};
    use crate::parser::wpl_field::wpl_pipe;
    use crate::parser::wpl_group::wpl_group;
    use crate::traits::{
        FieldProcessor, FiledExtendType, clear_field_processors, register_field_processor,
    };

    use super::*;

    pub static REG_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct NoopProcessor(&'static str);

    impl FieldProcessor for NoopProcessor {
        fn name(&self) -> &'static str {
            self.0
        }

        fn process(&self, _field: Option<&mut DataField>) -> Result<(), String> {
            Ok(())
        }
    }

    fn parse_fun(input: &str) -> WplFun {
        wpl_fun.parse(input).assert()
    }

    #[test]
    fn parse_has_and_select_functions() {
        let fun = parse_fun("f_has(src)");
        assert_eq!(
            fun,
            WplFun::TargetHas(TargetHas {
                target: Some("src".into())
            })
        );

        assert_eq!(parse_fun("has()"), WplFun::Has(Has));

        assert_eq!(
            parse_fun("take(src)"),
            WplFun::SelectTake(TakeField {
                target: "src".into(),
            })
        );

        assert_eq!(parse_fun("last()"), WplFun::SelectLast(SelectLast {}));
    }

    #[test]
    fn parse_digit_functions() {
        assert_eq!(
            parse_fun(r#"f_digit_in(src, [1,2,3])"#),
            WplFun::TargetDigitIn(TargetDigitIn {
                target: Some("src".into()),
                value: vec![1, 2, 3]
            })
        );

        assert_eq!(
            parse_fun("digit_has(42)"),
            WplFun::DigitHas(DigitHas { value: 42 })
        );
        assert_eq!(
            parse_fun("digit_in([4,5])"),
            WplFun::DigitIn(DigitIn { value: vec![4, 5] })
        );
    }

    #[test]
    fn parse_ip_functions() {
        assert_eq!(
            parse_fun(r#"f_ip_in(src, [127.0.0.1, 127.0.0.2])"#),
            WplFun::TargetIpIn(TargetIpIn {
                target: Some("src".into()),
                value: vec![
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2))
                ]
            })
        );

        assert_eq!(
            parse_fun(r#"ip_in([127.0.0.1,::1])"#),
            WplFun::IpIn(IpIn {
                value: vec![
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    IpAddr::V6(Ipv6Addr::LOCALHOST),
                ],
            })
        );

        assert_eq!(
            parse_fun(r#"f_ip_in(src, [::1, 2001:db8::1])"#),
            WplFun::TargetIpIn(TargetIpIn {
                target: Some("src".into()),
                value: vec![
                    IpAddr::V6(Ipv6Addr::LOCALHOST),
                    IpAddr::V6("2001:db8::1".parse().unwrap()),
                ]
            })
        );
    }

    #[test]
    fn parse_transform_functions() {
        assert_eq!(
            parse_fun("json_unescape()"),
            WplFun::TransJsonUnescape(JsonUnescape {})
        );
        assert!(wpl_fun.parse("json_unescape(decoded)").is_err());

        assert_eq!(
            parse_fun("base64_decode()"),
            WplFun::TransBase64Decode(Base64Decode {})
        );
        assert!(wpl_fun.parse("base64_decode(decoded)").is_err());
    }

    #[test]
    fn parse_char_functions() {
        assert_eq!(
            parse_fun("f_chars_has(_, foo)"),
            WplFun::TargetCharsHas(TargetCharsHas {
                target: None,
                value: "foo".into(),
            })
        );

        assert_eq!(
            parse_fun("chars_has(bar)"),
            WplFun::CharsHas(CharsHas {
                value: "bar".into(),
            })
        );

        assert_eq!(
            parse_fun("chars_has(中文)"),
            WplFun::CharsHas(CharsHas {
                value: "中文".into(),
            })
        );

        assert_eq!(
            parse_fun("chars_not_has(baz)"),
            WplFun::CharsNotHas(CharsNotHas {
                value: "baz".into(),
            })
        );

        assert_eq!(
            parse_fun("chars_in([foo,bar])"),
            WplFun::CharsIn(CharsIn {
                value: vec!["foo".into(), "bar".into()],
            })
        );
    }

    #[test]
    fn parse_extension_functions() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::MemChannel, NoopProcessor("ext_proc"));
        register_field_processor(FiledExtendType::InnerSource, NoopProcessor("|"));

        assert!(matches!(
            parse_fun("to_ext_pass()"),
            WplFun::TransExtPass(_)
        ));
        assert!(matches!(parse_fun("vec_to_src()"), WplFun::VecToSrc(_)));

        match parse_fun("split_to_src(|)") {
            WplFun::SplitToSrc(func) => assert_eq!(func.separator(), "|"),
            other => panic!("unexpected fun: {:?}", other),
        }
        match parse_fun("split_to_src('|')") {
            WplFun::SplitToSrc(func) => assert_eq!(func.separator(), "|"),
            other => panic!("unexpected fun: {:?}", other),
        }

        clear_field_processors();
    }

    #[test]
    fn test_parse_fun_pipe_1() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::InnerSource, NoopProcessor("|"));
        let pipe_expect = wpl_pipe.parse(r#"| vec_to_src()"#).assert();
        let group = wpl_group
            .parse(r#"( json ( array/chars@logs | vec_to_src()) )"#)
            .assert();
        assert_eq!(group.fields.len(), 1);
        assert_eq!(
            group.fields[0]
                .clone()
                .sub_fields
                .assert()
                .get("logs")
                .assert()
                .pipe[0],
            pipe_expect.clone()
        );
        clear_field_processors();
    }
}
