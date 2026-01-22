use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use super::{PlgPipeHandle, PlgPipeInstance};
use wpl::WplEvaluator;

/// plg_pipe builder function type
pub type PlgPipeBuilder = fn(&str, WplEvaluator) -> PlgPipeHandle;

/// Global plg_pipe registry with dynamic registration support
pub struct PlgPipeRegistry {
    builders: HashMap<String, PlgPipeBuilder>,
}

impl PlgPipeRegistry {
    /// Create a new plg_pipe registry with default stub pipe
    pub fn new() -> Self {
        let mut registry = Self {
            builders: HashMap::new(),
        };
        registry.register_all([("STUB", stub_builder as PlgPipeBuilder)]);
        registry
    }

    /// Register a new plugin builder
    pub fn register(&mut self, name: &str, builder: PlgPipeBuilder) {
        self.builders.insert(name.to_ascii_uppercase(), builder);
    }

    /// Register multiple plugins at once
    pub fn register_all<I>(&mut self, plugins: I)
    where
        I: IntoIterator<Item = (&'static str, PlgPipeBuilder)>,
    {
        for (name, builder) in plugins {
            self.register(name, builder);
        }
    }

    /// Create a plugin instance by name
    pub fn create(&self, name: &str, wpl_pipe: WplEvaluator) -> Option<PlgPipeInstance> {
        let normalized = name.to_ascii_uppercase();

        match self.builders.get(&normalized) {
            Some(builder) => Some(PlgPipeInstance::new(builder(name, wpl_pipe))),
            None => Some(fallback_stub(name, wpl_pipe)),
        }
    }

    /// List all registered plugin names
    pub fn list_plugins(&self) -> Vec<&str> {
        self.builders.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PlgPipeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global plugin registry instance (thread-safe, lazily initialized)
static PLG_PIPE_REGISTRY: Lazy<Mutex<PlgPipeRegistry>> =
    Lazy::new(|| Mutex::new(PlgPipeRegistry::new()));

/// Get or create the global plugin registry
pub fn get_plg_pipe_registry() -> MutexGuard<'static, PlgPipeRegistry> {
    PLG_PIPE_REGISTRY
        .lock()
        .expect("plg_pipe registry mutex poisoned")
}

/// Convenience macro for plugin registration
#[macro_export]
macro_rules! register_plg_pipe {
    ($name:expr, $builder:expr) => {
        $crate::core::parser::plg_pipes::factory::get_plg_pipe_registry()
            .register(
                $name,
                $builder as $crate::core::parser::plg_pipes::factory::PlgPipeBuilder,
            );
    };

    // Register multiple plugins
    ($($name:expr => $builder:expr),* $(,)?) => {
        $crate::core::parser::plg_pipes::factory::get_plg_pipe_registry()
            .register_all(vec![
                $(
                    $name =>
                        $builder as $crate::core::parser::plg_pipes::factory::PlgPipeBuilder
                ),*
            ]);
    };
}

pub struct PlgPipeFactory;

impl PlgPipeFactory {
    /// Create a plugin instance using the global registry
    pub fn create(name: &str, wpl_pipe: WplEvaluator) -> Option<PlgPipeInstance> {
        let registry = get_plg_pipe_registry();
        registry.create(name, wpl_pipe)
    }

    /// List all available plugins
    pub fn list_available() -> Vec<String> {
        let registry = get_plg_pipe_registry();
        registry
            .list_plugins()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }
}

/*
// Helper functions for stub plugin
fn stub_builder(_name: &str, _vm: WplEvaluator) -> PlgPipeHandle {
    Arc::new(Mutex::new(
        Box::new(StubParser {}) as Box<dyn Parsable + Send>
    ))
}

fn fallback_stub(name: &str, vm: WplEvaluator) -> PlgPipeInstance {
    warn_ctrl!("plg_pipe '{}' not registered; falling back to stub", name);
    PlgPipeInstance::new(stub_builder(name, vm))
}

*/
