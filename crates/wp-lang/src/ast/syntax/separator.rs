use std::fmt::Debug;
use std::rc::Rc;

use smol_str::SmolStr;

use crate::parser::utils::take_to_end;
use winnow::ascii::multispace0;
use winnow::combinator::preceded;
use winnow::combinator::{opt, trace};
use winnow::stream::{FindSlice, Stream};
use winnow::token::{literal, take_while};
use wp_parser::WResult as ModalResult;
use wp_parser::Parser;

#[derive(Debug)]
pub struct CharSep {
    sep_char: char,
    sep_str: SmolStr,
}

impl CharSep {
    #[allow(dead_code)]
    pub fn new(sep_char: char) -> Self {
        Self {
            sep_char,
            sep_str: SmolStr::from(sep_char.to_string()),
        }
    }
}

impl Separator for CharSep {
    fn sep_str(&self) -> &str {
        self.sep_str.as_str()
    }

    fn not_sep(&self, c: char) -> bool {
        c != self.sep_char
    }
    fn flag(&self) -> &str {
        self.sep_str.as_str()
    }

    fn get_field<'a>(&self, input: &mut &'a str) -> WResult<&'a str> {
        if self.sep_char == '0' {
            preceded(multispace0, take_to_end).parse_next(input)
        } else {
            preceded(multispace0, take_while(0.., |c| self.not_sep(c))).parse_next(input)
        }
    }
}

#[derive(Debug)]
pub struct StrSep {
    sep_str: SmolStr,
}

impl StrSep {
    #[allow(dead_code)]
    pub fn new<S: Into<SmolStr>>(sep_str: S) -> Self {
        let sep_str = sep_str.into();
        assert!(!sep_str.is_empty());
        Self { sep_str }
    }
    fn sep_tag(&self) -> &str {
        self.sep_str.as_str()
    }
}

impl Separator for StrSep {
    fn sep_str(&self) -> &str {
        self.sep_str.as_str()
    }

    fn not_sep(&self, c: char) -> bool {
        c.to_string() != self.sep_str
    }

    fn flag(&self) -> &str {
        self.sep_str.as_str()
    }

    fn get_field<'a>(&self, input: &mut &'a str) -> WResult<&'a str> {
        //trace(
        //format!("获取[{}]之前field", self.sep_str),
        //move |input: &mut &'a str|
        match input.find_slice(self.sep_tag()) {
            None => {
                let field = winnow::token::take_while(0.., |_| true).parse_next(input)?;
                Ok(field)
            }
            Some(index) => Ok(input.next_slice(index.start)),
        }
        //)
        //.parse_next(input)
    }
}

pub trait Separator: Debug {
    fn sep_str(&self) -> &str;
    fn not_sep(&self, c: char) -> bool;
    fn flag(&self) -> &str;

    fn tag<'a>(&self, input: &mut &'a str) -> WResult<Option<&'a str>> {
        let sep = self.sep_str();
        trace(format!("尝试忽略分隔符:[{}]", sep), opt(literal(sep))).parse_next(input)
    }
    fn get_field<'a>(&self, input: &mut &'a str) -> WResult<&'a str>;
}

pub type Hold<T> = Rc<T>;
pub type SeparatorHold = Hold<dyn Separator>;

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;

    use super::{CharSep, Separator, StrSep};

    #[test]
    fn test_sep_get_field() -> AnyResult<()> {
        let mut data = "kv=val;kv2=val2 ";
        assert_eq!(
            CharSep::new(',').get_field(&mut data),
            Ok("kv=val;kv2=val2 ")
        );

        let mut data = "kv=val;kv2=val2,";
        assert_eq!(
            CharSep::new(',').get_field(&mut data),
            Ok("kv=val;kv2=val2")
        );
        assert_eq!(data, ",");

        let mut data = "kv=val;kv2=val2, ";
        assert_eq!(
            CharSep::new(',').get_field(&mut data),
            Ok("kv=val;kv2=val2")
        );

        assert_eq!(data, ", ");

        let mut data = "kv=val;kv2=val2,xyz ";
        assert_eq!(
            CharSep::new('0').get_field(&mut data),
            Ok("kv=val;kv2=val2,xyz ")
        );

        let mut data = "kv=val;kv2=val2, xxx";
        assert_eq!(
            CharSep::new(',').get_field(&mut data),
            Ok("kv=val;kv2=val2")
        );
        assert_eq!(data, ", xxx");

        let mut data = "";
        assert_eq!(CharSep::new(',').get_field(&mut data), Ok(""));
        assert_eq!(data, "");
        Ok(())
    }

    #[test]
    fn test_sep_tag() -> AnyResult<()> {
        let mut data = "kv=val;kv2=val2 ";
        assert_eq!(CharSep::new(',').tag(&mut data), Ok(None));

        let mut data = "kv=val;kv2=val2,";
        assert_eq!(CharSep::new(',').tag(&mut data), Ok(None));

        let mut data = ",kv=val;kv2=val2,";
        assert_eq!(CharSep::new(',').tag(&mut data), Ok(Some(",")));

        let mut data = "kv=val;kv2=val2, ";
        assert_eq!(CharSep::new('k').tag(&mut data), Ok(Some("k")));

        let mut data = "kv=val;kv2=val2, ";
        assert_eq!(StrSep::new("kv").tag(&mut data), Ok(Some("kv")));
        Ok(())
    }

    #[test]
    fn test_diysep() -> AnyResult<()> {
        let mut data = "kv=val;kv2=val2, xxx";
        assert_eq!(
            StrSep::new(";".to_string()).get_field(&mut data),
            Ok("kv=val")
        );
        assert_eq!(data, ";kv2=val2, xxx");

        let mut data = "hello";
        assert_eq!(
            StrSep::new(";".to_string()).get_field(&mut data),
            Ok("hello")
        );
        assert_eq!(data, "");

        let mut data = "hello|!";
        assert_eq!(
            StrSep::new("|!".to_string()).get_field(&mut data),
            Ok("hello")
        );
        assert_eq!(data, "|!");

        let mut data = "hello|!hello|!|";
        assert_eq!(
            StrSep::new("|!|".to_string()).get_field(&mut data),
            Ok("hello|!hello")
        );
        assert_eq!(data, "|!|");
        Ok(())
    }
}
