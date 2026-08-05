use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use crate::language::{IntranetIp, Ip4ToInt, IpToBigUint, ip_to_biguint};
use std::net::{IpAddr, Ipv4Addr};
use wp_model_core::model::{DataField, DataType, Value};

use wp_knowledge::intranet_nets::is_intranet;

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

/// 统一 IP 编码：IPv4/IPv6 → 任意精度 BigUint，字段类型为 BigInt
fn ip_biguint_field(name: &str, ip: IpAddr) -> DataField {
    DataField::new(
        DataType::BigInt,
        name.to_string(),
        Value::BigUint(ip_to_biguint(ip)),
    )
}

/// 输入不是 IP 类型：明确报错（诊断 + 日志），输出 null 使下游查询跳过
fn ip_type_error(field: &DataField, detail: String) -> DataField {
    let msg = format!("ip_to_biguint: {} field={}", detail, field.get_name());
    warn_data!("{}", msg);
    diagnostics::push(OmlIssue::new(OmlIssueKind::ParseFail, msg));
    null_field_like(field)
}

impl ValueProcessor for IpToBigUint {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::IpAddr(ip) => ip_biguint_field(in_val.get_name(), *ip),
            // 无输入（缺失/空）：保持 null，不报错、不查询
            Value::Null | Value::Ignore(_) => null_field_like(&in_val),
            _ => ip_type_error(
                &in_val,
                format!("expect ip input, got {}", in_val.get_value().tag()),
            ),
        }
    }
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

/// 判断 IP 是否内网地址（支持 IPv4/IPv6，网段由 intranet_nets 配置驱动）
/// 返回 `内`/`外` 中文字符串；空/非法输入返回 Ignore（该富化无法判断，不参与下游）
impl ValueProcessor for IntranetIp {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::IpAddr(ip) => {
                let side = if is_intranet(ip) { "LAN" } else { "WAN" };
                DataField::from_chars(in_val.get_name().to_string(), side)
            }
            Value::Chars(value) => {
                let value = value.trim();
                if value.is_empty() {
                    return DataField::from_ignore(in_val.get_name().to_string());
                }
                value
                    .parse::<IpAddr>()
                    .map(|ip| {
                        let side = if is_intranet(&ip) { "LAN" } else { "WAN" };
                        DataField::from_chars(in_val.get_name().to_string(), side)
                    })
                    .unwrap_or_else(|_| DataField::from_ignore(in_val.get_name().to_string()))
            }
            Value::Null | Value::Ignore(_) => DataField::from_ignore(in_val.get_name().to_string()),
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
    use std::str::FromStr;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_private_ipv4() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "LAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_public_ipv4() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "WAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_ipv6_ula() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::from_str("fc00::1").unwrap(),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "LAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_ipv6_public() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::from_str("2001:4860:4860::8888").unwrap(),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "WAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_chars_input() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip",
            "192.168.1.100",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "LAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_empty_string_becomes_ignore() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let out = target.field("X").expect("X field").as_field();
        assert!(
            matches!(out.get_value(), Value::Ignore(_)),
            "空字符串应输出 Ignore, got {:?}",
            out.get_value()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_on_fail_unknown() {
        let cache = &mut FieldQueryCache::default();
        // 空字符串 → intranet_ip 输出 Ignore → on_fail 替换为 "unknown"
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip | on_fail('unknown') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "unknown");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_on_fail_empty_string() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip | on_fail('') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_intranet_ip_on_fail_keeps_success_value() {
        let cache = &mut FieldQueryCache::default();
        // 公网 IP → "WAN"，on_fail 不干预成功结果
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | intranet_ip | on_fail('unknown') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "WAN");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip_to_biguint_ipv4() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip_to_biguint ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let field = target.field("X").expect("X produced").as_field();
        assert_eq!(field.get_value().to_string(), "134744072");
        assert!(matches!(field.get_value(), Value::BigUint(_)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip_to_biguint_ipv6() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            IpAddr::from_str("2001:4860:4860::8888").unwrap(),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip_to_biguint ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let field = target.field("X").expect("X produced").as_field();
        assert_eq!(
            field.get_value().to_string(),
            "382824323044708348099391746388336347272"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip_to_biguint_ipv6_compressed_equals_full() {
        let cache = &mut FieldQueryCache::default();
        let v6_compressed = IpAddr::from_str("2001:4860:4860::8888").unwrap();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip",
            v6_compressed,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip_to_biguint ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let compressed = target.field("X").expect("X produced").as_field();

        let v6_full = IpAddr::from_str("2001:4860:4860:0:0:0:0:8888").unwrap();
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "src_ip", v6_full,
        ))];
        let src = DataRecord::from(data);
        let target = model.transform_async(src, cache).await;
        let full = target.field("X").expect("X produced").as_field();

        assert_eq!(compressed.get_value(), full.get_value());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_ip_to_biguint_non_ip_input_becomes_null() {
        // 非 IP 类型输入（Chars）→ 报错并输出 null（下游查询跳过）
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip",
            "not-an-ip",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip_to_biguint ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        assert!(target.field("X").is_none());
    }
}
