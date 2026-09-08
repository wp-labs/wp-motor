//! Lightweight OML 运行期诊断收集（可选）。
//! - 默认无操作（零开销）；开启 `oml-diag` feature 时启用收集。
//! - 使用线程局部缓冲，避免在 API 上到处传参。

#[derive(Debug, Clone, PartialEq)]
pub enum OmlIssueKind {
    MissingField,       // 读取/格式化变量缺失
    FmtVarMissing,      // fmt 中的变量缺失
    UnsupportedConvert, // 不支持的类型转换
    ParseFail,          // 文本解析为目标类型失败
    BatchNoMatch,       // 批量匹配 0 命中
    MathEvalFail,       // 数学表达式求值失败
}

#[derive(Debug, Clone)]
pub struct OmlIssue {
    pub kind: OmlIssueKind,
    pub detail: String,
}

impl OmlIssue {
    pub fn new(kind: OmlIssueKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
    pub fn to_brief(&self) -> String {
        match self.kind {
            OmlIssueKind::MissingField => format!("missing_field: {}", self.detail),
            OmlIssueKind::FmtVarMissing => format!("fmt_var_missing: {}", self.detail),
            OmlIssueKind::UnsupportedConvert => format!("unsupported_convert: {}", self.detail),
            OmlIssueKind::ParseFail => format!("parse_fail: {}", self.detail),
            OmlIssueKind::BatchNoMatch => format!("batch_no_match: {}", self.detail),
            OmlIssueKind::MathEvalFail => format!("math_eval_fail: {}", self.detail),
        }
    }
}

// 线程局部缓冲；未启用 feature 时保持空实现
#[cfg(feature = "oml-diag")]
mod inner {
    use super::OmlIssue;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use tokio::task;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum DiagScope {
        Task(task::Id),
        Thread(std::thread::ThreadId),
    }

    fn current_scope() -> DiagScope {
        if let Some(task_id) = task::try_id() {
            DiagScope::Task(task_id)
        } else {
            DiagScope::Thread(std::thread::current().id())
        }
    }

    fn diag_store() -> &'static Mutex<HashMap<DiagScope, Vec<OmlIssue>>> {
        static DIAG: OnceLock<Mutex<HashMap<DiagScope, Vec<OmlIssue>>>> = OnceLock::new();
        DIAG.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn reset() {
        let scope = current_scope();
        let mut store = diag_store()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        store.remove(&scope);
    }

    pub fn push(issue: OmlIssue) {
        // 给一个小上限，避免异常风暴
        let scope = current_scope();
        let mut store = diag_store()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let entries = store.entry(scope).or_default();
        if entries.len() < 16 {
            entries.push(issue);
        }
    }

    pub fn take() -> Vec<OmlIssue> {
        let scope = current_scope();
        let mut store = diag_store()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        store.remove(&scope).unwrap_or_default()
    }
}

#[cfg(not(feature = "oml-diag"))]
mod inner {
    use super::OmlIssue;
    pub fn reset() {}
    pub fn push(_issue: OmlIssue) {}
    pub fn take() -> Vec<OmlIssue> {
        Vec::new()
    }
}

pub use inner::{push, reset, take};

/// 取出并压缩成一行字符串，便于落盘到错误 sink
pub fn take_summary() -> Option<String> {
    let items = take();
    if items.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        out.push_str(&it.to_brief());
        if out.len() > 2048 {
            // 避免过长
            out.push_str("; ...");
            break;
        }
    }
    Some(out)
}
