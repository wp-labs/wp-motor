use crate::sinks::prelude::*;
use async_trait::async_trait;
use orion_error::UnifiedReason;
use std::time::Instant;
//use wp_model_core::instant::Instant;

use wp_connector_api::{SinkError, SinkReason, SinkResult};

/// 健康阶段表环境变量，格式为逗号分隔的 `状态:秒数`，例如 `up:2,down:3,up:3`。
/// 首个阶段在 sink 构建完成后立即生效；阶段表耗尽后 sink 视为健康。
const ENV_RESCUE_STAGES: &str = "WP_TEST_RESCUE_STAGES";
/// 阶段表循环开关（`1/true/yes/on`）：开启后阶段表按顺序反复执行。
const ENV_RESCUE_REPEAT: &str = "WP_TEST_RESCUE_REPEAT";
/// 默认阶段表：健康 2s -> 故障 2s，之后视为健康（与历史行为一致）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StgPoint {
    secs: usize,
    health: bool,
}

impl StgPoint {
    pub fn new(secs: usize, health: bool) -> Self {
        Self { secs, health }
    }
    /// 阶段表耗尽后的状态：健康且不再推进。
    fn healthy_forever() -> Self {
        Self {
            secs: 0,
            health: true,
        }
    }
}

#[derive(Debug)]
pub struct HealthController {
    cur_stg: StgPoint,
    /// 完整阶段表；`cursor` 指向下一个待切换的阶段。
    stages: Vec<StgPoint>,
    cursor: usize,
    repeat: bool,
    /// 阶段表已耗尽（非循环模式），不再推进计时。
    finished: bool,
    pot_beg: std::time::Instant,
}

impl Default for HealthController {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthController {
    /// 默认阶段表：健康 2s -> 故障 2s，之后视为健康。
    pub fn new() -> Self {
        Self::with_schedule(vec![StgPoint::new(2, true), StgPoint::new(2, false)], false)
    }

    /// 用显式阶段表构造；`stages` 为空时视为始终健康。
    pub(crate) fn with_schedule(stages: Vec<StgPoint>, repeat: bool) -> Self {
        let cur_stg = stages
            .first()
            .copied()
            .unwrap_or_else(StgPoint::healthy_forever);
        Self {
            cur_stg,
            finished: stages.is_empty(),
            stages,
            cursor: 1,
            repeat,
            pot_beg: Instant::now(),
        }
    }

    /// 从环境变量构造阶段表，供 e2e 用例构造确定性时序。
    ///
    /// - `WP_TEST_RESCUE_STAGES`：`up:2,down:3,up:3`（秒），未设置时用默认阶段表；
    /// - `WP_TEST_RESCUE_REPEAT`：阶段表循环开关。
    pub fn from_env() -> SinkResult<Self> {
        let stages = std::env::var(ENV_RESCUE_STAGES).ok();
        let repeat = std::env::var(ENV_RESCUE_REPEAT).ok();
        Self::from_lookup(stages.as_deref(), repeat.as_deref())
    }

    /// `from_env` 的纯函数版本，便于单测不碰进程环境变量。
    fn from_lookup(stages: Option<&str>, repeat: Option<&str>) -> SinkResult<Self> {
        let repeat = repeat.map(is_truthy).unwrap_or(false);
        let raw = stages.map(str::trim).filter(|s| !s.is_empty());
        let Some(raw) = raw else {
            let mut ctrl = Self::new();
            ctrl.repeat = repeat;
            return Ok(ctrl);
        };
        let stages = parse_schedule(raw).map_err(|e| {
            SinkReason::Uvs(UnifiedReason::core_conf())
                .err_detail(format!("invalid {ENV_RESCUE_STAGES}='{raw}': {e}"))
        })?;
        Ok(Self::with_schedule(stages, repeat))
    }

    fn use_next_stg(&mut self) {
        if self.cursor < self.stages.len() {
            self.cur_stg = self.stages[self.cursor];
            self.cursor += 1;
            self.pot_beg = Instant::now();
            return;
        }
        if self.repeat {
            self.cur_stg = self.stages[0];
            self.cursor = 1;
            self.pot_beg = Instant::now();
            return;
        }
        // 阶段表执行完毕：判定为健康并停止推进，避免"最后一个阶段是故障"导致 sink 永久不可用。
        self.cur_stg = StgPoint::healthy_forever();
        self.finished = true;
        self.pot_beg = Instant::now();
    }

    pub fn decide_health(&mut self) -> bool {
        if !self.finished
            && self.pot_beg.elapsed() > time::Duration::seconds(self.cur_stg.secs as i64)
        {
            self.use_next_stg();
        }
        self.cur_stg.health
    }
}

fn is_truthy(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn parse_schedule(raw: &str) -> Result<Vec<StgPoint>, String> {
    let mut stages = Vec::new();
    for item in raw.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (state, secs) = item
            .split_once(':')
            .ok_or_else(|| format!("stage '{item}' must look like 'up:2' or 'down:3'"))?;
        let health = match state.trim().to_ascii_lowercase().as_str() {
            "up" | "on" | "ok" | "healthy" => true,
            "down" | "off" | "dead" | "fail" => false,
            other => {
                return Err(format!(
                    "unknown stage state '{other}' (expected 'up' or 'down')"
                ));
            }
        };
        let secs = secs
            .trim()
            .parse::<usize>()
            .map_err(|e| format!("invalid seconds in stage '{item}': {e}"))?;
        stages.push(StgPoint::new(secs, health));
    }
    if stages.is_empty() {
        return Err("schedule must contain at least one stage".to_string());
    }
    Ok(stages)
}
pub struct ASinkTestProxy {
    ctrl_stg: HealthController,
    impl_ins: Box<dyn AsyncSink>,
}
impl ASinkTestProxy {
    pub fn new<T>(ins: T, ctrl_stg: HealthController) -> Self
    where
        T: AsyncSink + 'static,
    {
        Self {
            ctrl_stg,
            impl_ins: Box::new(ins),
        }
    }
}

#[async_trait]
impl AsyncCtrl for ASinkTestProxy {
    async fn stop(&mut self) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl).doing("stop send to AsinkTestStub"))?;
        }
        self.impl_ins.stop().await?;
        Ok(())
    }

    async fn reconnect(&mut self) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.reconnect().await?;
        Ok(())
    }
}

