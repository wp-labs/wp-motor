use crate::core::FieldExtractor;
use crate::core::prelude::*;
use crate::language::{BuiltinFunction, FunOperation, NowDate, NowHour, NowTime};
use chrono::{Datelike, Local, Timelike};
use wp_model_core::model::FieldStorage;
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

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
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

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
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

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
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

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
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

    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssertWithMsg;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    #[test]
    fn test_pipe() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("A1", "hello1")),
            FieldStorage::from_owned(DataField::from_chars("B2", "hello2")),
            FieldStorage::from_owned(DataField::from_chars("C3", "hello3")),
        ];
        let src = DataRecord::from(data);

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
