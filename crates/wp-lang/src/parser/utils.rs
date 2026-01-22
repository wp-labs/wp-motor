use smol_str::SmolStr;
use winnow::ascii::{multispace0, take_escaped};
use winnow::combinator::{alt, delimited, fail, opt, peek, preceded, separated_pair};
use winnow::error::{ContextError, ErrMode};
use winnow::stream::Stream;
use winnow::token::{literal, none_of, one_of, take, take_until, take_while};
use wp_model_core::model::Value;
use wp_parser::Parser;
use wp_parser::WResult;

use wp_parser::symbol::ctx_desc;

//#[allow(clippy::nonminimal_bool)]
pub fn take_ref_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanumeric()
            || c == '_'
            || c == '/'
            || c == '-'
            || c == '.'
            || c == '['
            || c == ']'
            || c == '*'
    })
    .parse_next(input)
}
pub fn take_exact_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanumeric() || c == '_' || c == '/' || c == '-' || c == '.'
    })
    .parse_next(input)
}

pub fn take_key<'a>(input: &mut &'a str) -> WResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanumeric() || c == '_' || c == '/' || c == '-' || c == '.'
    })
    .parse_next(input)
}

pub fn take_var_name<'a>(input: &mut &'a str) -> WResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanumeric() || c == '_' || c == '.' || c == '-'
    })
    .parse_next(input)
}

pub fn take_fun_name<'a>(input: &mut &'a str) -> WResult<&'a str> {
    //trace("var_name", move |input: &mut &'a str| {
    take_while(1.., |c: char| c.is_alphanumeric() || c == '_' || c == '.').parse_next(input)
    //})
    //.parse_next(input)
}

pub fn take_meta_name<'a>(input: &mut &'a str) -> WResult<&'a str> {
    //trace("keyword", move |input: &mut &'a str| {
    take_while(1.., |c: char| c.is_alphanumeric() || c == '_' || c == '/').parse_next(input)
    //})
    //.parse_next(input)
}

pub fn take_sql_tval(input: &mut &str) -> WResult<Value> {
    let chars = opt(alt((
        delimited('"', take_until(0.., "\""), '"'),
        delimited('\'', take_until(0.., "'"), '\''),
    )))
    .parse_next(input)?;
    if let Some(chars) = chars {
        return Ok(Value::Chars(chars.into()));
    }
    if let Some(value) = opt(take_while(0.., ('0'..='9', '.', '-', '+'))).parse_next(input)? {
        if let Ok(digit) = value.parse::<i64>() {
            return Ok(Value::Digit(digit));
        } else {
            return Ok(Value::Float(value.parse::<f64>().unwrap_or(0.0)));
        }
    }

    //fail get value;
    "fail-value".parse_next(input)?;
    Ok(Value::Chars("fail-value".into()))
}

#[inline]
pub fn quot_str<'a>(input: &mut &'a str) -> WResult<&'a str> {
    alt((
        duble_quot_str_impl.context(ctx_desc(
            "<quoted_string>::= '\"' , <character_sequence> , '\"' ",
        )),
        single_quot_str_impl.context(ctx_desc(
            "<quoted_string>::= '\"' , <character_sequence> , '\"' ",
        )),
    ))
    .parse_next(input)
}
#[inline]
pub fn interval_data<'a>(input: &mut &'a str) -> WResult<&'a str> {
    interval_impl
        .context(ctx_desc("extract bracketed segments: (), [], {}, <>"))
        .parse_next(input)
}

// 不要匹配 ‘\’ 和 ‘“’
// 引号字符串：允许任意非引号/反斜杠字符，转义支持 \" \\ \n \t \r \xHH
#[inline]
pub fn duble_quot_str_impl<'a>(input: &mut &'a str) -> WResult<&'a str> {
    literal('"')
        .context(ctx_desc("<beg>\""))
        .parse_next(input)?;
    let content = take_escaped(
        none_of(['\\', '"']),
        '\\',
        one_of(('0'..='9', ['"', 'n', 't', 'r', '\\', 'x'])),
    )
    .parse_next(input)?;
    literal('"')
        .context(ctx_desc("<end>\""))
        .parse_next(input)?;
    Ok(content)
}
#[inline]
pub fn single_quot_str_impl<'a>(input: &mut &'a str) -> WResult<&'a str> {
    literal('\'')
        .context(ctx_desc("<beg>'"))
        .parse_next(input)?;
    let content = take_escaped(
        none_of(['\\', '\'']),
        '\\',
        one_of(('0'..='9', ['"', '\'', 'n', 't', 'r', '\\', 'x'])),
    )
    .parse_next(input)?;
    literal('\'')
        .context(ctx_desc("<end>'"))
        .parse_next(input)?;
    Ok(content)
}

