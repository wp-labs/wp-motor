use crate::core::AsyncFieldExtractor;
use crate::language::prelude::*;
use crate::language::syntax::accessors::nested::arr::ArrOperation;
use crate::language::syntax::functions::FunOperation;
use crate::language::syntax::operations::access_direct::AccessDirectOperation;
use crate::language::syntax::operations::calc::CalcOperation;
use crate::language::syntax::operations::fmt::FmtOperation;
use crate::language::syntax::operations::lookup::LookupOperation;
use crate::language::syntax::operations::map::MapOperation;
use crate::language::syntax::operations::matchs::MatchOperation;
use crate::language::syntax::operations::pipe::PiPeOperation;
use crate::language::syntax::operations::record::RecordOperation;
use crate::language::syntax::operations::sql::SqlQuery;
use async_trait::async_trait;
use std::sync::Arc;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, FieldStorage, Value};

#[derive(Default, Builder, Clone, Getters, Debug)]
#[builder(setter(into))]
pub struct SingleEvalExp {
    target: Vec<EvaluationTarget>,
    eval_way: PreciseEvaluator,
}

impl SingleEvalExp {
    pub fn eval_way_mut(&mut self) -> &mut PreciseEvaluator {
        &mut self.eval_way
    }
}

impl Display for SingleEvalExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first_pos = true;
        for i in self.target() {
            if first_pos {
                write!(f, "{} ", i)?;
            } else {
                write!(f, ", {} ", i)?;
            }
            first_pos = false;
        }
        write!(f, " = {} ;  ", self.eval_way)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PreciseEvaluator {
    //Query(LookupQuery),
    Sql(SqlQuery),
    Calc(CalcOperation),
    AccessDirect(AccessDirectOperation),
    Match(MatchOperation),
    Lookup(LookupOperation),
    Obj(DataField),
    /// Arc-wrapped DataField for zero-copy sharing (from static symbols)
    ObjArc(Arc<DataField>),
    Tdc(RecordOperation),
    Map(MapOperation),
    Pipe(PiPeOperation),
    Fun(FunOperation),
    Fmt(FmtOperation),
    Collect(ArrOperation),
    Val(Value),
    /// Placeholder for static DSL symbol; resolved after parsing
    StaticSymbol(String),
}

impl Default for PreciseEvaluator {
    fn default() -> Self {
        PreciseEvaluator::Tdc(RecordOperation::default())
    }
}
impl Display for PreciseEvaluator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            //PreciseEvaluator::Query(x) => Display::fmt(x, f),
            PreciseEvaluator::Calc(x) => Display::fmt(x, f),
            PreciseEvaluator::AccessDirect(x) => Display::fmt(x, f),
            PreciseEvaluator::Match(x) => Display::fmt(x, f),
            PreciseEvaluator::Lookup(x) => Display::fmt(x, f),
            PreciseEvaluator::Sql(x) => Display::fmt(x, f),
            PreciseEvaluator::Obj(x) => Display::fmt(x, f),
            PreciseEvaluator::ObjArc(x) => Display::fmt(x.as_ref(), f),
            PreciseEvaluator::Tdc(x) => Display::fmt(x, f),
            PreciseEvaluator::Map(x) => Display::fmt(x, f),
            PreciseEvaluator::Pipe(x) => Display::fmt(x, f),
            PreciseEvaluator::Fun(x) => Display::fmt(x, f),
            PreciseEvaluator::Fmt(x) => Display::fmt(x, f),
            PreciseEvaluator::Collect(x) => Display::fmt(x, f),
            PreciseEvaluator::Val(x) => Display::fmt(x, f),
            PreciseEvaluator::StaticSymbol(sym) => {
                write!(f, "{}", sym)
            }
        }
    }
}

pub(crate) fn data_field_extract_one(
    field: &DataField,
    _target: &EvaluationTarget,
    _src: &mut DataRecordRef<'_>,
    _dst: &DataRecord,
) -> Option<DataField> {
    Some(field.clone())
}

pub(crate) fn data_field_extract_storage(
    field: &DataField,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &DataRecord,
) -> Option<FieldStorage> {
    data_field_extract_one(field, target, src, dst).map(FieldStorage::from_owned)
}

pub(crate) fn data_field_extract_more(
    _field: &DataField,
    _src: &mut DataRecordRef<'_>,
    _dst: &mut DataRecord,
    _cache: &mut FieldQueryCache,
) -> Vec<DataField> {
    Vec::new()
}

pub(crate) fn data_field_support_batch(_field: &DataField) -> bool {
    false
}

pub(crate) fn value_extract_one(
    value: &Value,
    target: &EvaluationTarget,
    _src: &mut DataRecordRef<'_>,
    _dst: &DataRecord,
) -> Option<DataField> {
    Some(DataField::new(
        DataType::Auto,
        target.safe_name(),
        value.clone(),
    ))
}

pub(crate) fn value_extract_storage(
    value: &Value,
    target: &EvaluationTarget,
    src: &mut DataRecordRef<'_>,
    dst: &DataRecord,
) -> Option<FieldStorage> {
    value_extract_one(value, target, src, dst).map(FieldStorage::from_owned)
}

pub(crate) fn value_extract_more(
    _value: &Value,
    _src: &mut DataRecordRef<'_>,
    _dst: &mut DataRecord,
    _cache: &mut FieldQueryCache,
) -> Vec<DataField> {
    Vec::new()
}

pub(crate) fn value_support_batch(_value: &Value) -> bool {
    false
}

#[async_trait]
impl AsyncFieldExtractor for DataField {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        data_field_extract_one(self, target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        data_field_extract_storage(self, target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        data_field_extract_more(self, _src, _dst, _cache)
    }
}
