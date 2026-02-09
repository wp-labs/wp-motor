use wp_error::parse_error::OMLCodeResult;

use crate::core::prelude::*;

pub trait FieldCollector {
    fn collect_item(&self, name: &str, src: &DataRecordRef<'_>, dst: &DataRecord)
    -> Vec<DataField>;
}

pub trait ExpEvaluator {
    fn eval_proc(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    );
}

pub trait BatchFetcher {
    fn extract_batch(
        &self,
        target: &BatchEvalTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField>;
}

#[enum_dispatch]
pub trait ValueProcessor {
    fn value_cacu(&self, in_val: DataField) -> DataField;
}

impl ExpEvaluator for EvalExp {
    fn eval_proc(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) {
        match self {
            EvalExp::Single(x) => {
                x.eval_proc(src, dst, cache);
            }
            EvalExp::Batch(x) => {
                x.eval_proc(src, dst, cache);
            }
        }
    }
}
#[allow(dead_code)]
pub trait LibUseAble {
    fn search(&self, lib_n: &str, cond: &DataField, need: &str) -> Option<DataField>;
}
pub trait ConfADMExt {
    fn load(path: &str) -> OMLCodeResult<Self>
    where
        Self: Sized;
}

pub trait DataTransformer {
    fn transform(&self, data: DataRecord, cache: &mut FieldQueryCache) -> DataRecord;
    fn transform_ref(&self, data: &DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        self.transform(data.clone(), cache)
    }
    fn append(&self, data: &mut DataRecord);

    /// Batch transform multiple records with shared cache (default implementation)
    ///
    /// This default implementation provides backward compatibility by processing
    /// records one by one. Implementations can override this for better performance
    /// by reusing the cache and compiled model across all records.
    fn transform_batch(
        &self,
        records: Vec<DataRecord>,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataRecord> {
        records
            .into_iter()
            .map(|record| self.transform(record, cache))
            .collect()
    }

    /// Batch transform multiple records (reference version)
    fn transform_batch_ref(
        &self,
        records: &[DataRecord],
        cache: &mut FieldQueryCache,
    ) -> Vec<DataRecord> {
        records
            .iter()
            .map(|record| self.transform_ref(record, cache))
            .collect()
    }
}
