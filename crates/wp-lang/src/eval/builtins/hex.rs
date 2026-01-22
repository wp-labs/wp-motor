use bytes::Bytes;
use orion_error::{ErrorOwe, ErrorWith};
use std::sync::Arc;

use wp_parse_api::{PipeProcessor, RawData, WparseResult};

#[derive(Debug)]
pub struct HexProc;

impl PipeProcessor for HexProc {
    /// Decodes hex-encoded data while preserving the input container type.
    /// For string inputs, attempts UTF-8 conversion but falls back to bytes on invalid UTF-8.
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let decoded = hex::decode(s.as_bytes()).owe_data().want("hex decode")?;
                let vstring = String::from_utf8(decoded).owe_data().want("to-json")?;
                Ok(RawData::from_string(vstring))
            }
            RawData::Bytes(b) => {
                let decoded = hex::decode(b.as_ref()).owe_data().want("hex decode")?;
                Ok(RawData::Bytes(Bytes::from(decoded)))
            }
            RawData::ArcBytes(b) => {
                let decoded = hex::decode(b.as_ref()).owe_data().want("hex decode")?;
                // 注意：RawData::ArcBytes 现在使用 Arc<Vec<u8>>
                Ok(RawData::ArcBytes(Arc::new(decoded)))
            }
        }
    }

    fn name(&self) -> &'static str {
        "decode/hex"
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;
    use bytes::Bytes;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_hex() -> AnyResult<()> {
        let data = RawData::from_string("48656c6c6f20776f726c6421".to_string());
        let y = HexProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&y),
            "Hello world!"
        );

        // Test with a longer complex hex string
        let data = RawData::from_string("5468697320697320612074657374206f6620612068657820656e636f64656420737472696e672120405f5f252026262a2b2b".to_string());
        let what = HexProc.process(data)?;
        let decoded = crate::eval::builtins::raw_to_utf8_string(&what);
        assert!(decoded.starts_with("This is a test of a hex encoded string! @__% &&*++"));

        let bytes_data = RawData::Bytes(Bytes::from_static(b"48656c6c6f"));
        let result = HexProc.process(bytes_data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "Hello");

        let arc_data = RawData::ArcBytes(Arc::new(b"48656c6c6f20776f726c64".to_vec()));
        let result = HexProc.process(arc_data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "Hello world"
        );
        Ok(())
    }
}
