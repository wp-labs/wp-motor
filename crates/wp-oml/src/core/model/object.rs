use super::super::{ConfADMExt, DataTransformer};
use crate::core::diagnostics;
use crate::core::evaluator::traits::ExpEvaluator;
use crate::core::prelude::*;
use crate::language::ObjModel;
use crate::parser::error::OMLCodeErrorTait;
use crate::parser::oml_parse_raw;
use orion_error::{ContextRecord, ErrorOwe, ErrorWith, WithContext};
use wp_data_model::cache::FieldQueryCache;
use wp_error::parse_error::{OMLCodeError, OMLCodeReason, OMLCodeResult};
use wp_model_core::model::DataRecord;
use wp_parser::comment::CommentParser;

impl DataTransformer for ObjModel {
    fn transform(&self, data: DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        self.transform_ref(&data, cache)
    }

    fn transform_ref(&self, data: &DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        diagnostics::reset();
        let mut out = DataRecord::default();
        let mut tdo_ref = DataRecordRef::from(data);
        for ado in &self.items {
            ado.eval_proc(&mut tdo_ref, &mut out, cache);
        }
        debug_data!("{} convert crate item : {}", self.name(), self.items.len());
        out
    }

    fn append(&self, data: &mut DataRecord) {
        let empty = DataRecord::default();
        let mut src = DataRecordRef::from(&empty);
        let mut cache = FieldQueryCache::default();
        for ado in &self.items {
            ado.eval_proc(&mut src, data, &mut cache);
        }
    }
}

impl ConfADMExt for ObjModel {
    fn load(path: &str) -> OMLCodeResult<Self>
    where
        Self: Sized,
    {
        let mut ctx = WithContext::want("load oml model");
        ctx.record("path", path);
        let content = std::fs::read_to_string(path)
            //.owe_rule::<OMLCodeError>()
            .owe(OMLCodeReason::NotFound("oml load fail".into()))
            .with(&ctx)?;
        let mut raw_code = content.as_str();
        let code = CommentParser::ignore_comment(&mut raw_code)
            .map_err(|e| OMLCodeError::from_syntax(e, raw_code, path))?;
        let mut pure_code = code.as_str();
        match oml_parse_raw(&mut pure_code) {
            Ok(res) => Ok(res),
            Err(e) => Err(OMLCodeError::from_syntax(e, pure_code, path)).with(&ctx),
        }
    }
}
