// This module was split into multiple files to keep responsibilities focused
// and reduce compile/view overhead. Public API remains under `core::manager`.

pub mod allocation;
mod conflicts;
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

// Route/index inspection shared by the daemon loader and `wparse check`. These
// stay crate-visible: the facade maps them to its report and never reaches into
// `ResManager` internals.
pub(crate) use conflicts::{
    BindingConflict, DuplicateGroupName, DuplicateMatcher, MatcherKind, UnmatchedBinding,
    find_binding_conflicts, find_duplicate_group_names, find_duplicate_matchers,
    find_unmatched_bindings,
};
pub(crate) use loading::{LoadedSinkGroup, LoadedSinkRoutes};
