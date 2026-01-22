use wp_model_core::model::DataType;

use crate::ParserFactory;
use crate::ast::{WplRule, WplSep, WplStatementType};
use crate::generator::{FmtField, FmtFieldVec, GenChannel, NamedFieldGF};
use crate::types::AnyResult;

type FieldGenFn = Box<dyn Fn(&mut GenChannel) -> AnyResult<FmtField> + Send + Sync>;

/// A precompiled generator for a whole rule (sequence of fields).
pub struct CompiledRule {
    fields: Vec<FieldGenFn>,
}

impl CompiledRule {
    pub(crate) fn new(fields: Vec<FieldGenFn>) -> Self {
        Self { fields }
    }
    /// Generate one record (Vec of fields).
    pub fn gen_one(&self) -> AnyResult<FmtFieldVec> {
        let mut ch = GenChannel::new();
        let mut out = FmtFieldVec::new();
        for f in &self.fields {
            out.push(f(&mut ch)?);
        }
        Ok(out)
    }
    /// Generate `count` records; start index is unused but kept for API parity.
    pub fn gen_batch(&self, _idx_begin: usize, count: usize) -> AnyResult<Vec<FmtFieldVec>> {
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            v.push(self.gen_one()?);
        }
        Ok(v)
    }
}

/// Compile a WPL rule with optional field generation configs into a precompiled generator.
pub fn compile_rule(
    rule: &WplRule,
    fields: &NamedFieldGF,
) -> crate::parser::error::WplCodeResult<CompiledRule> {
    match &rule.statement {
        WplStatementType::Express(expr) => {
            let mut items: Vec<FieldGenFn> = Vec::new();
            let ups_sep = WplSep::default();
            for group in &expr.group {
                let sep = group.resolve_sep(&ups_sep);
                for f_conf in &group.fields {
                    let gconf = f_conf
                        .name
                        .as_ref()
                        .and_then(|n| fields.get(n.as_str()))
                        .cloned();
                    let f_conf_cloned = f_conf.clone();
                    let sep_cloned = sep.clone();
                    let field_fn: FieldGenFn = Box::new(move |ch: &mut GenChannel| {
                        let meta = DataType::from(f_conf_cloned.meta_name.as_str())?;
                        let parser = ParserFactory::create(&meta)?;
                        let f = parser.generate(ch, &sep_cloned, &f_conf_cloned, gconf.as_ref())?;
                        Ok(f)
                    });
                    items.push(field_fn);
                }
            }
            Ok(CompiledRule::new(items))
        }
    }
}