#[async_trait]
impl AsyncRecordSink for ASinkTestProxy {
    async fn sink_record(&mut self, data: &DataRecord) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_record(data).await
    }

    async fn sink_records(&mut self, data: Vec<std::sync::Arc<DataRecord>>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_records(data).await
    }
}

#[async_trait]
impl AsyncRawdatSink for ASinkTestProxy {
    async fn sink_str(&mut self, data: &str) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_str(data).await
    }
    async fn sink_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_bytes(data).await
    }

    async fn sink_str_batch(&mut self, data: Vec<&str>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_str_batch(data).await
    }

    async fn sink_bytes_batch(&mut self, data: Vec<&[u8]>) -> SinkResult<()> {
        if !self.ctrl_stg.decide_health() {
            Err(SinkError::from(SinkReason::StgCtrl))?;
        }
        self.impl_ins.sink_bytes_batch(data).await
    }
}
// Blanket impl of AsyncSink is provided by wp-sink-api; no explicit impl needed here.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_schedule_accepts_up_and_down_stages() {
        let stages = parse_schedule("up:2, down:3,up:4").expect("valid schedule");
        assert_eq!(
            stages,
            vec![
                StgPoint::new(2, true),
                StgPoint::new(3, false),
                StgPoint::new(4, true)
            ]
        );
    }

    #[test]
    fn parse_schedule_rejects_malformed_input() {
        assert!(parse_schedule("up").is_err(), "missing seconds");
        assert!(parse_schedule("sideways:1").is_err(), "unknown stage state");
        assert!(parse_schedule("up:abc").is_err(), "non numeric seconds");
        assert!(parse_schedule(" , ").is_err(), "empty schedule");
    }

    #[test]
    fn zero_length_stages_advance_on_each_check() {
        // 阶段时长为 0 时每次检查都会推进；插 sleep 是为了不依赖 Instant 的时钟精度。
        let mut ctrl = HealthController::with_schedule(
            vec![StgPoint::new(0, true), StgPoint::new(0, false)],
            false,
        );
        advance(&mut ctrl);
        assert!(!ctrl.decide_health(), "advance into the down stage");
        advance(&mut ctrl);
        assert!(
            ctrl.decide_health(),
            "schedule exhausted -> healthy forever"
        );
        assert!(ctrl.decide_health(), "finished controller stays healthy");
    }

    #[test]
    fn repeat_cycles_the_schedule() {
        let mut ctrl = HealthController::with_schedule(
            vec![StgPoint::new(0, true), StgPoint::new(0, false)],
            true,
        );
        let mut seen = Vec::new();
        for _ in 0..4 {
            advance(&mut ctrl);
            seen.push(ctrl.decide_health());
        }
        assert_eq!(seen, vec![false, true, false, true]);
    }

    /// 让上一次 `pot_beg` 明确过期，避免零时长阶段被视为“还没到切换时间”。
    fn advance(ctrl: &mut HealthController) {
        std::thread::sleep(std::time::Duration::from_millis(2));
        let _ = ctrl;
    }

    #[test]
    fn default_schedule_starts_healthy() {
        let mut ctrl = HealthController::new();
        assert!(ctrl.decide_health(), "default schedule starts healthy");
    }

    #[test]
    fn empty_schedule_is_always_healthy() {
        let mut ctrl = HealthController::with_schedule(Vec::new(), true);
        assert!(ctrl.decide_health());
        assert!(ctrl.decide_health());
    }

    #[test]
    fn from_lookup_without_stages_uses_default_schedule() {
        let mut ctrl = HealthController::from_lookup(None, None).expect("default schedule");
        assert!(ctrl.decide_health(), "default starts healthy");

        // 空字符串与未设置等价
        let ctrl = HealthController::from_lookup(Some("   "), None);
        assert!(ctrl.is_ok(), "blank stages fall back to default");
    }

    #[test]
    fn from_lookup_parses_stages_and_repeat() {
        let ctrl =
            HealthController::from_lookup(Some("up:1,down:2"), Some("1")).expect("valid schedule");
        assert_eq!(ctrl.stages.len(), 2);
        assert!(ctrl.repeat, "'1' enables repeat");

        for truthy in ["1", "true", "YES", "on"] {
            let ctrl =
                HealthController::from_lookup(Some("up:1"), Some(truthy)).expect("valid schedule");
            assert!(ctrl.repeat, "'{truthy}' should enable repeat");
        }
        for falsy in ["", "0", "false", "off", "nope"] {
            let ctrl =
                HealthController::from_lookup(Some("up:1"), Some(falsy)).expect("valid schedule");
            assert!(!ctrl.repeat, "'{falsy}' should not enable repeat");
        }
    }

    #[test]
    fn from_lookup_rejects_invalid_stages() {
        let err = HealthController::from_lookup(Some("up:1,sideways:2"), None)
            .expect_err("invalid stage state must fail");
        let text = err.to_string();
        assert!(
            text.contains(ENV_RESCUE_STAGES),
            "err mentions the env var: {text}"
        );
        assert!(
            text.contains("sideways"),
            "err mentions the bad token: {text}"
        );
    }
}
