use ahash::RandomState;
use std::collections::HashMap;
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

use crate::ast::WplFun;
use crate::eval::runtime::group::WplEvalGroup;

pub struct FieldIndex {
    map: HashMap<String, usize, RandomState>,
}

impl FieldIndex {
    pub fn build(fields: &[DataField]) -> Self {
        let mut map: HashMap<String, usize, RandomState> =
            HashMap::with_capacity_and_hasher(fields.len(), RandomState::default());
        for (i, f) in fields.iter().enumerate() {
            map.entry(f.get_name().to_string()).or_insert(i);
        }
        FieldIndex { map }
    }
    pub fn get(&self, name: &str) -> Option<usize> {
        self.map.get(name).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldSelectorSpec<'a> {
    Take(&'a str),
    Last,
}

impl<'a> FieldSelectorSpec<'a> {
    pub fn requires_index(&self) -> bool {
        matches!(self, FieldSelectorSpec::Take(_))
    }
}

pub trait FieldSelector {
    fn select(
        &self,
        fields: &mut Vec<DataField>,
        index: Option<&FieldIndex>,
    ) -> ModalResult<Option<usize>>;

    fn requires_index(&self) -> bool {
        false
    }
}

pub trait FieldPipe {
    fn process(&self, field: Option<&mut DataField>) -> ModalResult<()>;

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        None
    }
}

#[derive(Clone)]
pub enum PipeEnum {
    Fun(WplFun),
    Group(WplEvalGroup),
}
