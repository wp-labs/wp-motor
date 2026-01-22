use crate::core::GenRuleUnit;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::generator::act_gen::{RuleGenerator, SampleGenerator};
use crate::runtime::sink::act_sink::ActSink;
use crate::runtime::supervisor::monitor::ActorMonitor;

use std::path::PathBuf;

use crate::runtime::actor::constants::ACTOR_IDLE_TICK_MS;
use rand::Rng;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

use wp_error::RunReason;
use wp_model_core::model::fmt_def::TextFmt;

use crate::runtime::actor::command::CmdSubscriber;

use crate::orchestrator::config::models::StatConf;
use crate::orchestrator::config::models::warp::stat_reqs_from;
use crate::runtime::actor::{TaskGroup, TaskManager};
use crate::sinks::SinkRuntime;
use crate::sinks::{SinkBackendType, SinkDatASender, SinkDataEnum, make_blackhole_sink};
use crate::stat::MonSend;
use crate::types::AnyResult;
use orion_error::{ErrorOwe, ToStructError, UvsBizFrom};
use std::any::type_name_of_val;
use wp_conf::structure::SinkInstanceConf;
use wp_error::run_error::RunResult;
use wp_stat::StatRequires;
use wp_stat::StatStage;

pub async fn rule_gen_run(
    args: RuleGRA,
    rules: Vec<GenRuleUnit>,
    out_io: SinkBackendType,
) -> RunResult<()> {
    if rules.is_empty() {
        return Err(RunReason::from_biz("gen rule is empty!").to_err());
    }
    // 限速为全局语义：当 speed>0 时，将并发限制为 min(parallel, speed)
    let mut g = args.gen_conf.clone();
    if g.gen_speed > 0 {
        g.parallel = std::cmp::max(1, std::cmp::min(g.parallel, g.gen_speed));
    }
    gen_run(
        g.rescue.clone(),
        RuleGenRoutine::new(rules),
        g,
        out_io,
        stat_reqs_from(&StatConf::gen_default()),
    )
    .await
}

pub async fn sample_gen_run(
    args: SampleGRA,
    out_io: SinkBackendType,
    samples: Vec<PathBuf>,
) -> RunResult<()> {
    assert!(!samples.is_empty());
    // 限速为全局语义：当 speed>0 时，将并发限制为 min(parallel, speed)
    let mut g = args.gen_conf.clone();
    if g.gen_speed > 0 {
        g.parallel = std::cmp::max(1, std::cmp::min(g.parallel, g.gen_speed));
    }
    gen_run(
        g.rescue.clone(),
        SampleGenRoutine::new(samples),
        g,
        out_io,
        stat_reqs_from(&StatConf::gen_default()),
    )
    .await
}

