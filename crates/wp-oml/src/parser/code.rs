use derive_getters::Getters;

use wp_error::parse_error::{OMLCodeError, OMLCodeReason, OMLCodeResult, OmlCodeResult};
use wp_parser::comment::CommentParser;

use crate::{
    core::ConfADMExt,
    language::{DataModel, ObjModel},
};

#[derive(Debug, Clone, Getters)]
pub struct OMLCode {
    path: String,
    code: String,
}

impl From<(String, String)> for OMLCode {
    fn from(v: (String, String)) -> Self {
        Self::build(v.0, v.1.as_str()).expect("comment error")
    }
}

impl From<(String, &str)> for OMLCode {
    fn from(v: (String, &str)) -> Self {
        Self::build(v.0, v.1).expect("comment error")
    }
}

impl OMLCode {
    pub fn build(path: String, code: &str) -> OmlCodeResult<Self> {
        let mut in_code = code;
        let pure_code = CommentParser::ignore_comment(&mut in_code).map_err(|e| {
            OMLCodeError::from(OMLCodeReason::Syntax(format!("comment proc error: {}", e)))
        })?;
        Ok(Self {
            path,
            code: pure_code,
        })
    }

    pub fn load(path: &str) -> OMLCodeResult<DataModel> {
        debug_rule!("{} will load", path);
        if std::path::Path::new(path).exists() && path.ends_with(".oml") {
            Ok(DataModel::Object(ObjModel::load(path)?))
        } else {
            warn_rule!("{} not exists !", path);
            Ok(DataModel::use_null())
        }
    }
}
