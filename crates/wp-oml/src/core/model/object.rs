use super::super::{AsyncDataTransformer, ConfADMExt};
use crate::core::diagnostics;
use crate::core::evaluator::traits::AsyncExpEvaluator;
use crate::core::prelude::*;
use crate::language::ObjModel;
use crate::parser::error::OMLCodeErrorTait;
use crate::parser::oml_parse_raw;
use async_trait::async_trait;
use orion_error::{
    ErrorWith, compat_prelude::ErrorOweBase, runtime::ContextRecord, runtime::WithContext,
};
use tokio::fs;
use wp_error::parse_error::{OMLCodeError, OMLCodeReason, OMLCodeResult};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::DataRecord;
use wp_model_core::model::FieldStorage;
use wp_primitives::comment::CommentParser;

impl ObjModel {
    fn cleanup_temp_fields(&self, out: &mut DataRecord) {
        if self.has_temp_fields() {
            for field in &mut out.items {
                if field.get_name().starts_with("__") {
                    *field = FieldStorage::from_owned(DataField::from_ignore(field.get_name()));
                }
            }
        }
    }
}

#[async_trait]
impl AsyncDataTransformer for ObjModel {
    async fn transform_async(&self, data: DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        self.transform_ref_async(&data, cache).await
    }

    async fn transform_ref_async(
        &self,
        data: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> DataRecord {
        diagnostics::reset();
        let mut out = DataRecord::default();
        let mut tdo_ref = DataRecordRef::from(data);
        for ado in &self.items {
            ado.eval_proc_async(&mut tdo_ref, &mut out, cache).await;
        }
        debug_data!("{} convert crate item : {}", self.name(), self.items.len());
        self.cleanup_temp_fields(&mut out);
        out
    }

    async fn transform_batch_async(
        &self,
        records: Vec<DataRecord>,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataRecord> {
        let mut results = Vec::with_capacity(records.len());
        for record in records {
            diagnostics::reset();
            let mut out = DataRecord::default();
            let mut tdo_ref = DataRecordRef::from(&record);
            for ado in &self.items {
                ado.eval_proc_async(&mut tdo_ref, &mut out, cache).await;
            }
            self.cleanup_temp_fields(&mut out);
            results.push(out);
        }
        results
    }

    async fn transform_batch_ref_async(
        &self,
        records: &[DataRecord],
        cache: &mut FieldQueryCache,
    ) -> Vec<DataRecord> {
        let mut results = Vec::with_capacity(records.len());
        for record in records {
            diagnostics::reset();
            let mut out = DataRecord::default();
            let mut tdo_ref = DataRecordRef::from(record);
            for ado in &self.items {
                ado.eval_proc_async(&mut tdo_ref, &mut out, cache).await;
            }
            self.cleanup_temp_fields(&mut out);
            results.push(out);
        }
        results
    }
}

#[async_trait]
impl ConfADMExt for ObjModel {
    async fn load(path: &str) -> OMLCodeResult<Self>
    where
        Self: Sized,
    {
        let mut ctx = WithContext::doing("load oml model");
        ctx.record("path", path);
        let content = fs::read_to_string(path)
            .await
            //.owe_rule::<OMLCodeError>()
            .owe(OMLCodeReason::NotFound("oml load fail".into()))
            .with_context(&ctx)?;
        let mut raw_code = content.as_str();
        let code = CommentParser::ignore_comment(&mut raw_code)
            .map_err(|e| OMLCodeError::from_syntax(e, raw_code, path))?;
        let mut pure_code = code.as_str();
        match oml_parse_raw(&mut pure_code).await {
            Ok(res) => Ok(res),
            Err(e) => Err(OMLCodeError::from_syntax(e, pure_code, path)).with_context(&ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use wp_model_core::model::Value;

    #[tokio::test(flavor = "current_thread")]
    async fn transform_batch_ref_async_transforms_multiple_records() {
        let mut code = r#"
name : batch_async_model
rule :
    /batch/async
---
converted : chars = chars(done) ;
"#;
        let model = oml_parse_raw(&mut code).await.expect("parse oml model");

        let mut rec1 = DataRecord::default();
        rec1.append(DataField::from_chars("src", "alpha"));
        let mut rec2 = DataRecord::default();
        rec2.append(DataField::from_chars("src", "beta"));
        let input = vec![rec1, rec2];

        let mut cache = FieldQueryCache::default();
        let outputs = model.transform_batch_ref_async(&input, &mut cache).await;

        assert_eq!(outputs.len(), 2);
        for output in outputs {
            assert!(matches!(
                output.get_value("converted"),
                Some(Value::Chars(value)) if value == "done"
            ));
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn load_reads_oml_via_async_fs() {
        let mut file = NamedTempFile::new().expect("create temp oml");
        write!(
            file,
            r#"
name : async_load
---
result : chars = chars(done) ;
"#
        )
        .expect("write temp oml");

        let model = ObjModel::load(file.path().to_str().expect("utf8 path"))
            .await
            .expect("load temp oml");
        assert_eq!(model.name(), "async_load");
        assert_eq!(model.items.len(), 1);
    }
}
