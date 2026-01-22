use crate::types::WildMap;

use super::field::FieldEvalUnit;

#[derive(Clone)]
pub struct SubUnitManager {
    subs_fpu: WildMap<FieldEvalUnit>,
}

impl Default for SubUnitManager {
    fn default() -> Self {
        Self::new()
    }
}
impl SubUnitManager {
    pub fn new() -> Self {
        Self {
            subs_fpu: WildMap::new(),
        }
    }

    pub fn add(&mut self, key: String, unit: FieldEvalUnit) {
        self.subs_fpu.insert(key, unit);
    }

    pub fn get(&self, key: &str) -> Option<&FieldEvalUnit> {
        self.subs_fpu.get(key)
    }
}
