use crate::ast::AnnFun;
use crate::{WparseError, WparseReason};
use orion_error::{ToStructError, UvsDataFrom};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use wp_connector_api::SourceEvent;
use wp_model_core::model::{DataField, DataRecord};
use wp_parse_api::RawData;

pub trait AnnotationFunc {
    fn proc(&self, src: &SourceEvent, data: &mut DataRecord) -> Result<(), WparseError>;
}

#[derive(Clone, Debug)]
pub struct TagAnnotation {
    args: BTreeMap<SmolStr, SmolStr>,
}

impl AnnotationFunc for TagAnnotation {
    fn proc(&self, _src: &SourceEvent, data: &mut DataRecord) -> Result<(), WparseError> {
        for (key, val) in &self.args {
            data.append(DataField::from_chars(key.clone(), val.clone()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NoopAnnotation;

impl AnnotationFunc for NoopAnnotation {
    fn proc(&self, _src: &SourceEvent, _data: &mut DataRecord) -> Result<(), WparseError> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RawCopy {
    raw_key: SmolStr,
}

impl AnnotationFunc for RawCopy {
    fn proc(&self, src: &SourceEvent, data: &mut DataRecord) -> Result<(), WparseError> {
        match &src.payload {
            RawData::String(raw) => {
                data.append(DataField::from_chars(self.raw_key.clone(), raw.clone()));
            }
            RawData::Bytes(raw) => match std::str::from_utf8(raw) {
                Ok(str) => {
                    data.append(DataField::from_chars(self.raw_key.clone(), str.to_string()));
                }
                Err(e) => {
                    return Err(WparseReason::from_data(
                        format!("[u8] to string error :{}", e),
                        None,
                    )
                    .to_err());
                }
            },
            RawData::ArcBytes(raw) => match std::str::from_utf8(raw) {
                Ok(str) => {
                    data.append(DataField::from_chars(self.raw_key.clone(), str.to_string()));
                }
                Err(e) => {
                    return Err(WparseReason::from_data(
                        format!("ArcBytes to string error :{}", e),
                        None,
                    )
                    .to_err());
                }
            },
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum AnnotationType {
    Tag(TagAnnotation),
    Copy(RawCopy),
    Null(NoopAnnotation),
}

impl AnnotationFunc for AnnotationType {
    fn proc(&self, src: &SourceEvent, data: &mut DataRecord) -> Result<(), WparseError> {
        match self {
            AnnotationType::Tag(func) => func.proc(src, data),
            AnnotationType::Null(func) => func.proc(src, data),
            AnnotationType::Copy(func) => func.proc(src, data),
        }
    }
}

impl AnnotationType {
    pub fn convert(ann: &Option<AnnFun>) -> Vec<Self> {
        let mut vec = vec![];
        if let Some(ann) = ann {
            if !ann.tags.is_empty() {
                vec.push(AnnotationType::Tag(TagAnnotation {
                    args: ann.tags.clone(),
                }));
            }

            if let Some((k, v)) = &ann.copy_raw {
                if k == "name" {
                    vec.push(AnnotationType::Copy(RawCopy { raw_key: v.clone() }));
                } else {
                    vec.push(AnnotationType::Null(NoopAnnotation {}))
                }
            }
        } else {
            vec.push(AnnotationType::Null(NoopAnnotation {}))
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pkg::DEFAULT_KEY;
    use orion_error::TestAssert;
    use std::collections::BTreeMap;
    use wp_connector_api::{SourceEvent, Tags};
    use wp_model_core::model::DataRecord;
    use wp_parse_api::RawData;

    #[test]
    fn test_tag_fun() {
        let ann = AnnFun {
            tags: BTreeMap::from([("tag_1".into(), "x".into())]),
            copy_raw: None,
        };
        let tag = AnnotationType::convert(&Some(ann));
        let mut data = DataRecord::test_value();
        let src = SourceEvent::new(
            1,
            DEFAULT_KEY.to_string(),
            RawData::String("test".to_string()),
            Tags::new().into(),
        );
        tag.first().unwrap().proc(&src, &mut data).assert();
        let expected = DataField::from_chars("tag_1", "x");
        assert_eq!(data.field("tag_1").map(|s| s.as_field()), Some(&expected));
    }

    #[test]
    fn test_copy_fun() {
        let ann = AnnFun {
            tags: Default::default(),
            copy_raw: Some(("name".into(), "raw".into())),
        };
        let tag = AnnotationType::convert(&Some(ann));
        let mut data = DataRecord::test_value();
        let src = SourceEvent::new(
            1,
            DEFAULT_KEY.to_string(),
            RawData::String("test".to_string()),
            Tags::new().into(),
        );
        tag.first().unwrap().proc(&src, &mut data).unwrap();
        let expected = DataField::from_chars("raw", "test");
        assert_eq!(data.field("raw").map(|s| s.as_field()), Some(&expected));
    }
}
