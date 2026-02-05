use std::sync::Arc;

use wp_parse_api::{PipeProcessor, RawData, WparseResult};

/// BOM (Byte Order Mark) 清除处理器
///
/// 移除数据开头的 BOM 标记，支持以下编码的 BOM：
/// - UTF-8 BOM: 0xEF 0xBB 0xBF
/// - UTF-16 LE BOM: 0xFF 0xFE
/// - UTF-16 BE BOM: 0xFE 0xFF
/// - UTF-32 LE BOM: 0xFF 0xFE 0x00 0x00
/// - UTF-32 BE BOM: 0x00 0x00 0xFE 0xFF
///
/// # 行为
/// - 检测并移除开头的 BOM 字节序列
/// - 如果没有 BOM，返回原始数据
/// - 保持输入容器类型不变
#[derive(Debug)]
pub struct BomClearProc;

/// 检测并移除 BOM 标记
///
/// # 返回
/// - `Some(n)` - BOM 长度，应该跳过前 n 个字节
/// - `None` - 无 BOM
fn detect_bom(data: &[u8]) -> Option<usize> {
    // UTF-8 BOM: EF BB BF
    if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        return Some(3);
    }

    // UTF-32 LE BOM: FF FE 00 00 (必须在 UTF-16 LE 之前检查)
    if data.len() >= 4 && data[0] == 0xFF && data[1] == 0xFE && data[2] == 0x00 && data[3] == 0x00 {
        return Some(4);
    }

    // UTF-32 BE BOM: 00 00 FE FF (必须在 UTF-16 BE 之前检查)
    if data.len() >= 4 && data[0] == 0x00 && data[1] == 0x00 && data[2] == 0xFE && data[3] == 0xFF {
        return Some(4);
    }

    // UTF-16 LE BOM: FF FE
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xFE {
        return Some(2);
    }

    // UTF-16 BE BOM: FE FF
    if data.len() >= 2 && data[0] == 0xFE && data[1] == 0xFF {
        return Some(2);
    }

    None
}

