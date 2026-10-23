//! Runtime state directory resolution (`[runtime].state_dir`).
//!
//! Engine state (pid file, single-instance lock, rule mapping, authority DB and
//! the `project_remote` cache) used to be hard-coded under `<work_root>/.run`.
//! Deployments that mount the work root read-only, or that keep state on a
//! dedicated volume, need to move that state elsewhere.
//!
//! [`StateDir`] is the single place that resolves, validates and materializes
//! that path. Callers must go through [`StateDir::file`] instead of joining
//! `.run` themselves, so a future path change cannot split engine state across
//! two directories.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

use orion_variate::EnvDict;
use orion_variate::EnvEvaluable;

use crate::engine::EngineConfig;

/// Default state directory, relative to the work root.
pub const DEFAULT_STATE_DIR: &str = ".run";

/// Resolved runtime state directory.
///
/// The value is always absolute and normalized (no `.`/`..` components), so two
/// independently constructed values for the same work root compare equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDir {
    path: PathBuf,
}

#[derive(Debug)]
pub enum StateDirError {
    /// The configured value cannot be used as a state directory.
    Invalid { raw: String, reason: String },
    /// Filesystem access failed while preparing or cleaning the directory.
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for StateDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateDirError::Invalid { raw, reason } => {
                write!(f, "invalid state_dir '{}': {}", raw, reason)
            }
            StateDirError::Io {
                action,
                path,
                source,
            } => write!(f, "{} '{}' failed: {}", action, path.display(), source),
        }
    }
}

impl Error for StateDirError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StateDirError::Invalid { .. } => None,
            StateDirError::Io { source, .. } => Some(source),
        }
    }
}

impl StateDir {
    /// State directory used when `[runtime].state_dir` is not configured.
    pub fn default_for(work_root: &Path) -> Self {
        Self {
            path: work_root.join(DEFAULT_STATE_DIR),
        }
    }

