use orion_error::{UvsFrom, conversion::ToStructError};
use std::env;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use wp_error::run_error::{RunReason, RunResult};

pub trait Wc<T1, T2> {
    fn wc_of(&self, file: T1) -> RunResult<T2>;
}

pub struct Usecase {
    path: String,
}

impl Usecase {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
    pub fn run(&self, sh: &str) -> RunResult<(String, String)> {
        let sh_path = format!("{}/{}", self.path, sh);
        if !std::path::Path::new(sh_path.as_str()).exists() {
            return Err(RunReason::from_sys()
                .to_err()
                .with_detail(format!("script not found: {}", sh_path)));
        }
        if let (Some(path), Some(_home)) = (env::var_os("PATH"), env::var_os("HOME")) {
            //let bin = Path::new(&home).join("bin");

            let mut path_vec = env::split_paths(&path).collect::<Vec<_>>();
            let project_root = std::env::current_dir().map_err(|e| {
                RunReason::from_sys()
                    .to_err()
                    .with_detail("resolve current dir failed")
                    .with_source(e)
            })?;

            let target_dir = project_root
                .join(env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target/debug".to_string()));
            path_vec.push(target_dir);

            let new_path = env::join_paths(path_vec).map_err(|e| {
                RunReason::from_sys()
                    .to_err()
                    .with_detail("join PATH entries failed")
                    .with_source(e)
            })?;
            unsafe {
                env::set_var("PATH", new_path);
            }
        }
        // 告知用例脚本跳过再次构建（build_and_setup_path 会读取该变量）
        unsafe {
            env::set_var("SKIP_BUILD", "1");
        }
        // 默认为 debug，避免脚本找不到 release 二进制而触发构建
        if env::var_os("PROFILE").is_none() {
            unsafe {
                env::set_var("PROFILE", "debug");
            }
        }
        let uc_cmd = Command::new("sh")
            .current_dir(self.path.as_str())
            .arg(sh)
            .output()
            .map_err(|e| {
                RunReason::from_sys()
                    .to_err()
                    .with_detail(format!("run script failed: {}", sh_path))
                    .with_source(e)
            })?;
        println!(" out: {}", String::from_utf8_lossy(&uc_cmd.stdout));
        println!(" err: {}", String::from_utf8_lossy(&uc_cmd.stderr));
        Ok((
            String::from_utf8_lossy(&uc_cmd.stdout).to_string(),
            String::from_utf8_lossy(&uc_cmd.stderr).to_string(),
        ))
    }
    pub fn get_count(path: &str) -> RunResult<usize> {
        if !std::path::Path::new(path).exists() {
            return Err(RunReason::from_sys()
                .to_err()
                .with_detail(format!("file not found: {}", path)));
        }
        let cmd = Command::new("wc")
            .arg("-l")
            .arg(path)
            .output()
            .map_err(|e| {
                RunReason::from_sys()
                    .to_err()
                    .with_detail(format!("run wc failed for {}", path))
                    .with_source(e)
            })?;
        let binding = String::from_utf8(cmd.stdout).map_err(|e| {
            RunReason::from_sys()
                .to_err()
                .with_detail(format!("decode wc output failed for {}", path))
                .with_source(e)
        })?;
        let stdout: Vec<&str> = binding.trim().split(' ').collect();
        let count: usize = stdout[0].parse().map_err(|e| {
            RunReason::from_sys()
                .to_err()
                .with_detail(format!("parse wc output failed for {}", path))
                .with_source(e)
        })?;
        Ok(count)
    }
    pub fn open(&self, file: &str) -> RunResult<File> {
        let path = format!("{}/{}", self.path, file);
        if !Path::new(path.as_str()).exists() {
            return Err(RunReason::from_sys()
                .to_err()
                .with_detail(format!("file not found: {}", path)));
        }
        File::open(&path).map_err(|e| {
            RunReason::from_sys()
                .to_err()
                .with_detail(format!("open file failed: {}", path))
                .with_source(e)
        })
    }
}

impl Wc<&str, usize> for Usecase {
    fn wc_of(&self, file: &str) -> RunResult<usize> {
        Usecase::get_count(format!("{}/{}", self.path, file).as_str())
    }
}

impl Wc<(&str, &str), (usize, usize)> for Usecase {
    fn wc_of(&self, files: (&str, &str)) -> RunResult<(usize, usize)> {
        let count1 = Usecase::get_count(format!("{}/{}", self.path, files.0).as_str())?;
        let count2 = Usecase::get_count(format!("{}/{}", self.path, files.1).as_str())?;
        println!("wc:{},{}", count1, count2);
        Ok((count1, count2))
    }
}

impl Wc<(&str, &str, &str), (usize, usize, usize)> for Usecase {
    fn wc_of(&self, files: (&str, &str, &str)) -> RunResult<(usize, usize, usize)> {
        let count1 = Usecase::get_count(format!("{}/{}", self.path, files.0).as_str())?;
        let count2 = Usecase::get_count(format!("{}/{}", self.path, files.1).as_str())?;
        let count3 = Usecase::get_count(format!("{}/{}", self.path, files.2).as_str())?;
        println!("wc:{},{},{}", count1, count2, count3);
        Ok((count1, count2, count3))
    }
}
