use std::fmt::Debug;

use thiserror::Error;

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum FailureAction {
    #[default]
    Continue,
    Break,
    Panic,
}

#[derive(Debug, Clone)]
pub struct Checker {
    err_end: bool,
    level_ctrl: [bool; 5],
}

#[derive(Error)]
pub struct CheckFail(pub String);

impl Checker {
    pub fn new(err_end: bool, level: usize) -> Self {
        let safe_level = if level > 5 { 5 } else { level };
        let mut obj = Self {
            err_end,
            level_ctrl: [false; 5],
        };
        for i in 0..safe_level {
            obj.level_ctrl[i] = true;
        }
        obj
    }
    pub fn need_notice(&self, level: usize) -> bool {
        debug_assert!(level < 5);
        self.level_ctrl[level]
    }
    #[inline]
    pub fn will_end(&self) -> bool {
        self.err_end
    }
    pub fn fail_action(&self, format_args: std::fmt::Arguments<'_>) -> Result<(), CheckFail> {
        if self.err_end {
            return Err(CheckFail(format!("{}", format_args)));
        } else {
            info!("{}", format_args);
        }
        Ok(())
    }
    pub fn suc_notice(&self, format_args: std::fmt::Arguments<'_>) {
        if self.need_notice(0) {
            println!("[ck-suc notice:] {}", format_args);
        }
    }
    pub fn step_notice(&self, level: usize, format_args: std::fmt::Arguments<'_>) {
        if self.need_notice(level) {
            println!("[ck-step {} :] {}", level, format_args);
        }
    }
}

mod std_trait_impl {
    use std::fmt::{Debug, Display, Formatter};

    use super::*;

    impl Debug for CheckFail {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "CheckFail! ({})", self.0)
        }
    }

    impl Display for CheckFail {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "CheckFail! ({})", self.0)
        }
    }

    impl Default for Checker {
        fn default() -> Self {
            Self::new(false, 0)
        }
    }

    impl Display for FailureAction {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                FailureAction::Continue => write!(f, "continue"),
                FailureAction::Break => write!(f, "break"),
                FailureAction::Panic => write!(f, "panic"),
            }
        }
    }

    impl From<&str> for FailureAction {
        fn from(value: &str) -> Self {
            if value == "break" {
                FailureAction::Break
            } else if value == "panic" {
                FailureAction::Panic
            } else {
                FailureAction::Continue
            }
        }
    }
}
