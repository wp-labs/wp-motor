use super::common::{
    DEFAULT_UNIT_SIZE, build_sink_instance, default_batch, per_pipeline_speed,
    split_total_among_parallel,
};
use super::speed::{DynamicRateLimiter, SpeedProfile};
use crate::compat::LegacyOwe;
use crate::orchestrator::config::models::stat_reqs_from;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::generator::rule_source::RuleGenSource;
use crate::runtime::generator::types::GenGRA;
use crate::runtime::supervisor::monitor::{ActorMonitor, MonitorSinkHandle};
use crate::sinks::SinkBackendType;
use orion_error::conversion::ErrorWith;
use orion_variate::EnvDict;
use tokio::task::JoinHandle;
use wp_conf::stat::StatConf;
use wp_conf::structure::SinkInstanceConf;
use wp_error::run_error::RunReason;
use wp_error::run_error::{RunErrorOwe, RunResult};
use wp_log::info_ctrl;
use wp_stat::StatRecorder;
use wp_stat::StatStage; // for record_task

/// 批量发送一个“单元”的规则生成结果：逐条生成，按动态字节预算(`policy`)切分成多个子批，逐批 sink_str_batch。
async fn send_unit_rules(
    sink: &mut SinkBackendType,
    src: &std::sync::Arc<RuleGenSource>,
    cur_idx: &mut usize,
    unit_cnt: usize,
    collectors: &mut crate::stat::metric_collect::MetricCollectors,
    policy: &mut super::common::BatchSizePolicy,
) -> RunResult<usize> {
    use super::common::BATCH_FLUSH_LINES;
    use wp_stat::StatRecorder; // bring trait for record_task
    let rules_len = src.rule_len().max(1);
    if unit_cnt == 0 {
        return Ok(0);
    }
    // 逐条生成，按动态字节预算切分成多个子批，逐批 sink_str_batch。
    let mut held: Vec<String> = Vec::with_capacity(BATCH_FLUSH_LINES.min(unit_cnt));
    let mut batch_bytes: usize = 0;
    let mut sent = 0usize;
    // 局部宏：把当前 held 借出为 &str 子批发送、统计、清空（不重置 batch_bytes，由调用点负责）
    macro_rules! flush_held {
        () => {{
            if held.is_empty() {
                0usize
            } else {
                let cnt = held.len();
                let refs: Vec<&str> = held.iter().map(|s| s.as_str()).collect();
                wp_connector_api::AsyncRawDataSink::sink_str_batch(sink, refs)
                    .await
                    .owe_sink()
                    .with_context("gen_direct_rule")
                    .doing("write rule batch to sink")?;
                for _ in 0..cnt {
                    collectors.record_task("gen_direct_rule", ());
                }
                sent += cnt;
                held.clear();
                cnt
            }
        }};
    }
    for _ in 0..unit_cnt {
        let ffv = src
            .gen_one(*cur_idx)
            .owe(RunReason::core_conf())
            .with_context(format!("rule_idx={}", *cur_idx))
            .doing("generate rule record")?;
        *cur_idx = (*cur_idx + 1) % rules_len;
        let raw_line = wpl::generator::RAWGenFmt(&ffv).to_string();
        let len = raw_line.len();
        // 达到动态预算或行数兼底，先下发当前子批（先判后观，预算不含本行）
        if (batch_bytes + len > policy.budget_bytes() && !held.is_empty())
            || held.len() >= BATCH_FLUSH_LINES
        {
            flush_held!();
            batch_bytes = 0;
        }
        policy.observe_line(len);
        batch_bytes += len;
        held.push(raw_line);
    }
    // 尾批
    if !held.is_empty() {
        flush_held!();
    }
    Ok(sent)
}

