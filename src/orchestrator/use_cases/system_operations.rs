use crate::types::AnyResult;
use anyhow::anyhow;
use std::env;
use std::fs::File;
use std::path::Path;
use std::process::Command;

pub trait Wc<T1, T2> {
    fn wc_of(&self, file: T1) -> AnyResult<T2>;
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
    pub fn run(&self, sh: &str) -> AnyResult<(String, String)> {
        let sh_path = format!("{}/{}", self.path, sh);
        if !std::path::Path::new(sh_path.as_str()).exists() {
            return Err(anyhow!("{} not exists", sh_path));
        }
        if let (Some(path), Some(_home)) = (env::var_os("PATH"), env::var_os("HOME")) {
            //let bin = Path::new(&home).join("bin");

            let mut path_vec = env::split_paths(&path).collect::<Vec<_>>();
            let project_root = std::env::current_dir()?;

            let target_dir = project_root
                .join(env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target/debug".to_string()));
            path_vec.push(target_dir);

            let new_path = env::join_paths(path_vec)?;
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
            .unwrap();
        println!(" out: {}", String::from_utf8_lossy(&uc_cmd.stdout));
        println!(" err: {}", String::from_utf8_lossy(&uc_cmd.stderr));
        Ok((
            String::from_utf8_lossy(&uc_cmd.stdout).to_string(),
            String::from_utf8_lossy(&uc_cmd.stderr).to_string(),
        ))
    }
    pub fn get_count(path: &str) -> AnyResult<usize> {
        if !std::path::Path::new(path).exists() {
            return Err(anyhow!("{} not exists", path));
        }
        let cmd = Command::new("wc").arg("-l").arg(path).output()?;
        let binding = String::from_utf8(cmd.stdout)?;
        let stdout: Vec<&str> = binding.trim().split(' ').collect();
        let count: usize = stdout[0].parse()?;
        Ok(count)
    }
    pub fn open(&self, file: &str) -> AnyResult<File> {
        let path = format!("{}/{}", self.path, file);
        if !Path::new(path.as_str()).exists() {
            return Err(anyhow!("{} not exists", path));
        }
        Ok(File::open(&path)?)
    }
}

impl Wc<&str, usize> for Usecase {
    fn wc_of(&self, file: &str) -> AnyResult<usize> {
        Usecase::get_count(format!("{}/{}", self.path, file).as_str())
    }
}

impl Wc<(&str, &str), (usize, usize)> for Usecase {
    fn wc_of(&self, files: (&str, &str)) -> AnyResult<(usize, usize)> {
        let count1 = Usecase::get_count(format!("{}/{}", self.path, files.0).as_str())?;
        let count2 = Usecase::get_count(format!("{}/{}", self.path, files.1).as_str())?;
        println!("wc:{},{}", count1, count2);
        Ok((count1, count2))
    }
}

impl Wc<(&str, &str, &str), (usize, usize, usize)> for Usecase {
    fn wc_of(&self, files: (&str, &str, &str)) -> AnyResult<(usize, usize, usize)> {
        let count1 = Usecase::get_count(format!("{}/{}", self.path, files.0).as_str())?;
        let count2 = Usecase::get_count(format!("{}/{}", self.path, files.1).as_str())?;
        let count3 = Usecase::get_count(format!("{}/{}", self.path, files.2).as_str())?;
        println!("wc:{},{},{}", count1, count2, count3);
        Ok((count1, count2, count3))
    }
}
