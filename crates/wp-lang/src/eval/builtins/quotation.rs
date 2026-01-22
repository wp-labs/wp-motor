use bytes::Bytes;
use orion_error::{ErrorOwe, ErrorWith};
use std::sync::Arc;
use wp_parse_api::{PipeProcessor, RawData, WparseResult};

#[derive(Debug)]
pub struct EscQuotaProc;

fn unescape_bytes(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len());
    let mut escaped = false;
    for &b in input {
        if !escaped && b == b'"' {
            continue;
        }
        if !escaped && b == b'\\' {
            escaped = true;
            continue;
        }
        escaped = false;
        out.push(b);
    }
    out
}

impl PipeProcessor for EscQuotaProc {
    /// Unquotes and unescapes string data while preserving the input container type.
    /// Removes surrounding quotes and processes escape sequences.
    /// For string inputs, attempts UTF-8 conversion but falls back to bytes on invalid UTF-8.
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let unescaped_bytes = unescape_bytes(s.as_bytes());
                let vstring = String::from_utf8(unescaped_bytes)
                    .owe_data()
                    .want("to-json")?;
                Ok(RawData::from_string(vstring))
            }
            RawData::Bytes(b) => {
                let unescaped_bytes = unescape_bytes(&b);
                Ok(RawData::Bytes(Bytes::from(unescaped_bytes)))
            }
            RawData::ArcBytes(b) => {
                let unescaped_bytes = unescape_bytes(&b);
                Ok(RawData::ArcBytes(Arc::from(unescaped_bytes)))
            }
        }
    }

    fn name(&self) -> &'static str {
        "unquote/unescape"
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;
    use bytes::Bytes;
    use std::sync::Arc;

    use super::*;
    use wp_parse_api::RawData;

    #[test]
    fn test_quotation() -> AnyResult<()> {
        let data = RawData::from_string(r#""hello""#.to_string());
        let x = EscQuotaProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&x), "hello");

        let data = RawData::from_string(r#""<14>""#.to_string());
        let x = EscQuotaProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&x), "<14>");

        let data = RawData::from_string(r#""{ \"a\" = 1, \"b\" = \"wparse\" }""#.to_string());
        let x = EscQuotaProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&x),
            r#"{ "a" = 1, "b" = "wparse" }"#
        );

        let data = RawData::from_string(r#""{ \"a\" = 1, \"b\" = \" 中国 \" }""#.to_string());
        let x = EscQuotaProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&x),
            r#"{ "a" = 1, "b" = " 中国 " }"#
        );

        // Test with Bytes input
        let bytes_data = RawData::Bytes(Bytes::from_static(br#""hello world""#));
        let result = EscQuotaProc.process(bytes_data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "hello world"
        );

        // Test with ArcBytes input
        let arc_data = RawData::ArcBytes(Arc::from(
            r#""test with \"quotes\" and \backslash""#.as_bytes().to_vec(),
        ));
        let result = EscQuotaProc.process(arc_data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            r#"test with "quotes" and backslash"#
        );

        Ok(())
    }
}