#[debug_requires(args.parallel > 0, "parallel must > 0")]
pub async fn gen_run(
    rescue: String,
    gen_ro: impl SpawnGen,
    args: GenGRA,
    out_io: SinkBackendType,
    stat_reqs: StatRequires,
) -> RunResult<()> {
    info_ctrl!(
        "gen_run start: total_line={:?}, parallel={}, speed={}, stat_sec={}, stat_print={}",
        args.total_line,
        args.parallel,
        args.gen_speed,
        args.stat_sec,
        args.stat_print
    );
    // --- 构建并发 Sink 消费：按 pkg_id 一致性分片到 N 个消费协程 ---
    let replica_cnt = std::cmp::max(1, args.parallel);
    let (dat_s_main, mut dat_r_main) = mpsc::channel::<(u64, SinkDataEnum)>(100000);
    let mut shard_senders: Vec<mpsc::Sender<(u64, SinkDataEnum)>> = Vec::with_capacity(replica_cnt);
    let mut rt_admin = TaskManager::default();

    // 监控组
    let mut moni_group = TaskGroup::new("moni", ShutdownCmd::Timeout(200));
    let mut actor_mon =
        ActorMonitor::new(moni_group.subscribe(), None, args.stat_print, args.stat_sec);
    let monitor_reqs = stat_reqs.get_all().clone();
    let mon_s = actor_mon.send_agent();
    moni_group.append(tokio::spawn(async move {
        let _ = actor_mon.stat_proc(monitor_reqs).await;
    }));
    rt_admin.append_group(moni_group);
    info_ctrl!("start monitor coroutine");
    // Sink 组：多副本并行消费
    // 若 out_io 为 BlackHoleSink，可安全复制多份；否则退化为单副本消费（避免非法克隆真实外部连接）
    let is_blackhole_sink = match &out_io {
        SinkBackendType::Proxy(f) => type_name_of_val(&**f).contains("BlackHoleSink"),
    };
    let eff_replica_cnt = if is_blackhole_sink { replica_cnt } else { 1 };
    let mut sink_group = TaskGroup::new("sink", ShutdownCmd::Timeout(200));
    // 非 null 情况：仅首个副本使用 out_io（move），其余不存在
    let mut out_io_opt = Some(out_io);
    for i in 0..eff_replica_cnt {
        let (s, r) = mpsc::channel::<(u64, SinkDataEnum)>(100000);
        shard_senders.push(s);
        let sink_rt = SinkRuntime::new(
            rescue.clone(),
            format!("gen_sink_{}", i),
            SinkInstanceConf::null_new("gen".to_string(), TextFmt::Raw, None),
            if is_blackhole_sink {
                SinkBackendType::Proxy(make_blackhole_sink())
            } else {
                out_io_opt.take().expect("single out_io move")
            },
            None,
            stat_reqs.get_stage_items(StatStage::Sink),
        );
        let mut act_sink = ActSink::new(mon_s.clone(), sink_group.subscribe(), None);
        sink_group.append(tokio::spawn(async move {
            let _ = act_sink.async_proc(sink_rt, r).await;
        }));
    }
    info_ctrl!(
        "start {} sink consumers (null_clone={})",
        eff_replica_cnt,
        is_blackhole_sink
    );

    // 路由组：将主通道数据分发到各个分片
    let mut route_group = TaskGroup::new("router", ShutdownCmd::Timeout(200));
    let mut route_cmd = route_group.subscribe();
    route_group.append(tokio::spawn(async move {
        info_ctrl!("router start: eff_replica_cnt={}", eff_replica_cnt);
        // router loop：收到 Stop/Isolate 或主通道关闭即退出
        loop {
            tokio::select! {
                res = dat_r_main.recv() => {
                    match res {
                        Some((pkg_id, data)) => {
                            let idx = (pkg_id as usize) % eff_replica_cnt;
                            // 忽略发送错误（下游已关闭时退出）
                            if shard_senders[idx].send((pkg_id, data)).await.is_err() { break; }
                        }
                        None => { info_ctrl!("router: main channel closed"); break; }
                    }
                }
                Ok(cmd) = route_cmd.recv() => {
                    // 任意 Stop/Isolate 立即退出（无需等待下一 tick）
                    match cmd {
                        crate::runtime::actor::command::ActorCtrlCmd::Stop(s) => { info_ctrl!("router recv stop: {}", s); break; },
                        crate::runtime::actor::command::ActorCtrlCmd::Isolate => { info_ctrl!("router recv isolate"); break; },
                        _ => {}
                    }
                }
                _ = sleep(Duration::from_millis(ACTOR_IDLE_TICK_MS)) => {  }
            }
        }
        // route 结束：shard_senders 在此 drop，从而关闭所有 shard 通道
        info_ctrl!("router exit");
    }));
    rt_admin.append_group(route_group);

    let mut gen_group = TaskGroup::new("gen", ShutdownCmd::Immediate);
    // 计算每个 worker 的任务量：均分 + 余数前置分配，严格等于 total_line
    let mut per_counts: Vec<Option<usize>> = Vec::with_capacity(args.parallel);
    if let Some(total) = args.total_line {
        let base = total / args.parallel;
        let rem = total % args.parallel;
        info_ctrl!(
            "assign per-worker counts: total={}, base={}, remainder={}",
            total,
            base,
            rem
        );
        for i in 0..args.parallel {
            let add = if i < rem { 1 } else { 0 };
            per_counts.push(Some(base + add));
        }
    } else {
        per_counts.resize(args.parallel, None);
    }
    for (i, total) in per_counts.iter().copied().enumerate() {
        let mut g_one = args.clone();
        g_one.total_line = total;
        let h = gen_ro
            .spawn_gen(
                gen_group.subscribe(),
                dat_s_main.clone(),
                mon_s.clone(),
                g_one.clone(),
            )
            .owe_sys()?;
        info_ctrl!(
            "spawn gen worker {} with total_line={:?}, speed={}, parallel={}",
            i,
            g_one.total_line,
            g_one.gen_speed,
            g_one.parallel
        );
        gen_group.append(h);
    }
    // 释放主发送句柄；仅保留生产者持有的克隆，便于在生产全部结束后关闭路由接收端
    drop(dat_s_main);
    rt_admin.append_group(sink_group);
    rt_admin.set_main(gen_group);

    //rt_admin.signal_wait_grace_down().await?;
    rt_admin.all_down_wait_signal().await?;

    info_ctrl!("gen coroutine all end!");
    Ok(())
}

