//! Component trait hierarchy for wp-proj
//!
//! This module defines the core trait system that provides a unified interface
//! for all wp-proj components (Wpl, Oml, Sources, Sinks, Connectors, Knowledge).
//!
//! ## Trait Hierarchy
//!
//! - **Component**: Base trait - all components must implement this
//! - **Checkable**: For components that can be validated
//! - **HasExamples**: For components that support example initialization
//! - **HasStatistics**: For components that provide statistical operations
//!
//! ## Design Rationale
//!
//! Different components have different capabilities:
//! - **Models** (Wpl, Oml): Checkable + HasExamples
//! - **I/O** (Sources, Sinks): Checkable + HasStatistics
//! - **Connectors**: Checkable
//! - **Knowledge**: Checkable
//!
//! This trait system allows WarpProject to work with components in a unified way
//! while still allowing component-specific features.

use crate::types::CheckStatus;
use wp_error::run_error::RunResult;

/// Base trait for all wp-proj components.
///
/// Every component in the wp-proj system must implement this trait,
/// providing a consistent interface for identification.
///
/// # Examples
///
/// ```rust,no_run
/// use wp_proj::traits::Component;
///
/// struct MyComponent;
///
/// impl Component for MyComponent {
///     fn component_name(&self) -> &'static str {
///         "MyComponent"
///     }
/// }
/// ```
pub trait Component {
    /// Returns the human-readable name of this component.
    ///
    /// This should be a static string identifier like "WPL", "OML",
    /// "Sources", "Sinks", etc.
    fn component_name(&self) -> &'static str;
}

/// Trait for components that can be validated/checked.
///
/// Checkable components can verify their configuration and state,
/// returning a CheckStatus indicating whether they are properly configured.
///
/// # Examples
///
/// ```rust,no_run
/// # use wp_proj::traits::{Component, Checkable};
/// # use wp_proj::types::CheckStatus;
/// # use wp_error::run_error::RunResult;
/// #
/// struct MyCheckableComponent;
///
/// impl Component for MyCheckableComponent {
///     fn component_name(&self) -> &'static str { "Test" }
/// }
///
/// impl Checkable for MyCheckableComponent {
///     fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
///         Ok(CheckStatus::Suc)
///     }
/// }
/// ```
pub trait Checkable: Component {
    /// Checks the component's configuration and state.
    ///
    /// # Parameters
    ///
    /// - `dict` - Environment variable dictionary for configuration parsing
    ///
    /// # Returns
    ///
    /// - `Ok(CheckStatus::Suc)` - Component is properly configured
    /// - `Ok(CheckStatus::Miss)` - Component configuration is missing
    /// - `Err(RunError)` - Component has configuration errors
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus>;
}

/// Trait for components that support initialization with examples.
///
/// Components implementing this trait can generate example configuration
/// files to help users get started quickly.
///
/// # Examples
///
/// ```rust,no_run
/// # use wp_proj::traits::{Component, HasExamples};
/// # use wp_error::run_error::RunResult;
/// #
/// struct MyExampleComponent;
///
/// impl Component for MyExampleComponent {
///     fn component_name(&self) -> &'static str { "Test" }
/// }
///
/// impl HasExamples for MyExampleComponent {
///     fn init_with_examples(&self) -> RunResult<()> {
///         // Create example files...
///         Ok(())
///     }
/// }
/// ```
pub trait HasExamples: Component {
    /// Initializes the component with example configuration files.
    ///
    /// This typically creates template files that demonstrate the component's
    /// usage and provide a starting point for users.
    ///
    /// # Returns
    ///
    /// `RunResult<()>` indicating success or failure of initialization
    fn init_with_examples(&self) -> RunResult<()>;
}

/// Trait for components that provide statistical operations.
///
/// Components implementing this trait can report statistics about their
/// state, such as counts of items, file sizes, etc.
pub trait HasStatistics: Component {
    /// Returns whether this component has statistical data available.
    ///
    /// Components may not have statistics if they haven't been initialized
    /// or don't have any data to report.
    fn has_statistics(&self) -> bool {
        false // Default implementation
    }
}

/// Trait for components that support lifecycle management (initialization).
///
/// Components implementing this trait can initialize themselves, creating
/// necessary configuration files and directory structures.
///
/// # Method Semantics
///
/// - `init()`: Initialize the component, creating necessary configuration files and directories
///   - If configuration already exists, should preserve existing configuration (not overwrite)
///   - Used for first-time setup or ensuring minimal configuration exists
///   - Should be an idempotent operation (safe to call multiple times)
///
/// # Examples
///
/// ```rust,no_run
/// use wp_proj::traits::{Component, ComponentLifecycle};
/// use wp_error::run_error::RunResult;
/// use orion_variate::EnvDict;
///
/// struct MyComponent;
///
/// impl Component for MyComponent {
///     fn component_name(&self) -> &'static str { "MyComponent" }
/// }
///
/// impl ComponentLifecycle for MyComponent {
///     fn init(&self, dict: &orion_variate::EnvDict) -> RunResult<()> {
///         // Create configuration files and directories
///         // If they already exist, preserve them
///         Ok(())
///     }
/// }
/// ```
pub trait ComponentLifecycle: Component {
    /// Initializes the component, creating necessary configuration and directories
    ///
    /// # Parameters
    ///
    /// - `dict`: Environment variable dictionary for configuration template variable substitution
    ///
    /// # Behavior Contract
    ///
    /// - If configuration files already exist and are valid, should not overwrite them
    /// - Should create necessary directory structures
    /// - Should provide reasonable default configurations
    /// - Idempotent operation: multiple calls should be safe
    ///
    /// # Returns
    ///
    /// `RunResult<()>` indicating success or failure of initialization
    fn init(&self, dict: &orion_variate::EnvDict) -> RunResult<()>;
}
