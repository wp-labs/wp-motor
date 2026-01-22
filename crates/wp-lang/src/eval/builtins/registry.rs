use once_cell::sync::Lazy;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use super::PipeHold;

pub type PlgPipeUnitBuilder = fn() -> PipeHold;

#[derive(Default)]
struct PlgPipeUnitRegistry {
    builders: HashMap<SmolStr, PlgPipeUnitBuilder>,
}

impl PlgPipeUnitRegistry {
    fn register(&mut self, name: &str, builder: PlgPipeUnitBuilder) {
        self.builders
            .insert(SmolStr::from(name.to_ascii_uppercase()), builder);
    }

    fn create(&self, name: &str) -> Option<PipeHold> {
        self.builders
            .get(&SmolStr::from(name.to_ascii_uppercase()))
            .map(|builder| (builder)())
    }

    fn list(&self) -> Vec<SmolStr> {
        self.builders.keys().cloned().collect()
    }
}

static PIPE_UNIT_REGISTRY: Lazy<Mutex<PlgPipeUnitRegistry>> =
    Lazy::new(|| Mutex::new(PlgPipeUnitRegistry::default()));

fn registry() -> MutexGuard<'static, PlgPipeUnitRegistry> {
    PIPE_UNIT_REGISTRY
        .lock()
        .expect("plg_pipe unit registry poisoned")
}

pub fn register_pipe_unit(name: &str, builder: PlgPipeUnitBuilder) {
    registry().register(name, builder);
}

pub fn create_pipe_unit(name: &str) -> Option<PipeHold> {
    registry().create(name)
}

pub fn list_pipe_units() -> Vec<SmolStr> {
    registry().list()
}

#[macro_export]
macro_rules! register_wpl_pipe {
    ($name:expr, $builder:expr) => {
        $crate::eval::builtins::registry::register_pipe_unit(
            $name,
            $builder as $crate::eval::builtins::registry::PlgPipeUnitBuilder,
        );
    };

    ($($name:expr => $builder:expr),* $(,)?) => {
        $crate::eval::builtins::registry::register_pipe_unit_batch(vec![
            $(
                $name => $builder as $crate::eval::builtins::registry::PlgPipeUnitBuilder
            ),*
        ]);
    };
}

pub fn register_wpl_pipe_batch<I>(items: I)
where
    I: IntoIterator<Item = (&'static str, PlgPipeUnitBuilder)>,
{
    let mut guard = registry();
    for (name, builder) in items {
        guard.register(name, builder);
    }
}