impl PipeProcessor for BomClearProc {
    /// 清除数据开头的 BOM 标记
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let bytes = s.as_bytes();
                if let Some(bom_len) = detect_bom(bytes) {
                    // 移除 BOM
                    let without_bom = &bytes[bom_len..];
                    // 转换回字符串（应该总是有效的 UTF-8）
                    let result = String::from_utf8_lossy(without_bom).into_owned();
                    Ok(RawData::from_string(result))
                } else {
                    // 无 BOM，返回原始数据
                    Ok(RawData::from_string(s))
                }
            }
            RawData::Bytes(b) => {
                if let Some(bom_len) = detect_bom(&b) {
                    // 移除 BOM
                    let without_bom = b.slice(bom_len..);
                    Ok(RawData::Bytes(without_bom))
                } else {
                    // 无 BOM，返回原始数据
                    Ok(RawData::Bytes(b))
                }
            }
            RawData::ArcBytes(b) => {
                if let Some(bom_len) = detect_bom(&b) {
                    // 移除 BOM
                    let without_bom = b[bom_len..].to_vec();
                    Ok(RawData::ArcBytes(Arc::new(without_bom)))
                } else {
                    // 无 BOM，返回原始数据
                    Ok(RawData::ArcBytes(b))
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "strip/bom"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnyResult;

    #[test]
    fn test_detect_utf8_bom() {
        let data = &[0xEF, 0xBB, 0xBF, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_bom(data), Some(3));
    }

    #[test]
    fn test_detect_utf16_le_bom() {
        let data = &[0xFF, 0xFE, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_bom(data), Some(2));
    }

    #[test]
    fn test_detect_utf16_be_bom() {
        let data = &[0xFE, 0xFF, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_bom(data), Some(2));
    }

    #[test]
    fn test_detect_utf32_le_bom() {
        let data = &[0xFF, 0xFE, 0x00, 0x00, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_bom(data), Some(4));
    }

    #[test]
    fn test_detect_utf32_be_bom() {
        let data = &[0x00, 0x00, 0xFE, 0xFF, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_bom(data), Some(4));
    }

    #[test]
    fn test_detect_no_bom() {
        let data = b"hello world";
        assert_eq!(detect_bom(data), None);
    }

    #[test]
    fn test_detect_bom_too_short() {
        // 数据太短，无法包含完整的 BOM
        let data = &[0xEF, 0xBB];
        assert_eq!(detect_bom(data), None);
    }

    #[test]
    fn test_bom_clear_utf8_string() -> AnyResult<()> {
        // UTF-8 BOM + "hello"
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice(b"hello");
        let data = RawData::from_string(String::from_utf8(input)?);

        let result = BomClearProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "hello");
        Ok(())
    }

    #[test]
    fn test_bom_clear_utf16_le_bytes() -> AnyResult<()> {
        // UTF-16 LE BOM + "hello"
        let mut input = vec![0xFF, 0xFE];
        input.extend_from_slice(b"hello");
        let data = RawData::Bytes(Bytes::from(input));

        let result = BomClearProc.process(data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "hello");
        Ok(())
    }

    #[test]
    fn test_bom_clear_utf16_be_bytes() -> AnyResult<()> {
        // UTF-16 BE BOM + "world"
        let mut input = vec![0xFE, 0xFF];
        input.extend_from_slice(b"world");
        let data = RawData::Bytes(Bytes::from(input));

        let result = BomClearProc.process(data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "world");
        Ok(())
    }

    #[test]
    fn test_bom_clear_utf32_le_arc_bytes() -> AnyResult<()> {
        // UTF-32 LE BOM + "test"
        let mut input = vec![0xFF, 0xFE, 0x00, 0x00];
        input.extend_from_slice(b"test");
        let data = RawData::ArcBytes(Arc::new(input));

        let result = BomClearProc.process(data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "test");
        Ok(())
    }

    #[test]
    fn test_bom_clear_utf32_be_arc_bytes() -> AnyResult<()> {
        // UTF-32 BE BOM + "data"
        let mut input = vec![0x00, 0x00, 0xFE, 0xFF];
        input.extend_from_slice(b"data");
        let data = RawData::ArcBytes(Arc::new(input));

        let result = BomClearProc.process(data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "data");
        Ok(())
    }

    #[test]
    fn test_bom_clear_no_bom_string() -> AnyResult<()> {
        let data = RawData::from_string("hello world".to_string());
        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "hello world"
        );
        Ok(())
    }

    #[test]
    fn test_bom_clear_no_bom_bytes() -> AnyResult<()> {
        let data = RawData::Bytes(Bytes::from_static(b"no bom here"));
        let result = BomClearProc.process(data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "no bom here"
        );
        Ok(())
    }

    #[test]
    fn test_bom_clear_empty_string() -> AnyResult<()> {
        let data = RawData::from_string("".to_string());
        let result = BomClearProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "");
        Ok(())
    }

    #[test]
    fn test_bom_clear_only_bom() -> AnyResult<()> {
        // 只有 BOM，没有其他数据
        let input = vec![0xEF, 0xBB, 0xBF];
        let data = RawData::from_string(String::from_utf8(input)?);
        let result = BomClearProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "");
        Ok(())
    }

    #[test]
    fn test_bom_clear_chinese_with_utf8_bom() -> AnyResult<()> {
        // UTF-8 BOM + 中文
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice("你好世界".as_bytes());
        let data = RawData::from_string(String::from_utf8(input)?);

        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "你好世界"
        );
        Ok(())
    }

    #[test]
    fn test_bom_clear_preserves_container_type() -> AnyResult<()> {
        // 验证容器类型保持一致

        // String -> String
        let str_data = RawData::from_string("\u{FEFF}test".to_string());
        let str_result = BomClearProc.process(str_data)?;
        assert!(matches!(str_result, RawData::String(_)));

        // Bytes -> Bytes
        let bytes_data = RawData::Bytes(Bytes::from_static(&[0xEF, 0xBB, 0xBF, b't']));
        let bytes_result = BomClearProc.process(bytes_data)?;
        assert!(matches!(bytes_result, RawData::Bytes(_)));

        // ArcBytes -> ArcBytes
        let arc_data = RawData::ArcBytes(Arc::new(vec![0xEF, 0xBB, 0xBF, b't']));
        let arc_result = BomClearProc.process(arc_data)?;
        assert!(matches!(arc_result, RawData::ArcBytes(_)));

        Ok(())
    }
}
