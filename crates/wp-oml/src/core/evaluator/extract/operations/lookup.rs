use crate::core::AsyncFieldExtractor;
use crate::core::prelude::*;
use crate::language::LookupOperation;
use async_trait::async_trait;
use std::sync::Arc;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

fn normalize_lookup_key(raw: &str) -> String {
    raw.trim().to_lowercase()
}

impl LookupOperation {
    async fn resolve_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        let key_field = self.key().extract_one_async(target, src, dst).await;
        if let Some(field) = key_field
            && let Value::Chars(raw) = field.get_value()
            && let Some(dict) = self.compiled()
            && let Some(found) = dict.get(&normalize_lookup_key(raw))
        {
            return Some(FieldStorage::from_shared(Arc::clone(found)));
        }

        self.default().extract_storage_async(target, src, dst).await
    }
}

#[async_trait]
impl AsyncFieldExtractor for LookupOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.resolve_storage_async(target, src, dst)
            .await
            .map(FieldStorage::into_owned)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.resolve_storage_async(target, src, dst).await
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::testcase::TestAssert;
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

    #[tokio::test(flavor = "current_thread")]
    async fn test_lookup_nocase_matches_static_object_keys() {
        let mut conf = r#"
name : test
---
static {
    status_score = object {
        error = float(90.0);
        warning = float(70.0);
    };
}

risk_score : float = lookup_nocase(status_score, read(status), 40.0);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "status", " ERROR ",
        ))]);

        let target = model.transform_async(src, cache).await;
        let storage = target.field("risk_score").expect("risk_score field");
        assert_eq!(storage.get_name(), "risk_score");
        assert_eq!(storage.as_field().get_value(), &Value::Float(90.0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_lookup_nocase_falls_back_for_non_string_key() {
        let mut conf = r#"
name : test
---
static {
    status_score = object {
        error = float(90.0);
    };
}

risk_score : float = lookup_nocase(status_score, read(code), 40.0);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_digit(
            "code", 500,
        ))]);

        let target = model.transform_async(src, cache).await;
        let storage = target.field("risk_score").expect("risk_score field");
        assert_eq!(storage.get_name(), "risk_score");
        assert_eq!(storage.as_field().get_value(), &Value::Float(40.0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_lookup_nocase_matches_static_object_keys_async() {
        let mut conf = r#"
name : test
---
static {
    status_score = object {
        error = float(90.0);
        warning = float(70.0);
    };
}

risk_score : float = lookup_nocase(status_score, read(status), 40.0);
"#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let cache = &mut FieldQueryCache::default();
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "status", " ERROR ",
        ))]);

        let target = model.transform_async(src, cache).await;
        let storage = target.field("risk_score").expect("risk_score field");
        assert_eq!(storage.get_name(), "risk_score");
        assert_eq!(storage.as_field().get_value(), &Value::Float(90.0));
    }
}
