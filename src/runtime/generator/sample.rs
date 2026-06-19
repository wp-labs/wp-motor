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

/// 批量发送一个"单元"的样本：按字节预算(64KiB)切分成多个子批，逐批 sink_str_batch，
/// 使每次 write 大小稳定可控，不依赖行长。
async fn send_unit_samples(
    sink: &mut SinkBackendType,
    samples: &Arc<Vec<String>>,
    cur_idx: &mut usize,
    unit_cnt: usize,
    collectors: &mut MetricCollectors,
) -> RunResult<usize> {
    use super::common::{BATCH_FLUSH_BYTES, BATCH_FLUSH_LINES};
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
        // 达到字节预算或行数兼底，先下发当前子批
        if (batch_bytes + len > BATCH_FLUSH_BYTES && !batch.is_empty())
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
        let sent =
            match send_unit_samples(&mut sink, &samples, &mut cur_idx, reserved, &mut collectors)
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
    use crate::runtime::generator::common::{BATCH_FLUSH_BYTES, BATCH_FLUSH_LINES};
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

    /// 子批中一个子批的字节数（每行 + 1 字节换行，对齐 Line framing）
    fn batch_bytes(b: &[String]) -> usize {
        b.iter().map(|s| s.len() + 1).sum()
    }

    #[tokio::test]
    async fn empty_unit_sends_nothing() {
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let samples = make_samples(10, "x");
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, 0, &mut collectors)
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
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, 1000, &mut collectors)
            .await
            .unwrap();
        assert_eq!(sent, 1000);
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0, "must use sink_str_batch, not sink_str");
    }

    #[tokio::test]
    async fn short_lines_split_by_byte_budget() {
        // 10000 行 × 100 字节 = 1MB，远超 64KiB，应切成多个子批
        let line = "x".repeat(100);
        let unit = 10_000;
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, unit, &mut collectors)
            .await
            .unwrap();

        assert_eq!(sent, unit, "total sent must equal unit_cnt");
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0);

        // 总行数守恒
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, unit, "all lines must be flushed");

        // 应被切成多于 1 个子批（1MB > 64KiB）
        assert!(s.batches.len() > 1, "expected multiple sub-batches");

        // 除尾批外，每个子批要么达到字节预算、要么达到行数兼底
        // 字节预算约束：单子批字节数 <= 64KiB + 单行（含换行），因为单行可能填过预算
        for b in s.batches.iter() {
            assert!(b.len() <= BATCH_FLUSH_LINES, "sub-batch line cap breached");
        }
        // 非尾批应贴近字节预算（这里宽松校验：非尾批字节数应 >= 预算一半）
        for b in s.batches.iter().take(s.batches.len().saturating_sub(1)) {
            assert!(
                batch_bytes(b) > BATCH_FLUSH_BYTES / 2,
                "non-tail sub-batch too small: {} bytes",
                batch_bytes(b)
            );
        }

        // 内容顺序守恒：展平后应与输入序列一致
        let flat: Vec<String> = s.batches.iter().flatten().cloned().collect();
        for v in &flat {
            assert_eq!(v, &line);
        }
        // cur_idx 应正好绕回起点（unit 是 samples.len() 的整数倍）
        assert_eq!(idx, 0);
    }

    #[tokio::test]
    async fn huge_single_line_flushes_alone() {
        // 单行 > 64KiB：该行应独占一个子批（不会被合并进其他子批）
        let huge = "y".repeat(BATCH_FLUSH_BYTES + 1000);
        let samples = Arc::new(vec![huge.clone(), huge.clone(), huge.clone()]);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, 3, &mut collectors)
            .await
            .unwrap();
        assert_eq!(sent, 3);
        let s = state.lock().unwrap();
        // 每行都超过预算，每行应独占一个子批 → 3 个子批各 1 行
        assert_eq!(s.batches.len(), 3, "each huge line should be its own batch");
        for b in &s.batches {
            assert_eq!(b.len(), 1);
            assert_eq!(b[0], huge);
        }
    }

    #[tokio::test]
    async fn tiny_lines_respect_line_cap() {
        // 极短行（1 字节）+ 超多行：不触发字节预算，但触发行数兼底 4096
        let unit = BATCH_FLUSH_LINES * 3 + 7; // 跨多个行数兼底点
        let samples = make_samples(unit, "a");
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, unit, &mut collectors)
            .await
            .unwrap();
        assert_eq!(sent, unit);
        let s = state.lock().unwrap();
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, unit);
        // 每个子批行数不得超过行数兼底
        for b in &s.batches {
            assert!(b.len() <= BATCH_FLUSH_LINES, "line cap breached");
        }
    }

    #[tokio::test]
    async fn content_order_preserved_with_varied_lines() {
        // 不同长度的行混合：验证顺序与内容都不乱
        let samples = Arc::new(vec![
            "short".to_string(),
            "x".repeat(70_000), // 超预算
            "mid".repeat(100),  // ~300 字节
            "z".repeat(70_000), // 超预算
            "tail".to_string(),
        ]);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, 5, &mut collectors)
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

    /// 复刻 run_pipeline 里的 unit_size 计算逻辑，用于验证限速场景下的 unit 粒度。
    /// 语义：base_rate=0（不限速）→ 50000；否则 (base_rate/10).clamp(1,1000)。
    fn unit_size_for(base_rate: usize) -> usize {
        if base_rate > 0 {
            (base_rate / 10).clamp(1, 1000)
        } else {
            DEFAULT_UNIT_SIZE.max(1)
        }
    }

    #[test]
    fn unit_size_reflects_speed_profile() {
        // 不限速：满 unit，吞吐优先
        assert_eq!(unit_size_for(0), DEFAULT_UNIT_SIZE);
        // 高速：撞到行数兼底 1000
        assert_eq!(unit_size_for(100_000), 1000);
        // 中速
        assert_eq!(unit_size_for(5_000), 500);
        // 低速：unit 很小，发送会按 unit 粒度及时出，不死等字节预算
        assert_eq!(unit_size_for(1_000), 100);
        // 极低速：clamp 到 1
        assert_eq!(unit_size_for(5), 1);
    }

    #[tokio::test]
    async fn low_speed_small_unit_flushes_without_waiting_budget() {
        // 低速场景：unit_size 被 base_rate 压到很小（这里用 100 行 × 100B = 10KB）。
        // 10KB 远低于 256KiB 字节预算，不应被拆批，也不应死等——整个 unit 作为单个子批
        // （尾批）及时发出。这验证低速下数据不会被攒批逻辑拖出延迟。
        let unit = 100; // 模拟 unit_size_for(1_000)
        let line = "x".repeat(100); // 100B/行 → 整 unit 仅 ~10KB << 256KiB
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, unit, &mut collectors)
            .await
            .unwrap();

        assert_eq!(sent, unit, "all lines in the small unit must be sent");
        let s = state.lock().unwrap();
        assert_eq!(s.single_calls, 0, "still uses batch path");
        // 关键断言：小 unit 不足以触发字节预算切批 → 只应有 1 个子批（尾批）
        assert_eq!(
            s.batches.len(),
            1,
            "small unit (< byte budget) must flush as a single batch, no splitting"
        );
        assert_eq!(s.batches[0].len(), unit);
    }

    #[tokio::test]
    async fn high_speed_unit_splits_when_exceeding_budget() {
        // 高速场景：unit_size 被 clamp 到 1000，但每行很大（~300B）→ 1000 行 ≈ 300KB > 256KiB，
        // 应被字节预算切成多个子批。与低速用例形成对照：同样 unit_size=1000 量级，
        // 但因行长不同，切批行为不同。
        let unit = 1000;
        let line = "y".repeat(300); // 300B/行 → 整 unit ~300KB > 256KiB
        let samples = make_samples(unit, &line);
        let (rec, state) = RecordingSink::new();
        let mut sink = SinkBackendType::Proxy(Box::new(rec));
        let mut idx = 0usize;
        let mut collectors = empty_collectors();
        let sent = send_unit_samples(&mut sink, &samples, &mut idx, unit, &mut collectors)
            .await
            .unwrap();
        assert_eq!(sent, unit);
        let s = state.lock().unwrap();
        let total: usize = s.batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, unit);
        // 300KB > 256KiB，应切成多于 1 个子批
        assert!(
            s.batches.len() > 1,
            "unit exceeding byte budget must be split"
        );
    }
}
