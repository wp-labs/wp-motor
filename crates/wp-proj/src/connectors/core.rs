use std::path::Path;

use crate::connectors::{
    lint::lint_rows_from_root,
    templates::init_definitions,
    types::{LintRow, LintSeverity, SilentErrKind},
};
use crate::traits::Component;
use crate::types::CheckStatus;
use orion_error::{ToStructError, UvsConfFrom};
use orion_variate::EnvDict;

use super::paths::ConnectorsPaths;
use wp_error::run_error::{RunReason, RunResult};

#[derive(Clone)]
#[allow(dead_code)] // paths field is used in tests but not detected
pub struct Connectors {
    pub paths: ConnectorsPaths,
}

impl Connectors {
    pub fn new(paths: ConnectorsPaths) -> Self {
        Self { paths }
    }

    pub fn lint_rows_from_root<P: AsRef<Path>>(
        &self,
        work_root: P,
        dict: &EnvDict,
    ) -> Vec<LintRow> {
        lint_rows_from_root(work_root, dict)
    }

    pub fn init_definition<P: AsRef<Path>>(&self, work_root: P) -> RunResult<()> {
        // 仅生成 connectors/ 内的模板目录，避免在工作根制造 legacy source.d/sink.d
        init_definitions(work_root)
    }

    /// 检查连接器配置是否有效
    ///
    /// # 参数
    ///
    /// - `work_root`: 项目工作目录
    ///
    /// # 返回
    ///
    /// - `Ok(CheckStatus::Suc)` - 验证通过
    /// - `Err(RunError)` - 验证失败，包含错误详情
    ///
    /// # 注意
    ///
    /// Connectors 需要外部传入 work_root 和 dict 参数，因为它不持有完整的组件上下文。
    /// 这是有意的设计，反映了 Connectors 作为横切关注点的特殊性。
    ///
    /// 虽然此方法签名与 Checkable trait 不同，但返回类型已统一为 RunResult<CheckStatus>。
    pub fn check<P: AsRef<Path>>(&self, work_root: P, dict: &EnvDict) -> RunResult<CheckStatus> {
        let errors = self.collect_lint_errors(work_root.as_ref(), dict);

        if errors.is_empty() {
            println!("✓ Connectors validation passed");
            Ok(CheckStatus::Suc)
        } else {
            Err(RunReason::from_conf(format!(
                "Connectors lint failed: {} error(s)\n{}",
                errors.len(),
                errors.join("\n")
            ))
            .to_err())
        }
    }

    /// 收集所有 lint 错误
    fn collect_lint_errors(&self, work_root: &Path, dict: &EnvDict) -> Vec<String> {
        let mut errors = Vec::new();
        for row in self.lint_rows_from_root(work_root, dict) {
            if matches!(row.sev, LintSeverity::Error) {
                errors.push(format_lint_error(&row));
            }
        }
        errors
    }
}

fn format_lint_error(row: &LintRow) -> String {
    match row.silent_err {
        Some(SilentErrKind::BadIdChars) => {
            format!("{}: bad id chars: {} in {}", row.scope, row.id, row.file)
        }
        Some(SilentErrKind::SourcesIdMustEndSrc) => format!(
            "{}: id must end with _src: {} in {}",
            row.scope, row.id, row.file
        ),
        Some(SilentErrKind::SinksIdMustEndSink) => format!(
            "{}: id must end with _sink: {} in {}",
            row.scope, row.id, row.file
        ),
        None => format!(
            "{}: parse failed for {}: {}",
            row.scope,
            row.file,
            row.msg.replace("parse failed: ", ""),
        ),
    }
}

// Trait implementations for unified component interface
impl Component for Connectors {
    fn component_name(&self) -> &'static str {
        "Connectors"
    }
}

// Note: Connectors does not implement Checkable trait because its check() method
// requires a work_root parameter, which differs from the trait signature.
//
// However, the return type has been unified to RunResult<CheckStatus> for consistency.
// This is intentional design that reflects Connectors' special nature as a cross-cutting concern.
