use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use orion_error::{ErrorOwe, ErrorWith};
use wp_error::run_error::RunResult;

pub struct PidRec {
    pid_file: String,
}
impl PidRec {
    pub fn current(name: &str) -> RunResult<Self> {
        let id = sysinfo::get_current_pid()
            .owe_sys()
            .want("want current pid")?;
        // 将进程ID写入文件
        let path = Path::new(name);
        let mut file = File::create(path).owe_sys().want("crate Pid file")?;
        file.write_all(id.to_string().as_bytes()).owe_sys()?;
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
