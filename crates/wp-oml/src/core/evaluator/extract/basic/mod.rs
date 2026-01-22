use crate::core::prelude::*;
use crate::{core::FieldCollector, language::DirectAccessor};
mod batch;
mod read;
mod take;

impl FieldCollector for DirectAccessor {
    fn collect_item(
        &self,
        name: &str,
        src: &DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField> {
        match self {
            DirectAccessor::Take(o) => o.collect_item(name, src, dst),
            DirectAccessor::Read(o) => o.collect_item(name, src, dst),
        }
    }
}

impl FieldExtractor for DirectAccessor {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            DirectAccessor::Take(o) => o.extract_one(target, src, dst),
            DirectAccessor::Read(o) => o.extract_one(target, src, dst),
        }
    }

    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            DirectAccessor::Take(o) => o.extract_more(src, dst, cache),
            DirectAccessor::Read(o) => o.extract_more(src, dst, cache),
        }
    }

    fn support_batch(&self) -> bool {
        match self {
            DirectAccessor::Take(o) => o.support_batch(),
            DirectAccessor::Read(o) => o.support_batch(),
        }
    }
}
