use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use std::collections::HashMap;
use strfmt::{DisplayStr, Formatter, strfmt};
use wp_data_fmt::{RecordFormatter, Raw};
use wp_model_core::model::FieldStorage;
impl FieldExtractor for FmtOperation {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
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
            if let Some(obj) = item.extract_one(&cur, src, dst) {
                args.insert(obj.get_name().to_string(), FmtVal(obj));
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
}

#[derive(Debug)]
pub struct FmtVal(pub DataField);
impl DisplayStr for FmtVal
where
//for<'a> RawFmt<&'a T>: Display,
{
    fn display_str(&self, f: &mut Formatter) -> strfmt::Result<()> {
        let raw_fmt = Raw;
        let str = raw_fmt.fmt_field(&FieldStorage::Owned(self.0.clone())).to_string();
        f.str(str.as_str())
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
    fn test_fmt() {
        let data = vec![
            FieldStorage::Owned(DataField::from_chars("A1", "h1")),
            FieldStorage::Owned(DataField::from_chars("B2", "h2")),
            FieldStorage::Owned(DataField::from_chars("C3", "h3")),
        ];
        let src = DataRecord::from(data);
        let mut cache = FieldQueryCache::default();

        let mut conf = r#"
        name : test
        ---
        name  = chars(wplab) ;
        X : chars =  fmt ( "{name}:{A1}-{B2}_{C3}" ,@name,@A1 , read(B2), read(C3) ) ;
         "#;
        let model = oml_parse_raw(&mut conf).assert("oml_conf");

        let target = model.transform(src, &mut cache);

        let expect = DataField::from_chars("X".to_string(), "wplab:h1-h2_h3".to_string());
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }
}
