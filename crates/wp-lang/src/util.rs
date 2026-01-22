use anyhow::Context;
use glob::glob;
use std::{fs::File, io::Read, path::PathBuf};

use orion_error::{ContextRecord, ErrorOwe, ErrorWith, OperationContext};
use wp_log::info_ctrl;

use crate::{WplCode, parser::error::WplCodeResult, types::AnyResult};

pub fn fetch_wpl_data(path: &str, target: &str) -> WplCodeResult<Vec<WplCode>> {
    let mut wpl_vec = Vec::new();
    let mut ctx = OperationContext::want("load wpl");
    ctx.record("path", path);
    let files = find_conf_files(path, target).owe_conf().with(&ctx)?;

    for f_name in &files {
        info_ctrl!("load conf file: {:?}", f_name);
        let mut f = File::open(f_name).owe_conf().with(&ctx)?;
        let mut buffer = Vec::with_capacity(10240);
        f.read_to_end(&mut buffer).owe_conf().with(&ctx)?;
        let file_data = String::from_utf8(buffer).owe_conf().with(&ctx)?;
        wpl_vec.push(WplCode::try_from((PathBuf::from(f_name), file_data))?)
    }
    Ok(wpl_vec)
}
fn find_conf_files(path: &str, target: &str) -> AnyResult<Vec<PathBuf>> {
    let mut found = Vec::new();
    info_ctrl!("find conf files in: {}", path);
    let glob_path = format!("{}/**/{}", path, target);
    for entry in glob(glob_path.as_str()).with_context(|| format!("read_dir  fail: {}", path))? {
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
