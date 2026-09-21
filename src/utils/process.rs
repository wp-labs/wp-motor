use crate::compat::LegacyOwe;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::Path;

use orion_error::conversion::{ErrorWith, ToStructError};
use wp_error::run_error::{RunReason, RunResult};

pub struct PidRec {
    pid_file: String,
}
impl PidRec {
    pub fn current(name: &str) -> RunResult<Self> {
        let id = sysinfo::get_current_pid()
            .owe(RunReason::system_error())
            .doing("want current pid")?;
        // 将进程ID写入文件
        let path = Path::new(name);
        let mut file = File::create(path)
            .owe(RunReason::system_error())
            .doing("crate Pid file")?;
        file.write_all(id.to_string().as_bytes())
            .owe(RunReason::system_error())?;
        Ok(Self {
            pid_file: name.to_string(),
        })
    }
}
impl Drop for PidRec {
    fn drop(&mut self) {
        let path = Path::new(self.pid_file.as_str());
        if path.exists()
            && let Err(e) = fs::remove_file(path)
        {
            error_ctrl!("删除pid文件失败：{}", e)
        }
    }
}

/// 单实例锁：对 `<work-root>/.run/.lock` 取 `flock(LOCK_EX | LOCK_NB)`，由进程生命周期持有。
/// 进程退出/崩溃时内核自动释放，因此无需依赖 pidfile 判断是否已有实例在跑；
/// 同一 work-root 的第二个实例会在此处被拒绝并返回非 0 退出码。
///
/// 边界：`flock` 是内核级劝告锁，只在同一内核内互斥。Linux 宿主↔容器共享内核（bind mount
/// 同一 inode），天然生效；macOS 宿主与容器分属两套内核，跨该边界不互斥（需 O_EXCL+心跳兜底），
/// 见 wp-labs/warp-parse#365。
#[derive(Debug)]
pub struct RunLock {
    _file: File,
}

impl RunLock {
    /// 尝试获取单实例锁。`work_root` 仅用于错误提示，`lock_path` 为实际锁文件路径。
    pub fn acquire(work_root: &str, lock_path: &str) -> RunResult<Self> {
        let path = Path::new(lock_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .owe(RunReason::system_error())
                .doing(format!("create lock dir {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .owe(RunReason::system_error())
            .doing(format!("open single-instance lock file {}", path.display()))?;
        Self::try_lock(file, path, work_root)
    }

    fn try_lock(file: File, path: &Path, work_root: &str) -> RunResult<Self> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;

            let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if rc != 0 {
                let err = std::io::Error::last_os_error();
                return Err(match err.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        RunReason::business_error().to_err().with_detail(format!(
                            "another wparse instance is already using work-root {} (lock: {})",
                            work_root,
                            path.display()
                        ))
                    }
                    _ => RunReason::system_error()
                        .to_err()
                        .with_detail(format!("lock {} failed", path.display()))
                        .with_source(err),
                });
            }
        }
        Ok(Self { _file: file })
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;

            let _ = unsafe { libc::flock(self._file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, tempdir};

    fn lock_path_in(dir: &TempDir) -> String {
        dir.path()
            .join(".run/.lock")
            .to_str()
            .expect("utf8 lock path")
            .to_string()
    }

    #[test]
    fn run_lock_rejects_second_instance() {
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_path_in(&dir);

        let _first = RunLock::acquire("/data", &lock_path).expect("first acquire");
        let second = RunLock::acquire("/data", &lock_path);
        assert!(second.is_err(), "second acquire should be rejected");

        let msg = second.unwrap_err().to_string();
        assert!(msg.contains("another wparse instance"), "got: {msg}");
        assert!(msg.contains("/data"), "work-root should be in error: {msg}");
        assert!(
            msg.contains(&lock_path),
            "lock path should be in error: {msg}"
        );
    }

    #[test]
    fn run_lock_conflict_is_business_error() {
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_path_in(&dir);

        let _first = RunLock::acquire("/data", &lock_path).expect("first acquire");
        let err = RunLock::acquire("/data", &lock_path).unwrap_err();
        assert_eq!(
            err.reason(),
            &RunReason::business_error(),
            "conflict should be a business rejection (non-zero exit code)"
        );
    }

    #[test]
    fn run_lock_released_after_drop() {
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_path_in(&dir);

        let first = RunLock::acquire("/data", &lock_path).expect("first acquire");
        drop(first);
        // drop 后内核释放锁，可再次获取
        let _again = RunLock::acquire("/data", &lock_path).expect("reacquire after drop");
    }

    #[test]
    fn run_lock_different_paths_do_not_conflict() {
        let dir = tempdir().expect("tempdir");
        let a = dir.path().join(".run/a.lock");
        let b = dir.path().join(".run/b.lock");
        let a = a.to_str().expect("utf8 path");
        let b = b.to_str().expect("utf8 path");

        // 不同 work-root 的锁文件互不影响
        let _la = RunLock::acquire("/wa", a).expect("acquire a");
        let _lb = RunLock::acquire("/wb", b).expect("acquire b");
    }

    #[test]
    fn run_lock_conflict_is_file_based_not_label_based() {
        let dir = tempdir().expect("tempdir");
        let lock_path = lock_path_in(&dir);

        // work_root 仅用于错误提示；锁的标识是锁文件本身。
        // 相同锁文件、不同 work_root 标签仍应冲突。
        let _first = RunLock::acquire("/data", &lock_path).expect("first acquire");
        let second = RunLock::acquire("/other-root", &lock_path);
        assert!(
            second.is_err(),
            "same lock file must conflict regardless of work-root label"
        );
    }
}