#[inline]
pub fn interval_impl<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let s = *input;
    let mut chars = s.char_indices();
    let Some((_, first)) = chars.next() else {
        return fail
            .context(ctx_desc("interval requires leading bracket"))
            .parse_next(input);
    };

    let Some(first_close) = closing_for_bracket(first) else {
        return fail
            .context(ctx_desc("interval must start with [ ( { <"))
            .parse_next(input);
    };
    let mut stack: Vec<char> = vec![first_close];
    let mut iter = chars.peekable();

    while let Some((idx, ch)) = iter.next() {
        if ch == '\\' {
            // skip escaped character outside of quoted sections
            let _ = iter.next();
            continue;
        }
        match ch {
            '[' | '(' | '{' | '<' => {
                if let Some(close) = closing_for_bracket(ch) {
                    stack.push(close);
                }
            }
            ']' | ')' | '}' | '>' => {
                let expected = stack.pop().unwrap();
                if ch != expected {
                    return fail
                        .context(ctx_desc("interval bracket mismatch"))
                        .parse_next(input);
                }
                if stack.is_empty() {
                    let end = idx + ch.len_utf8();
                    let (matched, rest) = s.split_at(end);
                    *input = rest;
                    return Ok(matched);
                }
            }
            '"' | '\'' => {
                let quote = ch;
                let mut escaped = false;
                for (_, qc) in iter.by_ref() {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if qc == '\\' {
                        escaped = true;
                        continue;
                    }
                    if qc == quote {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fail.context(ctx_desc("interval missing closing bracket"))
        .parse_next(input)
}

fn closing_for_bracket(ch: char) -> Option<char> {
    match ch {
        '[' => Some(']'),
        '(' => Some(')'),
        '{' => Some('}'),
        '<' => Some('>'),
        _ => None,
    }
}

pub fn window_path<'a>(input: &mut &'a str) -> WResult<&'a str> {
    literal('"').parse_next(input)?;
    let content = take_until(0.., "\"").parse_next(input)?;
    literal('"').parse_next(input)?;
    Ok(content)
}

/// 原始字符串（首选）：r#"..."#，内容不做转义处理；
/// 兼容旧写法：r"..."（仅为向后兼容，未来可能移除）。
pub fn quot_r_str<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let s = *input;
    // 优先解析 r#"..."#
    if let Some(rest) = s.strip_prefix("r#\"") {
        if let Some(pos) = rest.find("\"#") {
            let content = &rest[..pos];
            let new_rest = &rest[pos + 2..];
            *input = new_rest;
            return Ok(content);
        } else {
            return fail
                .context(ctx_desc("raw string not closed: r#\"...\"#"))
                .parse_next(input);
        }
    }
    // 回退兼容 r"..."
    if let Some(rest) = s.strip_prefix("r\"") {
        if let Some(pos) = rest.find('"') {
            let content = &rest[..pos];
            let new_rest = &rest[pos + 1..];
            *input = new_rest;
            return Ok(content);
        } else {
            return fail
                .context(ctx_desc("raw string not closed: r\"...\""))
                .parse_next(input);
        }
    }
    // 不匹配
    fail.parse_next(input)
}

pub fn quot_raw<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let cp = input.checkpoint();
    literal('"').parse_next(input)?;
    let content =
        take_escaped(none_of(['\\', '"']), '\\', one_of(['"', 'n', '\\'])).parse_next(input)?;
    literal('"').parse_next(input)?;
    let len = content.len() + 2;
    input.reset(&cp);
    let raw = take(len).parse_next(input)?;
    Ok(raw)
}

pub fn take_parentheses<'a>(input: &mut &'a str) -> WResult<&'a str> {
    literal('(').parse_next(input)?;
    let content = take_escaped(none_of(['\\', ')']), '\\', one_of([')'])).parse_next(input)?;
    literal(')').parse_next(input)?;
    Ok(content)
}

