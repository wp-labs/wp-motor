use glob::glob;
use orion_conf::ErrorWith;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{
    UvsFrom, UvsReason, compat_prelude::ErrorOweBase, conversion::WrapStructErrorAs,
};
use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use wp_conf::structure::SinkInstanceConf;
use wp_engine::facade::config::{WarpConf, WpGenResolved};
use wp_engine::facade::generator::SampleGRA;
use wp_engine::runtime::generator::run_rule_direct;
use wp_engine::runtime::generator::run_sample_direct;
use wp_error::run_error::{RunReason, RunResult};
use wp_log::info_ctrl;

/// 加载 wpgen 配置并解析输出；支持命令行覆盖输出到文件路径
pub fn load_wpgen_resolved(
    conf_name: &str,
    god: &WarpConf,
    dict: &EnvDict,
) -> OrionConfResult<WpGenResolved> {
    let rt = god
        .load_wpgen_config(conf_name, dict)
        .owe(ConfIOReason::Uvs(UvsReason::core_conf()))?;
    Ok(rt)
}

/// 统一日志输出（解析后的 out_sink 关键信息）
pub fn log_resolved_out_sink(wgr: &WpGenResolved) {
    let kind = wgr.out_sink.resolved_kind_str();
    let params = wgr.out_sink.resolved_params_table();
    // 不再按固定键提取（不同 sink 的参数不同）；直接输出完整 params，便于排查
    info_ctrl!(
        "wpgen out sink resolved: kind={}, params={:?}",
        kind,
        params
    );
}

/// 初始化 wpgen 配置（conf/wpgen.toml），纯函数：不打印、不初始化日志
pub fn gen_conf_init<P: AsRef<Path>>(work_root: P) -> OrionConfResult<()> {
    use wp_engine::facade::config::{WPGEN_TOML, WarpConf, WpGenConfig};
    let work_root = work_root.as_ref();
    let god = WarpConf::new(work_root);
    god.create_config_delegate::<WpGenConfig>(WPGEN_TOML)
        .init()?;
    Ok(())
}

/// 清理 wpgen 配置（conf/wpgen.toml 及相关生成物，遵循 safe_clean），纯函数：不打印
pub fn gen_conf_clean<P: AsRef<Path>>(work_root: P) -> OrionConfResult<()> {
    use wp_engine::facade::config::{WPGEN_TOML, WarpConf, WpGenConfig};
    let work_root = work_root.as_ref();
    let god = WarpConf::new(work_root);
    god.create_config_delegate::<WpGenConfig>(WPGEN_TOML)
        .safe_clean()
}

/// 检查 wpgen 配置是否存在且可解析
pub fn gen_conf_check<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> OrionConfResult<()> {
    use wp_engine::facade::config::{WPGEN_TOML, WarpConf};
    let work_root = work_root.as_ref();
    let god = WarpConf::new(work_root);
    let _ = load_wpgen_resolved(WPGEN_TOML, &god, dict)?;
    Ok(())
}

/// 单文件输出清理报告（针对 wpgen 的 out sink）
#[derive(Debug, Clone, Default)]
pub struct GenCleanReport {
    pub path: Option<String>,
    pub existed: bool,
    pub cleaned: bool,
    /// 备注：如非 file sink、配置缺失等
    pub message: Option<String>,
}

fn absolute_output_path(work_root: &str, raw: &str) -> PathBuf {
    let resolved_path = Path::new(raw);
    if resolved_path.is_absolute() {
        resolved_path.to_path_buf()
    } else {
        Path::new(work_root).join(resolved_path)
    }
}

fn shard_glob_pattern(primary: &Path) -> Option<String> {
    let file_name = primary.file_name()?.to_str()?;
    let parent = primary.parent()?;
    if let Some((stem, ext)) = file_name.rsplit_once('.') {
        Some(
            parent
                .join(format!("{stem}-r*.{ext}"))
                .to_string_lossy()
                .to_string(),
        )
    } else {
        Some(
            parent
                .join(format!("{file_name}-r*"))
                .to_string_lossy()
                .to_string(),
        )
    }
}

fn cleanup_targets(conf: &WpGenResolved, work_root: &str) -> Vec<PathBuf> {
    let Some(raw) = conf.out_sink.resolve_file_path() else {
        return Vec::new();
    };
    let primary = absolute_output_path(work_root, &raw);
    let mut targets = vec![primary.clone()];

    if let Some(pattern) = shard_glob_pattern(&primary)
        && let Ok(entries) = glob(&pattern)
    {
        for entry in entries.flatten() {
            if entry.is_file() {
                targets.push(entry);
            }
        }
    }

    targets.sort();
    targets.dedup();
    targets
}

/// 清理 wpgen 的输出文件（如果 out sink 为 file），返回结构化报告；不打印
pub fn clean_wpgen_output_file(
    work_root: &str,
    conf_name: &str,
    local_only: bool,
    dict: &EnvDict,
) -> RunResult<GenCleanReport> {
    if !local_only {
        return Ok(GenCleanReport {
            message: Some("local_only=false (skip)".to_string()),
            ..Default::default()
        });
    }
    let god = WarpConf::new(work_root);
    let conf = load_wpgen_resolved(conf_name, &god, dict)
        .map_err(|e| e.wrap_as(RunReason::from_conf(), format!("load {conf_name}")))?;
    if let Some(p) = conf.out_sink.resolve_file_path() {
        let full_path = absolute_output_path(work_root, &p);
        let targets = cleanup_targets(&conf, work_root);
        let mut existed = false;
        let mut cleaned_any = false;
        for target in targets {
            if target.exists() {
                existed = true;
                std::fs::remove_file(&target)
                    .owe(RunReason::from_conf())
                    .with_context(&target)
                    .doing("remove wpgen output")?;
                cleaned_any = true;
            }
        }
        Ok(GenCleanReport {
            path: Some(full_path.to_string_lossy().to_string()),
            existed,
            cleaned: cleaned_any,
            message: None,
        })
    } else {
        Ok(GenCleanReport {
            path: None,
            existed: false,
            cleaned: false,
            message: Some("output target is not a file sink".to_string()),
        })
    }
}

/// 直连执行（规则：样本），使用 out_sink 规格按副本构建 sink 实例。
pub async fn sample_exec_direct_core(
    rule_root: &str,
    find_name: &str,
    prepared: (SampleGRA, SinkInstanceConf),
    rate_limit_rps: usize,
) -> RunResult<()> {
    let g = prepared.0.gen_conf.clone();
    run_sample_direct(rule_root, find_name, &g, &prepared.1, rate_limit_rps).await
}
/// 直连执行（规则）：使用 out_sink 规格按副本构建 sink 实例，预编译规则后直接发送。
pub async fn rule_exec_direct_core(
    stat_print: bool,
    rule_root: &str,
    prepared: (
        wp_engine::facade::generator::RuleGRA,
        wp_conf::structure::SinkInstanceConf,
    ),
    rate_limit_rps: usize,
    dict: &EnvDict,
) -> RunResult<()> {
    let g = prepared.0.gen_conf.clone();
    // stat_print 目前只用于日志输出控制；此处不额外处理
    let _ = stat_print;
    run_rule_direct(rule_root, &g, &prepared.1, rate_limit_rps, dict).await
}
