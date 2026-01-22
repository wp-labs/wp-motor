//! Global registry for connector kind adapters (engine-side composition root).
//!
//! This replaces the former registry in `wp-connector-api` so the API crate
//! remains pure (traits/types only). Engines and apps should register their
//! adapters here during startup.

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::RwLock;
use wp_connector_api::ConnectorKindAdapter;

type Reg = RwLock<HashMap<&'static str, &'static dyn ConnectorKindAdapter>>;
static REG: OnceCell<Reg> = OnceCell::new();

fn registry() -> &'static Reg {
    REG.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn register_adapter(adapter: &'static dyn ConnectorKindAdapter) {
    if let Ok(mut w) = registry().write() {
        w.insert(adapter.kind(), adapter);
    } else {
        log::error!(
            "adapter registry poisoned; register adapter '{}' failed",
            adapter.kind()
        );
    }
}

pub fn get_adapter(kind: &str) -> Option<&'static dyn ConnectorKindAdapter> {
    registry().read().ok().and_then(|r| r.get(kind).copied())
}

pub fn list_kinds() -> Vec<&'static str> {
    registry()
        .read()
        .ok()
        .map(|r| r.keys().copied().collect())
        .unwrap_or_default()
}

#[cfg(any(test, feature = "dev-tools"))]
pub fn clear_all() {
    if let Ok(mut w) = registry().write() {
        w.clear();
    }
}
