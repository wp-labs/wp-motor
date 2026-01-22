use orion_exp::Expression;
use orion_exp::RustSymbol;
use wp_data_model::conditions::evaluate_expression;
use wp_model_core::model::{DataField, DataRecord};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AlertSeverity {
    /// 正常操作状态
    Normal = 1,
    /// 需要关注的异常情况
    Warning = 2,
    /// 需要立即干预的错误
    Critical = 3,
    /// 系统不可用状态
    Fatal = 4,
}
impl From<&str> for AlertSeverity {
    fn from(data: &str) -> Self {
        match data {
            "warn" => AlertSeverity::Warning,
            "error" => AlertSeverity::Critical,
            "fatal" => AlertSeverity::Fatal,
            _ => panic!("not support notify level"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MonitPhase {
    ParserSummary = 1,
    ParserRealtime = 2,
    RecoveryRealtime = 3,
    RecoverySummary = 4,
}

impl From<&str> for MonitPhase {
    fn from(data: &str) -> Self {
        match data {
            "end_sum" => MonitPhase::ParserSummary,
            "realtime" => MonitPhase::ParserRealtime,
            "rcov_end_sum" => MonitPhase::RecoverySummary,
            "rcov_realtime" => MonitPhase::RecoveryRealtime,
            _ => panic!("not support usecase"),
        }
    }
}
const NOTIFY_KEY: &str = "moni_notify";
const PROMPT_KEY: &str = "moni_prompt";

#[derive(Clone, Debug, PartialEq)]
pub struct AlertEvent {
    severity: AlertSeverity,
    message: String,
}

impl AlertEvent {
    pub fn new(notify: AlertSeverity, prompt: String) -> Self {
        Self {
            severity: notify,
            message: prompt,
        }
    }
    pub fn normal() -> Self {
        Self {
            severity: AlertSeverity::Normal,
            message: "".to_string(),
        }
    }
    pub fn is_normal(&self) -> bool {
        self.severity == AlertSeverity::Normal
    }
    pub fn level(&self) -> AlertSeverity {
        self.severity
    }
    pub fn prompt(&self) -> &str {
        self.message.as_str()
    }
}
pub struct AlertRule {
    condition: Expression<DataField, RustSymbol>,
    severity: AlertSeverity,
    message: String,
    cases: Vec<MonitPhase>,
}
impl AlertRule {
    pub fn alarm_proc(&self, case: MonitPhase, tdc: &mut DataRecord) -> AlertEvent {
        if self.cases.contains(&case) && evaluate_expression(&self.condition, tdc) {
            debug_mtrc!("alarm {:?} proc: {:?}", self.severity, tdc);
            tdc.remove_field(NOTIFY_KEY);
            tdc.append(DataField::from_digit(NOTIFY_KEY, self.severity as i64));
            tdc.remove_field(PROMPT_KEY);
            tdc.append(DataField::from_chars(PROMPT_KEY, self.message.as_str()));
            return AlertEvent::new(self.severity, self.message.clone());
        }
        AlertEvent::normal()
    }
    pub fn new(
        cond: Expression<DataField, RustSymbol>,
        notify: AlertSeverity,
        prompt: String,
        cases: Vec<MonitPhase>,
    ) -> Self {
        Self {
            condition: cond,
            severity: notify,
            message: prompt,
            cases,
        }
    }
}

pub struct MonitorParser {}
impl MonitorParser {}

#[cfg(test)]
mod tests {}
