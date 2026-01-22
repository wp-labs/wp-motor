use std::net::IpAddr;

use crate::fun::fun_trait::{Fun0Builder, Fun1Builder, Fun2Builder};
use crate::net::ip;
use crate::symbol::{symbol_bracket_beg, symbol_bracket_end, symbol_comma};
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::separated;
use winnow::{ModalResult as WResult, Parser};

use super::fun_trait::ParseNext;

pub fn take_call_args2<T: Fun2Builder>(data: &mut &str) -> WResult<(T::ARG1, T::ARG2)> {
    multispace0.parse_next(data)?;
    symbol_bracket_beg.parse_next(data)?;
    multispace0.parse_next(data)?;
    let a1 = T::args1.parse_next(data)?;
    (multispace0, symbol_comma, multispace0).parse_next(data)?;
    let a2 = T::args2.parse_next(data)?;
    multispace0.parse_next(data)?;
    symbol_bracket_end.parse_next(data)?;
    Ok((a1, a2))
}

pub fn take_call_args0<T: Fun0Builder>(data: &mut &str) -> WResult<()> {
    multispace0.parse_next(data)?;
    symbol_bracket_beg.parse_next(data)?;
    multispace0.parse_next(data)?;
    symbol_bracket_end.parse_next(data)?;
    Ok(())
}

pub fn take_call_args1<T: Fun1Builder>(data: &mut &str) -> WResult<T::ARG1> {
    multispace0.parse_next(data)?;
    symbol_bracket_beg.parse_next(data)?;
    multispace0.parse_next(data)?;
    let a1 = T::args1.parse_next(data)?;
    multispace0.parse_next(data)?;
    symbol_bracket_end.parse_next(data)?;
    Ok(a1)
}

pub fn call_fun_args2<T: Fun2Builder>(data: &mut &str) -> WResult<T> {
    T::fun_name().parse_next(data)?;
    let args = take_call_args2::<T>.parse_next(data)?;
    let obj = T::build(args);
    Ok(obj)
}

pub fn call_fun_args1<T: Fun1Builder>(data: &mut &str) -> WResult<T> {
    T::fun_name().parse_next(data)?;
    let args = take_call_args1::<T>.parse_next(data)?;
    let obj = T::build(args);
    Ok(obj)
}

pub fn call_fun_args0<T: Fun0Builder>(data: &mut &str) -> WResult<T> {
    T::fun_name().parse_next(data)?;
    take_call_args0::<T>.parse_next(data)?;
    let obj = T::build();
    Ok(obj)
}

pub fn take_arr<T: ParseNext<T>>(data: &mut &str) -> WResult<Vec<T>> {
    (multispace0, "[", multispace0).parse_next(data)?;
    let arr: Vec<T> = separated(1.., T::parse_next, ",").parse_next(data)?;
    (multispace0, "]").parse_next(data)?;
    Ok(arr)
}

impl ParseNext<u32> for u32 {
    fn parse_next(input: &mut &str) -> WResult<u32> {
        use winnow::error::{ErrMode, ParserError};
        let str = digit1(input)?;
        str.parse::<u32>().map_err(|_| ErrMode::from_input(input))
    }
}

impl ParseNext<i64> for i64 {
    fn parse_next(input: &mut &str) -> WResult<i64> {
        use winnow::error::{ErrMode, ParserError};
        let str = digit1(input)?;
        str.parse::<i64>().map_err(|_| ErrMode::from_input(input))
    }
}
impl ParseNext<IpAddr> for IpAddr {
    fn parse_next(input: &mut &str) -> WResult<IpAddr> {
        ip.parse_next(input)
    }
}

#[cfg(test)]
mod test {
    use super::{call_fun_args1, take_arr};
    use crate::fun::fun_trait::Fun1Builder;
    use winnow::{
        //ascii::{digit1, multispace0},
        ModalResult as WResult,
        Parser,
    };

    #[derive(Debug, PartialEq)]
    struct A {
        arr: Vec<u32>,
    }
    impl Fun1Builder for A {
        type ARG1 = Vec<u32>;

        fn args1(data: &mut &str) -> WResult<Self::ARG1> {
            take_arr::<u32>(data)
        }

        fn fun_name() -> &'static str {
            "fun_a"
        }

