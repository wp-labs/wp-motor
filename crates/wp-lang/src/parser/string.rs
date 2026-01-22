//! String parsing helpers for WPL: quoted string, raw string, simple quoted and escapes.

use winnow::ascii::take_escaped;
use winnow::token::{literal, none_of, one_of};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

/// Parse a quoted string content with common escapes: \" \\ \n \t \r \xHH.
pub fn parse_quoted_string<'a>(input: &mut &'a str) -> WResult<&'a str> {
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

/// Parse a raw string r#"..."# or r"..." (compat) without processing escapes.
pub fn parse_raw_string<'a>(input: &mut &'a str) -> WResult<&'a str> {
    let s = *input;
    if let Some(rest) = s.strip_prefix("r\"")
        && let Some(pos) = rest.find('"')
    {
        let content = &rest[..pos];
        *input = &rest[pos + 1..];
        return Ok(content);
    }
    if let Some(rest) = s.strip_prefix("r#\"")
        && let Some(pos) = rest.find("\"#")
    {
        let content = &rest[..pos];
        *input = &rest[pos + 2..];
        return Ok(content);
    }
    // Backtrack: let other branches try
    Err(winnow::error::ErrMode::Backtrack(
        winnow::error::ContextError::new(),
    ))
}

/// Parse a simple quoted string without escapes (read until next ").
pub fn parse_simple_quoted<'a>(input: &mut &'a str) -> WResult<&'a str> {
    literal('"').parse_next(input)?;
    let content = winnow::token::take_until(0.., "\"").parse_next(input)?;
    literal('"').parse_next(input)?;
    Ok(content)
}

/// Decode common escapes in a string (\" \\ \n \t \r \xHH) into UTF-8 string.
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
