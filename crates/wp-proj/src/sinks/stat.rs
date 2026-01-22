use std::path::{Path, PathBuf};

use orion_conf::{ToStructError, UvsConfFrom};
use orion_variate::EnvDict;
use wp_cli_core as wlib;
use wp_engine::facade::config;
use wp_error::run_error::{RunReason, RunResult};

pub struct SinkStatFilters<'a> {
    pub work_root: &'a str,
    pub group_filters: &'a [String],
    pub sink_filters: &'a [String],
    pub path_like: &'a Option<String>,
    pub total_only: bool,
}

impl<'a> SinkStatFilters<'a> {
    pub fn new(
        work_root: &'a str,
        group_filters: &'a [String],
        sink_filters: &'a [String],
        path_like: &'a Option<String>,
    ) -> Self {
        Self {
            work_root,
            group_filters,
            sink_filters,
            path_like,
            total_only: false,
        }
    }
}

pub struct SinkStatResult {
    pub rows: Vec<wlib::Row>,
    pub total: u64,
}

pub struct CombinedStatResult {
    pub src: Option<wp_cli_core::SrcLineReport>,
    pub sink: SinkStatResult,
}

pub fn stat_sink_files(filters: &SinkStatFilters<'_>, dict: &EnvDict) -> RunResult<SinkStatResult> {
    let (cm, main) = config::load_warp_engine_confs(filters.work_root, dict)?;
    let ctx = build_ctx(&cm.work_root_path(), filters);
    let sink_root = Path::new(&cm.work_root_path()).join(main.sink_root());
    ensure_sink_dirs(&sink_root, main.sink_root())?;
    let (rows, total) = wp_cli_core::collect_sink_statistics(&sink_root, &ctx, dict)
        .map_err(|e| RunReason::from_conf(e.to_string()).to_err())?;
    Ok(SinkStatResult { rows, total })
}

pub fn stat_file_combined(
    filters: &SinkStatFilters<'_>,
    dict: &EnvDict,
) -> RunResult<CombinedStatResult> {
    let (cm, main) = config::load_warp_engine_confs(filters.work_root, dict)?;
    let ctx = build_ctx(&cm.work_root_path(), filters);

    // Collect source statistics
    let src = wp_cli_core::list_file_sources_with_lines(
        Path::new(&cm.work_root_path()),
        &main,
        &ctx,
        dict,
    );

    // Collect sink statistics
    let sink_root = Path::new(&cm.work_root_path()).join(main.sink_root());
    let (rows, total) = wp_cli_core::collect_sink_statistics(&sink_root, &ctx, dict)
        .map_err(|e| RunReason::from_conf(e.to_string()).to_err())?;

    Ok(CombinedStatResult {
        src,
        sink: SinkStatResult { rows, total },
    })
}

pub(crate) fn build_ctx(root: &str, filters: &SinkStatFilters<'_>) -> wlib::Ctx {
    wlib::Ctx {
        work_root: PathBuf::from(root),
        group_filters: filters.group_filters.to_vec(),
        sink_filters: filters.sink_filters.to_vec(),
        path_like: filters.path_like.clone(),
        total_only: filters.total_only,
    }
}

pub(crate) fn ensure_sink_dirs(sink_root: &Path, sink_root_conf: &str) -> RunResult<()> {
    if has_v2_dirs(sink_root) {
        Ok(())
    } else {
        Err(RunReason::from_conf(format!(
            "缺少 sinks 配置目录：在 '{}' 下未发现 business.d/ 或 infra.d/",
            sink_root_conf
        ))
        .to_err())
    }
}

pub(crate) fn has_v2_dirs(p: &Path) -> bool {
    p.join("business.d").exists() || p.join("infra.d").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{ensure_dir, temp_workdir};

    #[test]
    fn build_ctx_copies_filters() {
        let filters = SinkStatFilters {
            work_root: "/tmp",
            group_filters: &["a".into()],
            sink_filters: &["b".into()],
            path_like: &Some("demo".into()),
            total_only: true,
        };
        let ctx = build_ctx("/tmp", &filters);
        assert_eq!(ctx.group_filters, vec!["a"]);
        assert_eq!(ctx.sink_filters, vec!["b"]);
        assert_eq!(ctx.path_like, Some("demo".into()));
        assert!(ctx.total_only);
    }

    #[test]
    fn has_v2_dirs_detects_business_and_infra() {
        let temp = temp_workdir();
        let root = ensure_dir(temp.path(), "connectors");
        assert!(!has_v2_dirs(&root));
        ensure_dir(&root, "business.d");
        assert!(has_v2_dirs(&root));
    }

    #[test]
    fn ensure_sink_dirs_errors_when_missing() {
        let temp = temp_workdir();
        let root = ensure_dir(temp.path(), "models/sinks");
        let err = ensure_sink_dirs(&root, "models/sinks").unwrap_err();
        assert!(err.reason().to_string().contains("缺少"));
    }
}