// #[tag(tag : "hello", raw_copy : "raw" ), copy_raw(name:"hello")]
pub fn decode_escapes(s: &str) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek().copied() {
                Some('"') => {
                    out.push(b'"');
                    it.next();
                }
                Some('\\') => {
                    out.push(b'\\');
                    it.next();
                }
                Some('n') => {
                    out.push(b'\n');
                    it.next();
                }
                Some('t') => {
                    out.push(b'\t');
                    it.next();
                }
                Some('r') => {
                    out.push(b'\r');
                    it.next();
                }
                Some('x') => {
                    it.next();
                    let h1 = it.next();
                    let h2 = it.next();
                    if let (Some(h1), Some(h2)) = (h1, h2) {
                        let hex = [h1, h2];
                        let val = hex
                            .iter()
                            .try_fold(0u8, |v, ch| ch.to_digit(16).map(|d| (v << 4) | (d as u8)));
                        if let Some(b) = val {
                            out.push(b);
                        } else {
                            out.extend_from_slice(b"\\x");
                            out.extend_from_slice(h1.to_string().as_bytes());
                            out.extend_from_slice(h2.to_string().as_bytes());
                        }
                    } else {
                        out.extend_from_slice(b"\\x");
                        if let Some(h1) = h1 {
                            out.extend_from_slice(h1.to_string().as_bytes());
                        }
                        if let Some(h2) = h2 {
                            out.extend_from_slice(h2.to_string().as_bytes());
                        }
                    }
                }
                Some(ch) => {
                    out.push(b'\\');
                    out.extend_from_slice(ch.to_string().as_bytes());
                    it.next();
                }
                None => {}
            }
        } else {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            out.extend_from_slice(s.as_bytes());
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn take_tag_kv(input: &mut &str) -> WResult<(SmolStr, SmolStr)> {
    // 值支持普通引号字符串与原始字符串；普通字符串会做一次反转义，原始字符串保持原样
    separated_pair(
        preceded(multispace0, take_key),
        (multispace0, ':', multispace0),
        alt((
            quot_r_str.map(|s: &str| SmolStr::from(s)),
            quot_str.map(|s: &str| SmolStr::from(decode_escapes(s))),
        )),
    )
    .map(|(k, v)| (SmolStr::from(k), v))
    .parse_next(input)
}

#[inline]
pub fn take_to_end<'a>(input: &mut &'a str) -> WResult<&'a str> {
    //trace("take_to_end", move |input: &mut &'a str| {
    take_while(0.., |_| true).parse_next(input)
    //})
    //.parse_next(input)
}

pub fn peek_str(what: &str, input: &mut &str) -> WResult<()> {
    // In winnow 0.7, `peek` over a string may produce `Result<_, ContextError>`.
    // Convert it into `ModalResult<()>` by wrapping the error in `ErrMode`.
    match peek(what).parse_next(input) {
        Ok(_) => Ok(()),
        Err(e) => Err(ErrMode::Backtrack(e)),
    }
}

pub fn peek_next<'a, O, ParseNext>(parser: ParseNext, input: &mut &'a str) -> WResult<()>
where
    ParseNext: Parser<&'a str, O, ContextError>,
{
    match peek(parser).parse_next(input) {
        Ok(_) => Ok(()),
        Err(e) => Err(ErrMode::Backtrack(e)),
    }
}
pub fn is_sep_next(input: &mut &str) -> bool {
    let _ = multispace0::<&str, ErrMode<ContextError>>.parse_next(input);
    if peek_str(",", input).is_ok() {
        let _: Result<&str, ErrMode<ContextError>> = literal(",").parse_next(input);
        return true;
    }
    false
}
pub fn is_next_unit(prefix: &str, input: &mut &str) -> bool {
    let _ = multispace0::<&str, ErrMode<ContextError>>.parse_next(input);
    if peek_str(prefix, input).is_ok() {
        return true;
    }
    false
}

