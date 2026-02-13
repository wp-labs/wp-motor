use derive_more::Display;

use crate::types::AnyResult;

/// 运行模式：批处理/常驻
#[derive(Clone, Debug, PartialEq, Eq, Display)]
pub enum RuntimeMode {
    /// 常驻服务，默认模式；启动所有接受器，长时间运行
    Daemon,
    /// 批处理；不启动接受器或将其置于非主组，文件源读尽后整体退出
    Batch,
}

#[derive(Clone, Debug)]
pub struct RuntimeArgs {
    pub line_max: Option<usize>,
    pub parallel: usize,
    pub speed_limit: usize,
    pub log: bool,
    pub check: usize,
    pub check_fail_stop: bool,
    pub need_complete: bool,
    pub stat_sec: usize,
    pub stat_print: bool,
    pub ldm_root: String,
    // 阶段开关：从 EngineConfig 派生
    pub skip_parse: bool,
    pub skip_sink: bool,
    // 语义分析开关：从 EngineConfig [semantic].enabled 派生
    pub semantic_enabled: bool,
}

impl Default for RuntimeArgs {
    fn default() -> Self {
        RuntimeArgs {
            line_max: None,
            parallel: 2,
            speed_limit: 10000,
            log: true,
            check: 0,
            check_fail_stop: false,
            need_complete: false,
            stat_sec: 1,
            stat_print: false,
            ldm_root: "./ldm".to_string(),
            skip_parse: false,
            skip_sink: false,
            semantic_enabled: false,
        }
    }
}

impl RuntimeArgs {
    pub fn for_test() -> AnyResult<Self> {
        Ok(Self {
            line_max: Some(10),
            ..Default::default()
        })
    }
    pub fn ge_max(&self, line_cnt: usize) -> bool {
        if let Some(max) = self.line_max
            && line_cnt >= max
        {
            return true;
        }
        false
    }
    pub fn gt_max(&self, line_cnt: usize) -> bool {
        if let Some(max) = self.line_max
            && line_cnt > max
        {
            return true;
        }
        false
    }
    pub fn need_flush(&self, line_cnt: usize) -> bool {
        line_cnt.is_multiple_of(100)
    }
    pub fn dry_run(&mut self) {
        self.line_max = Some(10);
        self.parallel = 2;
        self.speed_limit = 5;
        self.stat_sec = 1;
        self.stat_print = true;
    }
}

pub trait RuntimeArgsFrom {
    fn from_args(args: &RuntimeArgs) -> Self;
}
/*
impl RuntimeArgsFrom for ParseArsenals {
    fn from_args(args: &RuntimeArgs) -> Self {
        Self {
            pc_stg: ParseStrategy::new(args.mode.clone(), args.need_complete),
            check: Checker::new(args.check_fail_stop, args.check),
        }
    }
}

*/