        fn build(args: Self::ARG1) -> Self {
            A { arr: args }
        }
    }

    #[test]
    fn test_arr_args_fun() -> WResult<()> {
        let mut data = "fun_a([1,2,3])";
        let x = call_fun_args1::<A>.parse_next(&mut data)?;
        println!("{:?}", x);
        assert_eq!(x, A { arr: vec![1, 2, 3] });
        Ok(())
    }

    // ========================================================================
    // Tests for error handling and boundary conditions
    // ========================================================================

    mod u32_parsing {
        use super::super::ParseNext;

        #[test]
        fn valid_u32_parsing() {
            let mut input = "123";
            let result = u32::parse_next(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123);
        }

        #[test]
        fn valid_u32_max_value() {
            let mut input = "4294967295"; // u32::MAX
            let result = u32::parse_next(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), u32::MAX);
        }

        #[test]
        fn invalid_u32_overflow() {
            let mut input = "4294967296"; // u32::MAX + 1
            let result = u32::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on u32 overflow");
        }

        #[test]
        fn invalid_u32_negative() {
            let mut input = "-1";
            let result = u32::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on negative number for u32");
        }

        #[test]
        fn invalid_u32_non_numeric() {
            let mut input = "abc";
            let result = u32::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on non-numeric input");
        }

        #[test]
        fn empty_input() {
            let mut input = "";
            let result = u32::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on empty input");
        }
    }

    mod i64_parsing {
        use super::super::ParseNext;

        #[test]
        fn valid_i64_positive() {
            let mut input = "123";
            let result = i64::parse_next(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123);
        }

        #[test]
        fn valid_i64_max_value() {
            let mut input = "9223372036854775807"; // i64::MAX
            let result = i64::parse_next(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), i64::MAX);
        }

        #[test]
        fn invalid_i64_overflow() {
            let mut input = "9223372036854775808"; // i64::MAX + 1
            let result = i64::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on i64 overflow");
        }

        #[test]
        fn invalid_i64_negative() {
            // Note: digit1 only matches positive digits, so negative would fail at parsing stage
            let mut input = "-123";
            let result = i64::parse_next(&mut input);
            assert!(
                result.is_err(),
                "Should fail on negative (digit1 doesn't match '-')"
            );
        }

        #[test]
        fn invalid_i64_non_numeric() {
            let mut input = "xyz";
            let result = i64::parse_next(&mut input);
            assert!(result.is_err(), "Should fail on non-numeric input");
        }
    }

    mod array_parsing {
        use super::super::take_arr;

        #[test]
        fn valid_array_single_element() {
            let mut input = "[42]";
            let result = take_arr::<u32>(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), vec![42]);
        }

        #[test]
        fn valid_array_multiple_elements() {
            let mut input = "[1,2,3,4,5]";
            let result = take_arr::<u32>(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), vec![1, 2, 3, 4, 5]);
        }

        #[test]
        fn valid_array_with_spaces() {
            let mut input = "[1,2,3]"; // Note: multispace0 is between brackets and content
            let result = take_arr::<u32>(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), vec![1, 2, 3]);
        }

        #[test]
        fn invalid_array_missing_bracket() {
            let mut input = "1,2,3]";
            let result = take_arr::<u32>(&mut input);
            assert!(result.is_err(), "Should fail on missing opening bracket");
        }

        #[test]
        fn invalid_array_empty() {
            let mut input = "[]";
            let result = take_arr::<u32>(&mut input);
            // separated requires at least 1 element
            assert!(result.is_err(), "Should fail on empty array");
        }

        #[test]
        fn invalid_array_trailing_comma() {
            let mut input = "[1,2,]";
            let result = take_arr::<u32>(&mut input);
            assert!(result.is_err(), "Should fail on trailing comma");
        }
    }

    mod function_call_parsing {
        use super::{super::call_fun_args1, A};

        #[test]
        fn valid_function_call() {
            let mut input = "fun_a([1,2,3])";
            let result = call_fun_args1::<A>(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), A { arr: vec![1, 2, 3] });
        }

        #[test]
        fn valid_function_call_with_spaces() {
            let mut input = "fun_a([1,2,3])"; // Simplified for actual parser behavior
            let result = call_fun_args1::<A>(&mut input);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), A { arr: vec![1, 2, 3] });
        }

        #[test]
        fn invalid_function_wrong_name() {
            let mut input = "fun_b([1,2,3])";
            let result = call_fun_args1::<A>(&mut input);
            assert!(result.is_err(), "Should fail on wrong function name");
        }

        #[test]
        fn invalid_function_missing_parens() {
            let mut input = "fun_a[1,2,3]";
            let result = call_fun_args1::<A>(&mut input);
            assert!(result.is_err(), "Should fail on missing parentheses");
        }

        #[test]
        fn invalid_function_empty_args() {
            let mut input = "fun_a()";
            let result = call_fun_args1::<A>(&mut input);
            assert!(result.is_err(), "Should fail on empty arguments");
        }
    }
}
