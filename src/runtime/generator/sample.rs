use super::common::{DEFAULT_UNIT_SIZE, build_sink_instance};
use super::speed::{DynamicRateLimiter, SpeedProfile};
use crate::orchestrator::config::models::stat_reqs_from;
use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::generator::types::GenGRA;
use crate::runtime::supervisor::monitor::ActorMonitor;
use crate::sinks::SinkBackendType;
use crate::stat::metric_collect::MetricCollectors;
use orion_error::UvsReason;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::task::JoinHandle;
use wp_conf::stat::StatConf;
use wp_conf::structure::SinkInstanceConf;
use wp_error::run_error::{RunError, RunErrorOwe, RunResult};
use wp_log::info_ctrl;
use wp_stat::{StatRecorder, StatStage};

fn load_samples(rule_root: &str, find_name: &str) -> RunResult<Vec<String>> {
    use std::io::BufRead;
    // discover files
    let files = wp_conf::utils::find_conf_files(rule_root, find_name).map_err(|e| {
        RunError::from(wp_error::run_error::RunReason::Uvs(UvsReason::core_conf(
            e.to_string(),
        )))
    })?;
    info_ctrl!("run_sample_direct: found {} files", files.len());
    if files.is_empty() {
        return Err(RunError::from(wp_error::run_error::RunReason::Uvs(
            UvsReason::core_conf(format!("no {} file in {}", find_name, rule_root)),
        )));
    }
    // load lines
    let mut out = Vec::new();
    for f in files {
        let file = std::fs::File::open(&f).map_err(|e| {
            RunError::from(wp_error::run_error::RunReason::Uvs(UvsReason::core_conf(
                e.to_string(),
            )))
        })?;
        let reader = std::io::BufReader::new(file);
        for s in reader.lines().map_while(Result::ok) {
            out.push(s);
        }
    }
    Ok(out)
}

/// 批量发送一个"单元"的样本（逐条发送，但把本单元作为一个批次）。
async fn send_unit_samples(
    sink: &mut SinkBackendType,
    samples: &Arc<Vec<String>>,
    cur_idx: &mut usize,
    unit_cnt: usize,
    collectors: &mut MetricCollectors,
) -> RunResult<usize> {
    let n = samples.len().max(1);
    let mut sent = 0usize;
    for _ in 0..unit_cnt {
        let line = &samples[*cur_idx];
        wp_connector_api::AsyncRawDataSink::sink_str(sink, line.as_str())
            .await
            .owe_sink()?;
        // 按条统计
        collectors.record_task("gen_direct", ());
        *cur_idx = (*cur_idx + 1) % n;
        sent += 1;
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
    let mut actor_mon =
        ActorMonitor::new(moni_group.subscribe(), None, gar.stat_print, gar.stat_sec);
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
        let produced = t.await.map_err(|e| {
            RunError::from(wp_error::run_error::RunReason::Uvs(UvsReason::core_conf(
                e.to_string(),
            )))
        })??;
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
