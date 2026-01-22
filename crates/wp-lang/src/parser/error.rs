use crate::winnow::error::{ContextError, ParseError, StrContext};
use derive_more::From;
use orion_error::StructError;
use orion_error::{ErrorCode, UvsReason};
use thiserror::Error;
use winnow::error::{ErrMode, Needed};
use wp_error::util::split_string;
// use wp_error::DataErrKind; // kept for potential future conversions

fn translate_position(input: &[u8], index: usize) -> (usize, usize) {
    if input.is_empty() {
        return (0, index);
    }

    let safe_index = index.min(input.len() - 1);
    let column_offset = index - safe_index;
    let index = safe_index;

    let nl = input[0..index]
        .iter()
        .rev()
        .enumerate()
        .find(|(_, b)| **b == b'\n')
        .map(|(nl, _)| index - nl - 1);
    let line_start = match nl {
        Some(nl) => nl + 1,
        None => 0,
    };
    let line = input[0..line_start].iter().filter(|b| **b == b'\n').count();

    let column = std::str::from_utf8(&input[line_start..=index])
        .map(|s| s.chars().count() - 1)
        .unwrap_or_else(|_| index - line_start);
    let column = column + column_offset;

    (line, column)
}

pub fn error_detail(error: ParseError<&str, ContextError<StrContext>>) -> String {
    let offset = error.offset();
    let original = *error.input();
    let span = if offset == original.len() {
        offset..offset
    } else {
        offset..(offset + 1)
    };

    let mut msg = String::new();
    let (line, column) = translate_position(original.as_bytes(), span.start);
    let line_num = line + 1;
    let col_num = column + 1;
    let gutter = line_num.to_string().len();
    let content = original.split('\n').nth(line).expect("valid line number");

    msg.push_str(&format!(
        "parse error at line {}, column {}\n",
        line_num, col_num
    ));
    //   |
    for _ in 0..=gutter {
        msg.push(' ');
    }
    msg.push_str("|\n");

    // 1 | 00:32:00.a999999
    msg.push_str(&format!("{} | ", line_num));
    msg.push_str(&format!("{}\n", content));
    for _ in 0..=gutter {
        msg.push(' ');
    }
    msg.push('|');
    for _ in 0..=column {
        msg.push(' ');
    }
    // The span will be empty at eof, so we need to make sure we always print at least
    // one `^`
    msg.push('^');
    for _ in (span.start + 1)..(span.end.min(span.start + content.len())) {
        msg.push('^');
    }
    msg.push('\n');
    msg.push('\n');
    msg.push_str(&error.inner().to_string());
    msg
}

#[derive(Error, Debug, PartialEq, Serialize, From)]
pub enum WplCodeReason {
    #[from(skip)]
    #[error("plugin error >{0}")]
    Plugin(String),
    #[error("syntax error >{0}")]
    Syntax(String),
    #[from(skip)]
    #[error("wpl is empty >{0}")]
    Empty(String),
    #[from(skip)]
    #[error("unsupport > {0}")]
    UnSupport(String),
    #[error("{0}")]
    Uvs(UvsReason),
}
impl ErrorCode for WplCodeReason {
    fn error_code(&self) -> i32 {
        500
    }
}

pub type WplCodeError = StructError<WplCodeReason>;

pub type WplCodeResult<T> = Result<T, WplCodeError>;

pub trait WPLCodeErrorTrait {
    fn from_syntax(e: ErrMode<ContextError>, code: &str, path: &str) -> Self;
}
impl WPLCodeErrorTrait for StructError<WplCodeReason> {
    fn from_syntax(e: ErrMode<ContextError>, code: &str, path: &str) -> Self {
        match e {
            ErrMode::Incomplete(Needed::Size(u)) => {
                StructError::from(WplCodeReason::Syntax(format!("parsing require {u}")))
            }
            ErrMode::Incomplete(Needed::Unknown) => StructError::from(WplCodeReason::Syntax(
                "parsing require more data".to_string(),
            )),
            ErrMode::Backtrack(e) => {
                let where_in = split_string(code);
                StructError::from(WplCodeReason::Syntax(format!(
                    ":wpl code parse fail!\n[path ]: '{}'\n[where]: '{}'\n[error]: {}",
                    path, where_in, e
                )))
            }
            ErrMode::Cut(e) => {
                let where_in = split_string(code);
                StructError::from(WplCodeReason::Syntax(format!(
                    ":code parse fail\n[path ]: '{}'\n[where]: '{}'\n[error]: {}",
                    path, where_in, e
                )))
            }
        }
    }
}

/*
impl From<DataErrKind> for StructError<WplCodeReason> {
    fn from(value: DataErrKind) -> Self {
        WplCodeReason::from_data(format!("{}", value), None).to_err()
    }
}
*/
