use crate::core::prelude::*;
use crate::language::{
    PIPE_TIME_FROM_TS, PIPE_TIME_FROM_TS_MS, PIPE_TIME_FROM_TS_US, PIPE_TIME_TO_TS,
    PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, TimeFromTs, TimeFromTsMs,
    TimeFromTsUs, TimeStampUnit, TimeToTs, TimeToTsMs, TimeToTsUs, TimeToTsZone,
};
use chrono::{DateTime, FixedOffset, Utc};
use wp_model_core::model::{DataField, Value};

/// 秒/时换算
const SECS_PER_HOUR: i32 = 3600;

/// 构造时区偏移。zone 超出 FixedOffset 范围（整点 ±23h，含 i32 乘法溢出）时告警并返回 `None` → 调用方透传
fn zone_offset(zone_hours: i32, fun: &str) -> Option<FixedOffset> {
    match FixedOffset::east_opt(zone_hours.saturating_mul(SECS_PER_HOUR)) {
        Some(tz) => Some(tz),
        None => {
            warn_rule!(
                "{}: invalid zone {} (超出 FixedOffset 范围，按透传处理)",
                fun,
                zone_hours
            );
            None
        }
    }
}

/// 时间 → 时间戳。输入非 time 或 zone 无效时返回 `None`（调用方透传）
fn time_to_ts_value(
    in_val: &DataField,
    zone: Option<i32>,
    fun: &str,
    conv: impl Fn(&DateTime<FixedOffset>) -> i64,
) -> Option<DataField> {
    let Value::Time(x) = in_val.get_value() else {
        return None;
    };
    let tz = zone_offset(zone.unwrap_or(8), fun)?;
    let local = x.and_local_timezone(tz).single()?;
    Some(DataField::from_digit(
        in_val.get_name().to_string(),
        conv(&local),
    ))
}

/// 时间戳 → 时间。输入非 digit、时间戳越界或 zone 无效时返回 `None`（调用方透传）
fn ts_to_time_value(
    in_val: &DataField,
    zone: Option<i32>,
    fun: &str,
    conv: impl Fn(i64) -> Option<DateTime<Utc>>,
) -> Option<DataField> {
    let Value::Digit(x) = in_val.get_value() else {
        return None;
    };
    let dt = match conv(*x) {
        Some(dt) => dt,
        None => {
            warn_rule!("{}: timestamp {} 超出可表示范围，按透传处理", fun, x);
            return None;
        }
    };
    let tz = zone_offset(zone.unwrap_or(8), fun)?;
    let local = dt.with_timezone(&tz).naive_local();
    Some(DataField::from_time(in_val.get_name().to_string(), local))
}

/// `Time::to_ts([zone])`：时间 → 秒时间戳（zone 默认东8区）
impl ValueProcessor for TimeToTs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        time_to_ts_value(&in_val, self.zone, PIPE_TIME_TO_TS, |l| l.timestamp()).unwrap_or(in_val)
    }
}
impl ValueProcessor for TimeToTsMs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        time_to_ts_value(&in_val, self.zone, PIPE_TIME_TO_TS_MS, |l| {
            l.timestamp_millis()
        })
        .unwrap_or(in_val)
    }
}
impl ValueProcessor for TimeToTsUs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        time_to_ts_value(&in_val, self.zone, PIPE_TIME_TO_TS_US, |l| {
            l.timestamp_micros()
        })
        .unwrap_or(in_val)
    }
}
impl ValueProcessor for TimeToTsZone {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        let conv: fn(&DateTime<FixedOffset>) -> i64 = match self.unit {
            TimeStampUnit::MS => |l| l.timestamp_millis(),
            TimeStampUnit::US => |l| l.timestamp_micros(),
            TimeStampUnit::SS => |l| l.timestamp(),
        };
        time_to_ts_value(&in_val, Some(self.zone), PIPE_TIME_TO_TS_ZONE, conv).unwrap_or(in_val)
    }
}

