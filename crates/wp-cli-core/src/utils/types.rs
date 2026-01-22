use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Ctx {
    pub work_root: PathBuf,
    pub group_filters: Vec<String>,
    pub sink_filters: Vec<String>,
    pub path_like: Option<String>,
    pub total_only: bool,
}

impl Ctx {
    pub fn new<S: Into<PathBuf>>(work_root: S) -> Self {
        Self {
            work_root: work_root.into(),
            group_filters: vec![],
            sink_filters: vec![],
            path_like: None,
            total_only: false,
        }
    }
}

/// A flattened row for source line information
#[derive(Debug, Serialize, Clone)]
pub struct SrcLineItem {
    pub key: String,
    pub path: String,
    pub enabled: bool,
    pub lines: Option<u64>,
    pub error: Option<String>,
}

/// Report of source file line counts
#[derive(Debug, Serialize, Clone)]
pub struct SrcLineReport {
    pub total_enabled_lines: u64,
    pub items: Vec<SrcLineItem>,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub group: String,
    pub sink: String,
    pub path: String,
    pub infras: bool,
    pub lines: u64,
}

impl Row {
    pub fn ok(group: String, sink: String, path: PathBuf, framework: bool, lines: u64) -> Self {
        Row {
            group,
            sink,
            path: path.display().to_string(),
            infras: framework,
            lines,
        }
    }
    pub fn err(group: String, sink: String, path: PathBuf, framework: bool) -> Self {
        Row {
            group,
            sink,
            path: path.display().to_string(),
            infras: framework,
            lines: 0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JsonOut {
    pub total: u64,
    pub items: Vec<Row>,
}

#[derive(Clone)]
pub struct SinkAccum {
    pub name: String,
    pub lines: u64,
    pub expect: Option<wp_conf::structure::SinkExpectOverride>,
}

#[derive(Clone)]
pub struct GroupAccum {
    pub name: String,
    pub expect: Option<wp_conf::structure::GroupExpectSpec>,
    pub sinks: Vec<SinkAccum>,
}

impl GroupAccum {
    pub fn new(name: String, expect: Option<wp_conf::structure::GroupExpectSpec>) -> Self {
        Self {
            name,
            expect,
            sinks: Vec::new(),
        }
    }
    pub fn add_sink(&mut self, s: SinkAccum) {
        self.sinks.push(s);
    }
}

#[derive(Default)]
pub struct ValidateReport {
    pub items: Vec<ValidateItem>,
}

impl ValidateReport {
    pub fn has_error_fail(&self) -> bool {
        self.items
            .iter()
            .any(|it| matches!(it.severity, Severity::Error | Severity::Panic))
    }
}

#[derive(Clone, Copy)]
pub enum Severity {
    Warn,
    Error,
    Panic,
}

pub struct ValidateItem {
    pub group: String,
    pub sink: Option<String>,
    pub msg: String,
    pub severity: Severity,
}
