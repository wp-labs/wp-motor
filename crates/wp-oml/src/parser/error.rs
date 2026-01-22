use winnow::error::{ErrMode, Needed};
use wp_error::{
    parse_error::{OMLCodeError, OMLCodeReason},
    util::split_string,
};

use crate::winnow::error::ContextError;
pub trait OMLCodeErrorTait {
    fn from_syntax(e: ErrMode<ContextError>, code: &str, path: &str) -> Self;
}

impl OMLCodeErrorTait for OMLCodeError {
    fn from_syntax(e: ErrMode<ContextError>, code: &str, path: &str) -> Self {
        match e {
            ErrMode::Incomplete(Needed::Size(u)) => {
                OMLCodeError::from(OMLCodeReason::Syntax(format!("parsing require {u}")))
            }
            ErrMode::Incomplete(Needed::Unknown) => OMLCodeError::from(OMLCodeReason::Syntax(
                "parsing require more data".to_string(),
            )),
            ErrMode::Backtrack(e) => {
                let where_in = split_string(code);
                OMLCodeError::from(OMLCodeReason::Syntax(format!(
                    ":oml code parse fail!\n[path ]: '{}'\n[where]: '{}'\n[error]: {}",
                    path, where_in, e
                )))
            }
            ErrMode::Cut(e) => {
                let where_in = split_string(code);
                OMLCodeError::from(OMLCodeReason::Syntax(format!(
                    ":code parse fail\n[path ]: '{}'\n[where]: '{}'\n[error]: {}",
                    path, where_in, e
                )))
            }
        }
    }
}