/// `Time::from_ts([zone])`：秒时间戳 → 时间（zone 默认东8区）
impl ValueProcessor for TimeFromTs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        ts_to_time_value(&in_val, self.zone, PIPE_TIME_FROM_TS, |x| {
            DateTime::from_timestamp(x, 0)
        })
        .unwrap_or(in_val)
    }
}
impl ValueProcessor for TimeFromTsMs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        ts_to_time_value(&in_val, self.zone, PIPE_TIME_FROM_TS_MS, |x| {
            DateTime::from_timestamp_millis(x)
        })
        .unwrap_or(in_val)
    }
}
impl ValueProcessor for TimeFromTsUs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        ts_to_time_value(&in_val, self.zone, PIPE_TIME_FROM_TS_US, |x| {
            DateTime::from_timestamp_micros(x)
        })
        .unwrap_or(in_val)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::dev::testing::TestAssert;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_time() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        X  =  pipe  read(Y) | Time::to_ts ;
        Z  =  pipe  read(Y) | Time::to_ts_ms ;
        U  =  pipe  read(Y) | Time::to_ts_us ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        //let expect = TDOEnum::from_digit("X".to_string(), 971136000);
        let expect = DataField::from_digit("X".to_string(), 971107200);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
        let expect = DataField::from_digit("Z".to_string(), 971107200000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));

        let expect = DataField::from_digit("U".to_string(), 971107200000000);
        assert_eq!(target.field("U").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_utc() {
        let cache = &mut FieldQueryCache::default();
        // 1739000000000 ms = 2025-02-07 18:13:20 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms",
            1739000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(1739000000000)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_with_zone() {
        let cache = &mut FieldQueryCache::default();
        // 东 8 区：0 ms → 1970-01-01 08:00:00
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts_ms", 0))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(8) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(0)
            .unwrap()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .naive_local();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_ms_with_zone() {
        let cache = &mut FieldQueryCache::default();
        // to_ts_ms(0)：本地时间按 UTC 解释，2000-10-10 00:00:00 → 971136000000
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        Z  =  pipe  read(Y) | Time::to_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Z".to_string(), 971136000000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_ms_roundtrip_default_zone() {
        let cache = &mut FieldQueryCache::default();
        // 默认东 8 区往返互逆：from_ts_ms → to_ts_ms 复原原时间戳
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms",
            1739000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms ;
        Y  =  pipe read(X) | Time::to_ts_ms ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Y".to_string(), 1739000000000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_ms_roundtrip_utc_zone() {
        let cache = &mut FieldQueryCache::default();
        // 显式 UTC 往返互逆：from_ts_ms(0) → to_ts_ms(0) 复原原时间戳
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms",
            1739000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(0) ;
        Y  =  pipe read(X) | Time::to_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Y".to_string(), 1739000000000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_default_zone() {
        let cache = &mut FieldQueryCache::default();
        // 无参 from_ts_ms 默认东8区：0 ms → 1970-01-01 08:00:00
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts_ms", 0))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(0)
            .unwrap()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .naive_local();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_negative_zone() {
        let cache = &mut FieldQueryCache::default();
        // 西5区：0 ms → 1969-12-31 19:00:00
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts_ms", 0))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(-5) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(0)
            .unwrap()
            .with_timezone(&chrono::FixedOffset::east_opt(-5 * 3600).unwrap())
            .naive_local();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_ms_negative_zone() {
        let cache = &mut FieldQueryCache::default();
        // to_ts_ms(-5)：本地时间按 UTC-5 解释，2000-10-10 00:00:00 → 971154000000
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        Z  =  pipe  read(Y) | Time::to_ts_ms(-5) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Z".to_string(), 971154000000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_pass_through_non_digit() {
        let cache = &mut FieldQueryCache::default();
        // 非 digit 输入原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_chars("x", "abc"))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(x) | Time::from_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X", "abc");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_ms_pass_through_non_time() {
        let cache = &mut FieldQueryCache::default();
        // 非 time 输入原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_digit("x", 123))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(x) | Time::to_ts_ms ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X", 123);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_invalid_timestamp_pass_through() {
        let cache = &mut FieldQueryCache::default();
        // 超出 chrono 可表示范围的时间戳原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms",
            i64::MAX,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X", i64::MAX);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[test]
    fn test_zone_offset_range_defense() {
        // 防御层：parse_zone 已在解析期拦截范围外；此处直接验证 zone_offset 兜底
        //（含 i32 乘法溢出的 saturating_mul，确保不 panic）
        assert!(super::zone_offset(23, "Time::to_ts_ms").is_some());
        assert!(super::zone_offset(-23, "Time::from_ts_ms").is_some());
        assert!(super::zone_offset(24, "Time::to_ts_ms").is_none());
        assert!(super::zone_offset(-24, "Time::from_ts_ms").is_none());
        assert!(super::zone_offset(i32::MAX, "Time::to_ts_ms").is_none());
        assert!(super::zone_offset(i32::MIN, "Time::from_ts_ms").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_utc() {
        let cache = &mut FieldQueryCache::default();
        // from_ts(0)：971136000 秒 → 2000-10-10 00:00:00 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts", 971136000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts) | Time::from_ts(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp(971136000, 0)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_us_utc() {
        let cache = &mut FieldQueryCache::default();
        // from_ts_us(0)：971136000000000 微秒 → 2000-10-10 00:00:00 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_us",
            971136000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_us) | Time::from_ts_us(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_micros(971136000000000)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_roundtrip_default_zone() {
        let cache = &mut FieldQueryCache::default();
        // from_ts → to_ts 默认东8往返互逆，复原原秒时间戳
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts", 1739000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts) | Time::from_ts ;
        Y  =  pipe read(X) | Time::to_ts ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Y".to_string(), 1739000000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_us_roundtrip_utc_zone() {
        let cache = &mut FieldQueryCache::default();
        // from_ts_us(0) → to_ts_us(0) 显式 UTC 往返互逆，复原原微秒时间戳
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_us",
            1739000000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_us) | Time::from_ts_us(0) ;
        Y  =  pipe read(X) | Time::to_ts_us(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Y".to_string(), 1739000000000000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_pass_through_non_digit() {
        let cache = &mut FieldQueryCache::default();
        // 非 digit 输入原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_chars("x", "abc"))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(x) | Time::from_ts(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X", "abc");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_us_invalid_timestamp_pass_through() {
        let cache = &mut FieldQueryCache::default();
        // 超出 chrono 可表示范围的微秒时间戳原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_us",
            i64::MAX,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_us) | Time::from_ts_us(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X", i64::MAX);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_with_zone() {
        let cache = &mut FieldQueryCache::default();
        // to_ts(0)：本地时间按 UTC 解释，2000-10-10 00:00:00 → 971136000（秒）
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        Z  =  pipe  read(Y) | Time::to_ts(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Z".to_string(), 971136000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_us_with_zone() {
        let cache = &mut FieldQueryCache::default();
        // to_ts_us(0)：本地时间按 UTC 解释，2000-10-10 00:00:00 → 971136000000000（微秒）
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        Z  =  pipe  read(Y) | Time::to_ts_us(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Z".to_string(), 971136000000000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_negative_zone() {
        let cache = &mut FieldQueryCache::default();
        // to_ts(-5)：本地时间按 UTC-5 解释，2000-10-10 00:00:00 → 971154000（秒）
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        Z  =  pipe  read(Y) | Time::to_ts(-5) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Z".to_string(), 971154000);
        assert_eq!(target.field("Z").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_out_of_range_zone_rejected() {
        // 方案 A：|zone| > 24 在解析期即报错（不再运行时透传）
        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts) | Time::from_ts(999) ;
         "#;
        let model = oml_parse_raw(&mut conf).await;
        assert!(model.is_err(), "from_ts(999) 超 ±24h 应在解析期被拒绝");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_ms_negative() {
        let cache = &mut FieldQueryCache::default();
        // 负毫秒时间戳（1970 前）：-1000 ms → 1969-12-31 23:59:59.000 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms", -1000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(-1000)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_negative() {
        let cache = &mut FieldQueryCache::default();
        // 负秒时间戳（1970 前）：-1 s → 1969-12-31 23:59:59 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts", -1))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts) | Time::from_ts(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp(-1, 0)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_from_ts_us_negative() {
        let cache = &mut FieldQueryCache::default();
        // 负微秒时间戳（1970 前）：-1000000 us → 1969-12-31 23:59:59.000000 UTC
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_us", -1000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_us) | Time::from_ts_us(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = chrono::DateTime::<chrono::Utc>::from_timestamp_micros(-1000000)
            .unwrap()
            .naive_utc();
        let out = target.field("X").expect("X field").as_field();
        assert_eq!(out.get_value(), &wp_model_core::model::Value::Time(expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_ms_roundtrip_negative() {
        let cache = &mut FieldQueryCache::default();
        // 负时间戳（1970 前）默认东8往返互逆
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms", -1000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms ;
        Y  =  pipe read(X) | Time::to_ts_ms ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("Y".to_string(), -1000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_cross_zone_not_inverse() {
        let cache = &mut FieldQueryCache::default();
        // 文档「互逆需使用相同 zone」：from_ts_ms(8) 后接 to_ts_ms(0) 得原值 + 8h
        let data = vec![FieldStorage::from_owned(DataField::from_digit(
            "ts_ms",
            1739000000000,
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(8) ;
        Y  =  pipe read(X) | Time::to_ts_ms(0) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        // 东8墙钟再按 UTC 解释 → 原值 + 8h = 1739028800000
        let expect = DataField::from_digit("Y".to_string(), 1739028800000);
        assert_eq!(target.field("Y").map(|s| s.as_field()), Some(&expect));
    }
}
