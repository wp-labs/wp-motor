use crate::facade::test_helpers::SinkTerminal;
use crate::runtime::actor::command::{CmdSubscriber, TaskController, TaskEndReason};
use crate::sinks::{ProcMeta, RescueEntry, RescuePayload, SinkRouteAgent};
use crate::stat::metric_collect::MetricCollectors;
use crate::stat::{MonSend, STAT_INTERVAL_MS};
use chrono::NaiveDateTime;

use wp_error::RunErrorOwe;
use wp_error::run_error::RunResult;
use wp_stat::StatReq;

use crate::types::AnyResult;
use wp_stat::StatRecorder;

use orion_error::ErrorOwe;
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
        let mut check_point = CheckPoint::load_point().owe_data()?;
        let mut idle_since: Option<Instant> = None;
        loop {
            //当前还在写入的救急文件(文件后缀有.lock), 不能马上 recover.
            let paths = rescues.tack_lasts_file("dat").owe_data()?;
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
        if let Some((sink_agent, sink_kind)) = route_agent.get_sink_agent(sink_name.as_str()) {
            //所有sink 都在运行,并没有逻辑上的问题!?
            /*
            self.cmd_pub
                .broadcast(CtrlCmd::Work(DoScope::One(sink_name.clone())))
                .await?;
                */
            sleep(std::time::Duration::from_millis(100)).await;
            info_dfx!("recover file: {}", paths);
            match self
                .pick_file(
                    sink_agent,
                    &paths,
                    self.speed_limit,
                    check_point,
                    sink_kind,
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
        } else {
            error_dfx!("no sink agent for {}", sink_name);
        }
        Ok(())
    }

    async fn pick_file(
        &self,
        dat_s: SinkTerminal,
        file_path: &String,
        speed: usize,
        check_point: &mut CheckPoint,
        _sink_kind: String,
        stat_reqs: Vec<StatReq>,
    ) -> RunResult<TaskEndReason> {
        let mut run_ctrl = TaskController::from_speed_limit(
            "recover-file",
            self.cmd_sub.clone(),
            Some(speed),
            100,
        );
        let mut stat = MetricCollectors::new(file_path.clone(), stat_reqs);

        let stat_interval = Duration::from_millis(STAT_INTERVAL_MS as u64);
        let mut last_stat_tick = Instant::now();
        let file = File::open(file_path).await.owe_data()?;
        let mut reader = BufReader::new(file);
        #[allow(unused_assignments)]
        let mut end_reason = TaskEndReason::Interrupt;
        info_dfx!("recover begin! file : {}", file_path);
        loop {
            run_ctrl.rec_task_unit_reset();
            while !run_ctrl.is_unit_end() {
                let mut buffer = String::new();
                let size = reader.read_line(&mut buffer).await.owe_data()?;
                if size.eq(&0) {
                    stat.record_task(file_path.as_str(), None);
                    fs::remove_file(file_path).owe_sys()?;
                    check_point.remove_point(file_path);
                    info_dfx!("recover end! clean file : {}", file_path);
                    println!("recover file finished! : {}", file_path);
                    return Ok(TaskEndReason::SucEnded);
                }
                stat.record_begin(file_path.as_str(), None);
                let trimmed = buffer.trim_end_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    continue;
                }

                let entry = RescueEntry::parse(trimmed).owe_data()?;
                match entry.into_payload() {
                    RescuePayload::Record { record } => {
                        dat_s
                            .send_record(0, ProcMeta::Null, Arc::new(record))
                            .owe_sink()?;
                    }
                    RescuePayload::Raw { raw } => {
                        dat_s.send_raw(0, raw).owe_sink()?;
                    }
                }
                stat.record_end(file_path.as_str(), None);
                run_ctrl.rec_task_suc();
                check_point.rec_suc(file_path);
            }

            if last_stat_tick.elapsed() >= stat_interval {
                stat.send_stat(&self.mon_s).await.owe_res()?;
                check_point.save_point().owe_sys()?;
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
        check_point.save_point().owe_sys()?;
        stat.send_stat(&self.mon_s).await.owe_res()?;
        Ok(end_reason)
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

    pub fn tack_lasts_file(&self, ends: &str) -> AnyResult<Option<String>> {
        let mut files = Vec::new();
        let paths = WalkDir::new(&self.path);
        for entry in paths {
            let entry = entry?;
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
    pub fn save_point(&mut self) -> AnyResult<()> {
        let point = serde_json::to_string(self)?;
        let path = Path::new(POINT_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(POINT_PATH, point)?;
        Ok(())
    }

    pub fn load_point() -> AnyResult<CheckPoint> {
        let point = match fs::read_to_string(POINT_PATH) {
            Ok(val) => serde_json::from_str(&val)?,
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
    use crate::runtime::collector::recovery::{ActCovPicker, CheckPoint, RescueFiles};
    use crate::types::AnyResult;
    use orion_error::TestAssert;

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

    //test tack_lasts_file
    #[test]
    fn test_tack_lasts_file() -> AnyResult<()> {
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
    fn test_tack_lasts_file_nested() -> AnyResult<()> {
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
