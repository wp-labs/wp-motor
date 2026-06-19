use super::common::{DEFAULT_UNIT_SIZE, build_sink_instance};
use super::speed::{DynamicRateLimiter, SpeedProfile};
use crate::compat::LegacyOwe;
use crate::orchestrator::config::models::stat_reqs_from;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::generator::types::GenGRA;
use crate::runtime::supervisor::monitor::{ActorMonitor, MonitorSinkHandle};
use crate::sinks::SinkBackendType;
use crate::stat::metric_collect::MetricCollectors;
use orion_conf::ErrorWith;
use orion_error::conversion::ToStructError;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::task::JoinHandle;
use wp_conf::stat::StatConf;
use wp_conf::structure::SinkInstanceConf;
use wp_error::run_error::{RunErrorOwe, RunReason, RunResult};
use wp_log::info_ctrl;
use wp_stat::{StatRecorder, StatStage};

fn load_samples(rule_root: &str, find_name: &str) -> RunResult<Vec<String>> {
    use std::io::BufRead;
    // discover files
    let files = wp_conf::utils::find_conf_files(rule_root, find_name)
        .owe(RunReason::core_conf())
        .with_context(rule_root)
        .doing("find sample files")?;
    info_ctrl!("run_sample_direct: found {} files", files.len());
    if files.is_empty() {
        return Err(RunReason::core_conf().to_err().with_detail(format!(
            "sample files not found under '{}' for pattern '{}'",
            rule_root, find_name
        )));
    }
    // load lines
    let mut out = Vec::new();
    for f in files {
        let file = std::fs::File::open(&f)
            .owe(RunReason::core_conf())
            .with_context(&f)
            .doing("open sample file")?;
        let reader = std::io::BufReader::new(file);
        for s in reader.lines().map_while(Result::ok) {
            out.push(s);
        }
    }
    Ok(out)
}

/// 批量发送一个“单元”的样本：按动态字节预算(`policy`)切分成多个子批，逐批 sink_str_batch。
/// 预算由 BatchSizePolicy 根据 rate × EMA 行长 × 时间窗自适应，混合行长下也能稳定。
async fn send_unit_samples(
    sink: &mut SinkBackendType,
    samples: &Arc<Vec<String>>,
    cur_idx: &mut usize,
    unit_cnt: usize,
    collectors: &mut MetricCollectors,
    policy: &mut super::common::BatchSizePolicy,
) -> RunResult<usize> {
    use super::common::BATCH_FLUSH_LINES;
    let n = samples.len().max(1);
    if unit_cnt == 0 {
        return Ok(0);
    }
    let mut sent = 0usize;
    let mut batch: Vec<&str> = Vec::with_capacity(BATCH_FLUSH_LINES.min(unit_cnt));
    let mut batch_bytes: usize = 0;
    for _ in 0..unit_cnt {
        let line: &str = samples[*cur_idx].as_str();
        let len = line.len();
        // 达到动态预算或行数兼底，先下发当前子批（先判后观，预算不含本行）
        if (batch_bytes + len > policy.budget_bytes() && !batch.is_empty())
            || batch.len() >= BATCH_FLUSH_LINES
        {
            let cnt = batch.len();
            wp_connector_api::AsyncRawDataSink::sink_str_batch(sink, std::mem::take(&mut batch))
                .await
                .owe_sink()
                .with_context("gen_direct")
                .doing("write sample batch to sink")?;
            for _ in 0..cnt {
                collectors.record_task("gen_direct", ());
            }
            sent += cnt;
            batch_bytes = 0;
        }
        policy.observe_line(len);
        batch_bytes += len;
        batch.push(line);
        *cur_idx = (*cur_idx + 1) % n;
    }
    // 尾批
    if !batch.is_empty() {
        let cnt = batch.len();
        wp_connector_api::AsyncRawDataSink::sink_str_batch(sink, std::mem::take(&mut batch))
            .await
            .owe_sink()
            .with_context("gen_direct")
            .doing("write sample batch to sink")?;
        for _ in 0..cnt {
            collectors.record_task("gen_direct", ());
        }
        sent += cnt;
    }
    Ok(sent)
}

#[derive(Clone)]
struct SharedTotal {
    produced: Arc<AtomicUsize>,
    limit: usize,
}

impl SharedTotal {
    fn new(limit: usize) -> Self {
        Self {
            produced: Arc::new(AtomicUsize::new(0)),
            limit,
        }
    }

