use crate::core::prelude::*;
use crate::language::{TimeToTs, TimeToTsMs, TimeToTsUs, TimeToTsZone};
use chrono::FixedOffset;
use wp_model_core::model::{DataField, Value};

impl ValueProcessor for TimeToTs {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Time(x) => {
                let hour = 3600;
                if let Some(tz) = FixedOffset::east_opt(8 * hour)
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
                let hour = 3600;
                if let Some(tz) = FixedOffset::east_opt(8 * hour)
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
                let hour = 3600;
                if let Some(tz) = FixedOffset::east_opt(8 * hour)
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
                let hour = 3600;
                if let Some(tz) = FixedOffset::east_opt(self.zone * hour)
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
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_time() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![DataField::from_chars("A1", "<html>")];
        let src = DataRecord { items: data };

        let mut conf = r#"
        name : test
        ---
        Y  =  time(2000-10-10 0:0:0);
        X  =  pipe  read(Y) | Time::to_ts ;
        Z  =  pipe  read(Y) | Time::to_ts_ms ;
        U  =  pipe  read(Y) | Time::to_ts_us ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);
        //let expect = TDOEnum::from_digit("X".to_string(), 971136000);
        let expect = DataField::from_digit("X".to_string(), 971107200);
        assert_eq!(target.field("X"), Some(&expect));
        let expect = DataField::from_digit("Z".to_string(), 971107200000);
        assert_eq!(target.field("Z"), Some(&expect));

        let expect = DataField::from_digit("U".to_string(), 971107200000000);
        assert_eq!(target.field("U"), Some(&expect));
    }
}
