use crate::compat::LegacyOwe;
use crate::core::sinks::sync_sink::TrySendStatus;
use crate::facade::test_helpers::SinkTerminal;
use crate::runtime::actor::command::{CmdSubscriber, TaskController, TaskEndReason};
use crate::sinks::{ProcMeta, RescueEntry, RescuePayload, SinkRouteAgent};
use crate::stat::metric_collect::MetricCollectors;
use crate::stat::{MonSend, STAT_INTERVAL_MS};
use chrono::NaiveDateTime;
use wp_connector_api::{SinkReason, SinkResult};

use wp_error::RunErrorOwe;
use wp_error::run_error::{RunReason, RunResult};
use wp_stat::StatReq;

use wp_stat::StatRecorder;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
// use std::time::Duration; // merged below with Instant
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;
use walkdir::WalkDir;

// 读取到的文件的文件位置
const POINT_PATH: &str = "rescue/recover.lock";

/// 回放遇到"分发通道满"时的重试间隔与上限。
/// 通道满只是背压，不是 sink 故障，等消费端腾出空间即可，不应该切换候选。
const RECOVER_BUSY_BACKOFF: Duration = Duration::from_millis(2);
const RECOVER_BUSY_RETRY_MAX: usize = 2_500;

// 要求rescue文件里面的数据没有空行。
pub struct ActCovPicker {
    rescue_path: String,
    speed_limit: usize,
    cmd_sub: CmdSubscriber,
    mon_s: MonSend,
    /// 空闲退出阈值；若为 Some(d)，当连续找不到 rescue .dat 文件达 d 时，自动退出
    idle_exit: Option<Duration>,
}

impl ActCovPicker {
    pub fn new(
        cmd_sub: CmdSubscriber,
        path: &str,
        speed_limit: usize,
        mon_s: MonSend,
        idle_exit: Option<Duration>,
    ) -> Self {
        ActCovPicker {
            rescue_path: path.to_string(),
            speed_limit,
            cmd_sub,
            mon_s,
            idle_exit,
        }
    }

