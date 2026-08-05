use crate::language::PreciseEvaluator;
use crate::language::prelude::*;

/// `array { ... }` 对象数组：元素为完整的 object / 值表达式。
/// 每个元素独立求值后收集为 `Value::Array`（元素字段名不进入 JSON 输出）。
#[derive(Default, Builder, Debug, Clone, Getters)]
pub struct ObjArrayOperation {
    items: Vec<PreciseEvaluator>,
}

impl Display for ObjArrayOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, " array {{")?;
        for item in &self.items {
            writeln!(f, "{} ;", item)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl ObjArrayOperation {
    pub fn new(items: Vec<PreciseEvaluator>) -> Self {
        Self { items }
    }

    pub fn items_mut(&mut self) -> &mut Vec<PreciseEvaluator> {
        &mut self.items
    }
}
