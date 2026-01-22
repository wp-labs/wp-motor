use crate::core::prelude::*;
use crate::language::Ip4ToInt;
use wp_model_core::model::{DataField, Value};

impl ValueProcessor for Ip4ToInt {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::IpAddr(ip) => {
                if let std::net::IpAddr::V4(v4) = ip {
                    let as_u32 = u32::from(*v4) as i64;
                    return DataField::from_digit(in_val.get_name().to_string(), as_u32);
                }
                in_val
            }
            _ => in_val,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use std::net::{IpAddr, Ipv4Addr};
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_ip4_int() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        )];
        let src = DataRecord { items: data };

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip4_to_int ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        let expect = DataField::from_digit("X".to_string(), 2130706433);
        assert_eq!(target.field("X"), Some(&expect));
    }
}
