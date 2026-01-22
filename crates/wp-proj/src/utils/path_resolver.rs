use std::path::{Path, PathBuf};

/// Trait for components that need to resolve relative paths against a work root.
///
/// This trait provides a common pattern for converting potentially relative paths
/// from configuration into absolute paths. If the configured path is already absolute,
/// it is used as-is. Otherwise, it is resolved relative to the work root.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::{Path, PathBuf};
/// use wp_proj::utils::PathResolvable;
///
/// struct MyComponent {
///     work_root: PathBuf,
/// }
///
/// impl PathResolvable for MyComponent {
///     fn work_root(&self) -> &Path {
///         &self.work_root
///     }
/// }
///
/// let component = MyComponent {
///     work_root: PathBuf::from("/project"),
/// };
///
/// // Absolute path is returned as-is
/// let abs = component.resolve_path("/absolute/path");
/// assert_eq!(abs, PathBuf::from("/absolute/path"));
///
/// // Relative path is joined with work_root
/// let rel = component.resolve_path("relative/path");
/// assert_eq!(rel, PathBuf::from("/project/relative/path"));
/// ```
pub trait PathResolvable {
    /// Returns the work root directory for this component.
    ///
    /// This is the base directory against which relative paths will be resolved.
    fn work_root(&self) -> &Path;

    /// Resolves a path, converting relative paths to absolute by joining with work_root.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to resolve, which may be absolute or relative
    ///
    /// # Returns
    ///
    /// An absolute `PathBuf`. If `path` is already absolute, returns it as-is.
    /// If `path` is relative, returns `work_root().join(path)`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::path::{Path, PathBuf};
    /// # use wp_proj::utils::PathResolvable;
    /// # struct Component { work_root: PathBuf }
    /// # impl PathResolvable for Component {
    /// #     fn work_root(&self) -> &Path { &self.work_root }
    /// # }
    /// let component = Component {
    ///     work_root: PathBuf::from("/home/user/project"),
    /// };
    ///
    /// // Absolute path
    /// let abs = component.resolve_path("/etc/config");
    /// assert_eq!(abs, PathBuf::from("/etc/config"));
    ///
    /// // Relative path
    /// let rel = component.resolve_path("config/app.toml");
    /// assert_eq!(rel, PathBuf::from("/home/user/project/config/app.toml"));
    /// ```
    fn resolve_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let candidate = path.as_ref();
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.work_root().join(candidate)
        }
    }
}