    fn reserve(&self, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }
        loop {
            let current = self.produced.load(Ordering::Relaxed);
            if current >= self.limit {
                return 0;
            }
            let remaining = self.limit - current;
            let to_take = desired.min(remaining);
            if self
                .produced
                .compare_exchange(
                    current,
                    current + to_take,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return to_take;
            }
        }
    }

    fn release(&self, amount: usize) {
        if amount == 0 {
            return;
        }
        self.produced.fetch_sub(amount, Ordering::Relaxed);
    }
}

#[derive(Clone)]
enum WorkQuota {
    Unlimited,
    Shared(SharedTotal),
}

impl WorkQuota {
    fn from_total(total: Option<usize>) -> Self {
        total.map_or(Self::Unlimited, |limit| {
            Self::Shared(SharedTotal::new(limit))
        })
    }

    fn take(&self, desired: usize) -> usize {
        match self {
            WorkQuota::Unlimited => desired,
            WorkQuota::Shared(shared) => shared.reserve(desired),
        }
    }

    fn release(&self, amount: usize) {
        if let WorkQuota::Shared(shared) = self {
            shared.release(amount);
        }
    }
}

/// 单条样本直连流水线：按微批次生成并发送，返回本流水线产出的总条数。
async fn run_pipeline(
    mut sink: SinkBackendType,
    samples: Arc<Vec<String>>,
    quota: WorkQuota,
    speed_profile: SpeedProfile,
    pipe_idx: usize,
    mon_s: crate::stat::MonSend,
    sink_reqs: Vec<wp_stat::StatReq>,
) -> RunResult<usize> {
    // 统计/速率器
    let unit_size_cfg = DEFAULT_UNIT_SIZE;
    let mut collectors = MetricCollectors::new("gen_direct".to_string(), sink_reqs);
    let base_rate = speed_profile.base_rate();
    let unit_size = if base_rate > 0 {
        (base_rate / 10).clamp(1, 1000)
    } else {
        unit_size_cfg.max(1)
    };
    let mut limiter =
        DynamicRateLimiter::new(speed_profile, &format!("gen_sample_pipe_{}", pipe_idx));
    // 动态批量大小策略：基于 base_rate × EMA 行长 × 时间窗
    let mut batch_policy = super::common::BatchSizePolicy::new(base_rate);

    // 迭代状态
    let mut cur_idx = 0usize;
    let mut produced = 0usize; // 全局累计
    // 不做微批缓冲：逐条发送

    // 批量发送一个"单元"，然后统一进行限速；统计：按条进行。
    loop {
        let reserved = quota.take(unit_size);
        if reserved == 0 {
            break;
        }
        let sent = match send_unit_samples(
            &mut sink,
            &samples,
            &mut cur_idx,
            reserved,
            &mut collectors,
            &mut batch_policy,
        )
        .await
        {
            Ok(sent) => {
                if sent < reserved {
                    quota.release(reserved - sent);
                }
                sent
            }
            Err(e) => {
                quota.release(reserved);
                return Err(e);
            }
        };
        produced += sent;
        // 单元完成后发一次快照
        let _ = collectors.send_stat(&mon_s).await;
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

pub async fn run_sample_direct(
    rule_root: &str,
    find_name: &str,
    gar: &GenGRA,
    out_conf: &SinkInstanceConf,
    rate_limit_rps: usize,
) -> RunResult<()> {
    // 全局限速目标（构建期提示）
    crate::sinks::set_global_rate_limit_rps(gar.base_speed());
    info_ctrl!(
        "run_sample_direct: rule_root='{}', find_name='{}', parallel={}, total_line={:?}",
        rule_root,
        find_name,
        gar.parallel,
        gar.total_line
    );
    // 查找并加载样本（包含空集检查与日志）
    let samples = load_samples(rule_root, find_name)?;
    info_ctrl!("run_sample_direct: loaded {} sample lines", samples.len());
    let samples = Arc::new(samples);
    let parallel = std::cmp::max(1, gar.parallel);
    let quota = WorkQuota::from_total(gar.total_line);

    // 速率配置
    let speed_profile = gar.get_speed_profile();
    info_ctrl!(
        "run_sample_direct: speed_profile={:?}, base_speed={}",
        speed_profile,
        gar.base_speed()
    );

    // 监控：启动监控任务
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
    let monitor_reqs = stat_reqs.get_all().clone();
    tokio::spawn(async move {
        let _ = actor_mon.stat_proc(monitor_reqs).await;
    });

    let start_at = std::time::Instant::now();
    let mut tasks: Vec<JoinHandle<RunResult<usize>>> = Vec::with_capacity(parallel);
    for i in 0..parallel {
        let sink = build_sink_instance(out_conf, i, parallel, rate_limit_rps).await?;
        let s = samples.clone();
        info_ctrl!(
            "run_sample_direct: spawn pipeline {} (shared_total={:?})",
            i,
            gar.total_line
        );
        let mon = mon_s.clone();
        let reqs = sink_reqs.clone();
        let profile = adjust_profile_for_pipeline(&speed_profile, parallel);
        let quota = quota.clone();
        let pipe_idx = i;
        tasks.push(tokio::spawn(async move {
            run_pipeline(sink, s, quota, profile, pipe_idx, mon, reqs).await
        }));
    }
    let mut total_produced: usize = 0;
    for t in tasks {
        let produced = t
            .await
            .owe(RunReason::core_conf())
            .with_context("gen_direct")
            .doing("join sample pipeline task")??;
        total_produced += produced;
    }
    info_ctrl!("run_sample_direct: all pipelines finished");
    let elapsed = start_at.elapsed();
    let ms = elapsed.as_millis();
    info_ctrl!(
        "run_sample_direct: summary generated={} lines, elapsed={} ms, parallel={} ",
        total_produced,
        ms,
        parallel,
    );
    println!(
        "wpgen summary: generated={} lines, elapsed={} ms, mode=direct, parallel={} ",
        total_produced, ms, parallel,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::generator::common::{
        BATCH_BUDGET_MAX, BATCH_BUDGET_MIN, BATCH_EMA_ALPHA, BATCH_FLUSH_LINES, BATCH_SEED_BUDGET,
        BatchSizePolicy,
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use wp_connector_api::{AsyncCtrl, AsyncRawDataSink, AsyncRecordSink, SinkResult};

    /// sink 记录的共享状态：sink move 进 Proxy 后仍可通过克隆的 Arc 读取。
    #[derive(Default, Clone)]
    struct RecState {
        /// 每次 sink_str_batch 收到的子批（每个子批 = Vec<String>）
        batches: Vec<Vec<String>>,
        /// 逐行 sink_str 被调用的次数（批量路径下应为 0）
        single_calls: usize,
    }

    /// 可观测 sink：通过共享 Arc<Mutex<RecState>> 记录所有收到的写入。
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

    fn empty_collectors() -> MetricCollectors {
        MetricCollectors::new("gen_direct".to_string(), Vec::new())
    }

    /// 构造 n 行样本，每行 = line。
    fn make_samples(n: usize, line: &str) -> Arc<Vec<String>> {
        Arc::new((0..n).map(|_| line.to_string()).collect())
    }

    /// 子批中一个子批的字节数（对齐发送循环的实际累加逻辑）
    fn batch_bytes(b: &[String]) -> usize {
        b.iter().map(|s| s.len()).sum()
    }

    // ===== BatchSizePolicy 单元测试：预算计算与 EMA 行为 =====

    #[test]
    fn policy_unlimited_rate_caps_at_max() {
        let mut p = BatchSizePolicy::new(0); // 不限速
        assert_eq!(
            p.budget_bytes(),
            BATCH_BUDGET_MAX,
            "unlimited -> MAX even before obs"
        );
        p.observe_line(100);
        assert_eq!(
            p.budget_bytes(),
            BATCH_BUDGET_MAX,
            "unlimited -> MAX after obs"
        );
    }

    #[test]
    fn policy_uses_seed_before_first_observation() {
        let p = BatchSizePolicy::new(10_000);
        assert_eq!(
            p.budget_bytes(),
            BATCH_SEED_BUDGET,
            "seed budget before first line"
        );
    }

    #[test]
    fn policy_first_observation_initializes_avg() {
        let mut p = BatchSizePolicy::new(10_000);
        p.observe_line(200);
        assert_eq!(p.avg_line_bytes(), 200.0);
        // byte_rate = 10000 * 200 = 2_000_000 B/s; ×0.1s = 200_000 -> clamp(MAX=1MiB)
        assert_eq!(p.budget_bytes(), 200_000);
    }

    #[test]
    fn policy_ema_converges_toward_recent_lines() {
        // α=0.02：喂入大量长行后，avg 应逐步趋近长行值
        let mut p = BatchSizePolicy::new(1);
        for _ in 0..5000 {
            p.observe_line(1000);
        }
        assert!(
            p.avg_line_bytes() > 990.0,
            "ema should converge near 1000, got {}",
            p.avg_line_bytes()
        );
    }

    #[test]
    fn policy_budget_scales_with_rate_and_line_size() {
        // 同样行长 100B，速率越高预算越大
        let mk = |rate| {
            let mut p = BatchSizePolicy::new(rate);
            p.observe_line(100);
            p.budget_bytes()
        };
        let low = mk(500); // 500*100*0.1 = 5_000 -> clamp MIN = 8KiB
        let high = mk(1_000_000); // 1e6*100*0.1 = 1e7 -> clamp MAX
        assert_eq!(low, BATCH_BUDGET_MIN, "low byte-rate floors at MIN");
        assert_eq!(high, BATCH_BUDGET_MAX, "high byte-rate caps at MAX");
        // 中间档：100_000 EPS × 100B × 0.1 = 1_000_000，在 [MIN,MAX] 内
        assert_eq!(mk(100_000), 1_000_000);
    }

    #[test]
    fn policy_budget_respects_time_window_formula() {
        // 50_000 EPS × 40B = 2_000_000 B/s × 0.1s = 200_000B，落在 [MIN,MAX] 内
        let mut p = BatchSizePolicy::new(50_000);
        p.observe_line(40);
        assert_eq!(p.budget_bytes(), 200_000);
    }

    // ===== send_unit_samples：批量路径与数据完整性 =====

    #[tokio::test]
    async fn empty_unit_sends_nothing() {
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let samples = make_samples(10, "x");
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            0,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 0);
        let s = state.lock().unwrap();
        assert!(s.batches.is_empty(), "no batch for unit_cnt=0");
        assert_eq!(s.single_calls, 0);
    }

    #[tokio::test]
    async fn batch_path_no_single_calls() {
        // 任何非零 unit 都应走 sink_str_batch，逐行 sink_str 调用次数 = 0
        let line = "x".repeat(100);
        let samples = make_samples(1000, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            1000,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 1000);
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0, "must use sink_str_batch, not sink_str");
    }

    #[tokio::test]
    async fn unlimited_rate_splits_large_unit_into_subbatches() {
        // 不限速：预算=MAX(1MiB)。10000 行 × 100B ≈ 1MB，接近上限，应切成多个子批。
        let line = "x".repeat(100);
        let unit = 10_000;
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            unit,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();

        assert_eq!(sent, unit, "total sent must equal unit_cnt");
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0);
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, unit, "all lines must be flushed");
        // 每个子批行数不超过行数兼底
        for b in s.batches.iter() {
            assert!(b.len() <= BATCH_FLUSH_LINES, "sub-batch line cap breached");
        }
        // 内容顺序守恒
        let flat: Vec<String> = s.batches.iter().flatten().cloned().collect();
        for v in &flat {
            assert_eq!(v, &line);
        }
        assert_eq!(idx, 0);
    }

    #[tokio::test]
    async fn huge_single_line_flushes_alone() {
        // 单行 > 预算：该行应独占一个子批（首行进批后，下一行触发切）
        let huge = "y".repeat(BATCH_BUDGET_MAX + 1000);
        let samples = Arc::new(vec![huge.clone(), huge.clone(), huge.clone()]);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            3,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 3);
        let s = state.lock().unwrap();
        // 每行都远超预算，每行独占一个子批 → 3 个子批各 1 行
        assert_eq!(s.batches.len(), 3, "each huge line should be its own batch");
        for b in &s.batches {
            assert_eq!(b.len(), 1);
            assert_eq!(b[0], huge);
        }
    }

    #[tokio::test]
    async fn tiny_lines_respect_line_cap() {
        // 极短行（1 字节）+ 超多行：不限速下预算=MAX，靠行数兼底 4096 切批
        let unit = BATCH_FLUSH_LINES * 3 + 7;
        let samples = make_samples(unit, "a");
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
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
        for b in &s.batches {
            assert!(b.len() <= BATCH_FLUSH_LINES, "line cap breached");
        }
    }

    #[tokio::test]
    async fn content_order_preserved_with_varied_lines() {
        // 不同长度的行混合：验证顺序与内容都不乱
        let samples = Arc::new(vec![
            "short".to_string(),
            "x".repeat(70_000),
            "mid".repeat(100),
            "z".repeat(70_000),
            "tail".to_string(),
        ]);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            5,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 5);
        let s = state.lock().unwrap();
        let flat: Vec<String> = s.batches.iter().flatten().cloned().collect();
        assert_eq!(flat.len(), 5);
        assert_eq!(
            flat,
            (*samples).clone(),
            "order and content must be preserved"
        );
    }

    // ===== 速度与行长的耦合（动态预算）=====

    #[tokio::test]
    async fn low_rate_keeps_batches_small_for_low_latency() {
        // 低速率 + 短行：预算被压到 MIN(8KiB)。一个 unit(1000 行 × 50B = 50KB)
        // 应被切成多个小批，每批不超过 MIN + 单行。这保证低延迟（不攒大包）。
        let line = "s".repeat(50);
        let unit = 1000;
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        // 低速率：50_000 EPS × 50B × 0.1 = 250_000 -> 实际会 clamp MAX？
        // 为真正触发 MIN，用极低速率：1_000 EPS × 50B × 0.1 = 5_000 -> clamp MIN
        let mut policy = BatchSizePolicy::new(1_000);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            unit,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, unit);
        let s = state.lock().unwrap();
        assert!(
            s.batches.len() > 1,
            "low rate -> small budget -> multiple batches"
        );
        // 每个子批字节数不应超过 MIN + 单行（50B+1）太多，证明批确实小
        for b in s.batches.iter() {
            assert!(
                batch_bytes(b) <= BATCH_BUDGET_MIN + 51,
                "low-rate batch too large: {} bytes",
                batch_bytes(b)
            );
        }
    }

    #[tokio::test]
    async fn high_rate_makes_fewer_larger_batches() {
        // 高速率：预算=MAX(1MiB)。同样 10000 行 × 100B ≈ 1MB，应切成很少几个大批。
        // 对比低速率用例：同样数据，高速率批数明显更少。
        let line = "x".repeat(100);
        let unit = 10_000;
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let mut policy = BatchSizePolicy::new(0); // 不限速 -> MAX
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            unit,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, unit);
        let s = state.lock().unwrap();
        // 不限速 budget=1MiB，但行数兼底 4096 触发：10000/4096=3 批
        // 对比低速(8KiB)：同样数据需 ~122 批
        assert_eq!(
            s.batches.len(),
            3,
            "10K lines x 100B → 3 sub-batches (line cap 4096), got {}",
            s.batches.len()
        );
    }

    #[tokio::test]
    async fn mixed_logs_ema_adapts_batch_size() {
        // 混合日志：先短行（建立小 avg → 小预算 → 多小批），
        // 再长行（avg 逐步升高 → 预算变大 → 批变大/批数变少）。
        // 构造样本：500 短行(20B) + 500 长行(10_000B)
        let mut lines: Vec<String> = (0..500).map(|_| "a".repeat(20)).collect();
        lines.extend((0..500).map(|_| "b".repeat(10_000)));
        let samples = Arc::new(lines);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        // 限速：5_000 EPS。短行段：5000*20*0.1=10_000->MIN(8K)；
        // 长行段 avg 升到 ~10000：5000*10000*0.1=5_000_000->MAX。预算随段变化。
        let mut policy = BatchSizePolicy::new(5_000);
        let sent = send_unit_samples(
            &mut sink,
            &samples,
            &mut idx,
            1000,
            &mut collectors,
            &mut policy,
        )
        .await
        .unwrap();
        assert_eq!(sent, 1000);
        let s = state.lock().unwrap();
        // 数据完整守恒
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, 1000);
        // EMA 最终应明显高于纯短行水平（被长行拉上去）
        assert!(
            policy.avg_line_bytes() > 1000.0,
            "ema should be pulled up by long lines, got {}",
            policy.avg_line_bytes()
        );
    }

    /// 复刻 run_pipeline 里的 unit_size 计算逻辑。
    fn unit_size_for(base_rate: usize) -> usize {
        if base_rate > 0 {
            (base_rate / 10).clamp(1, 1000)
        } else {
            DEFAULT_UNIT_SIZE.max(1)
        }
    }

    #[test]
    fn unit_size_reflects_speed_profile() {
        assert_eq!(unit_size_for(0), DEFAULT_UNIT_SIZE);
        assert_eq!(unit_size_for(100_000), 1000);
        assert_eq!(unit_size_for(5_000), 500);
        assert_eq!(unit_size_for(1_000), 100);
        assert_eq!(unit_size_for(5), 1);
    }

    // 防 EMA_ALPHA 被误改后静默失效的卫兵
    #[test]
    fn ema_alpha_is_small_positive() {
        assert!(BATCH_EMA_ALPHA > 0.0 && BATCH_EMA_ALPHA < 1.0);
    }
}
