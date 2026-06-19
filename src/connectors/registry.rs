//! Compatibility forwarding layer.
//!
//! Runtime connector registry has been moved to `wp_core_connectors`.
//! Keep this module so existing engine/proj callsites do not need to change.

pub use wp_core_connectors::registry::*;
