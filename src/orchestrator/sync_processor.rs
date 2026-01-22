use crate::core::parser::ParseOption;
use crate::core::parser::WplEngine;
use crate::core::parser::WplRepository;
use crate::orchestrator::engine::definition::WplCodePKG;
use crate::sinks::InfraSinkAgent;
use crate::sinks::SinkRegistry;
use crate::types::AnyResult;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use wp_conf::RunArgs;
use wp_connector_api::{SourceEvent, Tags};
use wp_parse_api::RawData;
use wpl::{DEFAULT_KEY, gen_pkg_id};
// Import macros
use wpl::{true_break, true_continue};

#[allow(dead_code)]
pub type DynRead = dyn std::io::Read;
#[allow(dead_code)]
pub type DynWrite = dyn std::io::Write;

// 旧版基于 SourceInstance 的三个同步函数已移除；改用文件路径版本。

/// 去除对旧 SourceInstance 的依赖，直接使用文件路径
pub fn engine_check(
    mut args: RunArgs,
    wpl_code: WplCodePKG,
    in_path: &str,
    _sinks: SinkRegistry,
) -> AnyResult<()> {
    args.need_complete = true;

    let wpl_pkgs = WplRepository::from_wpl_strict(wpl_code)?;
    let engine = WplEngine::from_code(&wpl_pkgs, InfraSinkAgent::use_null())?;

    let in_io = File::open(in_path)?;
    let source_key = Arc::new(DEFAULT_KEY.to_string());
    let source_tags = Arc::new(Tags::new());
    for (line_count, line_data) in BufReader::new(in_io)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        if args.ge_max(line_count) {
            break;
        }
        let setting = ParseOption::default();
        // 统一改为异步解析；在同步入口内通过 tokio runtime block_on 执行
        let event = SourceEvent::new(
            gen_pkg_id(), // 生成 event_id
            source_key.as_str(),
            RawData::String(line_data),
            Arc::clone(&source_tags),
        );
        let fut = engine.proc_async(event, &setting);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // 在已有 runtime 中，通过 block_in_place + handle.block_on 安全阻塞当前线程
            tokio::task::block_in_place(|| {
                let _ = handle.block_on(fut);
            });
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            let _ = rt.block_on(fut);
        }
    }
    Ok(())
}

/// 去除对旧 SourceInstance 的依赖，直接使用文件路径
pub fn engine_proc_file<T: AsRef<Path>>(
    wpl_code: WplCodePKG,
    in_path: T,
    infra: InfraSinkAgent,
    max_event: usize,
) -> AnyResult<()> {
    let wpl_pkgs = WplRepository::from_wpl_strict(wpl_code)?;
    let engine = WplEngine::from_code(&wpl_pkgs, infra)?;
    let in_io = File::open(in_path)?;

    let source_key = Arc::new(DEFAULT_KEY.to_string());
    let source_tags = Arc::new(Tags::new());
    for (line_count, line_data) in BufReader::new(in_io)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        true_break!(line_count > max_event);
        true_continue!(line_data.is_empty());
        println!("{}", line_data);
        println!("{:~^120}", "");
        let setting = ParseOption::default();
        let event = SourceEvent::new(
            gen_pkg_id(), // 生成 event_id
            source_key.as_str(),
            RawData::String(line_data),
            Arc::clone(&source_tags),
        );
        let proc_unit = engine.proc_async(event, &setting);
        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(proc_unit))
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            rt.block_on(proc_unit)
        };
        if let Err(e) = res {
            println!("{}: {}", line_count, e);
        }
        println!("\n{:-^120}", "");
    }
    Ok(())
}

/// 工作台解析（直接使用文件路径）
pub fn wpl_workshop_parse(
    args: RunArgs,
    wpl_code: WplCodePKG,
    in_path: &str,
    infra: InfraSinkAgent,
) -> AnyResult<()> {
    let wpl_pkgs = WplRepository::from_wpl_strict(wpl_code)?;
    let engine = WplEngine::from_code(&wpl_pkgs, infra)?;
    let in_io = File::open(in_path)?;
    let source_key = Arc::new(DEFAULT_KEY.to_string());
    let source_tags = Arc::new(Tags::new());
    for (line_count, line_data) in BufReader::new(in_io)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        if args.ge_max(line_count) {
            break;
        }
        let setting = ParseOption::default();
        let event = SourceEvent::new(
            gen_pkg_id(), // 生成 event_id
            source_key.as_str(),
            RawData::String(line_data),
            Arc::clone(&source_tags),
        );
        let fut = engine.proc_async(event, &setting);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                let _ = handle.block_on(fut);
            });
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            let _ = rt.block_on(fut);
        }
    }
    Ok(())
}

// 旧的 test_prepare 已移除；测试可直接使用 RunArgs::for_test() 与文件路径。