    /// Resolve a configured value against the work root.
    ///
    /// An empty value (or whitespace) keeps the backward-compatible default
    /// `<work_root>/.run`.
    ///
    /// Guardrails: the state directory must not be the work root itself or an
    /// ancestor of it. `wpadm data clean` clears the state directory, so a
    /// misconfigured value would otherwise delete the whole project.
    ///
    /// Comparisons are lexical: a work root reached through a symlink, or a path
    /// that differs only by case on a case-insensitive filesystem, is not
    /// recognized as the same directory.
    pub fn resolve(work_root: &Path, raw: &str) -> Result<Self, StateDirError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Self::default_for(work_root));
        }
        if raw.contains("${") {
            return Err(StateDirError::Invalid {
                raw: raw.to_string(),
                reason:
                    "unresolved environment variable; set the variable or remove the placeholder"
                        .to_string(),
            });
        }

        let work_root = absolutize(work_root)?;
        let candidate = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            work_root.join(raw)
        };
        let path = normalize(&candidate);

        if path == work_root {
            return Err(StateDirError::Invalid {
                raw: raw.to_string(),
                reason: "must not be the work root itself (cleanup would delete the project)"
                    .to_string(),
            });
        }
        if work_root.starts_with(&path) {
            return Err(StateDirError::Invalid {
                raw: raw.to_string(),
                reason:
                    "must not be an ancestor of the work root (cleanup would delete the project)"
                        .to_string(),
            });
        }

        Ok(Self { path })
    }

    /// Resolve the state directory configured for `work_root`.
    ///
    /// Used by maintenance commands that do not take a `[runtime]` override.
    /// Configuration errors fall back to the default directory with a warning,
    /// because refusing to clean up is worse than cleaning up the default
    /// location.
    pub fn resolve_for_work_root(work_root: &Path, dict: &EnvDict) -> Self {
        match EngineConfig::load(work_root, dict) {
            Ok(conf) => {
                let conf = conf.env_eval(dict).conf_absolutize(work_root);
                match Self::resolve(work_root, conf.runtime_state_dir()) {
                    Ok(dir) => dir,
                    Err(err) => {
                        log::warn!(
                            "{}; falling back to default state dir '{}'",
                            err,
                            DEFAULT_STATE_DIR
                        );
                        Self::default_for(work_root)
                    }
                }
            }
            Err(err) => {
                log::warn!(
                    "load engine config for state dir failed: {}; using default state dir '{}'",
                    err,
                    DEFAULT_STATE_DIR
                );
                Self::default_for(work_root)
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.path.clone()
    }

    /// Path of a state file inside the directory.
    ///
    /// `name` must be a relative file name; it is always joined below the
    /// resolved state directory.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Create the state directory if it does not exist yet.
    pub fn ensure(&self) -> Result<(), StateDirError> {
        std::fs::create_dir_all(&self.path).map_err(|source| StateDirError::Io {
            action: "create state dir",
            path: self.path.clone(),
            source,
        })
    }

    /// Remove the state directory.
    ///
    /// Two cases need special handling:
    ///
    /// * A **symlinked** state dir (e.g. `/state -> /mnt/state`) must keep the
    ///   link and clear the target instead. `remove_dir_all` removes the link
    ///   itself, which would leave the real state behind while the engine
    ///   reports a successful cleanup.
    /// * A state dir that is a **mount point**: its contents can be removed but
    ///   the directory itself cannot be unlinked (`EBUSY`). In that case the
    ///   contents are cleared and the (now empty) directory is kept.
    pub fn clear(&self) -> Result<(), StateDirError> {
        match std::fs::symlink_metadata(&self.path) {
            Ok(meta) if meta.file_type().is_symlink() => return self.clear_contents(),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(StateDirError::Io {
                    action: "stat state dir",
                    path: self.path.clone(),
                    source,
                });
            }
        }

        match std::fs::remove_dir_all(&self.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(remove_err) => self.clear_contents().map_err(|_| StateDirError::Io {
                action: "remove state dir",
                path: self.path.clone(),
                source: remove_err,
            }),
        }
    }

    /// Whether the state directory still holds any entry.
    ///
    /// Used instead of `exists()` because a mount-point state directory is
    /// expected to survive `clear()` as an empty directory.
    pub fn has_entries(&self) -> bool {
        match std::fs::read_dir(&self.path) {
            Ok(mut entries) => entries.next().is_some(),
            Err(err) => {
                // A permission error must not be reported as "nothing to clean":
                // that would silently skip cleanup and leave stale state behind.
                log::warn!(
                    "read state dir '{}' failed: {}; treating it as empty",
                    self.path.display(),
                    err
                );
                false
            }
        }
    }

    /// Remove the directory contents but keep the directory itself.
    ///
    /// `remove_dir_all` cannot be used here: the state directory may be a
    /// mount point, where removing the directory fails with `EBUSY` *after* the
    /// contents were already deleted.
    pub fn clear_contents(&self) -> Result<(), StateDirError> {
        let entries = match std::fs::read_dir(&self.path) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(StateDirError::Io {
                    action: "read state dir",
                    path: self.path.clone(),
                    source,
                });
            }
        };

        for entry in entries {
            let entry = entry.map_err(|source| StateDirError::Io {
                action: "read state dir entry",
                path: self.path.clone(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| StateDirError::Io {
                action: "stat state dir entry",
                path: path.clone(),
                source,
            })?;
            let result = if file_type.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            result.map_err(|source| StateDirError::Io {
                action: "remove state dir entry",
                path,
                source,
            })?;
        }
        Ok(())
    }

    /// Reject a state directory that lives inside one of `project_remote`'s
    /// managed directories.
    ///
    /// Managed directories are replaced wholesale by a remote publish, so state
    /// stored there would be backed up and published to the remote repository.
    pub fn reject_inside(
        &self,
        work_root: &Path,
        managed_dirs: &[&str],
    ) -> Result<(), StateDirError> {
        // `work_root` may be relative; normalizing it keeps the prefix check
        // meaningful (otherwise `starts_with` silently never matches).
        let work_root = absolutize(work_root)?;
        for rel in managed_dirs {
            let managed = normalize(&work_root.join(rel));
            if self.path.starts_with(&managed) {
                return Err(StateDirError::Invalid {
                    raw: self.path.display().to_string(),
                    reason: format!(
                        "must not live inside the project_remote-managed '{}' directory",
                        rel
                    ),
                });
            }
        }
        Ok(())
    }
}

fn absolutize(work_root: &Path) -> Result<PathBuf, StateDirError> {
    let path = if work_root.is_absolute() {
        work_root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| StateDirError::Io {
                action: "resolve work root",
                path: work_root.to_path_buf(),
                source,
            })?
            .join(work_root)
    };
    Ok(normalize(&path))
}

