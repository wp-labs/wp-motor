use super::super::DataTransformer;
use crate::core::prelude::*;
use crate::language::{DataModel, StubModel};

impl DataTransformer for StubModel {
    fn transform(&self, data: DataRecord, _cache: &mut FieldQueryCache) -> DataRecord {
        data
    }

    fn transform_ref(&self, data: &DataRecord, _cache: &mut FieldQueryCache) -> DataRecord {
        data.clone()
    }

    fn append(&self, _data: &mut DataRecord) {}
}
impl DataTransformer for DataModel {
    fn transform(&self, data: DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        match self {
            DataModel::Stub(null_model) => null_model.transform(data, cache),
            DataModel::Object(obj_model) => obj_model.transform(data, cache),
        }
    }

    fn transform_ref(&self, data: &DataRecord, cache: &mut FieldQueryCache) -> DataRecord {
        match self {
            DataModel::Stub(null_model) => null_model.transform_ref(data, cache),
            DataModel::Object(obj_model) => obj_model.transform_ref(data, cache),
        }
    }

    fn append(&self, data: &mut DataRecord) {
        match self {
            DataModel::Stub(null_model) => null_model.append(data),
            DataModel::Object(obj_model) => obj_model.append(data),
        }
    }
}
