#[derive(Debug, Clone, Default)]
pub enum RuntimeStautus {
    #[default]
    Ready,
    Freezing,
}

impl RuntimeStautus {
    pub fn is_ready(&self) -> bool {
        matches!(self, RuntimeStautus::Ready)
    }
    pub fn is_freezing(&self) -> bool {
        matches!(self, RuntimeStautus::Freezing)
    }
    pub fn freeze(&mut self) {
        *self = RuntimeStautus::Freezing;
    }
    pub fn ready(&mut self) {
        *self = RuntimeStautus::Ready;
    }
}
