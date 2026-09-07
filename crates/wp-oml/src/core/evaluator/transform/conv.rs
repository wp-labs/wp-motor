use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::language::EvaluationTarget;
use std::net::IpAddr;
use wp_data_fmt::{Raw, ValueFormatter};
use wp_model_core::model::{DataField, DataType, Value};

pub fn omlobj_meta_conv(ori: DataField, target: &EvaluationTarget) -> DataField {
    if target.data_type() == ori.get_meta() {
        return ori;
    }
    let raw = Raw;
    //to auto & chars
    match *target.data_type() {
        DataType::Chars => {
            return DataField::from_chars(
                target.safe_name(),
                raw.format_value(ori.get_value()).to_string(),
            );
        }
        DataType::Auto => return ori,
        _ => {}
    }
    if let Value::Chars(value) = ori.get_value() {
        return chars_to_omlobj(target, value);
    }
    warn_data!(
        " {} want covert {}, but now not support!",
        ori.get_meta(),
        target.data_type()
    );
    diagnostics::push(OmlIssue::new(
        OmlIssueKind::UnsupportedConvert,
        format!("from={} to={}", ori.get_meta(), target.data_type()),
    ));
    ori
}

fn null_ip_field(name: String) -> DataField {
    DataField::new(DataType::IP, name, Value::Null)
}

fn chars_to_omlobj(target: &EvaluationTarget, value: &str) -> DataField {
    match *target.data_type() {
        DataType::Bool => {
            if let Ok(v) = value.parse::<bool>() {
                return DataField::from_bool(target.safe_name(), v);
            }
            diagnostics::push(OmlIssue::new(
                OmlIssueKind::ParseFail,
                format!("var={}, expect=bool, val={}", target.safe_name(), value),
            ));
        }
        DataType::Digit => {
            if let Ok(v) = value.parse::<i64>() {
                return DataField::from_digit(target.safe_name(), v);
            }
            diagnostics::push(OmlIssue::new(
                OmlIssueKind::ParseFail,
                format!("var={}, expect=digit, val={}", target.safe_name(), value),
            ));
        }
        DataType::Float => {
            if let Ok(v) = value.parse::<f64>() {
                return DataField::from_float(target.safe_name(), v);
            }
            diagnostics::push(OmlIssue::new(
                OmlIssueKind::ParseFail,
                format!("var={}, expect=float, val={}", target.safe_name(), value),
            ));
        }
        DataType::IP => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                // 空字符串 → 判空返回空 IP，不刷解析报错。
                return null_ip_field(target.safe_name());
            }
            match trimmed.parse::<IpAddr>() {
                // 字符串 → IP 同时支持 IPv4 与 IPv6（压缩/完整/大写十六进制）。
                Ok(ip) => return DataField::from_ip(target.safe_name(), ip),
                Err(_) => {
                    diagnostics::push(OmlIssue::new(
                        OmlIssueKind::ParseFail,
                        format!("var={}, expect=ip, val={}", target.safe_name(), value),
                    ));
                    // 非法 IP：不得以字符串原样透传当作 IP，返回空 IP。
                    return null_ip_field(target.safe_name());
                }
            }
        }
        _ => {}
    }
    DataField::from_chars(target.safe_name(), value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn conv_str_to_ip(value: &str) -> DataField {
        let field = DataField::from_chars("src_ip", value.to_string());
        omlobj_meta_conv(field, &EvaluationTarget::new("src_ip".into(), DataType::IP))
    }

    fn assert_ip(field: &DataField, expect: IpAddr) {
        assert_eq!(field.get_meta(), &DataType::IP);
        match field.get_value() {
            Value::IpAddr(ip) => assert_eq!(*ip, expect, "value mismatch"),
            other => panic!("expect IpAddr, got {other:?}"),
        }
    }

    fn assert_null_ip(field: &DataField) {
        assert_eq!(field.get_meta(), &DataType::IP);
        assert!(
            matches!(field.get_value(), Value::Null),
            "expect null ip, got {:?}",
            field.get_value()
        );
    }

    #[test]
    fn chars_ipv4_converts_to_v4() {
        assert_ip(
            &conv_str_to_ip("1.2.3.4"),
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
        );
        // 容忍首尾空白。
        assert_ip(
            &conv_str_to_ip(" 10.0.0.1 "),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
    }

    #[test]
    fn chars_ipv6_converts_to_v6() {
        // 压缩写法。
        assert_ip(&conv_str_to_ip("fc00::1"), "fc00::1".parse().unwrap());
        assert_ip(
            &conv_str_to_ip("::ffff:1.2.3.4"),
            "::ffff:1.2.3.4".parse().unwrap(),
        );
        // 完整写法。
        let full = "2001:0db8:85a3:0000:0000:8a2e:0370:7334";
        assert_ip(&conv_str_to_ip(full), full.parse().unwrap());
        // 大写十六进制。
        assert_ip(&conv_str_to_ip("FC00::1"), "fc00::1".parse().unwrap());
        // 全零缩写是合法地址。
        assert_ip(&conv_str_to_ip("::"), IpAddr::from([0u8; 16]));
    }

    #[test]
    fn chars_invalid_ip_variants_become_null_ip() {
        for bad in [
            "not-an-ip",
            "1.2.3.4.5",
            "999.1.1.1",
            "1.2.3.4x",
            "1.2.3.4:5",
            "fc00::g",
            "fc00::1x",
            "2001:db8:::1",
        ] {
            let field = conv_str_to_ip(bad);
            assert_null_ip(&field);
            assert!(
                !matches!(field.get_value(), Value::Chars(_)),
                "{bad:?} must not pass through as chars"
            );
        }
    }

    #[test]
    fn converted_ipv6_feeds_ip_to_biguint_like_direct_ip() {
        // 链路：chars 接收的 IPv6 → 字符串→IP 转换 → ip_to_biguint，
        // 应与直接 Value::IpAddr::V6 输入一致（issue #358 主链路）。
        use crate::language::ip_to_biguint;
        for input in ["fc00::1", "2001:db8::1", "::ffff:192.0.2.1"] {
            let field = conv_str_to_ip(input);
            let Value::IpAddr(ip) = field.get_value() else {
                panic!("expect IpAddr after conversion for {input:?}")
            };
            assert_eq!(
                ip_to_biguint(*ip),
                ip_to_biguint(input.parse().unwrap()),
                "converted {input:?} must produce the same biguint as direct parse"
            );
        }
    }

    #[test]
    fn chars_empty_becomes_null_ip_without_parse_fail() {
        assert_null_ip(&conv_str_to_ip(""));
        assert_null_ip(&conv_str_to_ip("   "));
    }

    #[test]
    fn chars_invalid_ip_becomes_null_ip_not_chars() {
        let field = conv_str_to_ip("not-an-ip");
        assert_null_ip(&field);
        // 不得以字符串原样透传当作 IP。
        assert!(!matches!(field.get_value(), Value::Chars(_)));
    }
}
