//! Lightweight OML 运行期诊断收集（可选）。
//! - 默认无操作（零开销）；开启 `oml-diag` feature 时启用收集。
//! - 使用线程局部缓冲，避免在 API 上到处传参。

#[derive(Debug, Clone)]
pub enum OmlIssueKind {
    MissingField,       // 读取/格式化变量缺失
    FmtVarMissing,      // fmt 中的变量缺失
    UnsupportedConvert, // 不支持的类型转换
    ParseFail,          // 文本解析为目标类型失败
    BatchNoMatch,       // 批量匹配 0 命中
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
        }
    }
}

// 线程局部缓冲；未启用 feature 时保持空实现
#[cfg(feature = "oml-diag")]
mod inner {
    use super::OmlIssue;
    use std::cell::RefCell;

    thread_local! {
        static DIAG: RefCell<Vec<OmlIssue>> = const { RefCell::new(Vec::new()) };
    }

    pub fn reset() {
        DIAG.with(|b| b.borrow_mut().clear());
    }
    pub fn push(issue: OmlIssue) {
        // 给一个小上限，避免异常风暴
        DIAG.with(|b| {
            let mut v = b.borrow_mut();
            if v.len() < 16 {
                v.push(issue);
            }
        });
    }
    pub fn take() -> Vec<OmlIssue> {
        DIAG.with(|b| std::mem::take(&mut *b.borrow_mut()))
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