pub fn is_next<'a, O, ParseNext>(parser: ParseNext, input: &mut &'a str) -> bool
where
    ParseNext: Parser<&'a str, O, ContextError>,
{
    let _ = multispace0::<&str, ErrMode<ContextError>>.parse_next(input);
    if peek_next(parser, input).is_ok() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::error::error_detail;
    use crate::parser::utils::{quot_str, take_key, take_parentheses, take_to_end};
    use crate::parser::wpl_pkg::wpl_package;
    use orion_error::TestAssert;
    use winnow::LocatingSlice;
    use wp_parser::WResult as ModalResult;

    #[test]
    fn test_take_val() -> ModalResult<()> {
        assert_eq!(
            Value::Chars("key".into()),
            take_sql_tval.parse_next(&mut "'key'")?
        );
        assert_eq!(Value::Digit(100), take_sql_tval.parse_next(&mut "100")?);
        assert_eq!(
            Value::Float(100.01),
            take_sql_tval.parse_next(&mut "100.01")?
        );
        assert_eq!(
            Value::Float(-100.01),
            take_sql_tval.parse_next(&mut "-100.01")?
        );
        Ok(())
    }

    #[test]
    fn test_key_ident() {
        assert_eq!(Ok(("", "key")), take_key.parse_peek("key"));
        assert_eq!(Ok(("!", "key")), take_key.parse_peek("key!"));
        assert_eq!(
            Ok(("!", "http/request")),
            take_key.parse_peek("http/request!")
        );
        assert_eq!(
            Ok(("!", "123http/request")),
            take_key.parse_peek("123http/request!")
        );
    }
    #[test]
    fn test_quot_str() {
        assert_eq!(quot_str.parse_peek("\"123\""), Ok(("", "123")));
        assert_eq!(quot_str.parse_peek("'123'"), Ok(("", "123")));
        assert_eq!(quot_str.parse_peek("\"1-?#ab\""), Ok(("", "1-?#ab")));
        assert_eq!(quot_str.parse_peek(r#""12\"3""#), Ok(("", r#"12\"3"#)));
        assert_eq!(quot_str.parse_peek(r#"'12\"3'"#), Ok(("", r#"12\"3"#)));
        // 支持 Unicode
        assert_eq!(quot_str.parse_peek("\"中文🙂\""), Ok(("", "中文🙂")));
        //assert_eq!(quot_str.parse_peek(r#""sddD:\招标项目\6-MSS\mss日志映射表""#),
        assert_eq!(
            window_path.parse_peek(r#""sddD:\招标项目\6-MSS\mss日志映射表""#),
            Ok(("", r#"sddD:\招标项目\6-MSS\mss日志映射表"#))
        );
    }
    #[test]
    fn test_quot_r_str() {
        use crate::parser::utils::quot_r_str;
        // r#"..."# 支持内部包含引号
        assert_eq!(
            quot_r_str.parse_peek("r#\"a\\b \"c\"\"#"),
            Ok(("", "a\\b \"c\""))
        );
        assert_eq!(quot_r_str.parse_peek("r#\"end\"#"), Ok(("", "end")));
        // 兼容旧写法 r"..."
        assert_eq!(quot_r_str.parse_peek("r\"raw\""), Ok(("", "raw")));
    }
    #[test]
    fn test_take_pat() {
        assert_eq!(take_parentheses.parse_peek("(123)"), Ok(("", "123")));
        assert_eq!(
            take_parentheses.parse_peek(r#"(12\)3)"#),
            Ok(("", r#"12\)3"#))
        );
    }

    #[test]
    fn test_take_to_end() {
        let input = "";
        let x = take_to_end.parse(input).assert();
        assert_eq!(x, "");

        let input = "hello 你好 😂 😁 π \u{3001} \n \t en";
        let x = take_to_end.parse(input).assert();
        assert_eq!(x, input);
    }

    #[test]
    fn test_prefix() {
        let data = "{ (digit, time,sn,chars,time,kv,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request,http/agent)}";
        if let Err(err) = crate::parser::parse_code::segment.parse(data) {
            println!("{}", error_detail(err));
        }
        assert_eq!(
            crate::parser::parse_code::segment
                .parse(data)
                .assert()
                .to_string(),
            r#"  (
    digit,
    time,
    sn,
    chars,
    time,
    kv,
    ip,
    kv,
    chars,
    kv,
    kv,
    chars,
    kv,
    kv,
    chars,
    chars,
    ip,
    chars,
    http/request,
    http/agent
  )"#
        );
    }
    #[test]
    fn test_meta() {
        let input = r#"    package test {
                rule test { (
                time,
                time_timestamp
                ) }
        }
    "#;

        assert_eq!(
            wpl_package
                .parse(&LocatingSlice::new(input))
                .assert()
                .to_string(),
            r#"package test {
  rule test {
    (
      time,
      time_timestamp
    )
  }
}
"#
        );
    }

    #[test]
    fn test_tag_kv_hex_escape() {
        use super::take_tag_kv;
        let mut s = "key:\"\\xE4\\xB8\\xAD\\xE6\\x96\\x87\"";
        let (k, v) = take_tag_kv.parse_next(&mut s).assert();
        assert_eq!(k, "key");
        assert_eq!(v, "中文");
    }

    #[test]
    fn test_interval_simple() {
        let mut input = "{payload}";
        let parsed = interval_data(&mut input).assert();
        assert_eq!(parsed, "{payload}");
        assert_eq!(input, "");
    }

    #[test]
    fn test_interval_nested_with_quotes() {
        let mut input = "<({\"[(foo)]\"}, ['x'])>tail";
        let parsed = interval_data(&mut input).assert();
        assert_eq!(parsed, "<({\"[(foo)]\"}, ['x'])>");
        assert_eq!(input, "tail");
    }

    #[test]
    fn test_interval_missing_closer() {
        let mut input = "[1,2";
        assert!(interval_data(&mut input).is_err());
    }
}
