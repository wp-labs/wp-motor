//! String parsing helpers for WPL: quoted string, raw string, simple quoted and escapes.

use wp_parser::WResult;

use crate::parser::utils::{self, quot_r_str, window_path};

/// Parse a quoted string content with common escapes: \" \\ \n \t \r \xHH.
pub fn parse_quoted_string<'a>(input: &mut &'a str) -> WResult<&'a str> {
    utils::duble_quot_str_impl(input)
}

/// Parse a raw string r#"..."# or r"..." (compat) without processing escapes.
pub fn parse_raw_string<'a>(input: &mut &'a str) -> WResult<&'a str> {
    quot_r_str(input)
}

/// Parse a simple quoted string without escapes (read until next ").
pub fn parse_simple_quoted<'a>(input: &mut &'a str) -> WResult<&'a str> {
    window_path(input)
}

/// Decode common escapes in a string (\" \\ \n \t \r \xHH) into UTF-8 string.
pub use utils::decode_escapes;
