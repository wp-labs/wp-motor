use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::language::EvaluationTarget;
use std::net::{IpAddr, Ipv4Addr};
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
            if let Ok(v) = value.parse::<Ipv4Addr>() {
                return DataField::from_ip(target.safe_name(), IpAddr::V4(v));
            }
            diagnostics::push(OmlIssue::new(
                OmlIssueKind::ParseFail,
                format!("var={}, expect=ip, val={}", target.safe_name(), value),
            ));
        }
        _ => {}
    }
    DataField::from_chars(target.safe_name(), value.to_string())
}
