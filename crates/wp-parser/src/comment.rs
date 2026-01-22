use winnow::ascii::{multispace0, newline, till_line_ending};
use winnow::combinator::opt;
use winnow::token::literal;
use winnow::{ModalResult as WResult, Parser};

#[derive(Debug)]
enum DslStatus {
    Comment,
    Code,
}
pub struct CommentParser {}
impl Default for CommentParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentParser {
    pub fn new() -> Self {
        CommentParser {}
    }
    pub fn ignore_comment(input: &mut &str) -> WResult<String> {
        let mut status = DslStatus::Code;
        let mut out = String::new();
        while !input.is_empty() {
            match status {
                DslStatus::Code => {
                    multispace0.parse_next(input)?;
                    let long_comment: WResult<&str> = literal("/*").parse_next(input);
                    if long_comment.is_ok() {
                        status = DslStatus::Comment;
                        continue;
                    }
                    let short_comment: WResult<&str> = literal("//").parse_next(input);
                    if short_comment.is_ok() {
                        let _ = till_line_ending.parse_next(input)?;
                        continue;
                    }
                    let code = till_line_ending.parse_next(input)?;
                    out += code;
                    if opt(newline).parse_next(input)?.is_some() {
                        out += "\n";
                    }
                    continue;
                }

                DslStatus::Comment => {
                    multispace0.parse_next(input)?;
                    let is_end: WResult<&str> = literal("*/").parse_next(input);
                    if is_end.is_ok() {
                        status = DslStatus::Code;
                        continue;
                    }
                    let _ = till_line_ending.parse_next(input)?;
                    opt(newline).parse_next(input)?;
                }
            }
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment() {
        let mut code = r#"
        a=1;
         //b=1;
           /*
        c=1;
        d=1;
        */
        x=1;
        "#;

        let p_code = CommentParser::ignore_comment(&mut code).expect("ignore comment fail");
        println!("{}", p_code)
    }
}
