use crate::symbol::symbol_colon;
use winnow::ascii::{multispace0, multispace1};
use winnow::combinator::fail;
use winnow::error::{StrContext, StrContextValue};
use winnow::token::{literal, take_till, take_while};
use winnow::{ModalResult as WResult, Parser};

pub fn take_var_name<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.'])).parse_next(input)
}

pub fn take_json_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    take_while(
        1..,
        ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.', '/', '[', ']']),
    )
    .parse_next(input)
}

pub fn take_wild_key<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    take_while(
        1..,
        (
            '0'..='9',
            'A'..='Z',
            'a'..='z',
            ['_', '.', '*', '/', '[', ']'],
        ),
    )
    .parse_next(input)
}

pub fn take_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.', '/'], '-')).parse_next(input)
}

pub fn take_string<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    take_while(1.., |c: char| {
        c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')
    })
    .parse_next(input)
}

pub fn take_obj_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    let key = take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '/'])).parse_next(input)?;
    let _ = multispace1.parse_next(input)?;
    Ok(key)
}

pub fn take_obj_wild_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let _ = multispace0.parse_next(input)?;
    let key =
        take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '/', '*'])).parse_next(input)?;
    let _ = multispace1.parse_next(input)?;
    Ok(key)
}

pub fn take_key_pair<'a>(input: &mut &'a str) -> WResult<(&'a str, &'a str)> {
    let _ = multispace0.parse_next(input)?;
    let key = take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.'])).parse_next(input)?;
    symbol_colon.parse_next(input)?;
    let _ = multispace0.parse_next(input)?;
    let val = take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.'])).parse_next(input)?;
    Ok((key, val))
}

pub fn take_key_val<'a>(input: &mut &'a str) -> WResult<(&'a str, &'a str)> {
    let _ = multispace0.parse_next(input)?;
    let key = take_while(1.., ('0'..='9', 'A'..='Z', 'a'..='z', ['_', '.'])).parse_next(input)?;
    symbol_colon.parse_next(input)?;
    let _ = multispace0.parse_next(input)?;
    let val = take_till(1.., |c| c == ',' || c == ';').parse_next(input)?;
    Ok((key, val))
}
pub fn take_empty(input: &mut &str) -> WResult<()> {
    let _ = multispace0.parse_next(input)?;
    Ok(())
}

pub fn take_parentheses_val(data: &mut &str) -> WResult<String> {
    use crate::scope::ScopeEval;
    use winnow::token::take;

    let _ = multispace0.parse_next(data)?;

    // Calculate the length of the complete balanced parentheses scope
    let scope_len = ScopeEval::len(data, '(', ')');

    if scope_len < 2 {
        return fail
            .context(StrContext::Label("syntax"))
            .context(StrContext::Expected(StrContextValue::Description(
                "need match '(x)', lack '(' or unbalanced parentheses",
            )))
            .parse_next(data);
    }

    // Parse opening parenthesis
    literal("(")
        .context(StrContext::Label("syntax"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need match '(x)', lack '('",
        )))
        .parse_next(data)?;

    // Extract the content (excluding the outer parentheses)
    let content = take(scope_len - 2).parse_next(data)?;

    // Parse closing parenthesis
    literal(")")
        .context(StrContext::Label("syntax"))
        .context(StrContext::Expected(StrContextValue::Description(
            "need match '(x)', lack ')'",
        )))
        .parse_next(data)?;

    let trimmed = content.trim();

    // Check if the content is a quoted string
    let result = if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        // Remove quotes and process escape sequences
        if trimmed.len() >= 2 {
            let unquoted = &trimmed[1..trimmed.len() - 1];
            // Process basic escape sequences
            unquoted
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t")
                .replace("\\\\", "\\")
                .replace("\\'", "'")
                .replace("\\\"", "\"")
        } else {
            String::new()
        }
    } else {
        // No quotes, return as-is
        trimmed.to_string()
    };

    Ok(result)
}

pub fn take_parentheses_scope<'a>(data: &mut &'a str) -> WResult<(&'a str, &'a str)> {
    let _ = multispace0.parse_next(data)?;
    literal("(").parse_next(data)?;
    let beg = take_till(0.., |x| x == ',').parse_next(data)?;
    literal(",").parse_next(data)?;
    let _ = multispace0.parse_next(data)?;
    let end = take_till(0.., |x| x == ')').parse_next(data)?;
    literal(")").parse_next(data)?;
    Ok((beg, end))
}

