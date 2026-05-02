#![allow(dead_code)]
use async_signal::{Signal, Signals};
use orion_error::{ErrorWith, UvsFrom, compat_prelude::ErrorOweBase};
use std::{
    fmt::Display,
    sync::atomic::{AtomicBool, Ordering},
};
use wp_error::run_error::{RunReason, RunResult};

#[derive(Debug, PartialEq, Clone)]
pub enum ShutdownCmd {
    Immediate,
    CountLimit(usize),
    Timeout(usize),
    NoOp,
}

impl Display for ShutdownCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.clone() {
            ShutdownCmd::Immediate => write!(f, "StopNow"),
            ShutdownCmd::CountLimit(limit) => write!(f, "LimitEnd({})", limit),
            ShutdownCmd::Timeout(millis) => write!(f, "EmptyWait({})", millis),
            ShutdownCmd::NoOp => write!(f, "Ignore"),
        }
    }
}

pub fn stop_signals() -> RunResult<Signals> {
    let signals = Signals::new([Signal::Term, Signal::Quit, Signal::Int])
        .owe(RunReason::from_sys())
        .doing("set signal")?;
    Ok(signals)
}

pub async fn get_stop(is_end: impl Fn() -> bool) -> RunResult<ShutdownCmd> {
    if is_end() {
        return Ok(ShutdownCmd::Immediate);
    }
    Ok(ShutdownCmd::NoOp)
}

pub fn is_routine_running() -> bool {
    GLOBAL_RUN_FLAG.load(Ordering::Relaxed)
}

pub fn stop_routine_run() {
    GLOBAL_RUN_FLAG.store(false, Ordering::Relaxed);
}

static GLOBAL_RUN_FLAG: AtomicBool = AtomicBool::new(true);
