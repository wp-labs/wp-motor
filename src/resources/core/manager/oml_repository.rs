use oml::parser::code::OMLCode;
use std::collections::HashMap;

#[derive(Default)]
pub struct OmlRepository {
    pub(crate) items: HashMap<String, OMLCode>,
}
impl OmlRepository {
    pub fn push(&mut self, code: OMLCode) {
        self.items.insert(code.path().clone(), code);
    }
}
