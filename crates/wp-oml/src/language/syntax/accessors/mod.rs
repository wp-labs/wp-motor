pub mod direct;
pub mod nested;

use crate::language::EvaluationTarget;
use crate::language::prelude::*;

use wp_data_fmt::Json;
use wp_data_model::cache::FieldQueryCache;

use std::fmt::{Display, Formatter};

use super::functions::FunOperation;
use super::operations::record::RecordOperation;
pub use direct::*;
pub use nested::arr::ArrOperation;
#[derive(Debug, Clone, PartialEq)]
pub enum NestedAccessor {
    Field(DataField),
    Direct(RecordOperation),
    Fun(FunOperation),
    Collect(ArrOperation),
    /// Placeholder for static symbol; resolved after parsing
    StaticSymbol(String),
}

impl FieldExtractor for NestedAccessor {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            NestedAccessor::Field(o) => o.extract_one(target, src, dst),
            NestedAccessor::Direct(o) => o.extract_one(target, src, dst),
            NestedAccessor::Fun(o) => o.extract_one(target, src, dst),
            NestedAccessor::Collect(o) => o.extract_one(target, src, dst),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            NestedAccessor::Field(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Direct(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Fun(o) => o.extract_more(src, dst, cache),
            NestedAccessor::Collect(o) => o.extract_more(src, dst, cache),
            NestedAccessor::StaticSymbol(sym) => {
                panic!("unresolved static symbol during execution: {sym}")
            }
        }
    }
    fn support_batch(&self) -> bool {
        match self {
            NestedAccessor::Field(o) => o.support_batch(),
            NestedAccessor::Direct(o) => o.support_batch(),
            NestedAccessor::Fun(o) => o.support_batch(),
            NestedAccessor::Collect(o) => o.support_batch(),
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
            NestedAccessor::Direct(x) => {
                write!(f, "{}", x)
            }
            NestedAccessor::Collect(x) => {
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
    Fun(FunOperation),
    StaticSymbol(String),
}

impl Display for GenericAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GenericAccessor::Field(x) => {
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

impl FieldExtractor for CondAccessor {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_one(target, src, dst),
            CondAccessor::Fun(x) => x.extract_one(target, src, dst),
            CondAccessor::Val(x) => x.extract_one(target, src, dst),
            CondAccessor::SqlFn(_x) => None, // SQL function is printed inline; params are collected separately
        }
    }
    fn extract_more(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        match self {
            CondAccessor::Tdc(x) => x.extract_more(src, dst, cache),
            CondAccessor::Fun(x) => x.extract_more(src, dst, cache),
            CondAccessor::Val(x) => x.extract_more(src, dst, cache),
            CondAccessor::SqlFn(_x) => Vec::new(),
        }
    }
    fn support_batch(&self) -> bool {
        match self {
            CondAccessor::Tdc(x) => x.support_batch(),
            CondAccessor::Fun(x) => x.support_batch(),
            CondAccessor::Val(x) => x.support_batch(),
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
                write!(f, "{}", json_fmt.fmt_value(x))
            }
            CondAccessor::SqlFn(x) => {
                let (sql, _params) = x.to_sql_and_params();
                write!(f, "{}", sql)
            }
        }
    }
}

impl CondAccessor {
    pub fn diy_fmt(&self, fmt: &impl DataFormat<Output = String>) -> String {
        match self {
            CondAccessor::Tdc(x) => format!("{}", x),
            CondAccessor::Fun(x) => format!("{}", x),
            CondAccessor::Val(x) => fmt.fmt_value(x),
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

impl FieldExtractor for Value {
    fn extract_one(
        &self,
        target: &EvaluationTarget,
        _src: &mut DataRecordRef<'_>,
        _dst: &DataRecord,
    ) -> Option<DataField> {
        Some(DataField::new(
            DataType::Auto,
            target.safe_name(),
            self.clone(),
        ))
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
