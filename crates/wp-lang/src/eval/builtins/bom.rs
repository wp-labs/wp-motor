use std::sync::Arc;

use wp_parse_api::{PipeProcessor, RawData, WparseResult};

/// BOM (Byte Order Mark) 清除处理器
///
/// 移除数据中所有位置的 BOM 标记，支持以下编码的 BOM：
/// - UTF-8 BOM: 0xEF 0xBB 0xBF
/// - UTF-16 LE BOM: 0xFF 0xFE
/// - UTF-16 BE BOM: 0xFE 0xFF
/// - UTF-32 LE BOM: 0xFF 0xFE 0x00 0x00
/// - UTF-32 BE BOM: 0x00 0x00 0xFE 0xFF
///
/// # 行为
/// - 扫描整个数据，移除所有位置出现的 BOM 字节序列
/// - 如果没有 BOM，返回原始数据
/// - 保持输入容器类型不变
#[derive(Debug)]
pub struct BomClearProc;

/// 检测指定位置是否为 BOM 标记
///
/// # 返回
/// - `Some(n)` - BOM 长度，应该跳过后续 n 个字节
/// - `None` - 当前位置无 BOM
fn detect_bom_at(data: &[u8], pos: usize) -> Option<usize> {
    let remaining = &data[pos..];

    // UTF-8 BOM: EF BB BF
    if remaining.len() >= 3 && remaining[0] == 0xEF && remaining[1] == 0xBB && remaining[2] == 0xBF
    {
        return Some(3);
    }

    // UTF-32 LE BOM: FF FE 00 00 (必须在 UTF-16 LE 之前检查)
    if remaining.len() >= 4
        && remaining[0] == 0xFF
        && remaining[1] == 0xFE
        && remaining[2] == 0x00
        && remaining[3] == 0x00
    {
        return Some(4);
    }

    // UTF-32 BE BOM: 00 00 FE FF (必须在 UTF-16 BE 之前检查)
    if remaining.len() >= 4
        && remaining[0] == 0x00
        && remaining[1] == 0x00
        && remaining[2] == 0xFE
        && remaining[3] == 0xFF
    {
        return Some(4);
    }

    // UTF-16 LE BOM: FF FE
    if remaining.len() >= 2 && remaining[0] == 0xFF && remaining[1] == 0xFE {
        return Some(2);
    }

    // UTF-16 BE BOM: FE FF
    if remaining.len() >= 2 && remaining[0] == 0xFE && remaining[1] == 0xFF {
        return Some(2);
    }

    None
}

/// 移除字节数据中所有位置的 BOM
fn remove_all_boms(data: &[u8]) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut has_bom = false;
    let mut pos = 0;

    while pos < data.len() {
        if let Some(bom_len) = detect_bom_at(data, pos) {
            // 发现 BOM，跳过
            has_bom = true;
            pos += bom_len;
        } else {
            // 非 BOM 字节，添加到结果
            result.push(data[pos]);
            pos += 1;
        }
    }

    if has_bom { Some(result) } else { None }
}

impl PipeProcessor for BomClearProc {
    /// 清除数据中所有位置的 BOM 标记
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let bytes = s.as_bytes();
                if let Some(cleaned) = remove_all_boms(bytes) {
                    // 转换回字符串（应该总是有效的 UTF-8）
                    let result = String::from_utf8_lossy(&cleaned).into_owned();
                    Ok(RawData::from_string(result))
                } else {
                    // 无 BOM，返回原始数据
                    Ok(RawData::from_string(s))
                }
            }
            RawData::Bytes(b) => {
                if let Some(cleaned) = remove_all_boms(&b) {
                    Ok(RawData::Bytes(cleaned.into()))
                } else {
                    // 无 BOM，返回原始数据
                    Ok(RawData::Bytes(b))
                }
            }
            RawData::ArcBytes(b) => {
                if let Some(cleaned) = remove_all_boms(&b) {
                    Ok(RawData::ArcBytes(Arc::new(cleaned)))
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
    use bytes::Bytes;

    use super::*;
    use crate::types::AnyResult;

    /// 检测数据开头是否为 BOM（用于测试）
    fn detect_bom(data: &[u8]) -> Option<usize> {
        detect_bom_at(data, 0)
    }

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

    #[test]
    fn test_bom_in_middle_of_data() -> AnyResult<()> {
        // 测试数据中间出现的 BOM
        let mut input = b"hello".to_vec();
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
        input.extend_from_slice(b"world");
        let data = RawData::Bytes(Bytes::from(input));

        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "helloworld"
        );
        Ok(())
    }

    #[test]
    fn test_multiple_boms_in_data() -> AnyResult<()> {
        // 测试数据中出现多个 BOM
        let mut input = vec![0xEF, 0xBB, 0xBF]; // 开头 UTF-8 BOM
        input.extend_from_slice(b"start");
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // 中间 UTF-8 BOM
        input.extend_from_slice(b"middle");
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // 末尾 UTF-8 BOM
        input.extend_from_slice(b"end");
        let data = RawData::from_string(String::from_utf8(input)?);

        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "startmiddleend"
        );
        Ok(())
    }

    #[test]
    fn test_mixed_bom_types() -> AnyResult<()> {
        // 测试混合不同类型的 BOM
        let mut input = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        input.extend_from_slice(b"utf8");
        input.extend_from_slice(&[0xFF, 0xFE]); // UTF-16 LE BOM
        input.extend_from_slice(b"utf16");
        input.extend_from_slice(&[0xFE, 0xFF]); // UTF-16 BE BOM
        input.extend_from_slice(b"data");
        let data = RawData::Bytes(Bytes::from(input));

        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "utf8utf16data"
        );
        Ok(())
    }

    #[test]
    fn test_bom_at_end() -> AnyResult<()> {
        // 测试末尾的 BOM
        let mut input = b"data".to_vec();
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
        let data = RawData::Bytes(Bytes::from(input));

        let result = BomClearProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "data");
        Ok(())
    }

    #[test]
    fn test_consecutive_boms() -> AnyResult<()> {
        // 测试连续的 BOM
        let mut input = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // 又一个 UTF-8 BOM
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // 再一个 UTF-8 BOM
        input.extend_from_slice(b"text");
        let data = RawData::ArcBytes(Arc::new(input));

        let result = BomClearProc.process(data)?;
        assert_eq!(crate::eval::builtins::raw_to_utf8_string(&result), "text");
        Ok(())
    }

    #[test]
    fn test_bom_removal_with_chinese() -> AnyResult<()> {
        // 测试中文数据中的 BOM
        let mut input = b"start".to_vec();
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
        input.extend_from_slice("中文".as_bytes());
        input.extend_from_slice(&[0xEF, 0xBB, 0xBF]); // UTF-8 BOM
        input.extend_from_slice("内容".as_bytes());
        let data = RawData::from_string(String::from_utf8(input)?);

        let result = BomClearProc.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "start中文内容"
        );
        Ok(())
    }
}
