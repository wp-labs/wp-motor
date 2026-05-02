use crate::core::AsyncFieldExtractor;
use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use async_trait::async_trait;
use std::collections::HashMap;
use strfmt::{DisplayStr, Formatter, strfmt};
use wp_data_fmt::{Raw, RecordFormatter};
use wp_model_core::model::FieldStorage;
impl FmtOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let mut args = HashMap::new();
        let mut not_find_items = Vec::new();
        for item in self.subs() {
            let cur = EvaluationTarget::new(
                item.dat_get()
                    .field_name()
                    .clone()
                    .unwrap_or("_fmt_".to_string()),
                DataType::Auto,
            );
            // Use extract_storage to preserve zero-copy for Arc variants
            if let Some(storage) = item.extract_storage(&cur, src, dst) {
                args.insert(storage.get_name().to_string(), FmtVal(storage));
            } else {
                not_find_items.push(item.dat_get());
            }
        }
        // 诊断：记录 fmt 中未命中的变量
        if !not_find_items.is_empty() {
            for miss in &not_find_items {
                let name = miss
                    .field_name()
                    .clone()
                    .unwrap_or_else(|| "_fmt_".to_string());
                diagnostics::push(OmlIssue::new(OmlIssueKind::FmtVarMissing, name));
            }
        }
        debug_edata!(dst.id, "fmt:{}, val:{:?}", self.fmt_str(), args);
        debug_edata!(
            dst.id,
            " oml fmt not get data from : {}, vars:{:?}",
            dst,
            not_find_items
        );
        let data = if let Ok(msg) = strfmt(self.fmt_str().as_str(), &args) {
            msg
        } else {
            "".to_string()
        };
        let name = target.safe_name();
        Some(DataField::from_chars(name, data))
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
impl AsyncFieldExtractor for FmtOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let mut args = HashMap::new();
        let mut not_find_items = Vec::new();
        for item in self.subs() {
            let cur = EvaluationTarget::new(
                item.dat_get()
                    .field_name()
                    .clone()
                    .unwrap_or("_fmt_".to_string()),
                DataType::Auto,
            );
            if let Some(storage) = item.extract_storage_async(&cur, src, dst).await {
                args.insert(storage.get_name().to_string(), FmtVal(storage));
            } else {
                not_find_items.push(item.dat_get());
            }
        }
        if !not_find_items.is_empty() {
            for miss in &not_find_items {
                let name = miss
                    .field_name()
                    .clone()
                    .unwrap_or_else(|| "_fmt_".to_string());
                diagnostics::push(OmlIssue::new(OmlIssueKind::FmtVarMissing, name));
            }
        }
        debug_edata!(dst.id, "fmt:{}, val:{:?}", self.fmt_str(), args);
        debug_edata!(
            dst.id,
            " oml fmt not get data from : {}, vars:{:?}",
            dst,
            not_find_items
        );
        let data = if let Ok(msg) = strfmt(self.fmt_str().as_str(), &args) {
            msg
        } else {
            "".to_string()
        };
        let name = target.safe_name();
        Some(DataField::from_chars(name, data))
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one_async(target, src, dst)
            .await
            .map(FieldStorage::from_owned)
    }
}

#[derive(Debug)]
pub struct FmtVal(pub FieldStorage);
impl DisplayStr for FmtVal
where
//for<'a> RawFmt<&'a T>: Display,
{
    fn display_str(&self, f: &mut Formatter) -> strfmt::Result<()> {
        let raw_fmt = Raw;
        // Directly use the FieldStorage without cloning
        let str = raw_fmt.fmt_field(&self.0).to_string();
        f.str(str.as_str())
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
    async fn test_fmt() {
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("A1", "h1")),
            FieldStorage::from_owned(DataField::from_chars("B2", "h2")),
            FieldStorage::from_owned(DataField::from_chars("C3", "h3")),
        ];
        let src = DataRecord::from(data);
        let mut cache = FieldQueryCache::default();

        let mut conf = r#"
        name : test
        ---
        name  = chars(wplab) ;
        X : chars =  fmt ( "{name}:{A1}-{B2}_{C3}" ,@name,@A1 , read(B2), read(C3) ) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert("oml_conf");

        let target = model.transform_async(src, &mut cache).await;

        let expect = DataField::from_chars("X".to_string(), "wplab:h1-h2_h3".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_fmt_async() {
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("A1", "h1")),
            FieldStorage::from_owned(DataField::from_chars("B2", "h2")),
            FieldStorage::from_owned(DataField::from_chars("C3", "h3")),
        ];
        let src = DataRecord::from(data);
        let mut cache = FieldQueryCache::default();

        let mut conf = r#"
        name : test
        ---
        name  = chars(wplab) ;
        X : chars =  fmt ( "{name}:{A1}-{B2}_{C3}" ,@name,@A1 , read(B2), read(C3) ) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert("oml_conf");

        let target = model.transform_async(src, &mut cache).await;

        let expect = DataField::from_chars("X".to_string(), "wplab:h1-h2_h3".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }
}
