use crate::core::FieldExtractor;
use crate::core::prelude::*;
use crate::language::{BuiltinFunction, FunOperation, NowDate, NowHour, NowTime};
use chrono::{Datelike, Local, Timelike};
impl FieldExtractor for NowTime {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        let now = Local::now();
        let name = target.name().clone().unwrap_or("_".to_string());
        Some(DataField::from_time(name, now.naive_local()))
    }
}

impl FieldExtractor for FunOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        self.fun().extract_one(target, src, dst)
    }
}

impl FieldExtractor for BuiltinFunction {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            BuiltinFunction::NowTime(x) => x.extract_one(target, src, dst),
            BuiltinFunction::NowDate(x) => x.extract_one(target, src, dst),
            BuiltinFunction::NowHour(x) => x.extract_one(target, src, dst),
        }
    }
}
impl FieldExtractor for NowDate {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        let now = Local::now().naive_local();
        let name = target.safe_name();

        Some(DataField::from_digit(
            name,
            now.year() as i64 * 10000 + now.month() as i64 * 100 + now.day() as i64,
        ))
    }
}

impl FieldExtractor for NowHour {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        let now = Local::now().naive_local();
        let name = target.safe_name();

        Some(DataField::from_digit(
            name,
            now.year() as i64 * 1000000
                + now.month() as i64 * 10000
                + now.day() as i64 * 100
                + now.hour() as i64,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssertWithMsg;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            DataField::from_chars("A1", "hello1"),
            DataField::from_chars("B2", "hello2"),
            DataField::from_chars("C3", "hello3"),
        ];
        let src = DataRecord { items: data };

        let mut conf = r#"
        name : test
        ---
        X : chars =  Now::time() ;
        X1 =  Now::date() ;
        X2 =  Now::time() ;
        X3 =  Now::hour() ;
         "#;
        let model = oml_parse_raw(&mut conf).assert("oml_conf");

        let target = model.transform(src, cache);

        assert!(target.field("X").is_some());
        println!("{}", target);

        assert!(target.field("X1").is_some());
        println!("{}", target);

        assert!(target.field("X2").is_some());
        println!("{}", target);

        assert!(target.field("X3").is_some());
        println!("{}", target);
    }
}
