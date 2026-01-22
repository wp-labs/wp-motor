use std::path::Path;

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub work_root: String,
    pub what: String,
    pub console: bool,
    pub fail_fast: bool,
    pub json: bool,
    pub only_fail: bool,
}

impl CheckOptions {
    pub fn new<P: AsRef<Path>>(work_root: P) -> Self {
        Self {
            work_root: work_root.as_ref().to_string_lossy().to_string(),
            ..Default::default()
        }
    }
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            work_root: String::new(),
            what: String::new(),
            console: false,
            fail_fast: false,
            json: false,
            only_fail: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CheckComponents {
    pub engine: bool,
    pub sources: bool,
    pub connectors: bool,
    pub sinks: bool,
    pub wpl: bool,
    pub oml: bool,
}

impl CheckComponents {
    pub fn disable_all(&mut self) {
        self.engine = false;
        self.sources = false;
        self.connectors = false;
        self.sinks = false;
        self.wpl = false;
        self.oml = false;
    }

    pub fn enable<I>(&mut self, components: I)
    where
        I: IntoIterator<Item = CheckComponent>,
    {
        for component in components {
            self.set(component, true);
        }
    }

    pub fn with_only<I>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = CheckComponent>,
    {
        self.disable_all();
        self.enable(components);
        self
    }

    pub fn is_enabled(&self, component: CheckComponent) -> bool {
        match component {
            CheckComponent::Engine => self.engine,
            CheckComponent::Sources => self.sources,
            CheckComponent::Connectors => self.connectors,
            CheckComponent::Sinks => self.sinks,
            CheckComponent::Wpl => self.wpl,
            CheckComponent::Oml => self.oml,
        }
    }

    fn set(&mut self, component: CheckComponent, value: bool) {
        match component {
            CheckComponent::Engine => self.engine = value,
            CheckComponent::Sources => self.sources = value,
            CheckComponent::Connectors => self.connectors = value,
            CheckComponent::Sinks => self.sinks = value,
            CheckComponent::Wpl => self.wpl = value,
            CheckComponent::Oml => self.oml = value,
        }
    }
}

impl Default for CheckComponents {
    fn default() -> Self {
        Self {
            engine: true,
            sources: true,
            connectors: true,
            sinks: true,
            wpl: true,
            oml: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckComponent {
    Engine,
    Sources,
    Connectors,
    Sinks,
    Wpl,
    Oml,
}
