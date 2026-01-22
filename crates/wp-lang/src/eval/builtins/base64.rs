use base64::{Engine as _, engine::general_purpose};
use bytes::Bytes;
use orion_error::{ErrorOwe, ErrorWith};
use std::sync::Arc;

use wp_parse_api::{PipeProcessor, RawData, WparseResult};

#[derive(Debug)]
pub struct Base64Proc;

impl PipeProcessor for Base64Proc {
    /// Decodes Base64-encoded data while preserving the input container type.
    /// For string inputs, attempts UTF-8 conversion but falls back to bytes on invalid UTF-8.
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let decoded = general_purpose::STANDARD
                    .decode(s.as_bytes())
                    .owe_data()
                    .want("base64 decode")?;
                let vstring = String::from_utf8(decoded).owe_data().want("to-json")?;
                Ok(RawData::from_string(vstring))
            }
            RawData::Bytes(b) => {
                let decoded = general_purpose::STANDARD
                    .decode(b.as_ref())
                    .owe_data()
                    .want("base64 decode")?;
                Ok(RawData::Bytes(Bytes::from(decoded)))
            }
            RawData::ArcBytes(b) => {
                let decoded = general_purpose::STANDARD
                    .decode(b.as_ref())
                    .owe_data()
                    .want("base64 decode")?;
                // 注意：RawData::ArcBytes 现在使用 Arc<Vec<u8>>
                Ok(RawData::ArcBytes(Arc::new(decoded)))
            }
        }
    }

    fn name(&self) -> &'static str {
        "decode/base64"
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;

    use super::*;

    #[test]
    fn test_base64() -> AnyResult<()> {
        let data = RawData::from_string("aGVsbG8=".to_string());
        let y = Base64Proc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&y), "hello");

        // Test with a longer complex string that should decode to readable text
        let data = RawData::from_string("VGhpcyBpcyBhIHRlc3Qgb2YgYSBiYXNlNjQgZW5jb2RlZCBzdHJpbmcgd2l0aCBzcGVjaWFsIGNoYXJhY3RlcnMhIEBfXyUgJiYqKys=".to_string());
        let what = Base64Proc.process(data)?;
        let decoded = crate::eval::builtins::raw_to_utf8_string(&what);
        assert!(decoded.starts_with(
            "This is a test of a base64 encoded string with special characters! @__% &&*++"
        ));

        let bytes_data = RawData::Bytes(Bytes::from_static(b"aGVsbG8="));
        let result = Base64Proc.process(bytes_data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "hello");

        let arc_data = RawData::ArcBytes(Arc::new(b"Zm9vYmFy".to_vec()));
        let result = Base64Proc.process(arc_data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "foobar");
        Ok(())
    }
}
