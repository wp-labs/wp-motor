use crate::res::simple_ins_run_res;
use glob::glob;
use orion_error::{ErrorOwe, ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
use std::fs;
use std::path::{Path, PathBuf};
use wp_engine::facade::config::load_warp_engine_confs;
use wp_engine::facade::kit::engine_proc_file;
use wp_error::run_error::{RunReason, RunResult};

pub fn parse_wpl_samples(work_root: &str, dict: &EnvDict) -> RunResult<()> {
    let jobs = discover_sample_jobs(work_root, dict)?;
    if jobs.is_empty() {
        return Err(RunReason::from_conf("未在 wpl 目录中找到 sample.dat").to_err());
    }

    let mut results: u32 = 0;
    for job in jobs {
        println!("→ 解析样本 {}", job.label);

        match parse_single_run(&job.sample, &job.rule) {
            Ok(_) => {
                results += 1;
            }
            Err(e) => {
                println!("✗ 样本 {} 解析失败: {}", job.label, e);
            }
        }
    }

    println!("✓ 共解析 {} 个样本", results);
    Ok(())
}

fn parse_single_run<P: AsRef<Path> + Clone>(data_path: P, rule_file: P) -> RunResult<()> {
    let (work_rule, sinks) = simple_ins_run_res(Some(rule_file), None)?;
    let infra = sinks.infra_agent();
    engine_proc_file(work_rule, &data_path, infra, 1).owe_biz()?;
    Ok(())
}

fn discover_sample_jobs(work_root: &str, dict: &EnvDict) -> RunResult<Vec<SampleJob>> {
    let (cm, main) = load_warp_engine_confs(work_root, dict)
        .map_err(|e| RunReason::from_conf(format!("加载 wparse.toml 失败: {}", e)).to_err())?;
    let rule_root = Path::new(main.rule_root());
    let wpl_root = if rule_root.is_absolute() {
        rule_root.to_path_buf()
    } else {
        Path::new(&cm.work_root_path()).join(rule_root)
    };
    if !wpl_root.exists() {
        return Ok(Vec::new());
    }
    let pattern = format!("{}/**/sample.dat", wpl_root.display());
    let mut jobs = Vec::new();
    let walker = glob(&pattern)
        .map_err(|e| RunReason::from_conf(format!("扫描样本失败: {}", e)).to_err())?;
    for entry in walker {
        match entry {
            Ok(sample_path) => {
                if !sample_path.is_file() {
                    continue;
                }
                if let Some(dir) = sample_path.parent() {
                    if let Some(rule_file) = locate_rule_file(dir)? {
                        let rel = sample_path
                            .strip_prefix(&wpl_root)
                            .unwrap_or(&sample_path)
                            .display()
                            .to_string();
                        jobs.push(SampleJob {
                            label: rel,
                            sample: sample_path,
                            rule: rule_file,
                        });
                    } else {
                        eprintln!("跳过样本 {}: 未找到对应的 .wpl 文件", sample_path.display());
                    }
                }
            }
            Err(e) => {
                eprintln!("样本遍历警告: {}", e);
            }
        }
    }
    Ok(jobs)
}

fn locate_rule_file(dir: &Path) -> RunResult<Option<PathBuf>> {
    let preferred = dir.join("parse.wpl");
    if preferred.exists() {
        return Ok(Some(preferred));
    }
    let mut first = None;
    for entry in fs::read_dir(dir).map_err(|e| {
        RunReason::from_conf(format!("列举目录失败 {}: {}", dir.display(), e)).to_err()
    })? {
        let entry = entry.map_err(|e| {
            RunReason::from_conf(format!("读取目录项失败 {}: {}", dir.display(), e)).to_err()
        })?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("wpl"))
            .unwrap_or(false)
        {
            first = Some(path);
            break;
        }
    }
    Ok(first)
}

struct SampleJob {
    label: String,
    sample: PathBuf,
    rule: PathBuf,
}
