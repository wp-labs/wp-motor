pub mod direct;
pub mod nested;

use crate::core::AsyncFieldExtractor;
use crate::language::EvaluationTarget;
use crate::language::PiPeOperation;
use crate::language::prelude::*;
use async_trait::async_trait;

use wp_data_fmt::Json;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::FieldStorage;

use std::fmt::{Display, Formatter};
use std::sync::Arc;

use super::functions::FunOperation;
use super::operations::map::MapOperation;
use super::operations::obj_array::ObjArrayOperation;
use super::operations::record::RecordOperation;
pub use direct::*;
pub use nested::arr::ArrOperation;
#[derive(Debug, Clone)]
pub enum NestedAccessor {
    Field(DataField),
    /// Arc-wrapped DataField for zero-copy sharing (from static symbols)
    FieldArc(Arc<DataField>),
    Direct(RecordOperation),
    Fun(FunOperation),
    Collect(ArrOperation),
    /// 嵌套对象字面量（`object { ... }` 作为 object 子项）
    Map(MapOperation),
    /// 对象数组字面量（`array { object {...}; ... }` 作为 object 子项）
    ObjArray(ObjArrayOperation),
    /// 管道表达式（`pipe read(...) | fun | ...` 作为 object 子项）
    Pipe(PiPeOperation),
    /// Placeholder for static symbol; resolved after parsing
    StaticSymbol(String),
}

