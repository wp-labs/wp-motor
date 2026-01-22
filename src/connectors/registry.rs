//! Engine-side wrappers for source/sink factory registries.
//! These forward to the registries defined in wp-connector-api to avoid
//! dependency cycles from config crates back to engine.

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::panic::Location;
use std::sync::{Arc, RwLock};
use wp_conf::connectors::ConnectorDef;
use wp_connector_api::{SinkFactory, SourceFactory};

type SinkRec = (Arc<dyn SinkFactory>, &'static Location<'static>);
type SrcRec = (Arc<dyn SourceFactory>, &'static Location<'static>);
type SinkReg = RwLock<HashMap<String, SinkRec>>;
type SrcReg = RwLock<HashMap<String, SrcRec>>;
static SINKS: OnceCell<SinkReg> = OnceCell::new();
static SRCS: OnceCell<SrcReg> = OnceCell::new();

fn sink_reg() -> &'static SinkReg {
    SINKS.get_or_init(|| RwLock::new(HashMap::new()))
}
fn src_reg() -> &'static SrcReg {
    SRCS.get_or_init(|| RwLock::new(HashMap::new()))
}

// ---------- Sink ----------
#[track_caller]
pub fn register_sink_factory<F>(f: F)
where
    F: SinkFactory,
{
    let base: Arc<F> = Arc::new(f);
    let ex_arc: Arc<dyn SinkFactory> = base.clone();
    let kind = ex_arc.kind().to_string();
    if let Ok(mut w) = sink_reg().write() {
        w.insert(kind, (ex_arc.clone(), Location::caller()));
    }
}

pub fn get_sink_factory(kind: &str) -> Option<Arc<dyn SinkFactory>> {
    sink_reg()
        .read()
        .ok()
        .and_then(|r| r.get(kind).map(|(f, _)| f.clone() as Arc<dyn SinkFactory>))
}
pub fn list_sink_kinds() -> Vec<String> {
    sink_reg()
        .read()
        .ok()
        .map(|r| r.keys().cloned().collect())
        .unwrap_or_default()
}

/// Collect connector definitions from all registered sink factories.
pub fn registered_sink_defs() -> Vec<ConnectorDef> {
    sink_reg()
        .read()
        .map(|reg| reg.values().flat_map(|(f, _)| f.sink_defs()).collect())
        .unwrap_or_default()
}

// ---------- Source ----------
#[track_caller]
pub fn register_source_factory<F>(f: F)
where
    F: SourceFactory,
{
    let base: Arc<F> = Arc::new(f);
    let ex_arc: Arc<dyn SourceFactory> = base.clone();
    let kind = ex_arc.kind().to_string();
    if let Ok(mut w) = src_reg().write() {
        w.insert(kind, (ex_arc.clone(), Location::caller()));
    }
}
pub fn get_source_factory(kind: &str) -> Option<Arc<dyn SourceFactory>> {
    src_reg().read().ok().and_then(|r| {
        r.get(kind)
            .map(|(f, _)| f.clone() as Arc<dyn SourceFactory>)
    })
}
pub fn list_source_kinds() -> Vec<String> {
    src_reg()
        .read()
        .ok()
        .map(|r| r.keys().cloned().collect())
        .unwrap_or_default()
}

/// Collect connector definitions from all registered source factories.
pub fn registered_source_defs() -> Vec<ConnectorDef> {
    src_reg()
        .read()
        .map(|reg| reg.values().flat_map(|(f, _)| f.source_defs()).collect())
        .unwrap_or_default()
}

pub fn sink_diagnostics() -> Vec<(String, &'static Location<'static>)> {
    sink_reg()
        .read()
        .ok()
        .map(|r| r.iter().map(|(k, (_f, loc))| (k.clone(), *loc)).collect())
        .unwrap_or_default()
}

pub fn source_diagnostics() -> Vec<(String, &'static Location<'static>)> {
    src_reg()
        .read()
        .ok()
        .map(|r| r.iter().map(|(k, (_f, loc))| (k.clone(), *loc)).collect())
        .unwrap_or_default()
}

// ---------- Import from API registries (for compatibility) ----------
// Note: no compatibility import layer. Runtime only recognizes engine registry now.