/// Remove `.` and `..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn empty_value_keeps_default() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "").expect("resolve");
        assert_eq!(state.path(), dir.path().join(".run"));
        assert_eq!(state, StateDir::default_for(dir.path()));
    }

    #[test]
    fn relative_value_resolves_against_work_root() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state").expect("resolve");
        assert_eq!(state.path(), dir.path().join("state"));
    }

    #[test]
    fn absolute_value_is_used_verbatim() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "/var/lib/wparse").expect("resolve");
        assert_eq!(state.path(), Path::new("/var/lib/wparse"));
    }

    #[test]
    fn dot_components_are_normalized() {
        let dir = tempdir().expect("tempdir");
        let raw = dir.path().join("state/../state/./run");
        let state = StateDir::resolve(dir.path(), raw.to_str().expect("utf8")).expect("resolve");
        assert_eq!(state.path(), dir.path().join("state/run"));
    }

    #[test]
    fn work_root_itself_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let err = StateDir::resolve(dir.path(), dir.path().to_str().expect("utf8")).unwrap_err();
        assert!(
            err.to_string().contains("must not be the work root"),
            "{err}"
        );
    }

    #[test]
    fn ancestor_of_work_root_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let work_root = dir.path().join("project");
        fs::create_dir_all(&work_root).expect("create work root");
        let err = StateDir::resolve(&work_root, dir.path().to_str().expect("utf8")).unwrap_err();
        assert!(err.to_string().contains("ancestor"), "{err}");
    }

    #[test]
    fn filesystem_root_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let err = StateDir::resolve(dir.path(), "/").unwrap_err();
        assert!(err.to_string().contains("ancestor"), "{err}");
    }

    #[test]
    fn unresolved_env_placeholder_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let err = StateDir::resolve(dir.path(), "${WPARSE_STATE_DIR}").unwrap_err();
        assert!(err.to_string().contains("unresolved environment"), "{err}");
    }

    #[test]
    fn managed_dir_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "models/.state").expect("resolve");
        let err = state
            .reject_inside(dir.path(), &["conf", "models", "topology", "connectors"])
            .unwrap_err();
        assert!(err.to_string().contains("project_remote-managed"), "{err}");
    }

    #[test]
    fn clear_removes_a_regular_directory() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state").expect("resolve");
        state.ensure().expect("ensure");
        fs::write(state.file("authority.sqlite"), b"x").expect("write file");

        state.clear().expect("clear");

        assert!(
            !state.path().exists(),
            "regular state dir should be removed"
        );
        assert!(!state.has_entries());
    }

    #[test]
    fn clear_is_noop_for_missing_dir() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "missing").expect("resolve");
        state.clear().expect("clear missing dir");
    }

    #[test]
    fn has_entries_detects_state_files() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state").expect("resolve");
        assert!(!state.has_entries());
        state.ensure().expect("ensure");
        assert!(!state.has_entries());
        fs::write(state.file("rule_mapping.dat"), b"x").expect("write file");
        assert!(state.has_entries());
    }

    #[test]
    fn clear_contents_keeps_the_directory() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state").expect("resolve");
        state.ensure().expect("ensure");
        fs::write(state.file("authority.sqlite"), b"x").expect("write file");
        fs::create_dir_all(state.file("project_remote")).expect("create nested");
        fs::write(state.file("project_remote/state.json"), b"{}").expect("write nested");

        state.clear_contents().expect("clear");

        assert!(state.path().exists(), "state dir must survive");
        assert!(!state.file("authority.sqlite").exists());
        assert!(!state.file("project_remote").exists());
    }

    #[test]
    fn clear_contents_is_noop_for_missing_dir() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "missing").expect("resolve");
        state.clear_contents().expect("clear missing dir");
    }

    #[test]
    fn dot_value_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let err = StateDir::resolve(dir.path(), ".").unwrap_err();
        assert!(err.to_string().contains("work root"), "{err}");
    }

    #[test]
    fn relative_parent_is_rejected() {
        let dir = tempdir().expect("tempdir");
        let work_root = dir.path().join("project");
        fs::create_dir_all(&work_root).expect("create work root");
        let err = StateDir::resolve(&work_root, "..").unwrap_err();
        assert!(err.to_string().contains("ancestor"), "{err}");
    }

    #[test]
    fn trailing_separator_is_normalized() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state/").expect("resolve");
        assert_eq!(state.path(), dir.path().join("state"));
        assert_eq!(
            state,
            StateDir::resolve(dir.path(), "state").expect("resolve")
        );
    }

    #[test]
    fn absolute_state_dir_inside_work_root_is_allowed() {
        let dir = tempdir().expect("tempdir");
        let raw = dir.path().join("state");
        let state = StateDir::resolve(dir.path(), raw.to_str().expect("utf8")).expect("resolve");
        assert_eq!(state.path(), dir.path().join("state"));
    }

    #[test]
    fn reject_inside_allows_state_outside_managed_dirs() {
        let dir = tempdir().expect("tempdir");
        let state = StateDir::resolve(dir.path(), "state").expect("resolve");
        state
            .reject_inside(dir.path(), &["conf", "models", "topology", "connectors"])
            .expect("state dir outside managed dirs must be accepted");
    }

    #[test]
    fn reject_inside_absolutizes_relative_work_root() {
        let cwd = std::env::current_dir().expect("current dir");
        let state = StateDir::resolve(&cwd, "models/.state").expect("resolve");
        let err = state
            .reject_inside(Path::new("."), &["models"])
            .unwrap_err();
        assert!(err.to_string().contains("project_remote-managed"), "{err}");
    }

    #[test]
    fn sibling_path_with_shared_prefix_is_not_treated_as_ancestor() {
        let dir = tempdir().expect("tempdir");
        let work_root = dir.path().join("work");
        fs::create_dir_all(&work_root).expect("create work root");
        let sibling = dir.path().join("work2");

        let state =
            StateDir::resolve(&work_root, sibling.to_str().expect("utf8")).expect("resolve");

        assert_eq!(state.path(), sibling);
    }

    #[test]
    fn clear_reports_non_directory_state_path() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("state-file");
        fs::write(&file, b"x").expect("write file");

        let state = StateDir::resolve(dir.path(), file.to_str().expect("utf8")).expect("resolve");
        let err = state.clear().unwrap_err();

        assert!(
            err.to_string().contains("remove state dir"),
            "a non-directory state path must be reported: {err}"
        );
        assert!(file.exists(), "the file must not be deleted");
    }

    #[cfg(unix)]
    #[test]
    fn clear_on_symlinked_dir_keeps_the_link() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("real-state");
        fs::create_dir_all(&target).expect("create target");
        fs::write(target.join("authority.sqlite"), b"x").expect("write file");
        let link = dir.path().join("state-link");
        symlink(&target, &link).expect("create symlink");

        let state = StateDir::resolve(dir.path(), "state-link").expect("resolve");
        state.clear().expect("clear symlinked state dir");

        assert!(link.exists(), "symlink itself must remain");
        assert!(target.exists(), "target directory must remain");
        assert!(
            !target.join("authority.sqlite").exists(),
            "contents must be cleared through the link"
        );
        assert!(
            !state.has_entries(),
            "cleared symlinked state dir must report no entries"
        );
    }

    #[test]
    fn resolve_for_work_root_uses_configured_state_dir() {
        let dir = tempdir().expect("tempdir");
        let conf_dir = dir.path().join("conf");
        fs::create_dir_all(&conf_dir).expect("create conf dir");
        fs::write(
            conf_dir.join("wparse.toml"),
            "version = \"1.0\"\n\n[runtime]\nstate_dir = \"state\"\n",
        )
        .expect("write config");

        let state = StateDir::resolve_for_work_root(dir.path(), &EnvDict::new());
        assert_eq!(state.path(), dir.path().join("state"));
    }

    #[test]
    fn resolve_for_work_root_falls_back_to_default_on_invalid_config() {
        let dir = tempdir().expect("tempdir");
        let conf_dir = dir.path().join("conf");
        fs::create_dir_all(&conf_dir).expect("create conf dir");
        fs::write(conf_dir.join("wparse.toml"), "not toml [[[").expect("write config");

        let state = StateDir::resolve_for_work_root(dir.path(), &EnvDict::new());
        assert_eq!(state.path(), dir.path().join(".run"));
    }
}
