use derive_getters::Getters;
use oml::language::DataModel;

#[derive(Getters, Clone, Default)]
pub struct SinkResUnit {
    aggregate_mdl: Vec<DataModel>,
}

impl SinkResUnit {
    pub fn push_model(&mut self, mdl: DataModel) {
        self.aggregate_mdl.push(mdl)
    }
    pub fn use_null() -> Self {
        Self {
            aggregate_mdl: Vec::new(),
        }
    }
}
