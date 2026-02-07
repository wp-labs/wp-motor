use crate::core::prelude::*;
use wp_model_core::model::{DataField, Value};

// ValueProcessor implementations for string matching and value mapping functions

impl ValueProcessor for crate::language::StartsWith {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(value) => {
                if value.starts_with(&self.prefix) {
                    // 匹配成功,返回原字段
                    in_val
                } else {
                    // 不匹配，转换为 ignore 类型
                    DataField::from_ignore(in_val.get_name())
                }
            }
            _ => {
                // 非字符串类型也转换为 ignore
                DataField::from_ignore(in_val.get_name())
            }
        }
    }
}

impl ValueProcessor for crate::language::MapTo {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        use crate::language::MapValue;

        // 检查字段是否为 ignore 类型
        if matches!(in_val.get_value(), Value::Ignore(_)) {
            // 如果是 ignore 类型，保持不变
            in_val
        } else {
            // 如果不是 ignore，根据参数类型创建对应的字段
            let name = in_val.get_name().to_string();
            match &self.value {
                MapValue::Chars(s) => DataField::from_chars(name, s.clone()),
                MapValue::Digit(d) => DataField::from_digit(name, *d),
                MapValue::Float(f) => DataField::from_float(name, *f),
                MapValue::Bool(b) => DataField::from_bool(name, *b),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_start_with() {
        // 测试匹配的情况
        let cache = &mut FieldQueryCache::default();
        let data = vec![DataField::from_chars("url", "https://example.com")];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe take(url) | starts_with('https://');
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        let expect = DataField::from_chars("X".to_string(), "https://example.com".to_string());
        assert_eq!(target.field("X"), Some(&expect));

        // 测试不匹配的情况 - 使用独立的 cache 和 model
        let cache2 = &mut FieldQueryCache::default();
        let data2 = vec![DataField::from_chars("url", "http://example.com")];
        let src2 = DataRecord::from(data2);

        let mut conf2 = r#"
        name : test
        ---
        X  =  pipe take(url) | starts_with('https://');
         "#;
        let model2 = oml_parse_raw(&mut conf2).assert();
        let target2 = model2.transform(src2, cache2);

        // 不匹配时应该返回 ignore 字段
        assert_eq!(
            target2.field("X"),
            Some(DataField::from_ignore("X")).as_ref()
        );
    }

    #[test]
    fn test_pipe_map_to() {
        let cache = &mut FieldQueryCache::default();

        // 测试映射到字符串
        let data = vec![DataField::from_chars("status", "200")];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        A  =  pipe take(status) | map_to('success');
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        let expect = DataField::from_chars("A".to_string(), "success".to_string());
        assert_eq!(target.field("A"), Some(&expect));

        // 测试映射到整数
        let cache2 = &mut FieldQueryCache::default();
        let data2 = vec![DataField::from_chars("level", "ERROR")];
        let src2 = DataRecord::from(data2);

        let mut conf2 = r#"
        name : test
        ---
        B  =  pipe take(level) | map_to(1);
         "#;
        let model2 = oml_parse_raw(&mut conf2).assert();
        let target2 = model2.transform(src2, cache2);

        let expect2 = DataField::from_digit("B".to_string(), 1);
        assert_eq!(target2.field("B"), Some(&expect2));

        // 测试映射到浮点数
        let cache3 = &mut FieldQueryCache::default();
        let data3 = vec![DataField::from_chars("temp", "high")];
        let src3 = DataRecord::from(data3);

        let mut conf3 = r#"
        name : test
        ---
        C  =  pipe take(temp) | map_to(36.5);
         "#;
        let model3 = oml_parse_raw(&mut conf3).assert();
        let target3 = model3.transform(src3, cache3);

        let expect3 = DataField::from_float("C".to_string(), 36.5);
        assert_eq!(target3.field("C"), Some(&expect3));

        // 测试映射到布尔值
        let cache4 = &mut FieldQueryCache::default();
        let data4 = vec![DataField::from_chars("flag", "yes")];
        let src4 = DataRecord::from(data4);

        let mut conf4 = r#"
        name : test
        ---
        D  =  pipe take(flag) | map_to(true);
         "#;
        let model4 = oml_parse_raw(&mut conf4).assert();
        let target4 = model4.transform(src4, cache4);

        let expect4 = DataField::from_bool("D".to_string(), true);
        assert_eq!(target4.field("D"), Some(&expect4));

        // 测试 ignore 字段保持不变
        let cache5 = &mut FieldQueryCache::default();
        let data5 = vec![DataField::from_chars("url", "http://example.com")];
        let src5 = DataRecord::from(data5);

        let mut conf5 = r#"
        name : test
        ---
        E  =  pipe take(url) | starts_with('https://') | map_to('secure');
         "#;
        let model5 = oml_parse_raw(&mut conf5).assert();
        let target5 = model5.transform(src5, cache5);

        // 字段为 ignore 时，应该保持 ignore
        assert_eq!(
            target5.field("E"),
            Some(DataField::from_ignore("E")).as_ref()
        );
    }
}
