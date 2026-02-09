use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use crate::language::BatchEvalTarget;
use crate::language::{BatchEvalExp, BatchEvaluation, RecordOperation};
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::data::record::RecordItem;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

impl ExpEvaluator for BatchEvalExp {
    fn eval_proc(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) {
        let needs = self.eval_way().extract_batch(self.target(), src, dst);
        if needs.is_empty() {
            // 诊断：批量匹配 0 命中
            let pat = self
                .target()
                .origin()
                .name()
                .clone()
                .unwrap_or_else(|| "_".to_string());
            diagnostics::push(OmlIssue::new(OmlIssueKind::BatchNoMatch, pat));
        }
        let mut wrapped_needs: Vec<FieldStorage> =
            needs.into_iter().map(FieldStorage::Owned).collect();
        dst.items.append(&mut wrapped_needs);
    }
}

impl BatchFetcher for BatchEvaluation {
    fn extract_batch(
        &self,
        target: &BatchEvalTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField> {
        match self {
            BatchEvaluation::Get(x) => x.extract_batch(target, src, dst),
        }
    }
}
impl BatchFetcher for RecordOperation {
    fn extract_batch(
        &self,
        target: &BatchEvalTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Vec<DataField> {
        let mut needs: Vec<DataField> = Vec::with_capacity(10);
        let mut used = Vec::with_capacity(10);
        for (idx, i) in src.iter().enumerate() {
            if !dst.items.iter().any(|x| x.get_name() == i.get_name()) && target.match_it(i) {
                needs.push((*i).clone());
                used.push(idx);
            }
        }
        used.reverse();
        for idx in used {
            src.remove(idx);
        }
        needs
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataRecordRef;
    use crate::core::evaluator::traits::ExpEvaluator;
    use crate::language::BatchEvalExp;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage};

    #[test]
    fn test_value_arr1() {
        let cache = &mut FieldQueryCache::default();

        let data = vec![
            FieldStorage::Owned(DataField::from_chars("details[0]/process_name", "hello1")),
            FieldStorage::Owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::Owned(DataField::from_chars("details[2]/process_name", "hello3")),
            FieldStorage::Owned(DataField::from_chars("details[3]/process_name", "hello4")),
        ];
        let src = DataRecord::from(data.clone());

        let target = BatchEvalExp::new("*".to_string(), DataType::Auto);
        let mut needs = DataRecord::default();
        let mut src_ref = DataRecordRef::from(&src);
        target.eval_proc(&mut src_ref, &mut needs, cache);
        assert_eq!(src, needs);
        assert!(src_ref.is_empty());

        let target = BatchEvalExp::new("details*".to_string(), DataType::Auto);
        let mut needs = DataRecord::default();
        let mut src_ref = DataRecordRef::from(&src);
        target.eval_proc(&mut src_ref, &mut needs, cache);
        assert_eq!(src, needs);
        assert!(src_ref.is_empty());
    }

    #[test]
    fn test_value_arr2() {
        let cache = &mut FieldQueryCache::default();

        let data = vec![
            FieldStorage::Owned(DataField::from_chars("details[0]/process_name", "hello1")),
            FieldStorage::Owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::Owned(DataField::from_chars("details[11]/process_name", "hello2")),
            FieldStorage::Owned(DataField::from_chars("details[2]/process_name", "hello3")),
            FieldStorage::Owned(DataField::from_chars("details[3]/process_name", "hello4")),
        ];
        let src = DataRecord::from(data.clone());
        let expect_data = vec![
            FieldStorage::Owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::Owned(DataField::from_chars("details[11]/process_name", "hello2")),
        ];
        let expect = DataRecord::from(expect_data.clone());

        let target = BatchEvalExp::new("details[1*process_name".to_string(), DataType::Auto);
        let mut needs = DataRecord::default();
        let mut src_ref = DataRecordRef::from(&src);
        target.eval_proc(&mut src_ref, &mut needs, cache);
        assert_eq!(expect, needs);
        assert_eq!(src_ref.len(), 3);
    }
}
