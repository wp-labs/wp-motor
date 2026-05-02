use crate::core::AsyncFieldExtractor;
use crate::core::prelude::*;
use crate::language::{BuiltinFunction, FunOperation, NowDate, NowHour, NowTime};
use async_trait::async_trait;
use chrono::{Datelike, Local, Timelike};
use wp_model_core::model::FieldStorage;
#[allow(dead_code)]
impl NowTime {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
    ) -> Option<DataField> {
        let now = Local::now();
        let name = target.name().clone().unwrap_or("_".to_string());
        Some(DataField::from_time(name, now.naive_local()))
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for NowTime {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }
}

#[allow(dead_code)]
impl FunOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.fun().extract_one(target, src, dst)
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for FunOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.fun().extract_one_async(target, src, dst).await
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.fun().extract_storage_async(target, src, dst).await
    }
}

#[allow(dead_code)]
impl BuiltinFunction {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            BuiltinFunction::NowTime(x) => x.extract_one(target, src, dst),
            BuiltinFunction::NowDate(x) => x.extract_one(target, src, dst),
            BuiltinFunction::NowHour(x) => x.extract_one(target, src, dst),
        }
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for BuiltinFunction {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            BuiltinFunction::NowTime(x) => x.extract_one_async(target, src, dst).await,
            BuiltinFunction::NowDate(x) => x.extract_one_async(target, src, dst).await,
            BuiltinFunction::NowHour(x) => x.extract_one_async(target, src, dst).await,
        }
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            BuiltinFunction::NowTime(x) => x.extract_storage_async(target, src, dst).await,
            BuiltinFunction::NowDate(x) => x.extract_storage_async(target, src, dst).await,
            BuiltinFunction::NowHour(x) => x.extract_storage_async(target, src, dst).await,
        }
    }
}
#[allow(dead_code)]
impl NowDate {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
    ) -> Option<DataField> {
        let now = Local::now().naive_local();
        let name = target.safe_name();

        Some(DataField::from_digit(
            name,
            now.year() as i64 * 10000 + now.month() as i64 * 100 + now.day() as i64,
        ))
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for NowDate {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }
}

#[allow(dead_code)]
impl NowHour {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
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

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for NowHour {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::testcase::TestAssertWithMsg;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe() {
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
        let model = oml_parse_raw(&mut conf).await.assert("oml_conf");

        let target = model.transform_async(src, cache).await;

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
