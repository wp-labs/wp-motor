pub mod core;
mod defaults;
pub mod lint;
pub mod paths;
pub mod templates;
pub mod types;
// Re-export for convenience
pub use core::Connectors;
pub use paths::ProjectPaths;
pub use types::LintSeverity;
