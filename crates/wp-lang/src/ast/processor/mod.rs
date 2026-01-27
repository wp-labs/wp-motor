mod function;
mod pipe;
pub(crate) use function::normalize_target;
pub use function::{
    Base64Decode, CharsHas, CharsIn, CharsInArg, CharsNotHas, CharsNotHasArg, CharsValue, DigitHas,
    DigitHasArg, DigitIn, DigitInArg, ExtPassFunc, Has, HasArg, IpIn, IpInArg, JsonUnescape,
    SelectLast, SplitInnerSrcFunc, TakeField, TargetCharsHas, TargetCharsIn, TargetCharsNotHas,
    TargetDigitHas, TargetDigitIn, TargetHas, TargetIpIn, VecToSrcFunc,
};
pub use pipe::WplFun;
pub use pipe::WplPipe;
