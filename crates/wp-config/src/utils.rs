use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_conf::{ToStructError, TomlIO};
use orion_error::{ErrorOwe, ErrorWith, UvsValidationFrom};
use orion_variate::EnvEvaluable;
use serde::Serialize;
use wp_connector_api::ParamMap;
use wp_error::config_error::{ConfReason, ConfResult};
use wp_log::info_ctrl;

use crate::types::AnyResult;
use glob::glob;
use serde::de::DeserializeOwned;

pub fn ignore_check(ignore: bool, msg: &str) -> OrionConfResult<()> {
    if ignore {
        info_ctrl!("ignore! : {}", msg);
    } else {
        return ConfIOReason::from_validation(msg).err_result();
    }
    Ok(())
}

pub fn save_conf<T, P: AsRef<Path>>(conf: Option<T>, path: P, ignore: bool) -> OrionConfResult<()>
where
    T: serde::Serialize + DeserializeOwned + TomlIO<T>,
{
    if let Some(conf) = conf {
        let path_ref = path.as_ref();
        if path_ref.exists() {
            ignore_check(ignore, &format!("{} exists!", path_ref.display()))?;
        } else {
            // ensure parent directory exists
            if let Some(parent) = path_ref.parent() {
                std::fs::create_dir_all(parent)
                    .owe_conf()
                    .want("crate dir")
                    .with(parent)?;
            }
            //export_toml(&conf, path)?;
            conf.save_toml(&PathBuf::from(path_ref))?;
            info_ctrl!("save toml file suc: {} ", path_ref.display());
        }
    }
    Ok(())
}
pub fn save_data<P: AsRef<Path>>(
    conf: Option<String>,
    dst: P,
    ignore: bool,
) -> OrionConfResult<()> {
    if let Some(conf) = conf {
        let dst_ref = dst.as_ref();
        if dst_ref.exists() {
            ignore_check(ignore, &format!("{} exists!", dst_ref.display()))?;
        } else {
            let path = dst_ref;
            if let Some(value) = path.parent() {
                std::fs::create_dir_all(value)
                    .owe_conf()
                    .want("create dir")
                    .with(value)?;
            }
            let mut file = std::fs::File::create(path)
                .owe_conf()
                .want("create file")
                .with(path)?;
            file.write_all(conf.as_bytes())
                .owe_conf()
                .want("save data")
                .with(path)?;
            info_ctrl!("save data file suc : {} ", dst_ref.display());
        }
    }
    Ok(())
}

pub fn backup_clean<P: AsRef<Path>>(path: P) -> OrionConfResult<()> {
    let path_ref = path.as_ref();
    if path_ref.exists() {
        std::fs::copy(path_ref, format!("{}.bak", path_ref.display()))
            .owe_conf()
            .want("copy file")
            .with(path_ref)?;
        std::fs::remove_file(path_ref)
            .owe_conf()
            .want("remove file")
            .with(path_ref)?;
    }
    Ok(())
}

pub fn file_clear<P: AsRef<Path>>(path: P) {
    let path_ref = path.as_ref();
    if path_ref.exists()
        && let Err(e) = std::fs::remove_file(path_ref)
    {
        error!("clean {} failed: {}", path_ref.display(), e);
    }
}
pub fn conf_init<T, P: AsRef<Path>>(conf: T, path: P) -> AnyResult<T>
where
    T: Serialize + DeserializeOwned + Clone + TomlIO<T>,
{
    save_conf(Some(conf.clone()), path, true)?;
    Ok(conf)
}

pub fn some_str(s: &str) -> Option<String> {
    Some(s.to_string())
}

//pub type NomResult<I, O> = IResult<I, O, nom::error::VerboseError<I>>;

pub fn find_conf_files<P: AsRef<Path>>(path: P, target: &str) -> AnyResult<Vec<PathBuf>> {
    let path_ref = path.as_ref();
    let mut found = Vec::new();
    info_ctrl!("find conf files in: {}", path_ref.display());
    let glob_path = format!("{}/**/{}", path_ref.display(), target);
    for entry in glob(glob_path.as_str())
        .with_context(|| format!("read_dir  fail: {}", path_ref.display()))?
    {
        match entry {
            Ok(path) => {
                found.push(path);
            }
            Err(e) => {
                error!("find_conf files fail: {}", e);
            }
        }
    }
    Ok(found)
}

pub fn find_group_conf(
    path: &str,
    target_fst: &str,
    target_sec: &str,
) -> ConfResult<Vec<PathGroup>> {
    let mut found = Vec::new();
    let entries = fs::read_dir(path)
        .owe(ConfReason::NotFound("file miss".into()))
        .with(path.to_string())?;
    let mut first = None;
    let mut second = None;
    for entry in entries {
        let entry = entry.owe(ConfReason::Syntax("bad entry".into()))?;
        let file_type = entry
            .file_type()
            .owe(ConfReason::NotFound("file type error".into()))?;
        if file_type.is_dir() {
            let sub = entry.path();
            if let Some(sub_str) = sub.to_str() {
                let mut sub_found = find_group_conf(sub_str, target_fst, target_sec)?;
                found.append(&mut sub_found);
            } else {
                // Skip non-UTF8 paths instead of panicking
                continue;
            }
            continue;
        } else if file_type.is_file() {
            let file_name = entry.file_name();
            if file_name == target_fst {
                first = Some(entry.path());
            }
            if file_name == target_sec {
                second = Some(entry.path());
            }
        }
    }
    if first.is_some() || second.is_some() {
        found.push(PathGroup::new(first, second));
    }
    Ok(found)
}

pub struct PathGroup {
    pub fst: Option<PathBuf>,
    pub sec: Option<PathBuf>,
}
impl PathGroup {
    pub fn new(fst: Option<PathBuf>, sec: Option<PathBuf>) -> Self {
        PathGroup { fst, sec }
    }
}

pub fn env_eval_params(mut params: ParamMap, dict: &orion_variate::EnvDict) -> ParamMap {
    for (_, v) in params.iter_mut() {
        if let serde_json::Value::String(str_val) = v {
            *str_val = str_val.clone().env_eval(dict);
        }
    }
    params
}

pub fn env_eval_vec<T: EnvEvaluable<T> + Clone>(
    mut params: Vec<T>,
    dict: &orion_variate::EnvDict,
) -> Vec<T> {
    for v in params.iter_mut() {
        *v = v.clone().env_eval(dict);
    }
    params
}

#[cfg(test)]
mod test {
    use crate::types::AnyResult;

    #[test]
    fn test_find_conf_files() -> AnyResult<()> {
        // 使用 crate 根目录进行定位，避免受当前工作目录影响
        let base = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(base).join("src").join("structure");
        let files = super::find_conf_files(path.to_str().unwrap(), "*.rs")?;
        // 验证关键文件是否存在，避免对文件数量的脆弱依赖（sink 模块已拆分为目录）
        let must_have = ["mod.rs", "group.rs", "io.rs", "framework.rs"];
        for name in must_have {
            assert!(
                files
                    .iter()
                    .any(|p| p.file_name().and_then(|x| x.to_str()) == Some(name)),
                "missing expected conf file: {}",
                name
            );
        }
        Ok(())
    }
    #[test]
    fn test_find_group_files() -> AnyResult<()> {
        // 查找同时含有 mod.rs 与 group.rs 的目录对（至少一组）
        let base = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(base).join("src").join("structure");
        let files = super::find_group_conf(path.to_str().unwrap(), "mod.rs", "group.rs")?;
        assert!(!files.is_empty());
        assert!(files.iter().any(|pg| pg.fst.is_some()));
        Ok(())
    }
}
