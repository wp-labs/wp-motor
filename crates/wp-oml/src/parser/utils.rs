#[cfg(test)]
pub mod for_test {
    use std::fmt::Display;
    use winnow::{ModalResult, Parser};
    use wp_error::OMLCodeError;

    use crate::parser::error::OMLCodeErrorTait;
    pub type OmlExp<T> = fn(&mut &str) -> ModalResult<T>;

    pub fn assert_oml_parse<T: Display>(code: &mut &str, mut exp: OmlExp<T>) {
        let expect = code.to_string();
        match exp.parse_next(code) {
            Ok(o) => {
                let real = format!("{}", o);
                fmt_assert_eq(real.as_str(), expect.as_str());
                println!("{}", o);
            }
            Err(e) => {
                println!("{}", e);
                panic!("err!");
            }
        }
    }
    pub fn err_of_oml<T: Display>(code: &mut &str, mut exp: OmlExp<T>) -> OMLCodeError {
        match exp.parse_next(code) {
            Ok(_) => {
                panic!("unfound error!")
            }
            Err(e) => OMLCodeError::from_syntax(e, code, ""),
        }
    }

    pub fn assert_oml_parse_ext<T: Display>(code: &mut &str, mut exp: OmlExp<T>, expect: &str) {
        match exp.parse_next(code) {
            Ok(o) => {
                let real = format!("{}", o);
                fmt_assert_eq(real.as_str(), expect);
                println!("{}", o);
            }
            Err(e) => {
                println!("{}", e);
                panic!("err!");
            }
        }
    }

    pub fn fmt_assert_eq(real: &str, expect: &str) {
        let fmt_real = real.replace(" ", "").replace("\n", "").replace("\t", "");
        let fmt_expect = expect.replace(" ", "").replace("\n", "").replace("\t", "");
        assert_eq!(fmt_real, fmt_expect);
    }
}

/*
pub trait WNEBuildable<E> {
    fn to_wn_err(&self) -> ErrMode<ContextError>;
}
impl<E: Error + Display> WNEBuildable<E> for E {
    fn to_wn_err(&self) -> ErrMode<ContextError> {
        ErrMode::Backtrack(ParserError::from_error_kind(
            &format!("build: {}", self).as_str(),
            ErrorKind::Verify,
        ))
    }
}

*/
/*
pub fn err_from(error: ErrMode<ContextError>, input: &str, kind: CondErrKind) -> CondError {
    let message = error.to_string();
    let up = StreamParseError::current(input, CondErrKind::Syntax(message));
    let mut cur = StreamParseError::current(input, kind);
    cur.upstream(up);
    cur.bind_origin(input);
    cur
}

 */
