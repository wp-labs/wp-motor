use crate::core::prelude::*;
use crate::language::{
    TimeFromTsMs, TimeToTs, TimeToTsMs, TimeToTsUs, TimeToTsZone, PIPE_TIME_FROM_TS_MS,
    PIPE_TIME_TO_TS, PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE,
};
use chrono::{DateTime, FixedOffset};
use wp_model_core::model::{DataField, Value};

/// 秒/时换算
const SECS_PER_HOUR: i32 = 3600;

/// 构造时区偏移。zone 超出 ±24h（含 i32 乘法溢出）时告警并返回 `None` → 调用方透传
fn zone_offset(zone_hours: i32, fun: &str) -> Option<FixedOffset> {
    match FixedOffset::east_opt(zone_hours.saturating_mul(SECS_PER_HOUR)) {
        Some(tz) => Some(tz),
        None => {
            warn_rule!("{}: invalid zone {} (超出 ±24h，按透传处理)", fun, zone_hours);
            None
        }
    }
}

/// `Time::from_ts_ms([zone])`：毫秒时间戳 → 时间（zone 默认东8区）
impl ValueProcessor for TimeFromTsMs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Digit(ms) => {
                let zone = self.zone.unwrap_or(8);
                match DateTime::from_timestamp_millis(*ms) {
                    Some(dt) => {
                        if let Some(tz) = zone_offset(zone, PIPE_TIME_FROM_TS_MS) {
                            let local = dt.with_timezone(&tz).naive_local();
                            return DataField::from_time(in_val.get_name().to_string(), local);
                        }
                    }
                    None => warn_rule!(
                        "{}: timestamp {} 超出可表示范围，按透传处理",
                        PIPE_TIME_FROM_TS_MS,
                        ms
                    ),
                }
                in_val
            }
            _ => in_val,
        }
    }
}

impl ValueProcessor for TimeToTs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Time(x) => {
                if let Some(tz) = zone_offset(self.zone.unwrap_or(8), PIPE_TIME_TO_TS)
                    && let Some(local) = x.and_local_timezone(tz).single()
                {
                    return DataField::from_digit(in_val.get_name().to_string(), local.timestamp());
                }
                in_val
                //TDOEnum::Time()
            }
            _ => in_val,
        }
    }
}
impl ValueProcessor for TimeToTsMs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Time(x) => {
                if let Some(tz) = zone_offset(self.zone.unwrap_or(8), PIPE_TIME_TO_TS_MS)
                    && let Some(local) = x.and_local_timezone(tz).single()
                {
                    return DataField::from_digit(
                        in_val.get_name().to_string(),
                        local.timestamp_millis(),
                    );
                }
                in_val
            }
            _ => in_val,
        }
    }
}
impl ValueProcessor for TimeToTsUs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Time(x) => {
                if let Some(tz) = zone_offset(self.zone.unwrap_or(8), PIPE_TIME_TO_TS_US)
                    && let Some(local) = x.and_local_timezone(tz).single()
                {
                    return DataField::from_digit(
                        in_val.get_name().to_string(),
                        local.timestamp_micros(),
                    );
                }
                in_val
            }
            _ => in_val,
        }
    }
}
impl ValueProcessor for TimeToTsZone {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Time(x) => {
                if let Some(tz) = zone_offset(self.zone, PIPE_TIME_TO_TS_ZONE)
                    && let Some(local) = x.and_local_timezone(tz).single()
                {
                    match self.unit {
                        crate::language::TimeStampUnit::MS => {
                            return DataField::from_digit(
                                in_val.get_name().to_string(),
                                local.timestamp_millis(),
                            );
                        }
                        crate::language::TimeStampUnit::US => {
                            return DataField::from_digit(
                                in_val.get_name().to_string(),
                                local.timestamp_micros(),
                            );
                        }
                        crate::language::TimeStampUnit::SS => {
                            return DataField::from_digit(
                                in_val.get_name().to_string(),
                                local.timestamp(),
                            );
                        }
                    }
                }
                in_val
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
    async fn test_time_from_ts_ms_out_of_range_zone_pass_through() {
        let cache = &mut FieldQueryCache::default();
        // zone 超出 FixedOffset 合法范围（±24h）时原样透传
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts_ms", 0))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(999) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_digit("X", 0);
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_to_ts_ms_out_of_range_zone_pass_through() {
        let cache = &mut FieldQueryCache::default();
        // zone 超出 FixedOffset 合法范围（±24h）时原样透传
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        B  =  time(2000-10-10 0:0:0);
        C  =  pipe  read(B) | Time::to_ts_ms(999) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let b = target.field("B").expect("B field").as_field().get_value().clone();
        assert_eq!(target.field("C").expect("C field").as_field().get_value(), &b);
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_time_ts_overflow_zone_no_panic() {
        let cache = &mut FieldQueryCache::default();
        // zone 极大（|zone| > 596523）时 i32 乘法溢出：应透传而非 panic（debug 溢出检查）
        let data = vec![FieldStorage::from_owned(DataField::from_digit("ts_ms", 0))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  pipe read(ts_ms) | Time::from_ts_ms(1000000) ;
        B  =  time(2000-10-10 0:0:0);
        Y  =  pipe read(B) | Time::to_ts_ms(1000000) ;
        C  =  pipe read(B) | Time::to_ts_zone(1000000, ms) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        assert_eq!(
            target.field("X").map(|s| s.as_field()),
            Some(&DataField::from_digit("X", 0))
        );
        let b = target.field("B").expect("B field").as_field().get_value().clone();
        assert_eq!(target.field("Y").expect("Y field").as_field().get_value(), &b);
        assert_eq!(target.field("C").expect("C field").as_field().get_value(), &b);
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
    async fn test_time_to_ts_out_of_range_zone_pass_through() {
        let cache = &mut FieldQueryCache::default();
        // to_ts/to_ts_us 无效 zone（±24h 外）原样透传
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "A1", "<html>",
        ))]);
        let mut conf = r#"
        name : test
        ---
        B  =  time(2000-10-10 0:0:0);
        C  =  pipe  read(B) | Time::to_ts(999) ;
        D  =  pipe  read(B) | Time::to_ts_us(999) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let b = target.field("B").expect("B field").as_field().get_value().clone();
        assert_eq!(target.field("C").expect("C field").as_field().get_value(), &b);
        assert_eq!(target.field("D").expect("D field").as_field().get_value(), &b);
    }
}
