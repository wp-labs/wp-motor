use crate::{facade::test_helpers::SinkTerminal, sinks::SinkGroupAgent};

#[derive(Clone)]
pub struct DfxSink {
    moni: SinkGroupAgent,
    error: SinkGroupAgent,
}

impl DfxSink {
    pub fn new(moni: SinkGroupAgent, error: SinkGroupAgent) -> Self {
        Self { moni, error }
    }
    pub fn null() -> Self {
        Self {
            moni: SinkGroupAgent::null(),
            error: SinkGroupAgent::null(),
        }
    }
    pub fn of_error(&self) -> &SinkTerminal {
        self.error.end()
    }
    pub fn of_moni(&self) -> &SinkTerminal {
        self.moni.end()
    }
}
