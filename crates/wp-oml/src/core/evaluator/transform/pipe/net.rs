use crate::core::prelude::*;
use crate::language::Ip4ToInt;
use std::net::{IpAddr, Ipv4Addr};
use wp_model_core::model::{DataField, DataType, Value};

fn null_field_like(field: &DataField) -> DataField {
    DataField::new(
        DataType::default(),
        field.get_name().to_string(),
        Value::Null,
    )
}

fn ipv4_to_field(name: &str, ip: Ipv4Addr) -> DataField {
    DataField::from_digit(name.to_string(), u32::from(ip) as i64)
}

impl ValueProcessor for Ip4ToInt {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::IpAddr(ip) => {
                if let IpAddr::V4(v4) = ip {
                    return ipv4_to_field(in_val.get_name(), *v4);
                }
                null_field_like(&in_val)
            }
            Value::Chars(value) => {
                let value = value.trim();
                if value.is_empty() {
                    return null_field_like(&in_val);
                }
                value
                    .parse::<Ipv4Addr>()
                    .map(|ip| ipv4_to_field(in_val.get_name(), ip))
                    .unwrap_or_else(|_| null_field_like(&in_val))
            }
            _ => in_val,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::dev::testing::TestAssert;
    use std::net::{IpAddr, Ipv4Addr};
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip4_int() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip4_to_int ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X".to_string(), 2130706433);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip4_int_converts_ipv4_string() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip",
            "10.18.190.27",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip4_to_int ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X".to_string(), 169000475);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip4_int_empty_string_becomes_null() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip4_to_int ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        assert!(target.field("X").is_none());
    }
}
