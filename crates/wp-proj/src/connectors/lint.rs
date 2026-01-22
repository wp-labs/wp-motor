use orion_variate::EnvDict;
use std::path::{Path, PathBuf};
use wp_conf::connectors::{ConnectorDef, ConnectorScope, load_connector_defs_from_dir};
use wp_conf::sources::io::resolve_connectors_base_dir;

use super::types::{LintRow, LintSeverity, Side, SilentErrKind};
fn kind_hint_from_filename_path(p: &Path) -> Option<String> {
    let stem = p.file_stem()?.to_str()?;
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 3 {
        return Some(parts[1].to_string());
    }
    if parts.len() >= 2 {
        let raw = parts[1];
        let mut iter = raw.split('_');
        if let Some(first) = iter.next() {
            if !first.is_empty() {
                return Some(first.to_string());
            }
        }
    }
    None
}
fn hint_kind_compatible(hint: &str, kind: &str) -> bool {
    if hint.eq_ignore_ascii_case(kind) {
        return true;
    }
    if hint.eq_ignore_ascii_case("file")
        && (kind.eq_ignore_ascii_case("file") || kind.eq_ignore_ascii_case("test_rescue"))
    {
        return true;
    }
    false
}
fn ok_id_chars_fn(id: &str) -> bool {
    id.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
fn sev_max(a: LintSeverity, b: LintSeverity) -> LintSeverity {
    if a >= b { a } else { b }
}
fn resolve_dir(side: Side, start: &Path) -> PathBuf {
    match side {
        Side::Sources => resolve_connectors_base_dir(start)
            .unwrap_or_else(|| start.join("connectors").join("source.d")),
        Side::Sinks => wp_conf::sinks::io::find_connectors_base_dir(start)
            .unwrap_or_else(|| start.join("connectors").join("sink.d")),
    }
}
fn validate_connector(
    side: Side,
    def: &ConnectorDef,
    hint: Option<&str>,
) -> (LintSeverity, String, Option<SilentErrKind>) {
    let mut sev = LintSeverity::Ok;
    let mut msg = String::new();
    let mut first_err: Option<SilentErrKind> = None;

    let kind = def.kind.to_ascii_lowercase();
    let id = def.id.as_str();

    if !ok_id_chars_fn(id) {
        sev = sev_max(sev, LintSeverity::Error);
        msg.push_str("id contains non [a-z0-9_] chars; ");
        if first_err.is_none() {
            first_err = Some(SilentErrKind::BadIdChars);
        }
    }

    match side {
        Side::Sources => {
            if !id.ends_with("_src") {
                sev = sev_max(sev, LintSeverity::Error);
                msg.push_str("sources id must end with '_src'; ");
                if first_err.is_none() {
                    first_err = Some(SilentErrKind::SourcesIdMustEndSrc);
                }
            }
            if kind == "file" && !id.starts_with("file_") {
                sev = sev_max(sev, LintSeverity::Warn);
                msg.push_str("file sources id should start with 'file_'; ");
            }
            if let Some(h) = hint {
                if !hint_kind_compatible(h, &kind) {
                    sev = sev_max(sev, LintSeverity::Warn);
                    msg.push_str(&format!("filename kind hint '{}' != type '{}'", h, kind));
                }
            }
        }
        Side::Sinks => {
            if kind == "file" {
                if !id.starts_with("file_") {
                    sev = sev_max(sev, LintSeverity::Warn);
                    msg.push_str("file sinks id should start with 'file_'; ");
                }
            } else if !id.ends_with("_sink") {
                sev = sev_max(sev, LintSeverity::Error);
                msg.push_str("sinks id (non-file) must end with '_sink'; ");
                if first_err.is_none() {
                    first_err = Some(SilentErrKind::SinksIdMustEndSink);
                }
            }
            if let Some(h) = hint {
                if h != kind {
                    sev = sev_max(sev, LintSeverity::Warn);
                    msg.push_str(&format!("filename kind hint '{}' != type '{}'", h, kind));
                }
            }
        }
    }

    (sev, msg, first_err)
}

fn lint_side_rows_from(start: &Path, side: Side, dict: &EnvDict) -> Vec<LintRow> {
    let dir = resolve_dir(side, start);
    let mut rows: Vec<LintRow> = Vec::new();
    let scope = match side {
        Side::Sources => ConnectorScope::Source,
        Side::Sinks => ConnectorScope::Sink,
    };
    match load_connector_defs_from_dir(&dir, scope, dict) {
        Ok(defs) => {
            for def in defs {
                let hint = def
                    .origin
                    .as_ref()
                    .and_then(|p| kind_hint_from_filename_path(Path::new(p)));
                let (sev, msg, silent_err) = validate_connector(side, &def, hint.as_deref());
                rows.push(LintRow {
                    scope: side.label(),
                    file: def
                        .origin
                        .clone()
                        .unwrap_or_else(|| dir.display().to_string()),
                    id: def.id.clone(),
                    kind: def.kind.to_ascii_lowercase(),
                    sev,
                    msg: if msg.is_empty() { "-".into() } else { msg },
                    silent_err,
                });
            }
        }
        Err(err) => rows.push(LintRow {
            scope: side.label(),
            file: dir.display().to_string(),
            id: "-".into(),
            kind: "-".into(),
            sev: LintSeverity::Error,
            msg: format!("load failed: {}", err),
            silent_err: None,
        }),
    }
    rows
}

pub fn lint_rows_from_root<P: AsRef<Path>>(work_root: P, dict: &EnvDict) -> Vec<LintRow> {
    let start = work_root.as_ref();
    let mut rows = Vec::new();
    rows.extend(lint_side_rows_from(start, Side::Sources, dict));
    rows.extend(lint_side_rows_from(start, Side::Sinks, dict));
    rows
}

#[cfg(test)]
mod tests {
    use orion_variate::EnvDict;
    use wp_conf::test_support::ForTest;
    use wp_connector_api::ParamMap;

    use super::*;
    use crate::test_utils::{temp_workdir, write_file};

    #[test]
    fn filename_hint_extracts_kind_segment() {
        let path = Path::new("source.d/00-file_src.toml");
        assert_eq!(kind_hint_from_filename_path(path), Some("file".into()));
    }

    #[test]
    fn validate_connector_enforces_source_suffix() {
        let def = ConnectorDef {
            id: "bad".into(),
            kind: "file".into(),
            scope: ConnectorScope::Source,
            allow_override: vec![],
            default_params: ParamMap::new(),
            origin: None,
        };
        let (sev, msg, silent) = validate_connector(Side::Sources, &def, Some("file"));
        assert_eq!(sev, LintSeverity::Error);
        assert!(msg.contains("_src"));
        assert!(matches!(silent, Some(SilentErrKind::SourcesIdMustEndSrc)));
    }

    #[test]
    fn lint_rows_reports_problems_for_both_sides() {
        let temp = temp_workdir();
        let root = temp.path();
        write_file(
            root,
            "connectors/source.d/10-http-file.toml",
            "[[connectors]]\nid = \"file_demo\"\ntype = \"file\"\n",
        );
        write_file(
            root,
            "connectors/sink.d/10-http-sink.toml",
            "[[connectors]]\nid = \"http\"\ntype = \"http\"\n",
        );

        let rows = lint_rows_from_root(root.to_str().unwrap(), &EnvDict::test_default());
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .any(|r| r.scope == "sources" && r.sev == LintSeverity::Error)
        );
        assert!(rows.iter().any(|r| r.scope == "sinks"));
    }
}
