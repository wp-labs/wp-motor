use chrono::{NaiveDate, NaiveTime};
use std::net::IpAddr;
use std::str::FromStr;
use wp_model_core::model::DateTimeValue;
use wp_model_core::model::{DataField, DataRecord};

#[allow(dead_code)]
pub fn test_tdo_crate() -> DataRecord {
    let items = vec![
        DataField::from_time(
            "recv_time",
            DateTimeValue::new(
                NaiveDate::from_ymd_opt(2023, 10, 1).unwrap(),
                NaiveTime::from_hms_opt(10, 10, 0).unwrap(),
            ),
        ),
        DataField::from_time(
            "occur_time",
            DateTimeValue::new(
                NaiveDate::from_ymd_opt(2022, 10, 1).unwrap(),
                NaiveTime::from_hms_opt(10, 10, 0).unwrap(),
            ),
        ),
        DataField::from_ip("from_ip", IpAddr::from_str("10.0.0.1").unwrap()),
        DataField::from_ip("src_ip", IpAddr::from_str("172.0.0.1").unwrap()),
        DataField::from_chars("src_city", "beijing"),
        DataField::from_chars("requ_uri", "http://baidu.com"),
        DataField::from_digit("resp_len", 1024),
        DataField::from_digit("resp_status", 200),
    ];
    DataRecord::from(items)
}
