use crate::scope::ScopeEval;
use crate::symbol::ctx_desc;
use std::fmt::Display;
use winnow::ModalResult as WResult;
use winnow::Parser;
use winnow::ascii::multispace0;
use winnow::combinator::{fail, peek};
use winnow::error::{AddContext, ContextError};
use winnow::stream::{Checkpoint, Stream};
use winnow::token::take;

pub fn get_scope<'a>(data: &mut &'a str, beg: char, end: char) -> WResult<&'a str> {
    use winnow::token::{any, take};

    multispace0.parse_next(data)?;
    let extend_len = ScopeEval::len(data, beg, end);
    if extend_len < 2 {
        return fail.context(ctx_desc("scope len <2 ")).parse_next(data);
    }

    // Use any() to parse a single char instead of converting to string
    any.verify(|&c| c == beg).parse_next(data)?;
    let group = take(extend_len - 2).parse_next(data)?;
    any.verify(|&c| c == end).parse_next(data)?;
    multispace0(data)?;
    Ok(group)
}

pub fn peek_one<'a>(data: &mut &'a str) -> WResult<&'a str> {
    peek(take(1usize)).parse_next(data)
}

pub trait RestAble {
    fn err_reset<'a>(self, data: &mut &'a str, point: &Checkpoint<&'a str, &'a str>) -> Self;
}

impl<T, E> RestAble for Result<T, E> {
    fn err_reset<'a>(self, data: &mut &'a str, point: &Checkpoint<&'a str, &'a str>) -> Self {
        if self.is_err() {
            data.reset(point);
        }
        self
    }
}

pub fn err_convert<T, E: Display>(result: Result<T, E>, msg: &'static str) -> WResult<T> {
    match result {
        Ok(obj) => Ok(obj),
        Err(_e) => fail.context(ctx_desc(msg)).parse_next(&mut ""),
    }
}

pub fn context_error(
    input: &str,
    start: &Checkpoint<&str, &str>,
    desc: &'static str,
) -> ContextError {
    let context = ContextError::default();
    context.add_context(&input, start, ctx_desc(desc))
}