pub trait SpawnGen {
    fn spawn_gen(
        &self,
        cmd_r: CmdSubscriber,
        dat_s: SinkDatASender,
        mon_s: MonSend,
        gen_conf: GenGRA,
    ) -> AnyResult<JoinHandle<()>>;
}

pub struct RuleGenRoutine {
    rules: Vec<GenRuleUnit>,
}
impl RuleGenRoutine {
    pub fn new(rules: Vec<GenRuleUnit>) -> Self {
        Self { rules }
    }
}
impl SpawnGen for RuleGenRoutine {
    fn spawn_gen(
        &self,
        cmd_r: CmdSubscriber,
        dat_s: SinkDatASender,
        mon_s: MonSend,
        gen_conf: GenGRA,
    ) -> AnyResult<JoinHandle<()>> {
        info_ctrl!("gen conf(worker) : {:?}", gen_conf);
        let rules_copy = self.rules.clone();
        let dat_s_cp = dat_s;
        let h = tokio::spawn(async move {
            let mut gen_actor = RuleGenerator::default();
            match gen_actor
                .gen_data(rules_copy, cmd_r, dat_s_cp, gen_conf, mon_s)
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    error_ctrl!("gen data error:{}", e);
                }
            }
        });
        Ok(h)
    }
}

pub struct SampleGenRoutine {
    samples: Vec<PathBuf>,
}
impl SampleGenRoutine {
    pub fn new(samples: Vec<PathBuf>) -> Self {
        Self { samples }
    }
}

impl SpawnGen for SampleGenRoutine {
    fn spawn_gen(
        &self,
        cmd_r: CmdSubscriber,
        dat_s: SinkDatASender,
        mon_s: MonSend,
        gen_conf: GenGRA,
    ) -> AnyResult<JoinHandle<()>> {
        info_ctrl!("gen samples from : {:?}", self.samples);
        let mut gen_actor = SampleGenerator::from_file(self.samples.clone())?;
        let dat_s_cp = dat_s;
        let h = tokio::spawn(async move {
            match gen_actor.gen_data(cmd_r, dat_s_cp, gen_conf, mon_s).await {
                Ok(_) => {}
                Err(e) => {
                    error_ctrl!("gen data error:{}", e);
                }
            }
        });
        Ok(h)
    }
}

pub fn rand_conf_idx(max: usize) -> usize {
    let mut rng = rand::rng();
    rng.gen_range(0..max)
}

#[derive(Clone, Debug)]
pub struct SampleGRA {
    //pub samples: Vec<PathBuf>,
    pub gen_conf: GenGRA,
}

#[derive(Clone, Debug)]
pub struct GenGRA {
    pub total_line: Option<usize>,
    pub gen_secs: Option<usize>,
    pub gen_speed: usize,
    pub parallel: usize,
    pub stat_sec: usize,
    pub stat_print: bool,
    pub rescue: String,
}

impl Default for GenGRA {
    fn default() -> Self {
        Self {
            total_line: Some(1000),
            gen_secs: None,
            gen_speed: 1000,
            parallel: 1,
            stat_sec: 1,
            stat_print: false,
            rescue: "./rescue".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuleGRA {
    pub gen_conf: GenGRA,
}
