// This module was split into multiple files to keep responsibilities focused
// and reduce compile/view overhead. Public API remains under `core::manager`.

pub mod allocation;
pub mod indexing;
pub mod loading;
pub mod oml_repository;
pub mod res_manager;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod flexgroup_rule_test;

// Re-export types so external modules keep importing `core::manager::*` as before.
pub use oml_repository::OmlRepository;
pub use res_manager::{ResManager, RuleMdlMapping};