#[cfg(test)]
mod tests {
    use crate::atom::{take_parentheses_val, take_var_name};
    use winnow::{ModalResult as WResult, Parser};

    mod var_name {
        use super::*;

        #[test]
        fn valid_simple_name() -> WResult<()> {
            let mut data = "x";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "x");
            Ok(())
        }

        #[test]
        fn valid_name_followed_by_parens() -> WResult<()> {
            let mut data = "x(10)";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "x");
            assert_eq!(data, "(10)");
            Ok(())
        }

        #[test]
        fn valid_alphanumeric() -> WResult<()> {
            let mut data = "x10(10)";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "x10");
            Ok(())
        }

        #[test]
        fn valid_with_leading_space() -> WResult<()> {
            let mut data = " x10 (10)";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "x10");
            Ok(())
        }

        #[test]
        fn valid_with_underscore() -> WResult<()> {
            let mut data = " x_1 (10)";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "x_1");
            Ok(())
        }

        #[test]
        fn valid_with_dot() -> WResult<()> {
            let mut data = "foo.bar.baz";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "foo.bar.baz");
            Ok(())
        }

        #[test]
        fn valid_complex_path() -> WResult<()> {
            let mut data = "user_data.profile.name_field";
            let key = take_var_name.parse_next(&mut data)?;
            assert_eq!(key, "user_data.profile.name_field");
            Ok(())
        }

        #[test]
        fn invalid_empty_input() {
            let mut data = "";
            let result = take_var_name.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on empty input");
        }

        #[test]
        fn invalid_starts_with_special() {
            let mut data = "@invalid";
            let result = take_var_name.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on special characters");
        }

        #[test]
        fn invalid_only_whitespace() {
            let mut data = "   ";
            let result = take_var_name.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on only whitespace");
        }
    }

    mod parentheses_val {
        use super::*;

        #[test]
        fn valid_simple_value() -> WResult<()> {
            let mut data = "(hello)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "hello");
            Ok(())
        }

        #[test]
        fn valid_nested_parens() -> WResult<()> {
            let mut data = "(outer(inner)value)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "outer(inner)value");
            Ok(())
        }

        #[test]
        fn valid_deeply_nested() -> WResult<()> {
            let mut data = "(a(b(c)d)e)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "a(b(c)d)e");
            Ok(())
        }

        #[test]
        fn valid_with_leading_space() -> WResult<()> {
            let mut data = "  (value)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "value");
            Ok(())
        }

        #[test]
        fn valid_trims_internal_space() -> WResult<()> {
            let mut data = "(  value  )";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "value");
            Ok(())
        }

        #[test]
        fn invalid_missing_open_paren() {
            let mut data = "value)";
            let result = take_parentheses_val.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on missing '('");
        }

        #[test]
        fn invalid_missing_close_paren() {
            let mut data = "(value";
            let result = take_parentheses_val.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on missing ')'");
        }

        #[test]
        fn invalid_unbalanced_nested() {
            let mut data = "(value"; // Simplified test case
            let result = take_parentheses_val.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on unbalanced parentheses");
        }

        #[test]
        fn invalid_empty_input() {
            let mut data = "";
            let result = take_parentheses_val.parse_next(&mut data);
            assert!(result.is_err(), "Should fail on empty input");
        }

        #[test]
        fn quoted_single_quotes() -> WResult<()> {
            let mut data = "('hello world')";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "hello world");
            Ok(())
        }

        #[test]
        fn quoted_double_quotes() -> WResult<()> {
            let mut data = r#"("hello world")"#;
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "hello world");
            Ok(())
        }

        #[test]
        fn quoted_with_escape_sequences() -> WResult<()> {
            let mut data = r#"('hello\nworld\ttab')"#;
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "hello\nworld\ttab");
            Ok(())
        }

        #[test]
        fn quoted_with_escaped_quotes() -> WResult<()> {
            let mut data = r#"('it\'s working')"#;
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "it's working");
            Ok(())
        }

        #[test]
        fn unquoted_backward_compatible() -> WResult<()> {
            let mut data = "(hello)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "hello");
            Ok(())
        }

        #[test]
        fn unquoted_with_special_chars() -> WResult<()> {
            let mut data = "(1.0.0)";
            let val = take_parentheses_val.parse_next(&mut data)?;
            assert_eq!(val, "1.0.0");
            Ok(())
        }
    }
}
