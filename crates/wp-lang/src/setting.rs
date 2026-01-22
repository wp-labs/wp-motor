pub fn check_level_or_stop(
    check_continue: Option<usize>,
    check_stop: Option<usize>,
) -> (usize, bool) {
    let lev = check_stop.or(check_continue).unwrap_or(0);
    if check_stop.is_some() {
        return (lev, true);
    }
    (lev, false)
}

#[derive(Clone, Debug)]
pub enum PattenMode {
    Automated,
    Precise,
}

#[derive(Clone, Debug)]
pub struct WplSetting {
    pub mode: PattenMode,
    pub need_complete: bool,
}

impl Default for WplSetting {
    fn default() -> Self {
        WplSetting {
            mode: PattenMode::Automated,
            need_complete: false,
        }
    }
}

impl WplSetting {
    pub fn new(mode: PattenMode, need_complete: bool) -> Self {
        WplSetting {
            mode,
            need_complete,
        }
    }
}