impl PartialEq for NestedAccessor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NestedAccessor::Field(a), NestedAccessor::Field(b)) => a == b,
            (NestedAccessor::FieldArc(a), NestedAccessor::FieldArc(b)) => a == b,
            (NestedAccessor::Direct(a), NestedAccessor::Direct(b)) => a == b,
            (NestedAccessor::Fun(a), NestedAccessor::Fun(b)) => a == b,
            (NestedAccessor::Collect(a), NestedAccessor::Collect(b)) => a == b,
            (NestedAccessor::StaticSymbol(a), NestedAccessor::StaticSymbol(b)) => a == b,
            (NestedAccessor::Map(a), NestedAccessor::Map(b)) => a == b,
            // 对象数组元素为 PreciseEvaluator（无 PartialEq），以规范化 Display 串比较
            (NestedAccessor::ObjArray(a), NestedAccessor::ObjArray(b)) => {
                a.to_string() == b.to_string()
            }
            // PiPeOperation 未派生 PartialEq，以规范化 Display 串比较
            (NestedAccessor::Pipe(a), NestedAccessor::Pipe(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

#[allow(dead_code)]
impl NestedAccessor {
    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // Static symbol: return Shared variant (zero-copy)
            // Skip extract_one to avoid unnecessary clone
            NestedAccessor::FieldArc(arc) => Some(FieldStorage::from_shared(arc.clone())),
            // Other variants: use default implementation
            _ => self
                .extract_one(target, src, dst)
                .map(FieldStorage::from_owned),
        }
    }

    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            NestedAccessor::Field(o) => {
                crate::language::data_field_extract_one(o, target, src, dst)
            }
            NestedAccessor::FieldArc(o) => {
                crate::language::data_field_extract_one(o.as_ref(), target, src, dst)
            }
            NestedAccessor::Direct(o) => o.extract_one(target, src, dst),
            NestedAccessor::Fun(o) => o.extract_one(target, src, dst),
            NestedAccessor::Collect(o) => o.extract_one(target, src, dst),
            NestedAccessor::Map(o) => o.extract_one(target, src, dst),
            NestedAccessor::ObjArray(o) => o.extract_one(target, src, dst),
            NestedAccessor::Pipe(o) => o.extract_one(target, src, dst),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
    pub(crate) fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            NestedAccessor::Field(o) => {
                crate::language::data_field_extract_more(o, src, dst, cache)
            }
            NestedAccessor::FieldArc(o) => {
                crate::language::data_field_extract_more(o.as_ref(), src, dst, cache)
            }
            NestedAccessor::Direct(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Fun(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Collect(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Map(o) => o.extract_more(src, dst, cache),
            NestedAccessor::ObjArray(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Pipe(o) => o.extract_more(src, dst, cache),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
    pub(crate) fn support_batch(&self) -> bool {
        match self {
            NestedAccessor::Field(o) => crate::language::data_field_support_batch(o),
            NestedAccessor::FieldArc(o) => crate::language::data_field_support_batch(o.as_ref()),
            NestedAccessor::Direct(o) => o.support_batch(),
            NestedAccessor::Fun(o) => o.support_batch(),
            NestedAccessor::Collect(o) => o.support_batch(),
            NestedAccessor::Map(o) => o.support_batch(),
            NestedAccessor::ObjArray(o) => o.support_batch(),
            NestedAccessor::Pipe(o) => o.support_batch(),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}

#[async_trait]
impl AsyncFieldExtractor for NestedAccessor {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            NestedAccessor::Field(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::FieldArc(o) => o.as_ref().extract_one_async(target, src, dst).await,
            NestedAccessor::Direct(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::Fun(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::Collect(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::Map(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::ObjArray(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::Pipe(o) => o.extract_one_async(target, src, dst).await,
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            NestedAccessor::Field(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::FieldArc(o) => o.as_ref().extract_storage_async(target, src, dst).await,
            NestedAccessor::Direct(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::Fun(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::Collect(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::Map(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::ObjArray(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::Pipe(o) => o.extract_storage_async(target, src, dst).await,
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            NestedAccessor::Field(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::FieldArc(o) => o.as_ref().extract_more_async(src, dst, cache).await,
            NestedAccessor::Direct(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::Fun(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::Collect(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::Map(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::ObjArray(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::Pipe(o) => o.extract_more_async(src, dst, cache).await,
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }

    fn support_batch_async(&self) -> bool {
        match self {
            NestedAccessor::Field(o) => o.support_batch_async(),
            NestedAccessor::FieldArc(o) => o.as_ref().support_batch_async(),
            NestedAccessor::Direct(o) => o.support_batch_async(),
            NestedAccessor::Fun(o) => o.support_batch_async(),
            NestedAccessor::Collect(o) => o.support_batch_async(),
            NestedAccessor::Map(o) => o.support_batch_async(),
            NestedAccessor::ObjArray(o) => o.support_batch_async(),
            NestedAccessor::Pipe(o) => o.support_batch_async(),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
}

impl Display for NestedAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NestedAccessor::Field(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::FieldArc(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Direct(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Collect(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Map(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::ObjArray(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Pipe(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Fun(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::StaticSymbol(sym) => {
                write!(f, "{}", sym)
            }
        }
    }
}

impl NestedAccessor {
    pub fn replace_with_field(&mut self, field: DataField) {
        *self = NestedAccessor::Field(field);
    }

    pub fn replace_with_field_arc(&mut self, field: Arc<DataField>) {
        *self = NestedAccessor::FieldArc(field);
    }

    pub fn as_static_symbol(&self) -> Option<&str> {
        if let NestedAccessor::StaticSymbol(sym) = self {
            Some(sym.as_str())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirectAccessor {
    Take(FieldTake),
    Read(FieldRead),
}

impl VarAccess for DirectAccessor {
    fn field_name(&self) -> &Option<String> {
        match self {
            DirectAccessor::Take(o) => o.field_name(),
            DirectAccessor::Read(o) => o.field_name(),
        }
    }
}

impl Display for DirectAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectAccessor::Take(o) => {
                write!(f, "{}", o)
            }
            DirectAccessor::Read(o) => {
                write!(f, "{}", o)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenericAccessor {
    Field(DataField),
    /// Arc-wrapped DataField for zero-copy sharing (from static symbols)
    FieldArc(Arc<DataField>),
    Fun(FunOperation),
    StaticSymbol(String),
}

impl Display for GenericAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GenericAccessor::Field(x) => {
                write!(f, "{}", x)
            }
            GenericAccessor::FieldArc(x) => {
                write!(f, "{}", x)
            }
            GenericAccessor::Fun(x) => {
                write!(f, "{}", x)
            }
            GenericAccessor::StaticSymbol(sym) => {
                write!(f, "{}", sym)
            }
        }
    }
}

impl GenericAccessor {
    pub fn replace_with_field(&mut self, field: DataField) {
        *self = GenericAccessor::Field(field);
    }

    pub fn replace_with_field_arc(&mut self, field: Arc<DataField>) {
        *self = GenericAccessor::FieldArc(field);
    }

    pub fn as_static_symbol(&self) -> Option<&str> {
        if let GenericAccessor::StaticSymbol(sym) = self {
            Some(sym.as_str())
        } else {
            None
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum CondAccessor {
    Tdc(RecordOperation),
    Fun(FunOperation),
    Val(Value),
    /// SQL function call expression embedded in WHERE (e.g., ip4_between(:ip, col_a, col_b))
    SqlFn(SqlFnExpr),
}

#[allow(dead_code)]
impl CondAccessor {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_one(target, src, dst),
            CondAccessor::Fun(x) => x.extract_one(target, src, dst),
            CondAccessor::Val(x) => crate::language::value_extract_one(x, target, src, dst),
            CondAccessor::SqlFn(_x) => None, // SQL function is printed inline; params are collected separately
        }
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
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_more(src, dst, cache),
            CondAccessor::Fun(x) => x.extract_more(src, dst, cache),
            CondAccessor::Val(x) => crate::language::value_extract_more(x, src, dst, cache),
            CondAccessor::SqlFn(_x) => Vec::new(),
        }
    }
    pub(crate) fn support_batch(&self) -> bool {
        match self {
            CondAccessor::Tdc(x) => x.support_batch(),
            CondAccessor::Fun(x) => x.support_batch(),
            CondAccessor::Val(x) => crate::language::value_support_batch(x),
            CondAccessor::SqlFn(_x) => false,
        }
    }
}

#[async_trait]
impl AsyncFieldExtractor for CondAccessor {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_one_async(target, src, dst).await,
            CondAccessor::Fun(x) => x.extract_one_async(target, src, dst).await,
            CondAccessor::Val(x) => x.extract_one_async(target, src, dst).await,
            CondAccessor::SqlFn(_x) => None,
        }
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            CondAccessor::Tdc(x) => x.extract_storage_async(target, src, dst).await,
            CondAccessor::Fun(x) => x.extract_storage_async(target, src, dst).await,
            CondAccessor::Val(x) => x.extract_storage_async(target, src, dst).await,
            CondAccessor::SqlFn(_x) => None,
        }
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_more_async(src, dst, cache).await,
            CondAccessor::Fun(x) => x.extract_more_async(src, dst, cache).await,
            CondAccessor::Val(x) => x.extract_more_async(src, dst, cache).await,
            CondAccessor::SqlFn(_x) => Vec::new(),
        }
    }

    fn support_batch_async(&self) -> bool {
        match self {
            CondAccessor::Tdc(x) => x.support_batch_async(),
            CondAccessor::Fun(x) => x.support_batch_async(),
            CondAccessor::Val(x) => x.support_batch_async(),
            CondAccessor::SqlFn(_x) => false,
        }
    }
}
impl Display for CondAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CondAccessor::Tdc(x) => {
                write!(f, "{}", x)
            }
            CondAccessor::Fun(x) => {
                write!(f, "{}", x)
            }
            CondAccessor::Val(x) => {
                let json_fmt = Json;
                write!(f, "{}", json_fmt.format_value(x))
            }
            CondAccessor::SqlFn(x) => {
                let (sql, _params) = x.to_sql_and_params();
                write!(f, "{}", sql)
            }
        }
    }
}

impl CondAccessor {
    pub fn diy_fmt(&self, fmt: &impl ValueFormatter<Output = String>) -> String {
        match self {
            CondAccessor::Tdc(x) => format!("{}", x),
            CondAccessor::Fun(x) => format!("{}", x),
            CondAccessor::Val(x) => fmt.format_value(x),
            CondAccessor::SqlFn(x) => {
                let (sql, _params) = x.to_sql_and_params();
                sql
            }
        }
    }
}

impl CondAccessor {
    pub fn from_read(name: String) -> Self {
        Self::Tdc(RecordOperation::new(DirectAccessor::Read(FieldRead::new(
            name,
        ))))
    }
}

#[async_trait]
impl AsyncFieldExtractor for Value {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        crate::language::value_extract_one(self, target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        crate::language::value_extract_storage(self, target, src, dst)
    }

    async fn extract_more_async(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        crate::language::value_extract_more(self, _src, _dst, _cache)
    }
}
// ---------------- SQL Function Expression (for WHERE) ----------------
use wp_model_core::model::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum SqlFnArg {
    /// SQL column identifier (printed as-is)
    Column(String),
    /// SQL literal (quoted or numeric)
    Literal(Value),
    /// A dynamic param (e.g., read()/take()) to be bound as a named parameter
    Param(Box<CondAccessor>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlFnExpr {
    pub name: String,
    pub args: Vec<SqlFnArg>,
}

impl SqlFnExpr {
    /// Render to SQL string and collect named parameters used inside.
    /// Param names are derived from inner accessors (e.g., read(src_ip) -> :src_ip).
    pub fn to_sql_and_params(&self) -> (String, std::collections::HashMap<String, CondAccessor>) {
        use std::collections::HashMap;
        let mut params: HashMap<String, CondAccessor> = HashMap::new();
        let mut parts: Vec<String> = Vec::with_capacity(self.args.len());
        for a in &self.args {
            match a {
                SqlFnArg::Column(c) => parts.push(c.to_string()),
                SqlFnArg::Literal(v) => parts.push(render_sql_literal(v)),
                SqlFnArg::Param(acc) => {
                    let key = derive_param_name(acc.as_ref());
                    // avoid duplicates; last wins is fine as they should be equal
                    let inner: CondAccessor = acc.as_ref().clone();
                    params.insert(key.clone(), inner);
                    parts.push(format!(":{}", key));
                }
            }
        }
        let sql = format!("{}({})", self.name, parts.join(","));
        (sql, params)
    }
}

fn render_sql_literal(v: &Value) -> String {
    match v {
        Value::Digit(d) => d.to_string(),
        Value::Float(f) => {
            // avoid scientific for simple cases
            if f.fract() == 0.0 {
                format!("{:.0}", f)
            } else {
                f.to_string()
            }
        }
        Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Chars(s) => {
            // single-quote with doubling quotes
            let esc = s.replace('\'', "''");
            format!("'{}'", esc)
        }
        _ => format!("'{}'", v), // fallback via Display
    }
}

fn derive_param_name(acc: &CondAccessor) -> String {
    // Prefer underlying var name (read/take get name). Fallback to generic key.
    match acc {
        CondAccessor::Tdc(op) => {
            if let Some(n) = op.dat_get().field_name() {
                sanitize_param_name(n)
            } else {
                "p".to_string()
            }
        }
        CondAccessor::Fun(_) => "_fun".to_string(),
        CondAccessor::Val(_) => "_val".to_string(),
        CondAccessor::SqlFn(f) => sanitize_param_name(&f.name),
    }
}

fn sanitize_param_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch)
        } else {
            out.push('_')
        }
    }
    if out.is_empty() { "p".to_string() } else { out }
}
