//! 文件系统操作工具模块
//!
//! 提供统一的文件和目录操作接口，统一错误处理。

use orion_error::{ToStructError, UvsConfFrom};
use std::fs;
use std::path::{Path, PathBuf};
use wp_error::run_error::{RunReason, RunResult};

use crate::utils::error_conv::ResultExt;

/// 文件系统操作工具
pub struct FsOps;

impl FsOps {
    /// 确保目录存在，如不存在则创建
    ///
    /// # 幂等性
    ///
    /// 如果目录已存在，不执行任何操作。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// let dir = Path::new("/tmp/test_dir");
    /// FsOps::ensure_dir(dir)?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn ensure_dir<P: AsRef<Path>>(path: P) -> RunResult<()> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)
                .to_run_err_with(|_| format!("创建目录失败: {}", path.display()))?;
        }
        Ok(())
    }

    /// 安全读取文件内容
    ///
    /// # 错误
    ///
    /// - 文件不存在
    /// - 无读取权限
    /// - 非 UTF-8 内容
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// let content = FsOps::read_to_string("/path/to/file.txt")?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn read_to_string<P: AsRef<Path>>(path: P) -> RunResult<String> {
        let path = path.as_ref();
        fs::read_to_string(path).to_run_err_with(|_| format!("读取文件失败: {}", path.display()))
    }

    /// 安全写入文件内容
    ///
    /// 自动创建父目录。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// FsOps::write("/path/to/file.txt", "content")?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn write<P: AsRef<Path>>(path: P, content: &str) -> RunResult<()> {
        let path = path.as_ref();

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            Self::ensure_dir(parent)?;
        }

        fs::write(path, content).to_run_err_with(|_| format!("写入文件失败: {}", path.display()))
    }

    /// 安全写入文件（带备份）
    ///
    /// 如果文件已存在，创建 .bak 备份。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// FsOps::write_with_backup("/path/to/file.txt", "new content")?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn write_with_backup<P: AsRef<Path>>(path: P, content: &str) -> RunResult<()> {
        let path = path.as_ref();

        // 如果文件已存在，先备份
        if path.exists() {
            let backup_path = path.with_extension("bak");
            fs::copy(path, &backup_path)
                .to_run_err_with(|_| format!("创建备份失败: {}", backup_path.display()))?;
        }

        Self::write(path, content)
    }

    /// 查找符合模式的配置文件
    ///
    /// # 参数
    ///
    /// - `dir`: 搜索目录
    /// - `pattern`: Glob 模式（如 "*.toml", "*.wpl"）
    ///
    /// # 返回
    ///
    /// 匹配的文件路径列表，按修改时间排序。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// let toml_files = FsOps::find_files("/path/to/dir", "*.toml")?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn find_files<P: AsRef<Path>>(dir: P, pattern: &str) -> RunResult<Vec<PathBuf>> {
        let dir = dir.as_ref();

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let search_pattern = format!("{}/{}", dir.display(), pattern);
        let entries = glob::glob(&search_pattern)
            .map_err(|e| RunReason::from_conf(format!("Glob 模式错误: {}", e)).to_err())?;

        let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).collect();

        // 按修改时间排序
        files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());

        Ok(files)
    }

    /// 检查文件是否存在且不为空
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use wp_proj::utils::fs::FsOps;
    /// # use std::path::Path;
    /// if FsOps::file_not_empty("/path/to/file.txt")? {
    ///     println!("File has content");
    /// }
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn file_not_empty<P: AsRef<Path>>(path: P) -> RunResult<bool> {
        let path = path.as_ref();

        if !path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(path)
            .to_run_err_with(|_| format!("获取文件信息失败: {}", path.display()))?;

        Ok(metadata.len() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::temp_workdir;

    #[test]
    fn test_ensure_dir_creates_directory() {
        let temp = temp_workdir();
        let dir = temp.path().join("test_dir");

        assert!(!dir.exists());
        FsOps::ensure_dir(&dir).unwrap();
        assert!(dir.exists());
    }

    #[test]
    fn test_ensure_dir_is_idempotent() {
        let temp = temp_workdir();
        let dir = temp.path().join("test_dir");

        FsOps::ensure_dir(&dir).unwrap();
        FsOps::ensure_dir(&dir).unwrap(); // 第二次调用应该成功
        assert!(dir.exists());
    }

    #[test]
    fn test_write_creates_parent_dirs() {
        let temp = temp_workdir();
        let file = temp.path().join("nested/dirs/file.txt");

        FsOps::write(&file, "test content").unwrap();
        assert!(file.exists());

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_write_with_backup() {
        let temp = temp_workdir();
        let file = temp.path().join("file.txt");

        // 第一次写入
        FsOps::write(&file, "original").unwrap();

        // 第二次写入（应创建备份）
        FsOps::write_with_backup(&file, "updated").unwrap();

        let backup = temp.path().join("file.bak");
        assert!(backup.exists());

        let backup_content = fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_content, "original");

        let current_content = fs::read_to_string(&file).unwrap();
        assert_eq!(current_content, "updated");
    }

    #[test]
    fn test_find_files() {
        let temp = temp_workdir();
        let dir = temp.path().join("configs");
        FsOps::ensure_dir(&dir).unwrap();

        // 创建测试文件
        FsOps::write(dir.join("app.toml"), "").unwrap();
        FsOps::write(dir.join("db.toml"), "").unwrap();
        FsOps::write(dir.join("readme.md"), "").unwrap();

        let toml_files = FsOps::find_files(&dir, "*.toml").unwrap();
        assert_eq!(toml_files.len(), 2);
    }

    #[test]
    fn test_file_not_empty() {
        let temp = temp_workdir();
        let empty_file = temp.path().join("empty.txt");
        let non_empty_file = temp.path().join("non_empty.txt");

        FsOps::write(&empty_file, "").unwrap();
        FsOps::write(&non_empty_file, "content").unwrap();

        assert_eq!(FsOps::file_not_empty(&empty_file).unwrap(), false);
        assert_eq!(FsOps::file_not_empty(&non_empty_file).unwrap(), true);
    }

    #[test]
    fn test_read_to_string() {
        let temp = temp_workdir();
        let file = temp.path().join("test.txt");

        FsOps::write(&file, "hello world").unwrap();
        let content = FsOps::read_to_string(&file).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_find_files_returns_empty_for_nonexistent_dir() {
        let temp = temp_workdir();
        let nonexistent = temp.path().join("nonexistent");

        let files = FsOps::find_files(&nonexistent, "*.txt").unwrap();
        assert!(files.is_empty());
    }
}
