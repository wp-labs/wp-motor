use chrono::Utc;
#[allow(unused_imports)]
use orion_variate::EnvDict;
use std::env;
use std::env::current_dir;
use std::fs;
use std::path::{Path, PathBuf};

pub trait ForTest<T> {
    fn test_default() -> T;
}

// 移除 #[cfg(test)] 限制，让其他 crate 的测试也能使用
impl ForTest<EnvDict> for EnvDict {
    fn test_default() -> Self {
        EnvDict::default()
    }
}

/// Resolve a deterministic target dir:
/// - honors `CARGO_TARGET_TMPDIR` / `CARGO_TARGET_DIR`
/// - falls back to `./target`
pub fn target_dir() -> PathBuf {
    if let Ok(p) = env::var("CARGO_TARGET_TMPDIR") {
        return PathBuf::from(p);
    }
    if let Ok(p) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(p);
    }
    if let Ok(current) = current_dir() {
        return current.join("target");
    }
    let mut tmp = env::temp_dir();
    tmp.push("wp-target");
    tmp
}

/// Test working directory helper, rooted at `target/test-work/<crate>/<test>-<pid>-<ts>`.
/// - Prints the directory path for easy discovery
/// - Creates/updates a `latest` symlink for quick access
/// - Cleans up on drop unless `WP_KEEP_TEST_DIR=1` or the test panicked
pub struct TestCasePath {
    path: PathBuf,
    keep: bool,
}

impl TestCasePath {
    /// Create a new work dir for a test.
    pub fn new(crate_name: &str, test_name: &str) -> anyhow::Result<Self> {
        let base = target_dir().join("test-work").join(crate_name);
        fs::create_dir_all(&base)?;
        let ts = Utc::now().timestamp_millis();
        let pid = std::process::id();
        // sanitize test name a bit (module path allowed as file name on most platforms)
        let leaf = format!("{}-{}-{}", test_name.replace(' ', "_"), pid, ts);
        let path = base.join(&leaf);
        fs::create_dir_all(&path)?;

        // update latest symlink (best effort)
        let latest = base.join("latest");
        let _ = fs::remove_file(&latest);
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&path, &latest);
        #[cfg(windows)]
        let _ = std::os::windows::fs::symlink_dir(&path, &latest);

        let keep = env::var("WP_KEEP_TEST_DIR")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        eprintln!("[test-work] {}", path.display());
        Ok(Self { path, keep })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn path_string(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
    pub fn join<P: AsRef<Path>>(&self, p: P) -> PathBuf {
        self.path.join(p)
    }

    /// Join a relative path and return an owned UTF-8 `String` path.
    /// Use this when a `String` is required by APIs.
    pub fn join_str<P: AsRef<Path>>(&self, p: P) -> String {
        self.path.join(p).to_string_lossy().to_string()
    }
}

impl Drop for TestCasePath {
    fn drop(&mut self) {
        if self.keep || std::thread::panicking() {
            return;
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Create a TestWork using current crate name and module path as test id.
#[macro_export]
macro_rules! test_work {
    () => {{ $crate::test_support::TestWork::new(env!("CARGO_PKG_NAME"), module_path!()) }};
    ($name:expr) => {{ $crate::test_support::TestWork::new(env!("CARGO_PKG_NAME"), $name) }};
}
