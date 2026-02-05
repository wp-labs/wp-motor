use std::fmt::Debug;
use std::sync::{Arc, Once};

use wp_parse_api::{PipeHold, RawData};

pub mod base64;
pub mod bom;
pub mod hex;
mod pipe_fun;
pub mod quotation;
pub mod registry;

use base64::Base64Proc;
use bom::BomClearProc;
use hex::HexProc;
use quotation::EscQuotaProc;

#[derive(Serialize, Deserialize, Debug)]
pub struct PipeLineResult {
    pub name: String,
    pub result: String,
}

pub fn raw_to_utf8_string(data: &RawData) -> String {
    match data {
        RawData::String(s) => s.clone(),
        RawData::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        RawData::ArcBytes(b) => String::from_utf8_lossy(b).into_owned(),
    }
}

static BUILTIN_PIPE_INIT: Once = Once::new();

fn decode_base64_stage() -> PipeHold {
    Arc::new(Base64Proc)
}

fn decode_hex_stage() -> PipeHold {
    Arc::new(HexProc)
}

fn unquote_unescape_stage() -> PipeHold {
    Arc::new(EscQuotaProc)
}

fn bom_strip_stage() -> PipeHold {
    Arc::new(BomClearProc)
}

/// Ensure core decode/unquote pipe units are registered in the plg_pipe registry.
pub fn ensure_builtin_pipe_units() {
    BUILTIN_PIPE_INIT.call_once(|| {
        registry::register_pipe_unit("decode/base64", decode_base64_stage);
        registry::register_pipe_unit("decode/hex", decode_hex_stage);
        registry::register_pipe_unit("unquote/unescape", unquote_unescape_stage);
        registry::register_pipe_unit("strip/bom", bom_strip_stage);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_pipe_units_registered() {
        ensure_builtin_pipe_units();

        // 验证所有内置处理器都已注册
        let units = registry::list_pipe_units();

        assert!(units.iter().any(|name| name.to_uppercase() == "DECODE/BASE64"));
        assert!(units.iter().any(|name| name.to_uppercase() == "DECODE/HEX"));
        assert!(units.iter().any(|name| name.to_uppercase() == "UNQUOTE/UNESCAPE"));
        assert!(units.iter().any(|name| name.to_uppercase() == "STRIP/BOM"));
    }

    #[test]
    fn test_strip_bom_can_be_created() {
        ensure_builtin_pipe_units();

        // 验证可以创建 strip/bom 处理器
        let processor = registry::create_pipe_unit("strip/bom");
        assert!(processor.is_some());

        // 验证处理器名称
        if let Some(proc) = processor {
            assert_eq!(proc.name(), "strip/bom");
        }
    }
}
