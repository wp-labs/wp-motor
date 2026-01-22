//! Facade: 生成器（规则/样本）对外入口（稳定 re-export）。

pub use crate::core::generator::rules::{GenRuleUnit, fetch_oml_data, load_gen_confs};
pub use crate::runtime::generator::types::{GenGRA, RuleGRA, SampleGRA};

use crate::sinks::SinkBackendType;
use wp_error::run_error::{RunError, RunReason, RunResult};

/// Backward‑compatible helper: run rule generator with preloaded rule units and a ready sink.
///
/// Notes:
/// - This is a lightweight, single‑sink runner intended for tests/benches and legacy callers.
/// - It honors `gen_conf.total_line`; other knobs (speed/parallel/stat) are ignored here.
pub async fn rule_gen_run(
    args: RuleGRA,
    rules: Vec<GenRuleUnit>,
    mut sink: SinkBackendType,
) -> RunResult<()> {
    // 全局 backoff gate 已移除；由发送单元在构建期与实时水位自决。
    use crate::runtime::generator::rule_source::RuleGenSource;
    use orion_error::UvsReason;
    use wp_error::run_error::RunErrorOwe;

    // Compile rules once; map compile error into RunError (UVS/conf)
    let src = RuleGenSource::from_units(rules)
        .map_err(|e| RunError::from(RunReason::Uvs(UvsReason::core_conf(e.to_string()))))?;

    let total = args.gen_conf.total_line.unwrap_or(0);
    if total == 0 {
        // nothing to do by request
        return Ok(());
    }

    // Generate in small batches to amortize overhead a bit (align with direct runner default)
    let batch = std::env::var("WF_GEN_BATCH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| (1..=4096).contains(&n))
        .unwrap_or(128);

    let mut produced = 0usize;
    let mut cur = 0usize;
    while produced < total {
        let remain = total - produced;
        let step = batch.min(remain).max(1);
        let rows = src
            .gen_batch(cur, step)
            .map_err(|e| RunError::from(RunReason::Uvs(UvsReason::core_conf(e.to_string()))))?;
        cur = (cur + step) % src.rule_len().max(1);
        for ffv in rows {
            // 将 FmtFieldVec 转换为字符串并调用 sink_str
            let raw_line = wpl::generator::RAWGenFmt(&ffv).to_string();
            wp_connector_api::AsyncRawDataSink::sink_str(&mut sink, &raw_line)
                .await
                .owe_sink()?; // map sink error into RunError(Dist)
            produced += 1;
        }
    }
    Ok(())
}

/// Backward‑compatible helper: run sample generator with given sample files and a ready sink.
///
/// Notes:
/// - Loads lines from the provided files and cycles through them until `gen_conf.total_line` is met.
/// - Intended for benches/tests; ignores speed/parallel/stat knobs.
pub async fn sample_gen_run(
    args: SampleGRA,
    mut sink: SinkBackendType,
    files: Vec<std::path::PathBuf>,
) -> RunResult<()> {
    // 全局 backoff gate 已移除；由发送单元在构建期与实时水位自决。
    use std::io::BufRead;
    use wp_connector_api::AsyncRawDataSink;
    use wp_error::run_error::RunErrorOwe;

    // Load all sample lines
    let mut samples: Vec<String> = Vec::new();
    for f in files {
        let file = std::fs::File::open(&f).map_err(|e| {
            RunError::from(RunReason::Uvs(orion_error::UvsReason::core_conf(
                e.to_string(),
            )))
        })?;
        let reader = std::io::BufReader::new(file);
        for s in reader.lines().map_while(Result::ok) {
            samples.push(s);
        }
    }
    if samples.is_empty() {
        return Ok(());
    }
    let total = args.gen_conf.total_line.unwrap_or(samples.len());
    for s in samples.iter().cycle().take(total) {
        AsyncRawDataSink::sink_str(&mut sink, s).await.owe_sink()?; // map sink error into RunError(Dist)
    }
    Ok(())
}
