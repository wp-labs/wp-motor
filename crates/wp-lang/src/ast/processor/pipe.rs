use derive_getters::Getters;
use smol_str::SmolStr;

use super::function::{
    CharsHas, CharsIn, CharsNotHas, DigitHas, DigitIn, Has, IpIn, JsonUnescape, SelectLast,
    TakeField, TargetCharsHas, TargetCharsIn, TargetCharsNotHas, TargetDigitHas, TargetDigitIn,
    TargetHas, TargetIpIn,
};
use crate::ast::{group::WplGroup, processor::Base64Decode};

#[derive(Debug, Clone, PartialEq)]
pub enum WplFun {
    SelectTake(TakeField),
    SelectLast(SelectLast),
    // Character comparison functions
    TargetCharsHas(TargetCharsHas),
    CharsHas(CharsHas),
    TargetCharsNotHas(TargetCharsNotHas),
    CharsNotHas(CharsNotHas),
    TargetCharsIn(TargetCharsIn),
    CharsIn(CharsIn),
    // Numeric comparison functions
    TargetDigitHas(TargetDigitHas),
    DigitHas(DigitHas),
    TargetDigitIn(TargetDigitIn),
    DigitIn(DigitIn),
    // IP address comparison
    TargetIpIn(TargetIpIn),
    IpIn(IpIn),
    // Field existence check
    TargetHas(TargetHas),
    Has(Has),
    // Transformation functions
    TransJsonUnescape(JsonUnescape),
    TransBase64Decode(Base64Decode),
}

#[derive(Debug, Clone, PartialEq, Getters)]
#[allow(dead_code)]
pub struct FunArg0 {
    name: SmolStr,
}
impl<S> From<S> for FunArg0
where
    S: Into<SmolStr>,
{
    fn from(value: S) -> Self {
        Self { name: value.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Getters)]
#[allow(dead_code)]
pub struct FunArg1 {
    name: SmolStr,
    arg1: SmolStr,
}

impl<S> From<(S, S)> for FunArg1
where
    S: Into<SmolStr>,
{
    fn from(value: (S, S)) -> Self {
        Self {
            name: value.0.into(),
            arg1: value.1.into(),
        }
    }
}

impl<S> From<(S, Option<S>)> for FunArg1
where
    S: Into<SmolStr>,
{
    fn from(value: (S, Option<S>)) -> Self {
        Self {
            name: value.0.into(),
            arg1: value.1.map(|f| f.into()).unwrap_or(SmolStr::from("_")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Getters)]
#[allow(dead_code)]
pub struct FunArg2 {
    name: SmolStr,
    arg1: SmolStr,
    arg2: SmolStr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WplPipe {
    Fun(WplFun),
    Group(WplGroup),
}

impl<S> From<(S, S, S)> for FunArg2
where
    S: Into<SmolStr>,
{
    fn from(value: (S, S, S)) -> Self {
        Self {
            name: value.0.into(),
            arg1: value.1.into(),
            arg2: value.2.into(),
        }
    }
}
