//! Offline, side-effect-free project configuration check.
//!
//! `wparse check` loads a project through the very same loader as the daemon
//! (route confs + WPL/OML + sink indexes) but never starts sinks, sources,
//! locks, or writes anything. It exists so a deployment can fail *before* a
//! working container is replaced with a config that would fail silently at
//! runtime — e.g. the same `oml` matcher bound by two `sink_group`s, which
//! makes the rule index and the model index disagree on ties and disables the
//! OML stage without a single ERROR line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use orion_variate::EnvDict;
use serde::Serialize;

use crate::facade::config::WarpConf;
use crate::resources::ResManager;
use crate::resources::core::manager::{
    BindingConflict, DuplicateGroupName, DuplicateMatcher, LoadedSinkGroup, LoadedSinkRoutes,
    MatcherKind, UnmatchedBinding, find_binding_conflicts, find_duplicate_group_names,
    find_duplicate_matchers, find_unmatched_bindings,
};
use crate::sinks::SinkGroupAgent;
use wp_error::run_error::{IntoRunError, RunError, RunResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSeverity {
    Error,
    Warning,
}

impl CheckSeverity {
    fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// Two route files declare the same `sink_group.name` (same `SinkID`).
    DuplicateSinkGroup,
    /// The same `oml` matcher is bound by two different `sink_group`s.
    DuplicateOmlMatcher,
    /// The same `rule` matcher is bound by two different `sink_group`s.
    DuplicateRuleMatcher,
    /// A rule is routed to one group while its OML model is bound to another,
    /// so that group's `SinkResUnit` has no matching model (silent passthrough).
    OmlBindingConflict,
    /// An `oml` matcher matches no loaded model (the binding is inert).
    UnmatchedOmlMatcher,
    /// A `rule` matcher matches no loaded rule (the binding is inert).
    UnmatchedRuleMatcher,
    /// The project config could not be loaded (TOML syntax, missing file, ...).
    ConfigLoadError,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckIssue {
    pub severity: CheckSeverity,
    pub kind: CheckKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// File the issue is attached to, when it is not a per-group issue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckSummary {
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub ok: bool,
    pub work_root: String,
    pub summary: CheckSummary,
    pub issues: Vec<CheckIssue>,
}

impl CheckReport {
    pub fn new(work_root: String, issues: Vec<CheckIssue>) -> Self {
        let errors = issues.iter().filter(|i| i.severity.is_error()).count();
        let warnings = issues.len() - errors;
        Self {
            ok: errors == 0,
            work_root,
            summary: CheckSummary { errors, warnings },
            issues,
        }
    }

    /// Build a report for a project that could not even be loaded.
    pub fn from_load_error(work_root: impl Into<String>, err: &RunError) -> Self {
        let message = err.display_chain();
        let file = err
            .position()
            .clone()
            .or_else(|| extract_toml_path(&message));
        let issue = CheckIssue {
            severity: CheckSeverity::Error,
            kind: CheckKind::ConfigLoadError,
            message,
            matcher: None,
            rule: None,
            model: None,
            file,
            groups: Vec::new(),
            hint: None,
        };
        Self::new(work_root.into(), vec![issue])
    }

    /// Mark the report as failed when it has any finding at all.
    ///
    /// Used by `--strict`, so the JSON `ok` field stays consistent with the
    /// process exit code.
    pub fn apply_strict(&mut self) {
        if self.has_findings() {
            self.ok = false;
        }
    }

    pub fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }

    pub fn has_findings(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Render the report for humans. Empty when there is nothing to say, so a
    /// clean run stays silent.
    pub fn render_human(&self) -> String {
        if self.issues.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        for issue in &self.issues {
            out.push_str(&format!("{}: {}\n", issue.severity.as_str(), issue.message));
            for group in &issue.groups {
                match group.file.as_deref() {
                    Some(file) => out.push_str(&format!("  - {} ({})\n", group.name, file)),
                    None => out.push_str(&format!("  - {}\n", group.name)),
                }
            }
            if let Some(file) = issue.file.as_deref() {
                out.push_str(&format!("  file: {}\n", file));
            }
            if let Some(hint) = issue.hint.as_deref() {
                out.push_str(&format!("  hint: {}\n", hint));
            }
        }
        out.push_str(&format!(
            "\nconfig check: {} error(s), {} warning(s)\n",
            self.summary.errors, self.summary.warnings
        ));
        out
    }
}

/// Load the project exactly like the daemon and report semantic config errors.
///
/// This performs reads only: no sockets, no lock files, no writes, no
/// `state_dir` creation, and no process CWD change.
pub async fn run_check(work_root: &Path, dict: &EnvDict) -> RunResult<CheckReport> {
    // Match the daemon's startup registration so plugin/connector validation
    // sees the same registry state.
    crate::connectors::startup::init_runtime_registries();
    oml::set_secrets_from_env_dict(dict);

    let conf_manager = WarpConf::new(work_root);
    // Same load chain as the daemon (`env_load_toml -> env_eval ->
    // conf_absolutize`) without switching CWD or resolving a state dir.
    let main_conf = conf_manager
        .load_engine_config(dict)
        .map_err(IntoRunError::into_run_err)?;
    // Match the daemon's robustness policy so WPL load tolerance is identical.
    wp_error::error_handling::switch_sys_robust_mode(main_conf.robust().clone());
    // Canonicalize the base so `strip_prefix` works for route origins (the
    // loader canonicalizes them), e.g. `/tmp` -> `/private/tmp` on macOS.
    let base = std::fs::canonicalize(conf_manager.work_root())
        .unwrap_or_else(|_| conf_manager.work_root().to_path_buf());

    // `ResManager::build` minus `InfraSinkService` / `SinkService`: the same
    // WPL/OML loading and the same `update_sink_*_index` calls the daemon uses.
    let mut res = ResManager::default();
    res.load_all_wpl_code(&main_conf, &SinkGroupAgent::null())
        .await?;
    res.load_all_ldm(main_conf.oml_root()).await?;
    let LoadedSinkRoutes { groups, .. } =
        res.load_all_sink_with_origins(main_conf.sinks_root(), dict, false)?;

    let mut issues = Vec::new();
    for dup in find_duplicate_group_names(&groups) {
        issues.push(duplicate_group_issue(&base, dup));
    }
    for dup in find_duplicate_matchers(&groups) {
        issues.push(duplicate_matcher_issue(&base, dup));
    }
    for conflict in find_binding_conflicts(&res, &groups) {
        issues.push(binding_conflict_issue(&base, &groups, conflict));
    }
    for unmatched in find_unmatched_bindings(&res, &groups) {
        issues.push(unmatched_binding_issue(&base, &groups, unmatched));
    }

    Ok(CheckReport::new(base.display().to_string(), issues))
}

fn origins_by_group(groups: &[LoadedSinkGroup]) -> BTreeMap<&str, &Option<PathBuf>> {
    groups
        .iter()
        .map(|g| (g.name.as_str(), &g.origin))
        .collect()
}

fn duplicate_group_issue(base: &Path, dup: DuplicateGroupName) -> CheckIssue {
    let refs: Vec<GroupRef> = dup
        .origins
        .iter()
        .map(|origin| GroupRef {
            name: dup.name.clone(),
            file: relative_origin(base, origin),
        })
        .collect();
    CheckIssue {
        severity: CheckSeverity::Error,
        kind: CheckKind::DuplicateSinkGroup,
        message: format!(
            "sink_group name '{}' is declared by {} route files",
            dup.name,
            refs.len()
        ),
        matcher: None,
        rule: None,
        model: None,
        file: None,
        groups: refs,
        hint: Some("group names are SinkIDs; duplicates merge routing and OML state".to_string()),
    }
}

fn duplicate_matcher_issue(base: &Path, dup: DuplicateMatcher) -> CheckIssue {
    let (kind, hint) = match dup.kind {
        MatcherKind::Oml => (
            CheckKind::DuplicateOmlMatcher,
            "duplicate bindings let the rule index and the model index disagree on ties, \
             silently disabling the OML stage; keep one group",
        ),
        MatcherKind::Rule => (
            CheckKind::DuplicateRuleMatcher,
            "on a tie the first group keeps the rule and the other never receives it; \
             keep one group",
        ),
    };
    let refs: Vec<GroupRef> = dup
        .groups
        .iter()
        .map(|(name, origin)| GroupRef {
            name: name.clone(),
            file: relative_origin(base, origin),
        })
        .collect();
    let names = refs
        .iter()
        .map(|g| g.name.as_str())
        .collect::<Vec<_>>()
        .join("', '");
    CheckIssue {
        severity: CheckSeverity::Error,
        kind,
        message: format!(
            "{} matcher '{}' is bound by {} sink_groups ('{names}')",
            dup.kind.as_str(),
            dup.matcher,
            refs.len()
        ),
        matcher: Some(dup.matcher),
        rule: None,
        model: None,
        file: None,
        groups: refs,
        hint: Some(hint.to_string()),
    }
}

fn binding_conflict_issue(
    base: &Path,
    groups: &[LoadedSinkGroup],
    conflict: BindingConflict,
) -> CheckIssue {
    let origins = origins_by_group(groups);
    let rule_file = origins
        .get(conflict.rule_group.as_str())
        .and_then(|origin| relative_origin(base, origin));
    let model_file = origins
        .get(conflict.model_group.as_str())
        .and_then(|origin| relative_origin(base, origin));
    CheckIssue {
        severity: CheckSeverity::Error,
        kind: CheckKind::OmlBindingConflict,
        message: format!(
            "rule '{}' is routed to sink_group '{}', but model '{}' is bound to sink_group \
             '{}'; '{}' has no matching OML model and would fall back to raw passthrough",
            conflict.rule,
            conflict.rule_group,
            conflict.model,
            conflict.model_group,
            conflict.rule_group
        ),
        matcher: Some(conflict.matcher),
        rule: Some(conflict.rule),
        model: Some(conflict.model),
        file: None,
        groups: vec![
            GroupRef {
                name: conflict.rule_group,
                file: rule_file,
            },
            GroupRef {
                name: conflict.model_group,
                file: model_file,
            },
        ],
        hint: Some(
            "the rule index and the model index diverge on a tie; remove one of the \
             duplicate bindings"
                .to_string(),
        ),
    }
}

fn unmatched_binding_issue(
    base: &Path,
    groups: &[LoadedSinkGroup],
    unmatched: UnmatchedBinding,
) -> CheckIssue {
    let origins = origins_by_group(groups);
    let file = origins
        .get(unmatched.group.as_str())
        .and_then(|origin| relative_origin(base, origin));
    let (kind, target, hint) = match unmatched.kind {
        MatcherKind::Oml => (
            CheckKind::UnmatchedOmlMatcher,
            "model",
            "check [models].oml and the model name",
        ),
        MatcherKind::Rule => (
            CheckKind::UnmatchedRuleMatcher,
            "rule",
            "check [models].wpl and the rule path",
        ),
    };
    CheckIssue {
        severity: CheckSeverity::Warning,
        kind,
        message: format!(
            "{} matcher '{}' in sink_group '{}' matches no loaded {target}; the binding is inert",
            unmatched.kind.as_str(),
            unmatched.matcher,
            unmatched.group
        ),
        matcher: Some(unmatched.matcher),
        rule: None,
        model: None,
        file: file.clone(),
        groups: vec![GroupRef {
            name: unmatched.group,
            file,
        }],
        hint: Some(hint.to_string()),
    }
}

fn relative_origin(base: &Path, origin: &Option<PathBuf>) -> Option<String> {
    origin.as_ref().map(|path| {
        path.strip_prefix(base)
            .map(|rel| rel.display().to_string())
            .unwrap_or_else(|_| path.display().to_string())
    })
}

/// Best-effort extraction of a `.toml` path from a loader error message.
///
/// `OrionConfError` often carries the file only inside its display chain (for
/// example `from path=...`), while `position()` stays empty.
fn extract_toml_path(message: &str) -> Option<String> {
    for token in message.split(|c: char| c.is_whitespace() || c == ',' || c == ']' || c == '}') {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '(' | ')'));
        let candidate = token.rsplit('=').next().unwrap_or(token);
        if candidate.ends_with(".toml") && candidate.contains('/') {
            return Some(candidate.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use wp_conf::test_support::ForTest;

    const WPL: &str = r#"package /nginx/ {
   rule example {
        (ip:sip,2*_,time/clf:recv_time<[,]>,http/request",http/status,digit,chars",http/agent",_")
   }
}
"#;

    const OML: &str = r#"name : nginx
rule : /nginx/*
---
* : auto = take() ;
"#;

    const CONNECTOR: &str = r#"[[connectors]]
allow_override = ["base", "file", "sync"]
id = "file_json_sink"
type = "file"

[connectors.params]
base = "./data/out_dat"
file = "default.json"
fmt = "json"
sync = false
"#;

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("tempdir");
            let path = root.path();
            write(path, "conf/wparse.toml", "version = \"1.0\"\n");
            write(path, "models/wpl/parse.wpl", WPL);
            write(path, "models/oml/example.oml", OML);
            write(path, "connectors/sink.d/00-file_json_sink.toml", CONNECTOR);
            Self { root }
        }

        fn route(&self, rel: &str, group: &str, oml: &str) -> &Self {
            self.write_route(rel, group, &format!("oml = {oml}"))
        }

        fn route_rule(&self, rel: &str, group: &str, rule: &str) -> &Self {
            self.write_route(rel, group, &format!("rule = {rule}"))
        }

        fn write_route(&self, rel: &str, group: &str, matcher_line: &str) -> &Self {
            let body = format!(
                r#"version = "2.0"

[sink_group]
name = "{group}"
{matcher_line}

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"

[sink_group.sinks.params]
file = "{group}.jsonl"
"#
            );
            write(
                self.root.path(),
                &format!("topology/sinks/business.d/{rel}"),
                &body,
            );
            self
        }

        fn path(&self) -> PathBuf {
            self.root.path().to_path_buf()
        }

        async fn check(&self) -> CheckReport {
            let dict = EnvDict::test_default();
            run_check(&self.path(), &dict).await.expect("check run")
        }
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(path, body).expect("write fixture");
    }

    fn kinds(report: &CheckReport) -> Vec<CheckKind> {
        report.issues.iter().map(|i| i.kind).collect()
    }

    #[tokio::test]
    async fn duplicate_concrete_oml_matcher_is_an_error() {
        let fixture = Fixture::new();
        fixture.route("a.toml", "a-metrics", r#"["nginx"]"#).route(
            "b.toml",
            "b-metrics",
            r#"["nginx"]"#,
        );

        let report = fixture.check().await;
        assert!(report.has_errors(), "expected errors: {:?}", report.issues);
        assert!(kinds(&report).contains(&CheckKind::DuplicateOmlMatcher));

        let issue = report
            .issues
            .iter()
            .find(|i| i.kind == CheckKind::DuplicateOmlMatcher)
            .expect("duplicate issue");
        assert_eq!(issue.matcher.as_deref(), Some("nginx"));
        let names: Vec<&str> = issue.groups.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"a-metrics") && names.contains(&"b-metrics"));
        for group in &issue.groups {
            let file = group.file.as_deref().expect("file");
            assert!(file.starts_with("topology/sinks/business.d/"), "{file}");
        }
    }

    #[tokio::test]
    async fn duplicate_star_oml_matcher_is_an_error() {
        let fixture = Fixture::new();
        fixture
            .route("a.toml", "a", r#"["*"]"#)
            .route("b.toml", "b", r#"["*"]"#);

        let report = fixture.check().await;
        assert!(report.has_errors());
        assert!(kinds(&report).contains(&CheckKind::DuplicateOmlMatcher));
    }

    #[tokio::test]
    async fn star_plus_concrete_is_not_a_violation() {
        let fixture = Fixture::new();
        fixture
            .route("a.toml", "a", r#"["*"]"#)
            .route("b.toml", "b", r#"["nginx"]"#);

        let report = fixture.check().await;
        assert!(
            !report.has_errors(),
            "unexpected errors: {:?}",
            report.issues
        );
        assert!(
            !kinds(&report).contains(&CheckKind::OmlBindingConflict),
            "star+concrete must not be reported as a conflict"
        );
    }

    #[tokio::test]
    async fn single_group_distinct_matchers_is_clean() {
        let fixture = Fixture::new();
        fixture.route("a.toml", "a", r#"["nginx"]"#);

        let report = fixture.check().await;
        assert!(report.issues.is_empty(), "unexpected: {:?}", report.issues);
        assert!(report.render_human().is_empty());
    }

    #[tokio::test]
    async fn inert_oml_matcher_is_a_warning() {
        let fixture = Fixture::new();
        fixture.route("a.toml", "a", r#"["does_not_exist"]"#);

        let report = fixture.check().await;
        assert!(!report.has_errors(), "{:?}", report.issues);
        assert!(kinds(&report).contains(&CheckKind::UnmatchedOmlMatcher));
    }

    #[tokio::test]
    async fn duplicate_rule_matcher_is_an_error() {
        let fixture = Fixture::new();
        fixture
            .route_rule("a.toml", "a", r#"["/nginx/*"]"#)
            .route_rule("b.toml", "b", r#"["/nginx/*"]"#);

        let report = fixture.check().await;
        assert!(report.has_errors(), "{:?}", report.issues);
        let issue = report
            .issues
            .iter()
            .find(|i| i.kind == CheckKind::DuplicateRuleMatcher)
            .expect("duplicate rule issue");
        assert_eq!(issue.matcher.as_deref(), Some("/nginx/*"));
        assert_eq!(issue.severity, CheckSeverity::Error);
    }

    #[tokio::test]
    async fn overlapping_equal_length_matchers_report_binding_conflict() {
        let fixture = Fixture::new();
        // Different strings, same normalized length, both match model `nginx`.
        fixture
            .route("a.toml", "a", r#"["ngin?"]"#)
            .route("b.toml", "b", r#"["n?inx"]"#);

        let report = fixture.check().await;
        assert!(
            !kinds(&report).contains(&CheckKind::DuplicateOmlMatcher),
            "distinct matcher strings are not duplicates: {:?}",
            report.issues
        );
        assert!(
            kinds(&report).contains(&CheckKind::OmlBindingConflict),
            "overlapping matchers must be caught post-index: {:?}",
            report.issues
        );
    }

    #[tokio::test]
    async fn duplicate_group_name_is_an_error() {
        let fixture = Fixture::new();
        fixture
            .route("a.toml", "dup", r#"["nginx"]"#)
            .route("b.toml", "dup", r#"["other"]"#);

        let report = fixture.check().await;
        let issue = report
            .issues
            .iter()
            .find(|i| i.kind == CheckKind::DuplicateSinkGroup)
            .expect("duplicate group issue");
        assert_eq!(issue.severity, CheckSeverity::Error);
        assert_eq!(issue.groups.len(), 2);
        assert!(issue.groups.iter().all(|g| g.name == "dup"));
    }

    #[test]
    fn strict_marks_report_not_ok() {
        let warning = CheckIssue {
            severity: CheckSeverity::Warning,
            kind: CheckKind::UnmatchedOmlMatcher,
            message: String::new(),
            matcher: None,
            rule: None,
            model: None,
            file: None,
            groups: Vec::new(),
            hint: None,
        };
        let mut report = CheckReport::new("/w".to_string(), vec![warning]);
        assert!(report.ok);
        report.apply_strict();
        assert!(!report.ok);

        let mut clean = CheckReport::new("/w".to_string(), Vec::new());
        clean.apply_strict();
        assert!(clean.ok);
    }

    #[test]
    fn extract_toml_path_reads_display_chain_tokens() {
        assert_eq!(
            extract_toml_path(
                "load route confs {from path=/tmp/w/topology/sinks/business.d/broken.toml, path=/tmp/x}"
            )
            .as_deref(),
            Some("/tmp/w/topology/sinks/business.d/broken.toml")
        );
        assert_eq!(extract_toml_path("no path here"), None);
    }

    #[tokio::test]
    async fn broken_route_toml_reports_load_error() {
        let fixture = Fixture::new();
        write(
            fixture.root.path(),
            "topology/sinks/business.d/broken.toml",
            "[sink_group\nname = \"x\"\n",
        );

        let dict = EnvDict::test_default();
        let err = run_check(&fixture.path(), &dict)
            .await
            .expect_err("broken toml must fail");
        let report = CheckReport::from_load_error(fixture.path().display().to_string(), &err);
        assert!(report.has_errors());
        assert_eq!(report.issues[0].kind, CheckKind::ConfigLoadError);
        assert!(
            report.issues[0].message.contains("broken.toml")
                || report.issues[0]
                    .file
                    .as_deref()
                    .is_some_and(|f| f.contains("broken.toml")),
            "load error should name the file: {:?}",
            report.issues[0]
        );
    }

    #[test]
    fn report_counts_and_flags() {
        let issue = |severity| CheckIssue {
            severity,
            kind: CheckKind::DuplicateOmlMatcher,
            message: String::new(),
            matcher: None,
            rule: None,
            model: None,
            file: None,
            groups: Vec::new(),
            hint: None,
        };
        let report = CheckReport::new(
            "/w".to_string(),
            vec![issue(CheckSeverity::Error), issue(CheckSeverity::Warning)],
        );
        assert!(report.has_errors());
        assert!(report.has_findings());
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.summary.warnings, 1);
        assert!(!report.ok);
    }
}
