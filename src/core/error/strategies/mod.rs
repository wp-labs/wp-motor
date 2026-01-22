pub mod err_4debug;
pub mod err_4normal;
pub mod err_4stoic;

use chrono::{DateTime, Local};
use err_4debug::Err4Debug;
use err_4normal::Err4Normal;
use err_4stoic::Err4Stoic;
use orion_error::ErrStrategy;
use std::fmt::{Display, Formatter};
use wp_connector_api::SinkError;
use wp_connector_api::SourceError;
use wp_error::error_handling::{ErrorHandlingStrategy, RobustnessMode, sys_robust_mode};
use wp_error::parse_error::OMLCodeError;
use wpl::WparseError;
use wpl::parser::error::WplCodeError;

#[allow(dead_code)]
pub trait ErrorHandlingPolicy {
    fn err4_send_to_sink(&self, err: &SinkError) -> ErrorHandlingStrategy;
    fn err4_load_oml(&self, err: &OMLCodeError) -> ErrStrategy;
    fn err4_load_wpl(&self, err: &WplCodeError) -> ErrStrategy;
    fn err4_engine_parse_data(&self, err: &WparseError) -> ErrorHandlingStrategy;
    fn err4_dispatch_data(&self, err: &SourceError) -> ErrorHandlingStrategy;
}

static ERR_STRATEGY_DEV: Err4Debug = Err4Debug::init();
static ERR_STRATEGY_BETA: Err4Normal = Err4Normal::init();
static ERR_STRATEGY_STOIC: Err4Stoic = Err4Stoic::init();

pub fn current_error_policy() -> &'static dyn ErrorHandlingPolicy {
    match sys_robust_mode() {
        RobustnessMode::Debug => &ERR_STRATEGY_DEV,
        RobustnessMode::Normal => &ERR_STRATEGY_BETA,
        RobustnessMode::Strict => &ERR_STRATEGY_STOIC,
    }
}

pub enum ReportEnum {
    WPLang,
    OMLang,
}
impl Display for ReportEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportEnum::WPLang => {
                write!(f, "WPL")?;
            }
            ReportEnum::OMLang => {
                write!(f, "OML")?;
            }
        }
        Ok(())
    }
}
pub struct ErrReport {
    e_type: ReportEnum,
    now: DateTime<Local>,
    summary: String,
    error: Option<String>,
    code: Option<String>,
}

impl ErrReport {
    pub(crate) fn add_code<S: Into<String>>(&mut self, code: S) {
        self.code = Some(code.into());
    }
}

impl ErrReport {
    pub(crate) fn add_error<E: Display>(&mut self, p0: E) {
        self.error = Some(p0.to_string())
    }
}

impl ErrReport {
    pub fn new_wpl<S: Into<String>>(summary: S) -> Self {
        Self {
            e_type: ReportEnum::WPLang,
            now: Local::now(),
            summary: summary.into(),
            error: None,
            code: None,
        }
    }
    pub fn new_oml<S: Into<String>>(summary: S) -> Self {
        Self {
            e_type: ReportEnum::OMLang,
            now: Local::now(),
            summary: summary.into(),
            error: None,
            code: None,
        }
    }
    pub fn generate_json(&self) -> String {
        let json = serde_json::json!({
            "describe": self.summary,
            "msg_time": self.now,
            "detail": self.to_string(),
        });
        json.to_string()
    }
}

impl Display for ErrReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let title = format!("{} Error Report : {}", self.e_type, self.now);
        writeln!(f, "{:=^80}", title)?;
        writeln!(f, "\n{}", self.summary)?;
        if let Some(e) = &self.error {
            writeln!(f, "error:\n{}", e)?;
        }
        if let Some(code) = &self.code {
            writeln!(f, "\n{:~^80}\n{}\n{:~^80}\n\n", "CODE", code, "code")?;
        }
        Ok(())
    }
}
