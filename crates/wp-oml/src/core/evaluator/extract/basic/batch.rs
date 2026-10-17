use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::evaluator::traits::AsyncExpEvaluator;
use crate::core::evaluator::transform::omlobj_meta_conv;
use crate::core::prelude::*;
use crate::language::BatchEvalTarget;
use crate::language::{BatchEvalExp, BatchEvaluation, PipeFun, RecordOperation};
use async_trait::async_trait;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage, Value};

impl BatchEvalExp {
    pub(crate) fn eval_proc(
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
        let mut wrapped_needs: Vec<FieldStorage> = Vec::with_capacity(needs.len());
        for field in needs {
            // `remove` markers must also delete targets that were already
            // written, so that a batch remove cannot leave a stale value.
            if field.get_meta() == &DataType::Ignore && matches!(field.get_value(), Value::Null) {
                dst.items
                    .retain(|item| item.as_field().get_name() != field.get_name());
                continue;
            }
            wrapped_needs.push(FieldStorage::from_owned(field));
        }
        dst.items.append(&mut wrapped_needs);
    }
}

#[async_trait]
impl AsyncExpEvaluator for BatchEvalExp {
    async fn eval_proc_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) {
        self.eval_proc(src, dst, cache);
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
            BatchEvaluation::Pipe(op, funs) => {
                let has_remove = funs.iter().any(|fun| matches!(fun, PipeFun::Remove(_)));
                // A remove must clear targets that already exist in `dst`, so do
                // not let the "already present" filter hide matching sources.
                let needs = if has_remove {
                    let empty = DataRecord::default();
                    op.extract_batch(target, src, &empty)
                } else {
                    op.extract_batch(target, src, dst)
                };
                let origin = target.origin();
                let mut out = Vec::with_capacity(needs.len());
                for field in needs {
                    let mut field = field;
                    for fun in funs {
                        field = fun.value_cacu(field);
                    }
                    if field.get_meta() == &DataType::Ignore
                        && matches!(field.get_value(), Value::Null)
                    {
                        // Keep the marker so `eval_proc` can delete an existing
                        // target with the same name.
                        out.push(field);
                        continue;
                    }
                    if matches!(field.get_value(), Value::Null) {
                        continue;
                    }
                    if origin.data_type() != &DataType::Auto
                        && field.get_meta() != origin.data_type()
                    {
                        field = omlobj_meta_conv(field, origin);
                    }
                    if !matches!(field.get_value(), Value::Null) {
                        out.push(field);
                    }
                }
                out
            }
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
    use crate::language::BatchEvalExp;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, DataType, FieldStorage};

    #[test]
    fn test_value_arr1() {
        let cache = &mut FieldQueryCache::default();

        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("details[0]/process_name", "hello1")),
            FieldStorage::from_owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::from_owned(DataField::from_chars("details[2]/process_name", "hello3")),
            FieldStorage::from_owned(DataField::from_chars("details[3]/process_name", "hello4")),
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
            FieldStorage::from_owned(DataField::from_chars("details[0]/process_name", "hello1")),
            FieldStorage::from_owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::from_owned(DataField::from_chars("details[11]/process_name", "hello2")),
            FieldStorage::from_owned(DataField::from_chars("details[2]/process_name", "hello3")),
            FieldStorage::from_owned(DataField::from_chars("details[3]/process_name", "hello4")),
        ];
        let src = DataRecord::from(data.clone());
        let expect_data = vec![
            FieldStorage::from_owned(DataField::from_chars("details[1]/process_name", "hello2")),
            FieldStorage::from_owned(DataField::from_chars("details[11]/process_name", "hello2")),
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