    pub async fn pick_data(
        &self,
        mut route_agent: SinkRouteAgent,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<()> {
        let mut run_ctrl =
            TaskController::new("recover", self.cmd_sub.clone(), Some(self.speed_limit));
        info_dfx!("recover begin");
        let rescues = RescueFiles::new(self.rescue_path.as_str());
        // 加载上次读取文件位置
        let mut check_point = CheckPoint::load_point().owe(RunReason::data_error())?;
        let mut idle_since: Option<Instant> = None;
        loop {
            //当前还在写入的救急文件(文件后缀有.lock), 不能马上 recover.
            let paths = rescues
                .tack_lasts_file("dat")
                .owe(RunReason::data_error())?;
            if let Some(paths) = paths {
                idle_since = None; // 重置空闲计时
                self.recov_file(paths, &mut check_point, &mut route_agent, stat_reqs.clone())
                    .await?;
                run_ctrl.rec_task_suc();
            } else {
                // 空闲检测：若开启了 idle_exit，则在超过阈值后退出
                if let Some(th) = self.idle_exit {
                    match idle_since {
                        None => idle_since = Some(Instant::now()),
                        Some(t0) => {
                            if t0.elapsed() >= th {
                                info_dfx!(
                                    "no rescue file for {:?}, idle timeout reached -> exit",
                                    th,
                                );
                                break;
                            }
                        }
                    }
                }
                info_dfx!("no rescue file, wait 1 sec");
            }
            tokio::select! {
               Ok(cmd)  = run_ctrl.cmds_sub_mut().recv() => {
                    run_ctrl.update_cmd(cmd);
               }
               _ =  sleep(Duration::from_secs(1)) => {
                   run_ctrl.rec_task_idle();
                   if run_ctrl.is_stop() {
                       break;
                   }

               }
            }
        }

        info_dfx!("read data end!");
        Ok(())
    }
    async fn recov_file(
        &self,
        paths: String,
        check_point: &mut CheckPoint,
        route_agent: &mut SinkRouteAgent,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<()> {
        let sink_name = Self::get_sink_name(paths.as_str());
        let sink_group = Self::get_sink_group(paths.as_str(), self.rescue_path.as_str());
        // 救援文件路径里带 sink 全名（group/name）：优先按「组 + 名」匹配。
        // 只按名匹配会把其它组的同名 sink（自动命名时都是 `[0]`）当候选，把数据投到错组。
        let mut sink_agents = if sink_group.is_empty() {
            Vec::new()
        } else {
            route_agent.get_sink_agents_in_group(sink_group.as_str(), sink_name.as_str())
        };
        if sink_agents.is_empty() {
            if !sink_group.is_empty() {
                warn_dfx!(
                    "no sink agent for {}/{}, fallback to name-only match",
                    sink_group,
                    sink_name
                );
            }
            sink_agents = route_agent.get_sink_agents(sink_name.as_str());
        }
        if sink_agents.is_empty() {
            error_dfx!("no sink agent for {}", sink_name);
            return Ok(());
        }

        //所有sink 都在运行,并没有逻辑上的问题!?
        /*
        self.cmd_pub
            .broadcast(CtrlCmd::Work(DoScope::One(sink_name.clone())))
            .await?;
            */
        sleep(std::time::Duration::from_millis(100)).await;
        info_dfx!(
            "recover file: {}, sink candidates: {}",
            paths,
            sink_agents.len()
        );
        match self
            .pick_file(
                sink_agents,
                &paths,
                self.speed_limit,
                check_point,
                stat_reqs,
            )
            .await?
        {
            TaskEndReason::SucEnded => {
                info_dfx!("recover end");
            }
            TaskEndReason::Interrupt => {
                info_dfx!("recover interrupt");
            }
        }
        Ok(())
    }

    async fn pick_file(
        &self,
        sink_agents: Vec<SinkTerminal>,
        file_path: &String,
        speed: usize,
        check_point: &mut CheckPoint,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<TaskEndReason> {
        let stat_target = relative_rescue_display_path(file_path, self.rescue_path.as_str());
        let mut run_ctrl = TaskController::from_speed_limit(
            "recover-file",
            self.cmd_sub.clone(),
            Some(speed),
            100,
        );
        let mut stat = MetricCollectors::new(stat_target.clone(), stat_reqs);

        let stat_interval = Duration::from_millis(STAT_INTERVAL_MS as u64);
        let mut last_stat_tick = Instant::now();
        let file = File::open(file_path).await.owe(RunReason::data_error())?;
        let mut reader = BufReader::new(file);
        #[allow(unused_assignments)]
        let mut end_reason = TaskEndReason::Interrupt;
        let mut active_sink_idx: usize = 0;
        info_dfx!(
            "recover begin! file : {}, sink candidates: {}",
            file_path,
            sink_agents.len()
        );
        loop {
            run_ctrl.rec_task_unit_reset();
            while !run_ctrl.is_unit_end() {
                let mut buffer = String::new();
                let size = reader
                    .read_line(&mut buffer)
                    .await
                    .owe(RunReason::data_error())?;
                if size.eq(&0) {
                    stat.record_task(stat_target.as_str(), None);
                    fs::remove_file(file_path).owe(RunReason::system_error())?;
                    check_point.remove_point(file_path);
                    info_dfx!("recover end! clean file : {}", file_path);
                    println!("recover file finished! : {}", file_path);
                    return Ok(TaskEndReason::SucEnded);
                }
                stat.record_begin(stat_target.as_str(), None);
                let trimmed = buffer.trim_end_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    continue;
                }

                let entry = RescueEntry::parse(trimmed).owe(RunReason::data_error())?;
                Self::send_payload_with_failover(
                    &sink_agents,
                    &mut active_sink_idx,
                    entry.into_payload(),
                )
                .await
                .owe_sink()?;
                stat.record_end(stat_target.as_str(), None);
                run_ctrl.rec_task_suc();
                check_point.rec_suc(file_path);
            }

            if last_stat_tick.elapsed() >= stat_interval {
                stat.send_stat(&self.mon_s)
                    .await
                    .owe(RunReason::resource_error())?;
                check_point.save_point().owe(RunReason::system_error())?;
                last_stat_tick = Instant::now();
            }
            let wait_during = run_ctrl.unit_speed_limit_left();
            tokio::select! {
               Ok(cmd)  = run_ctrl.cmds_sub_mut().recv() => {
                    run_ctrl.update_cmd(cmd);
               }
               _ =  sleep(wait_during) => {
                   run_ctrl.rec_task_idle();
                   if run_ctrl.is_stop() {
                       break;
                   }

               }
            }
        }

        end_reason = TaskEndReason::Interrupt;
        check_point.save_point().owe(RunReason::system_error())?;
        stat.send_stat(&self.mon_s)
            .await
            .owe(RunReason::resource_error())?;
        Ok(end_reason)
    }

    /// 回放时把一条救援数据发回目标 sink。
    ///
    /// 这里的终端是**分发通道**：通道满只代表背压，不代表 sink 故障。
    /// 背压时必须先在同一个候选上等待重试，切候选会把数据投到另一个 sink（可能是别的组）。
    async fn send_payload_with_failover(
        sink_agents: &[SinkTerminal],
        active_sink_idx: &mut usize,
        payload: RescuePayload,
    ) -> SinkResult<()> {
        match payload {
            // record 只深拷贝一次，重试只是 Arc::clone
            RescuePayload::Record { record } => {
                let record = Arc::new(record);
                Self::send_with_backpressure(sink_agents, active_sink_idx, |sink| {
                    sink.try_send_record(0, ProcMeta::Null, Arc::clone(&record))
                })
                .await
            }
            RescuePayload::Raw { raw } => {
                Self::send_with_backpressure(sink_agents, active_sink_idx, |sink| {
                    sink.try_send_raw(0, raw.clone())
                })
                .await
            }
        }
    }

    async fn send_with_backpressure<F>(
        sink_agents: &[SinkTerminal],
        active_sink_idx: &mut usize,
        mut send_fn: F,
    ) -> SinkResult<()>
    where
        F: FnMut(&SinkTerminal) -> TrySendStatus,
    {
        if sink_agents.is_empty() {
            return Err(SinkReason::sink("no sink candidates for recovery"));
        }

        let total = sink_agents.len();
        let mut last_error = None;
        for _ in 0..total {
            let idx = *active_sink_idx % total;
            let sink = &sink_agents[idx];
            let mut busy_retries = 0usize;
            loop {
                match send_fn(sink) {
                    TrySendStatus::Sended => {
                        *active_sink_idx = idx;
                        return Ok(());
                    }
                    TrySendStatus::Fulfilled(_, _) => {
                        // 分发通道满：等待消费端腾出空间，同一个 sink 上重试。
                        busy_retries += 1;
                        if busy_retries > RECOVER_BUSY_RETRY_MAX {
                            last_error = Some(SinkReason::sink(format!(
                                "sink dispatcher channel stayed full for {}s",
                                RECOVER_BUSY_BACKOFF.as_secs_f32() * RECOVER_BUSY_RETRY_MAX as f32
                            )));
                            *active_sink_idx = (idx + 1) % total;
                            break;
                        }
                        sleep(RECOVER_BUSY_BACKOFF).await;
                    }
                    TrySendStatus::Err(err) => {
                        let next_idx = (idx + 1) % total;
                        warn_dfx!(
                            "recover send failed on sink idx {}, switching to idx {}: {}",
                            idx,
                            next_idx,
                            err
                        );
                        last_error = Some(SinkReason::sink(err.to_string()));
                        *active_sink_idx = next_idx;
                        break;
                    }
                }
            }
        }

        Err(last_error.expect("last_error should exist when all candidates fail"))
    }

    /// 救援文件所在目录就是 sink 全名里的 group 部分。
    /// 例：`<rescue_root>/sink/benchmark/[0]-2026-10-26_14:41:46-0.dat` -> `sink/benchmark`
    ///
    /// 配置里的 root 与 WalkDir 返回的文件路径可能一个是相对一个绝对，两种都试一次。
    fn get_sink_group(path: &str, rescue_root: &str) -> String {
        let path = Path::new(path);
        let raw_root = Path::new(rescue_root);
        let canonical_root = fs::canonicalize(raw_root).ok();
        let rel = path
            .strip_prefix(raw_root)
            .ok()
            .or_else(|| {
                canonical_root
                    .as_deref()
                    .and_then(|root| path.strip_prefix(root).ok())
            })
            .unwrap_or(path);
        match rel.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => {
                dir.to_string_lossy().trim_matches('/').to_string()
            }
            _ => String::new(),
        }
    }

    fn get_sink_name(path: &str) -> String {
        let path = Path::new(path);
        let mut sink_name = "".to_string();
        if let Some(file) = path.file_name() {
            let file = file.to_string_lossy().to_string();
            let f: Vec<&str> = file.split('-').collect();
            sink_name = f[0].to_string();
        }
        sink_name
    }
}

fn relative_rescue_display_path(path: &str, rescue_root: &str) -> String {
    let path = Path::new(path);
    let rescue_root = Path::new(rescue_root);
    path.strip_prefix(rescue_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub struct RescueFiles {
    path: String,
}

impl RescueFiles {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    #[allow(clippy::ptr_arg)]
    fn sort_key(path: &PathBuf) -> i64 {
        let name = path.file_name().unwrap_or_default();
        let file_name = name.to_string_lossy().to_string();
        let file_name = file_name.strip_suffix(".dat").unwrap_or_default();
        let f: Vec<&str> = file_name.split('-').collect();
        let t = format!("{}-{}-{}", f[1], f[2], f[3].replace('_', " "));
        let time = NaiveDateTime::parse_from_str(&t, "%Y-%m-%d %H:%M:%S")
            .expect("解析时间字符串失败，期待时间格式为：%Y-%m-%d %H:%M:%S");
        time.and_utc().timestamp()
    }

    pub fn tack_lasts_file(&self, ends: &str) -> RunResult<Option<String>> {
        let mut files = Vec::new();
        let paths = WalkDir::new(&self.path);
        for entry in paths {
            let entry = entry.owe(RunReason::data_error())?;
            let path = entry.path();

            // 收集 rescue 根目录下的所有子孙文件（不再限制必须为直接子文件）
            if path.is_file() && path.extension().and_then(|x| x.to_str()) == Some(ends) {
                files.push(path.to_path_buf());
            }
        }
        // 排序顺序从小到大
        files.sort_by_key(Self::sort_key);
        if let Some(file) = files.pop() {
            return Ok(Some(file.display().to_string()));
        }
        Ok(None)
    }
}

// 被中断的文件可能有多个
#[derive(Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct CheckPoint(HashMap<String, usize>);

impl CheckPoint {
    pub fn save_point(&mut self) -> RunResult<()> {
        let point = serde_json::to_string(self).owe(RunReason::data_error())?;
        let path = Path::new(POINT_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).owe(RunReason::system_error())?;
        }
        fs::write(POINT_PATH, point).owe(RunReason::system_error())?;
        Ok(())
    }

    pub fn load_point() -> RunResult<CheckPoint> {
        let point = match fs::read_to_string(POINT_PATH) {
            Ok(val) => serde_json::from_str(&val).owe(RunReason::data_error())?,
            Err(_) => CheckPoint::default(),
        };
        Ok(point)
    }

    pub fn rec_suc(&mut self, path: &str) {
        if let Some(val) = self.0.get_mut(path) {
            *val += 1;
        } else {
            self.0.insert(path.to_string(), 1);
        }
    }

    pub fn remove_point(&mut self, path: &str) {
        self.0.remove(path);
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::collector::recovery::{
        ActCovPicker, CheckPoint, RescueFiles, SinkTerminal, relative_rescue_display_path,
    };
    use orion_error::dev::testing::TestAssert;
    use wp_error::run_error::RunResult;

    use std::fs;

    #[test]
    fn test_get_sink_name() {
        let path = "./rescue/http_accs_file_sink-2023-11-30_10:52:45.dat";
        let sink_name = ActCovPicker::get_sink_name(path);
        assert_eq!(sink_name, "http_accs_file_sink".to_string());
    }

    #[test]
    fn test_get_sink_name_nested() {
        let path = "./rescue/groupA/bench_sink-2025-10-14_03:10:12.dat";
        let sink_name = ActCovPicker::get_sink_name(path);
        assert_eq!(sink_name, "bench_sink".to_string());
    }

    // ---- 回放发送：背压 vs 真实故障 ----

    fn recovery_unit() -> crate::sinks::SinkRecUnit {
        crate::sinks::SinkRecUnit::new(
            0,
            crate::sinks::ProcMeta::Null,
            std::sync::Arc::new(wp_model_core::model::DataRecord::default()),
        )
    }

    fn record_payload() -> crate::sinks::RescuePayload {
        crate::sinks::RescuePayload::Record {
            record: wp_model_core::model::DataRecord::default(),
        }
    }

    #[tokio::test]
    async fn busy_dispatcher_is_waited_out_on_the_same_candidate() {
        use tokio::sync::mpsc;

        let (tx_a, mut rx_a) = mpsc::channel(1);
        let (tx_b, mut rx_b) = mpsc::channel(1);
        // 先把 A 的分发通道占满，模拟消费端跟不上
        tx_a.try_send(crate::sinks::SinkPackage::single(recovery_unit()))
            .expect("fill channel a");
        // 后台消费一条，让重试能成功；把 receiver 交还，便于断言重试后的落点
        let drainer = tokio::spawn(async move {
            let first = rx_a.recv().await;
            (rx_a, first)
        });

        let agents = vec![SinkTerminal::Channel(tx_a), SinkTerminal::Channel(tx_b)];
        let mut idx = 0usize;
        ActCovPicker::send_payload_with_failover(&agents, &mut idx, record_payload())
            .await
            .expect("backpressure must be retried, not treated as failure");

        assert_eq!(idx, 0, "背压不能把发送切到第二个候选");
        assert!(rx_b.try_recv().is_err(), "第二个候选不应该收到任何数据");
        let (mut rx_a, occupied) = drainer.await.unwrap();
        assert!(occupied.is_some(), "占位的包被消费");
        assert!(rx_a.recv().await.is_some(), "重试后数据落到原候选");
    }

    #[tokio::test]
    async fn closed_channel_fails_over_to_the_next_candidate() {
        use tokio::sync::mpsc;

        let (tx_a, rx_a) = mpsc::channel(1);
        drop(rx_a); // 关闭 A：真实的终端故障
        let (tx_b, mut rx_b) = mpsc::channel(1);

        let agents = vec![SinkTerminal::Channel(tx_a), SinkTerminal::Channel(tx_b)];
        let mut idx = 0usize;
        ActCovPicker::send_payload_with_failover(&agents, &mut idx, record_payload())
            .await
            .expect("closed candidate must fail over");

        assert_eq!(idx, 1, "切到下一个候选并记住它");
        assert!(rx_b.try_recv().is_ok(), "数据落到可用候选");
    }

    #[tokio::test]
    async fn all_candidates_closed_returns_error() {
        use tokio::sync::mpsc;

        let (tx_a, rx_a) = mpsc::channel(1);
        let (tx_b, rx_b) = mpsc::channel(1);
        drop(rx_a);
        drop(rx_b);

        let agents = vec![SinkTerminal::Channel(tx_a), SinkTerminal::Channel(tx_b)];
        let mut idx = 0usize;
        let err = ActCovPicker::send_payload_with_failover(&agents, &mut idx, record_payload())
            .await
            .expect_err("all candidates closed must surface an error");
        assert!(
            !err.to_string().is_empty(),
            "error keeps the underlying reason"
        );
    }

    #[test]
    fn test_get_sink_group_from_rescue_path() {
        let group = ActCovPicker::get_sink_group(
            "./data/rescue/sink/benchmark/[0]-2026-10-26_14:41:46-0.dat",
            "./data/rescue",
        );
        assert_eq!(group, "sink/benchmark");

        let group = ActCovPicker::get_sink_group(
            "./data/rescue/default/[0]-2026-10-26_10:00:00-0.dat",
            "./data/rescue",
        );
        assert_eq!(group, "default");

        // 嵌套组名（分组名本身带斜杠）
        let group = ActCovPicker::get_sink_group(
            "./data/rescue/a/b/c/bench_sink-2026-10-26_10:00:00-0.dat",
            "./data/rescue",
        );
        assert_eq!(group, "a/b/c");

        // 旧布局：直接放在 rescue 根目录下，没有 group 段，调用方回退到按名匹配
        let group = ActCovPicker::get_sink_group(
            "./data/rescue/bench_sink-2026-10-26_10:00:00.dat",
            "./data/rescue",
        );
        assert!(group.is_empty(), "legacy layout has no group segment");
    }

    #[test]
    fn test_relative_rescue_display_path() {
        let display = relative_rescue_display_path(
            "./data/rescue/sink/benchmark/[0]-2026-03-29_11:52:41-0.dat",
            "./data/rescue",
        );
        assert_eq!(display, "sink/benchmark/[0]-2026-03-29_11:52:41-0.dat");
    }

    //test tack_lasts_file
    #[test]
    fn test_tack_lasts_file() -> RunResult<()> {
        fs::create_dir_all("rescue1").assert();
        fs::write(
            "rescue1/benchmark_file_sink-2023-12-06_12:07:02.dat",
            "1222",
        )
        .assert();
        fs::write(
            "rescue1/http_accs_file_sink-2023-12-06_12:07:02.dat",
            "5666",
        )
        .assert();
        fs::write(
            "rescue1/http_accs_file_sink-2023-12-06_12:07:03.dat",
            "2333",
        )
        .assert();

        let rescues = RescueFiles::new("rescue1");
        let found = rescues.tack_lasts_file("dat")?;
        assert_eq!(
            found,
            Some("rescue1/http_accs_file_sink-2023-12-06_12:07:03.dat".to_string())
        );
        fs::remove_dir_all("rescue1").assert();
        Ok(())
    }

    // 支持递归子目录：嵌套路径也能被扫描与挑选
    #[test]
    fn test_tack_lasts_file_nested() -> RunResult<()> {
        fs::create_dir_all("rescue_nested/group1").assert();
        fs::write(
            "rescue_nested/group1/bench_sink-2025-10-14_03:10:11.dat",
            "a",
        )
        .assert();
        fs::write(
            "rescue_nested/group1/bench_sink-2025-10-14_03:10:12.dat",
            "b",
        )
        .assert();

        let rescues = RescueFiles::new("rescue_nested");
        let found = rescues.tack_lasts_file("dat")?;
        assert_eq!(
            found,
            Some("rescue_nested/group1/bench_sink-2025-10-14_03:10:12.dat".to_string())
        );
        fs::remove_dir_all("rescue_nested").assert();
        Ok(())
    }

    #[test]
    fn test_check_point() {
        let mut point = CheckPoint::default();

        point.rec_suc(".text.txt");
        point.rec_suc(".text.txt");

        point.rec_suc(".text1.txt");
        point.save_point().assert();

        let mut point = CheckPoint::load_point().assert();
        assert_eq!(point.0.remove(".text.txt"), Some(2));
        assert_eq!(point.0.remove(".text1.txt"), Some(1));
    }
}
