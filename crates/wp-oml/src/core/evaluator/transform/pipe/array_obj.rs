use crate::core::prelude::*;
use crate::language::{Get, Nth, SkipEmpty};
use std::collections::VecDeque;
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, FieldStorage, Value};

/// 数组索引访问 - nth(index)
impl ValueProcessor for Nth {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Array(arr) => {
                if let Some(found) = arr.get(self.index) {
                    return found.as_field().clone();
                }
                in_val
            }
            _ => in_val,
        }
    }
}

/// 跳过空值 - skip_empty
impl ValueProcessor for SkipEmpty {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Array(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Digit(x) => {
                if x.eq(&0) {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Float(x) => {
                if x.eq(&0.0) {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Chars(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            Value::Obj(x) => {
                if x.is_empty() {
                    return DataField::from_ignore(in_val.get_name());
                }
            }
            _ => {}
        }
        in_val
    }
}

/// 对象字段访问 - get(path)
impl ValueProcessor for Get {
    fn value_cacu(&self, mut in_val: DataField) -> DataField {
        if let Value::Obj(obj) = in_val.get_value_mut() {
            let mut keys: VecDeque<&str> = self.name.split('/').collect();
            while let Some(key) = keys.pop_front() {
                if let Some(val) = obj.get(key) {
                    if !keys.is_empty() {
                        if let Value::Obj(o) = val.get_value() {
                            *obj = o.clone();
                        }
                    } else {
                        return val.as_field().clone();
                    }
                }
            }
        }
        in_val
    }

    fn value_cacu_storage(&self, in_val: FieldStorage) -> FieldStorage {
        // Zero-copy path: extract field from Shared object without cloning the object
        if in_val.is_shared() {
            let field = in_val.as_field();
            if let Value::Obj(obj) = field.get_value() {
                let keys: Vec<&str> = self.name.split('/').collect();
                if let Some(result) = get_from_obj(obj, &keys) {
                    return result.clone();
                }
            }
        }

        // Fallback: use default implementation
        let field = in_val.into_owned();
        let result = self.value_cacu(field);
        FieldStorage::from_owned(result)
    }
}

// Helper function to navigate nested objects
fn get_from_obj<'a>(mut obj: &'a ObjectValue, keys: &[&str]) -> Option<&'a FieldStorage> {
    for (i, key) in keys.iter().enumerate() {
        if let Some(val) = obj.get(key) {
            if i == keys.len() - 1 {
                return Some(val);
            } else if let Value::Obj(nested) = val.get_value() {
                obj = nested;
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_skip() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            DataField::from_digit("A1", 0),
            DataField::from_arr("A2", vec![]),
        ];
        let src = DataRecord::from(data.clone());

        let mut conf = r#"
        name : test
        ---
        X  =  collect take(keys: [A1, A2]) ;
        Y  =  pipe  read(A1) | skip_empty ;
        Z  =  pipe  read(A2) | skip_empty ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        let expect = DataField::from_arr("X".to_string(), data);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
        assert_eq!(
            target.field("Y").map(|s| s.as_field()),
            Some(DataField::from_ignore("Y")).as_ref()
        );
        assert_eq!(
            target.field("Z").map(|s| s.as_field()),
            Some(DataField::from_ignore("Z")).as_ref()
        );
    }

    #[test]
    fn test_pipe_obj_get() {
        let val = r#"{"id":0,"items":[{"meta":{"array":"obj"},"name":"current_process","value":{"Array":[{"meta":"obj","name":"obj","value":{"Obj":{"ctime":{"meta":"digit","name":"ctime","value":{"Digit":1676340214}},"desc":{"meta":"chars","name":"desc","value":{"Chars":""}},"md5":{"meta":"chars","name":"md5","value":{"Chars":"d4ed19a8acd9df02123f655fa1e8a8e7"}},"path":{"meta":"chars","name":"path","value":{"Chars":"c:\\\\users\\\\administrator\\\\desktop\\\\domaintool\\\\x64\\\\childproc\\\\test_le9mwv.exe"}},"sign":{"meta":"chars","name":"sign","value":{"Chars":""}},"size":{"meta":"digit","name":"size","value":{"Digit":189446}},"state":{"meta":"digit","name":"state","value":{"Digit":0}},"type":{"meta":"digit","name":"type","value":{"Digit":1}}}}}]}}]}"#;
        let src: DataRecord = serde_json::from_str(val).unwrap();
        let cache = &mut FieldQueryCache::default();

        let mut conf = r#"
        name : test
        ---
        Y  =  pipe read(current_process) | nth(0) | get(current_process/path) ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        assert_eq!(
            target.field("Y").map(|s| s.as_field()),
            Some(DataField::from_chars(
                "Y",
                r#"c:\\users\\administrator\\desktop\\domaintool\\x64\\childproc\\test_le9mwv.exe"#
            ))
            .as_ref()
        );
    }
}
