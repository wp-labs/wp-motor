//! Trait definitions for wp-proj components
//!
//! This module provides the trait system that unifies all wp-proj components
//! under a common interface while allowing for component-specific capabilities.
//!
//! See [`component`] for the core trait definitions.

pub mod component;
pub mod component_base;

// Re-export main traits and types for convenience
pub use component::{Checkable, Component, ComponentLifecycle, HasExamples, HasStatistics};
pub use component_base::ComponentBase;