pub async fn run_rule_direct(
    rule_root: &str,
    gar: &GenGRA,
    out_conf: &SinkInstanceConf,
    rate_limit_rps: usize,
    dict: &EnvDict,
) -> RunResult<()> {
    // 全局限速目标（构建期提示）：生成器直连路径在构建 sink 前设置；0 表示无限速。
    crate::sinks::set_global_rate_limit_rps(gar.gen_speed);
    // 全局 backoff gate 移除；由发送单元在构建期与实时水位自决。
    info_ctrl!(
        "run_rule_direct: rule_root='{}', parallel={}, total_line={:?}",
        rule_root,
        gar.parallel,
        gar.total_line
    );
    let units = crate::core::generator::rules::load_gen_confs(rule_root, dict)
        .owe(RunReason::rule_error())
        .with_context(rule_root)
        .doing("load rule")?;
    info_ctrl!("run_rule_direct: loaded {} rule units", units.len());
    let source = RuleGenSource::from_units(units)
        .owe(RunReason::core_conf())
        .with_context(rule_root)
        .doing("build rule source from units")?;
    let source = std::sync::Arc::new(source);
    let parallel = std::cmp::max(1, gar.parallel);
    let batch = default_batch();
    info_ctrl!("run_rule_direct: batch={} (const)", batch);

    // 任务分配：平均 + 余数前置
    let per_counts = split_total_among_parallel(parallel, gar.total_line);

    // 监控
    let moni_group = TaskGroup::new("moni", ShutdownCmd::Timeout(200));
    let mut actor_mon = ActorMonitor::new(
        moni_group.subscribe(),
        MonitorSinkHandle::new(None),
        gar.stat_print,
        gar.stat_sec,
    );
    let mon_s = actor_mon.send_agent();
    let stat_reqs = stat_reqs_from(&StatConf::gen_default());
    let sink_reqs = stat_reqs.get_requ_items(StatStage::Sink);
    tokio::spawn(async move {
        let _ = actor_mon.stat_proc(Vec::new()).await;
    });

    // 速率配置：获取速度模型并分配给各 pipeline
    let base_speed = gar.base_speed();
    let speed_profile = gar.get_speed_profile();
    let per_speed = per_pipeline_speed(base_speed, parallel);
    info_ctrl!(
        "run_rule_direct: speed_profile={:?}, base_speed={}, per_pipeline={:?}",
        speed_profile,
        base_speed,
        per_speed
    );

    // 启动流水线
    let start_at = std::time::Instant::now();
    let mut tasks: Vec<JoinHandle<RunResult<usize>>> = Vec::with_capacity(parallel);
    for (i, cnt) in per_counts.iter().copied().enumerate().take(parallel) {
        let sink = build_sink_instance(out_conf, i, parallel, rate_limit_rps).await?;
        let src = source.clone();
        let mon = mon_s.clone();
        let reqs = sink_reqs.clone();
        let profile = speed_profile.clone();
        info_ctrl!("run_rule_direct: spawn pipeline {} with count={:?}", i, cnt);
        let unit_size_cfg = DEFAULT_UNIT_SIZE;
        let pipe_idx = i;
        let pipe_cnt = parallel;
        tasks.push(tokio::spawn(async move {
            run_rule_pipeline(
                sink,
                src,
                cnt,
                profile,
                pipe_idx,
                pipe_cnt,
                unit_size_cfg,
                mon,
                reqs,
            )
            .await
        }));
    }

    let mut total_produced: usize = 0;
    for t in tasks {
        let n = t
            .await
            .owe(RunReason::core_conf())
            .with_context("gen_direct_rule")
            .doing("join rule pipeline task")??;
        total_produced += n;
    }
    info_ctrl!("run_rule_direct: all pipelines finished");
    let elapsed = start_at.elapsed();
    let ms = elapsed.as_millis();
    info_ctrl!(
        "run_rule_direct: summary generated={} lines, elapsed={} ms, parallel={}, batch={}",
        total_produced,
        ms,
        parallel,
        batch
    );
    println!(
        "wpgen summary: generated={} lines, elapsed={} ms, mode=direct, parallel={}, batch={}",
        total_produced, ms, parallel, batch
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_rule_pipeline(
    mut sink: SinkBackendType,
    src: std::sync::Arc<RuleGenSource>,
    count: Option<usize>,
    speed_profile: SpeedProfile,
    pipe_idx: usize,
    pipe_cnt: usize,
    unit_size_cfg: usize,
    mon_s: crate::stat::MonSend,
    sink_reqs: Vec<wp_stat::StatReq>,
) -> RunResult<usize> {
    use crate::stat::metric_collect::MetricCollectors;

    // 统计/速率器
    let mut collectors = MetricCollectors::new("gen_direct_rule".to_string(), sink_reqs);

    // 为此 pipeline 创建速率限制器
    // 根据 pipeline 数量调整速度模型
    let adjusted_profile = adjust_profile_for_pipeline(&speed_profile, pipe_cnt);
    let base_rate = adjusted_profile.base_rate();
    let unit_size = if base_rate > 0 {
        (base_rate / 10).clamp(1, 1000)
    } else {
        unit_size_cfg.max(1)
    };
    let mut limiter =
        DynamicRateLimiter::new(adjusted_profile, &format!("gen_rule_pipe_{}", pipe_idx));
    // 动态批量大小策略：基于 base_rate × EMA 行长 × 时间窗
    let mut batch_policy = super::common::BatchSizePolicy::new(base_rate);

    // 迭代状态
    let mut produced = 0usize;
    let mut cur_idx = 0usize; // 当前规则索引起点
    let total_limit = count; // 可选总量限制

    // 批量发送一个"单元"，然后统一进行限速；统计以 1W 行为粒度计数，保持表格速度单位与"W(万)"对齐。
    const REPORT_LINES_PER_TASK: usize = 10_000; // 1W 行 -> 1 个 task
    let mut acc_lines: usize = 0;
    loop {
        if let Some(limit) = total_limit
            && produced >= limit
        {
            break;
        }
        let left_global = total_limit
            .map(|l| l.saturating_sub(produced))
            .unwrap_or(usize::MAX);
        if left_global == 0 {
            break;
        }
        let take = unit_size.min(left_global);
        let sent = send_unit_rules(
            &mut sink,
            &src,
            &mut cur_idx,
            take,
            &mut collectors,
            &mut batch_policy,
        )
        .await?;
        produced += sent;
        acc_lines += sent;
        let mut reported = 0usize;
        while acc_lines >= REPORT_LINES_PER_TASK {
            collectors.record_task("gen_direct_rule", ());
            acc_lines -= REPORT_LINES_PER_TASK;
            reported += 1;
        }
        if reported > 0 {
            let _ = collectors.send_stat(&mon_s).await;
        }
        // 使用动态速率限制器
        let wait = limiter.consume(sent);
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
    Ok(produced)
}

/// 根据 pipeline 数量调整速度模型
fn adjust_profile_for_pipeline(profile: &SpeedProfile, pipe_cnt: usize) -> SpeedProfile {
    if pipe_cnt <= 1 {
        return profile.clone();
    }

    match profile {
        SpeedProfile::Constant(rate) => SpeedProfile::Constant(rate / pipe_cnt),
        SpeedProfile::Sinusoidal {
            base,
            amplitude,
            period_secs,
        } => SpeedProfile::Sinusoidal {
            base: base / pipe_cnt,
            amplitude: amplitude / pipe_cnt,
            period_secs: *period_secs,
        },
        SpeedProfile::Stepped {
            steps,
            loop_forever,
        } => SpeedProfile::Stepped {
            steps: steps
                .iter()
                .map(|(dur, rate)| (*dur, rate / pipe_cnt))
                .collect(),
            loop_forever: *loop_forever,
        },
        SpeedProfile::Burst {
            base,
            burst_rate,
            burst_duration_ms,
            burst_probability,
        } => SpeedProfile::Burst {
            base: base / pipe_cnt,
            burst_rate: burst_rate / pipe_cnt,
            burst_duration_ms: *burst_duration_ms,
            burst_probability: *burst_probability,
        },
        SpeedProfile::Ramp {
            start,
            end,
            duration_secs,
        } => SpeedProfile::Ramp {
            start: start / pipe_cnt,
            end: end / pipe_cnt,
            duration_secs: *duration_secs,
        },
        SpeedProfile::RandomWalk { base, variance } => SpeedProfile::RandomWalk {
            base: base / pipe_cnt,
            variance: *variance,
        },
        SpeedProfile::Composite {
            profiles,
            combine_mode,
        } => SpeedProfile::Composite {
            profiles: profiles
                .iter()
                .map(|p| adjust_profile_for_pipeline(p, pipe_cnt))
                .collect(),
            combine_mode: combine_mode.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::generator::rules::GenRuleUnit;
    use crate::runtime::generator::common::{BATCH_FLUSH_LINES, BatchSizePolicy};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use wp_connector_api::{AsyncCtrl, AsyncRawDataSink, AsyncRecordSink, SinkResult};
    use wp_model_core::model::FNameStr;
    use wpl::WplCode;
    use wpl::generator::FieldGenBuilder;

    /// 构造一个稳定的 RuleGenSource：一条规则，一个 digit 字段 x ∈ [100,200)。
    fn make_rule_source() -> Arc<RuleGenSource> {
        let code = r#"package t { rule gen1 { ( digit : x ) } }"#;
        let pkg = WplCode::build(std::path::PathBuf::from("inline.wpl"), code)
            .expect("build wpl")
            .parse_pkg()
            .expect("parse pkg");
        let fields: HashMap<FNameStr, _> = FieldGenBuilder::new().digit("x", 100, 200).build();
        let unit = GenRuleUnit::new(pkg, fields);
        Arc::new(RuleGenSource::from_units(vec![unit]).expect("compile rule"))
    }

    #[derive(Default)]
    struct RecState {
        batches: Vec<Vec<String>>,
        single_calls: usize,
    }

    struct RecordingSink {
        state: Arc<Mutex<RecState>>,
    }

    impl RecordingSink {
        fn new() -> (Self, Arc<Mutex<RecState>>) {
            let state = Arc::new(Mutex::new(RecState::default()));
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    #[async_trait]
    impl AsyncCtrl for RecordingSink {
        async fn stop(&mut self) -> SinkResult<()> {
            Ok(())
        }
        async fn reconnect(&mut self) -> SinkResult<()> {
            Ok(())
        }
    }
    #[async_trait]
    impl AsyncRecordSink for RecordingSink {
        async fn sink_record(
            &mut self,
            _data: &wp_model_core::model::DataRecord,
        ) -> SinkResult<()> {
            Ok(())
        }
        async fn sink_records(
            &mut self,
            _data: Vec<std::sync::Arc<wp_model_core::model::DataRecord>>,
        ) -> SinkResult<()> {
            Ok(())
        }
    }
    #[async_trait]
    impl AsyncRawDataSink for RecordingSink {
        async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
            let mut s = self.state.lock().unwrap();
            s.single_calls += 1;
            s.batches.push(vec![data.to_string()]);
            Ok(())
        }
        async fn sink_bytes(&mut self, _data: &[u8]) -> SinkResult<()> {
            Ok(())
        }
        async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
            self.state
                .lock()
                .unwrap()
                .batches
                .push(data.into_iter().map(|s| s.to_string()).collect());
            Ok(())
        }
        async fn sink_bytes_batch(&mut self, _data: Vec<&[u8]>) -> SinkResult<()> {
            Ok(())
        }
    }

    fn empty_collectors() -> crate::stat::metric_collect::MetricCollectors {
        crate::stat::metric_collect::MetricCollectors::new(
            "gen_direct_rule".to_string(),
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn rule_empty_unit_sends_nothing() {
        let src = make_rule_source();
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_rules(&mut sink, &src, &mut idx, 0, &mut collectors, &mut policy)
            .await
            .unwrap();
        assert_eq!(sent, 0);
        let s = state.lock().unwrap();
        assert!(s.batches.is_empty());
        assert_eq!(s.single_calls, 0);
    }

    #[tokio::test]
    async fn rule_batch_path_no_single_calls() {
        let src = make_rule_source();
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_rules(
            &mut sink,
            &src,
            &mut idx,
            1000,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 1000, "sent must equal unit_cnt");
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0, "must use sink_str_batch, not sink_str");
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, 1000, "all generated lines must be flushed");
    }

    #[tokio::test]
    async fn rule_large_unit_splits_into_subbatches() {
        // 生成 10000 行，每行 digit 字段（短），应被字节预算切成多个子批。
        let src = make_rule_source();
        let unit = 10_000;
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_rules(
            &mut sink,
            &src,
            &mut idx,
            unit,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, unit);
        let s = state.lock().unwrap();
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, unit);
        // 每个子批行数不超过行数兼底
        for b in &s.batches {
            assert!(b.len() <= BATCH_FLUSH_LINES, "line cap breached");
        }
    }
}
