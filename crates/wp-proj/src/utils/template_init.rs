use crate::utils::config_path::ConfigPathResolver;
use std::path::PathBuf;
use wp_error::RunResult;

/// Helper for initializing template files in a directory.
///
/// This struct simplifies the common pattern of creating a directory
/// and writing multiple template files to it, which is used by
/// model initialization (WPL, OML, etc.).
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use wp_proj::utils::TemplateInitializer;
///
/// let target_dir = PathBuf::from("/project/models/wpl");
/// let initializer = TemplateInitializer::new(target_dir);
///
/// // Write a single file
/// initializer.write_file("example.wpl", "wpl content here")?;
///
/// // Write multiple files at once
/// initializer.write_files(&[
///     ("parse.wpl", "parse rule content"),
///     ("sample.dat", "sample data content"),
/// ])?;
/// # Ok::<(), wp_error::RunError>(())
/// ```
pub struct TemplateInitializer {
    target_dir: PathBuf,
}

impl TemplateInitializer {
    /// Creates a new `TemplateInitializer` for the specified target directory.
    ///
    /// # Arguments
    ///
    /// * `target_dir` - The directory where template files will be created
    pub fn new(target_dir: PathBuf) -> Self {
        Self { target_dir }
    }

    /// Writes a single template file, creating the directory if needed.
    ///
    /// This method ensures the target directory exists before writing the file.
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to create (will be joined with target_dir)
    /// * `content` - Content to write to the file
    ///
    /// # Returns
    ///
    /// `RunResult<()>` indicating success or failure
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::path::PathBuf;
    /// # use wp_proj::utils::TemplateInitializer;
    /// let initializer = TemplateInitializer::new(PathBuf::from("/project/models"));
    /// initializer.write_file("example.oml", "model Example { }")?;
    /// # Ok::<(), wp_error::RunError>(())
    /// ```
    pub fn write_file(&self, filename: &str, content: &str) -> RunResult<()> {
        ConfigPathResolver::ensure_dir_exists(&self.target_dir)?;
        let file_path = self.target_dir.join(filename);
        ConfigPathResolver::write_file_with_dir(&file_path, content)?;
        Ok(())
    }

    /// Writes multiple template files at once, creating the directory if needed.
    ///
    /// This is more efficient than calling `write_file` multiple times as it
    /// only ensures the directory exists once.
    ///
    /// # Arguments
    ///
    /// * `files` - Slice of (filename, content) pairs
    ///
    /// # Returns
    ///
    /// `RunResult<()>` indicating success or failure
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::path::PathBuf;
    /// # use wp_proj::utils::TemplateInitializer;
    /// let initializer = TemplateInitializer::new(PathBuf::from("/project/rules"));
    /// initializer.write_files(&[
    ///     ("parse.wpl", "wpl parse rules"),
    ///     ("sample.dat", "sample data"),
    /// ])?;
    /// # Ok::<(), wp_error::RunError>(())
    /// ```
    pub fn write_files(&self, files: &[(&str, &str)]) -> RunResult<()> {
        ConfigPathResolver::ensure_dir_exists(&self.target_dir)?;
        for (filename, content) in files {
            let file_path = self.target_dir.join(filename);
            ConfigPathResolver::write_file_with_dir(&file_path, content)?;
        }
        Ok(())
    }
}
