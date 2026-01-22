//! Component Base Structure
//!
//! This module provides the `ComponentBase` struct that encapsulates common
//! fields and behavior shared by all wp-proj components.
//!
//! ## Design Rationale
//!
//! All major components (Sources, Sinks, Wpl, Oml) share the same base structure:
//! - `work_root`: The project's working directory
//! - `eng_conf`: The engine configuration
//!
//! Instead of duplicating these fields and their associated methods in every
//! component, we provide `ComponentBase` as a composable building block.
//!
//! ## Usage Pattern
//!
//! Components should use `ComponentBase` through composition:
//!
//! ```rust
//! use wp_proj::traits::ComponentBase;
//! use std::sync::Arc;
//! use std::path::Path;
//! use wp_conf::engine::EngineConfig;
//!
//! pub struct MyComponent {
//!     base: ComponentBase,
//!     // component-specific fields...
//! }
//!
//! impl MyComponent {
//!     pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
//!         Self {
//!             base: ComponentBase::new(work_root, eng_conf),
//!         }
//!     }
//! }
//!
//! // Use Deref to access ComponentBase methods seamlessly
//! impl std::ops::Deref for MyComponent {
//!     type Target = ComponentBase;
//!     fn deref(&self) -> &Self::Target {
//!         &self.base
//!     }
//! }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_conf::engine::EngineConfig;

use crate::utils::PathResolvable;

/// Base structure for all wp-proj components
///
/// Encapsulates the core configuration that every component needs:
/// - Working directory (`work_root`)
/// - Engine configuration (`eng_conf`)
///
/// Components should compose this struct rather than duplicating these fields.
///
/// # Examples
///
/// ```rust
/// use wp_proj::traits::ComponentBase;
/// use std::sync::Arc;
/// use wp_conf::engine::EngineConfig;
///
/// // Create a component base
/// let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
/// let base = ComponentBase::new("/tmp/test", eng_conf);
///
/// // Access fields
/// assert_eq!(base.work_root().to_str().unwrap(), "/tmp/test");
/// ```
#[derive(Clone)]
pub struct ComponentBase {
    work_root: PathBuf,
    eng_conf: Arc<EngineConfig>,
}

impl ComponentBase {
    /// Creates a new component base structure
    ///
    /// # Parameters
    ///
    /// - `work_root`: The project's working directory
    /// - `eng_conf`: The engine configuration (wrapped in Arc for shared ownership)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wp_proj::traits::ComponentBase;
    /// use std::sync::Arc;
    /// use wp_conf::engine::EngineConfig;
    ///
    /// let eng_conf = Arc::new(EngineConfig::init("/tmp/project"));
    /// let base = ComponentBase::new("/tmp/project", eng_conf);
    /// ```
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            work_root: work_root.as_ref().to_path_buf(),
            eng_conf,
        }
    }

    /// Returns a reference to the working directory
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use wp_proj::traits::ComponentBase;
    /// # use std::sync::Arc;
    /// # use wp_conf::engine::EngineConfig;
    /// # let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
    /// # let base = ComponentBase::new("/tmp/test", eng_conf);
    /// let work_root = base.work_root();
    /// assert_eq!(work_root.to_str().unwrap(), "/tmp/test");
    /// ```
    pub fn work_root(&self) -> &Path {
        &self.work_root
    }

    /// Returns a reference to the engine configuration
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use wp_proj::traits::ComponentBase;
    /// # use std::sync::Arc;
    /// # use wp_conf::engine::EngineConfig;
    /// # let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
    /// # let base = ComponentBase::new("/tmp/test", eng_conf.clone());
    /// let conf = base.eng_conf();
    /// // Use the configuration...
    /// ```
    pub fn eng_conf(&self) -> &Arc<EngineConfig> {
        &self.eng_conf
    }

    /// Updates the engine configuration
    ///
    /// This is typically used when reloading configuration at runtime.
    ///
    /// # Parameters
    ///
    /// - `eng_conf`: The new engine configuration
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use wp_proj::traits::ComponentBase;
    /// # use std::sync::Arc;
    /// # use wp_conf::engine::EngineConfig;
    /// # let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
    /// # let mut base = ComponentBase::new("/tmp/test", eng_conf);
    /// let new_conf = Arc::new(EngineConfig::init("/tmp/test"));
    /// base.update_engine_conf(new_conf);
    /// ```
    pub fn update_engine_conf(&mut self, eng_conf: Arc<EngineConfig>) {
        self.eng_conf = eng_conf;
    }

    /// Resolves a path relative to the working directory
    ///
    /// # Parameters
    ///
    /// - `relative_path`: Path relative to the working directory
    ///
    /// # Returns
    ///
    /// The absolute path by joining `work_root` with `relative_path`
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use wp_proj::traits::ComponentBase;
    /// # use std::sync::Arc;
    /// # use wp_conf::engine::EngineConfig;
    /// # let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
    /// # let base = ComponentBase::new("/tmp/test", eng_conf);
    /// let config_path = base.resolve_path("config/app.toml");
    /// assert!(config_path.to_str().unwrap().contains("config/app.toml"));
    /// ```
    pub fn resolve_path<P: AsRef<Path>>(&self, relative_path: P) -> PathBuf {
        self.work_root.join(relative_path)
    }
}

// Automatically implement PathResolvable for ComponentBase
impl PathResolvable for ComponentBase {
    fn work_root(&self) -> &Path {
        &self.work_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_base_creation() {
        let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
        let base = ComponentBase::new("/tmp/test", eng_conf);

        assert_eq!(base.work_root().to_str().unwrap(), "/tmp/test");
    }

    #[test]
    fn test_component_base_path_resolution() {
        let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
        let base = ComponentBase::new("/tmp/test", eng_conf);

        let resolved = base.resolve_path("config/app.toml");
        assert_eq!(resolved.to_str().unwrap(), "/tmp/test/config/app.toml");
    }

    #[test]
    fn test_update_engine_conf() {
        let eng_conf1 = Arc::new(EngineConfig::init("/tmp/test1"));
        let mut base = ComponentBase::new("/tmp/test", eng_conf1);

        let eng_conf2 = Arc::new(EngineConfig::init("/tmp/test2"));
        base.update_engine_conf(eng_conf2.clone());

        // Verify the configuration was updated
        assert!(Arc::ptr_eq(base.eng_conf(), &eng_conf2));
    }

    #[test]
    fn test_path_resolvable_trait() {
        let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
        let base = ComponentBase::new("/tmp/test", eng_conf);

        // Test PathResolvable trait implementation
        let work_root: &Path = PathResolvable::work_root(&base);
        assert_eq!(work_root.to_str().unwrap(), "/tmp/test");
    }

    #[test]
    fn test_component_base_clone() {
        let eng_conf = Arc::new(EngineConfig::init("/tmp/test"));
        let base1 = ComponentBase::new("/tmp/test", eng_conf);
        let base2 = base1.clone();

        assert_eq!(base1.work_root(), base2.work_root());
        assert!(Arc::ptr_eq(base1.eng_conf(), base2.eng_conf()));
    }
}
