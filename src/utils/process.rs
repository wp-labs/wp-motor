use crate::compat::LegacyOwe;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use orion_error::conversion::ErrorWith;
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
