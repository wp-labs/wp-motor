//! 运行时错误的美化与提示收集，供各 CLI 共享使用。

use orion_error::{ErrorCode, ErrorReport, SourceFrame};
use wp_error::diagnostic_meta::{
    ConfigKind, HintCode, first_meta_enum, first_meta_hint, first_meta_str, key,
};
use wp_error::run_error::{RunError, RunReason};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticTriplet {
    reason: String,
    detail: Option<String>,
    location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticSummary {
    triplet: DiagnosticTriplet,
    parse_excerpt: Option<String>,
    root_cause: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnosticKind {
    Defaults,
    Route,
    Wpsrc,
}

fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
}
fn colorize(s: &str, code: &str) -> String {
    if no_color() {
        s.to_string()
    } else {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    }
}
fn red<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "31")
}
fn yellow<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "33")
}
fn bold<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "1")
}
fn bg_red<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "41;97")
}

/// 从长串嵌套错误中提取主原因、详情和位置信息。
fn derive_error_triplet(raw: &str) -> DiagnosticTriplet {
    let reason = if let Some(idx) = raw.find("StructError") {
        raw[..idx].trim_end().to_string()
    } else {
        raw.lines()
            .find(|line| !line.trim().is_empty())
            .map(str::trim)
            .unwrap_or(raw)
            .to_string()
    };
    let mut detail = raw
        .find("Details:")
        .and_then(|pos| raw[pos + "Details:".len()..].lines().next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if detail.is_none() {
        detail = raw
            .lines()
            .find_map(|line| line.trim().strip_prefix("detail:"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }
    if detail.is_none() {
        if let Some(pos) = raw.find("Core(\"") {
            let tail = &raw[pos + 6..];
            if let Some(end) = tail.find("\")") {
                let msg = &tail[..end];
                if !msg.is_empty() {
                    detail = Some(msg.to_string());
                }
            }
        } else if let Some(pos) = raw.find("ConfigError(\"") {
            let tail = &raw[pos + "ConfigError(\"".len()..];
            if let Some(end) = tail.find("\")") {
                let msg = &tail[..end];
                if !msg.is_empty() {
                    detail = Some(msg.to_string());
                }
            }
        } else if let Some(pos) = raw.find("detail: Some(\"") {
            let tail = &raw[pos + 14..];
            if let Some(end) = tail.find("\")") {
                let msg = &tail[..end];
                if !msg.is_empty() {
                    detail = Some(msg.to_string());
                }
            }
        }
    }

    DiagnosticTriplet {
        reason,
        detail: detail.map(|d| sanitize_detail(&d)),
        location: extract_location(raw),
    }
}

fn extract_location(raw: &str) -> Option<String> {
    let location = raw.lines().map(str::trim).find_map(|line| {
        if let Some(v) = line.strip_prefix("from path : ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("from path: ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("1. from path: ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("1. from path : ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("path : ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("path: ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("file : ") {
            return Some(v.to_string());
        }
        if let Some(v) = line.strip_prefix("file: ") {
            return Some(v.to_string());
        }
        if let Some(idx) = line.find("from path: ") {
            return Some(line[idx + "from path: ".len()..].trim().to_string());
        }
        if let Some(idx) = line.find("from path : ") {
            return Some(line[idx + "from path : ".len()..].trim().to_string());
        }
        if let Some(idx) = line.find("file: ") {
            let value = &line[idx + "file: ".len()..];
            return Some(
                value
                    .trim()
                    .trim_end_matches(')')
                    .trim_end_matches(',')
                    .trim()
                    .to_string(),
            );
        }
        if let Some(idx) = line.find("file : ") {
            let value = &line[idx + "file : ".len()..];
            return Some(
                value
                    .trim()
                    .trim_end_matches(')')
                    .trim_end_matches(',')
                    .trim()
                    .to_string(),
            );
        }
        None
    });

    location.or_else(|| {
        raw.lines()
            .map(str::trim)
            .find(|line| line.starts_with("(group:"))
            .map(|line| line.to_string())
    })
}

fn looks_like_file_location(location: &str) -> bool {
    let trimmed = location.trim();
    trimmed.starts_with('/')
        || trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.ends_with(".toml")
        || trimmed.ends_with(".yaml")
        || trimmed.ends_with(".yml")
        || trimmed.ends_with(".json")
        || trimmed.contains(":\\")
}

fn extract_toml_parse_excerpt(raw: &str) -> Option<String> {
    let anchor = raw.find("TOML parse error at line ")?;
    Some(truncate_diagnostic_block(&raw[anchor..]).to_string())
}

fn enrich_triplet_from_fallback(
    primary: DiagnosticTriplet,
    fallback_raw: &str,
) -> (DiagnosticTriplet, Option<String>) {
    let fallback = derive_error_triplet(fallback_raw);
    let use_primary_detail = has_effective_detail(&primary.reason, primary.detail.as_deref());
    let parse_excerpt = extract_toml_parse_excerpt(fallback_raw);
    let triplet = DiagnosticTriplet {
        reason: primary.reason,
        detail: if use_primary_detail {
            primary.detail
        } else {
            fallback.detail.or(primary.detail)
        },
        location: primary.location.or(fallback.location),
    };
    (triplet, parse_excerpt)
}

fn pretty_reason(raw: &str) -> String {
    raw.replace(
        "[50041] configuration error << core config > ",
        "配置错误: ",
    )
    .replace(
        "[50041] configuration error << core config - ",
        "配置错误: ",
    )
    .replace("[50041] configuration error << core config", "配置错误")
    .replace("configuration error << core config > ", "配置错误: ")
    .replace("configuration error << core config - ", "配置错误: ")
    .replace("configuration error << core config", "配置错误")
    .replace("[100] validation error << ", "校验失败: ")
    .replace("[100] validation error", "校验失败")
    .replace("validation error << ", "校验失败: ")
    .replace("validation error", "校验失败")
    .replace("syntax err:", "")
    .replace("sink validate error: ", "")
    .trim()
    .to_string()
}

fn sanitize_detail(raw: &str) -> String {
    if let Some(nested) = raw.lines().rev().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("-> Details:")
            .or_else(|| line.strip_prefix("Details:"))
            .or_else(|| line.strip_prefix("detail:"))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
    }) {
        return sanitize_detail(&nested);
    }
    let trimmed = raw.trim();
    let cut_markers = [
        "\"), position:",
        "\") , position:",
        "), position:",
        ", position:",
        ", context:",
        " }], source:",
        "\n  -> Want:",
        "\n-> Want:",
        "\n  -> Path:",
        "\n-> Path:",
        "\n  -> Context stack:",
        "\n-> Context stack:",
        "\n  -> Source:",
        "\n-> Source:",
        "\nCaused by:",
    ];
    let mut end = trimmed.len();
    for marker in cut_markers {
        if let Some(idx) = trimmed.find(marker) {
            end = end.min(idx);
        }
    }
    trimmed[..end].trim().trim_matches('"').to_string()
}

fn truncate_diagnostic_block(raw: &str) -> &str {
    let trimmed = raw.trim();
    let cut_markers = [
        "\n  -> Want:",
        "\n-> Want:",
        "\n  -> Path:",
        "\n-> Path:",
        "\n  -> Context stack:",
        "\n-> Context stack:",
        "\n  -> Source:",
        "\n-> Source:",
        "\nCaused by:",
    ];
    let mut end = trimmed.len();
    for marker in cut_markers {
        if let Some(idx) = trimmed.find(marker) {
            end = end.min(idx);
        }
    }
    trimmed[..end].trim()
}

fn normalize_message(raw: &str) -> Option<String> {
    let normalized = sanitize_detail(raw);
    (!normalized.trim().is_empty()).then_some(normalized)
}

fn is_generic_detail(detail: &str) -> bool {
    let d = detail.trim();
    d.is_empty() || d == "[100] validation error" || d == "validation error" || d == "校验失败"
}

fn has_effective_detail(reason: &str, detail: Option<&str>) -> bool {
    let Some(detail) = detail else {
        return false;
    };
    let pretty_detail = pretty_reason(detail);
    let pretty_reason_msg = pretty_reason(reason);
    !is_generic_detail(&pretty_detail) && pretty_detail.trim() != pretty_reason_msg.trim()
}

fn frame_location(frame: &SourceFrame) -> Option<String> {
    let embedded_location = frame
        .display
        .as_deref()
        .and_then(extract_location)
        .or_else(|| frame.detail.as_deref().and_then(extract_location))
        .or_else(|| extract_location(&frame.message));
    match (frame.path.as_deref(), embedded_location) {
        (Some(path), Some(location))
            if !looks_like_file_location(path) || looks_like_file_location(&location) =>
        {
            Some(location)
        }
        (Some(path), _) => Some(path.to_string()),
        (None, location) => location,
    }
}

fn frame_parse_excerpt(frame: &SourceFrame) -> Option<String> {
    frame
        .display
        .as_deref()
        .and_then(extract_toml_parse_excerpt)
        .or_else(|| frame.detail.as_deref().and_then(extract_toml_parse_excerpt))
        .or_else(|| extract_toml_parse_excerpt(&frame.message))
}

fn frame_detail_candidate(frame: &SourceFrame, reason: &str) -> Option<String> {
    let detail = frame
        .detail
        .as_deref()
        .and_then(normalize_message)
        .filter(|detail| has_effective_detail(reason, Some(detail.as_str())));
    if detail.is_some() {
        return detail;
    }

    normalize_message(&frame.message).filter(|detail| has_effective_detail(reason, Some(detail)))
}

fn root_cause_candidate(
    frame: &SourceFrame,
    reason: &str,
    detail: Option<&str>,
    parse_excerpt: Option<&str>,
) -> Option<String> {
    let candidate = normalize_message(&frame.message)
        .or_else(|| frame.detail.as_deref().and_then(normalize_message))?;
    if !has_effective_detail(reason, Some(candidate.as_str())) {
        return None;
    }
    if detail.is_some_and(|existing| sanitize_detail(existing) == candidate) {
        return None;
    }
    if parse_excerpt.is_some_and(|excerpt| excerpt.contains(candidate.as_str())) {
        return None;
    }
    Some(candidate)
}

fn first_metadata_str<'a>(report: &'a ErrorReport, key: &str) -> Option<&'a str> {
    first_meta_str(report, key)
}

fn first_hint_code_raw(report: &ErrorReport) -> Option<&str> {
    first_metadata_str(report, key::HINT_CODE)
}

fn first_metadata_path(report: &ErrorReport) -> Option<&str> {
    first_metadata_str(report, key::FILE_PATH).or_else(|| {
        report
            .source_frames
            .iter()
            .find_map(|frame| frame.path.as_deref())
    })
}

fn classify_toml_parse_kind(
    report: &ErrorReport,
    summary: &DiagnosticSummary,
    display_chain: &str,
) -> Option<DiagnosticKind> {
    summary.parse_excerpt.as_ref()?;

    match first_meta_enum::<ConfigKind>(report) {
        Some(ConfigKind::SinkDefaults) => return Some(DiagnosticKind::Defaults),
        Some(ConfigKind::SinkRoute) => return Some(DiagnosticKind::Route),
        Some(ConfigKind::Wpsrc) => return Some(DiagnosticKind::Wpsrc),
        _ => {}
    }

    let location = first_metadata_path(report)
        .or(summary.triplet.location.as_deref())
        .unwrap_or_default();
    let lower_location = location.to_lowercase();
    let lower_display = display_chain.to_lowercase();

    if lower_location.ends_with("/defaults.toml")
        || lower_display.contains("load sink defaults")
        || lower_display.contains("expected `defaults`")
    {
        return Some(DiagnosticKind::Defaults);
    }

    if lower_location.ends_with("/wpsrc.toml")
        || lower_display.contains("load wpsrc")
        || lower_display.contains("parse wpsrc")
        || lower_display.contains("source key")
    {
        return Some(DiagnosticKind::Wpsrc);
    }

    if lower_location.contains("/business.d/")
        || lower_location.contains("/infra.d/")
        || first_metadata_str(report, key::CONFIG_GROUP).is_some()
        || lower_display.contains("load infra sink routes")
        || lower_display.contains("load business sink routes")
        || lower_display.contains("sink_group")
    {
        return Some(DiagnosticKind::Route);
    }

    None
}

fn collect_report_hints(
    report: &ErrorReport,
    summary: &DiagnosticSummary,
    display_chain: &str,
) -> Vec<&'static str> {
    let mut hints = Vec::new();

    if let Some(hint_code) = first_meta_hint(report) {
        match hint_code {
            HintCode::WpsrcTomlSchema => push_hint_once(
                &mut hints,
                "检查 wpsrc.toml 结构：使用 [[sources]]，字段包含 key/type/enable/tags/path 等",
            ),
            HintCode::SinkRouteTomlSchema => push_hint_once(
                &mut hints,
                "检查 sink route TOML 结构：使用 [sink_group] 与 [[sink_group.sinks]]，确认文件内容是合法 TOML",
            ),
            HintCode::SinkDefaultsTomlSchema => push_hint_once(
                &mut hints,
                "检查 sinks/defaults.toml 结构：应使用 [defaults] 表，不要填写 route 的 version/sink_group 字段",
            ),
            HintCode::KafkaFeatureRequired => push_hint_once(
                &mut hints,
                "Kafka 源需要启用 'kafka' 特性：如 'cargo build --features kafka --bins' 或启用 'community'",
            ),
            HintCode::DuplicateSourceKey => push_hint_once(
                &mut hints,
                "sources 中存在重复 key；请确保每个源的 key 唯一",
            ),
            HintCode::InvalidSyslogProtocol => push_hint_once(
                &mut hints,
                "Syslog 协议仅支持 UDP/TCP：protocol='UDP' 或 'TCP'",
            ),
        }
    }

    if let Some("unknown_source_kind") = first_hint_code_raw(report) {
        push_hint_once(
            &mut hints,
            "type 取值必须是 'file'/'syslog'/'tcp'/'kafka'（kafka 需启用 'kafka' 特性）",
        );
    }

    if let Some(kind) = classify_toml_parse_kind(report, summary, display_chain) {
        match kind {
            DiagnosticKind::Defaults => push_hint_once(
                &mut hints,
                "检查 sinks/defaults.toml 结构：应使用 [defaults] 表，不要填写 route 的 version/sink_group 字段",
            ),
            DiagnosticKind::Route => push_hint_once(
                &mut hints,
                "检查 sink route TOML 结构：使用 [sink_group] 与 [[sink_group.sinks]]，确认文件内容是合法 TOML",
            ),
            DiagnosticKind::Wpsrc => push_hint_once(
                &mut hints,
                "检查 wpsrc.toml 结构：使用 [[sources]]，字段包含 key/type/enable/tags/path 等",
            ),
        }
    }

    for hint in collect_hints(display_chain) {
        push_hint_once(&mut hints, hint);
    }

    hints
}

fn summarize_run_error(e: &RunError) -> DiagnosticSummary {
    let report = e.report();
    let reason = report.reason.clone();
    let mut triplet = DiagnosticTriplet {
        reason: reason.clone(),
        detail: report.detail.clone().map(|detail| sanitize_detail(&detail)),
        location: report.path.clone(),
    };
    if let Some(detail_location) = triplet.detail.as_deref().and_then(extract_location) {
        triplet.location = Some(detail_location);
    }
    let mut parse_excerpt = triplet
        .detail
        .as_deref()
        .and_then(extract_toml_parse_excerpt)
        .or_else(|| {
            report
                .source_frames
                .iter()
                .find_map(frame_parse_excerpt)
                .or_else(|| extract_toml_parse_excerpt(&e.display_chain()))
        });

    for frame in &report.source_frames {
        if let Some(location) = frame_location(frame)
            && (triplet
                .location
                .as_deref()
                .is_none_or(|current| !looks_like_file_location(current))
                || looks_like_file_location(&location))
        {
            triplet.location = Some(location);
        }
        if parse_excerpt.is_none() {
            parse_excerpt = frame_parse_excerpt(frame);
        }
        if !has_effective_detail(&reason, triplet.detail.as_deref()) {
            triplet.detail = frame_detail_candidate(frame, &reason);
        }
    }

    if !has_effective_detail(&reason, triplet.detail.as_deref())
        || triplet.location.is_none()
        || parse_excerpt.is_none()
    {
        let display_chain = e.display_chain();
        let (enriched, fallback_excerpt) = enrich_triplet_from_fallback(triplet, &display_chain);
        triplet = enriched;
        parse_excerpt = parse_excerpt.or(fallback_excerpt);
    }

    let root_cause = e.root_cause_frame().and_then(|frame| {
        root_cause_candidate(
            frame,
            &reason,
            triplet.detail.as_deref(),
            parse_excerpt.as_deref(),
        )
    });

    DiagnosticSummary {
        triplet,
        parse_excerpt,
        root_cause,
    }
}

fn push_hint_once(hints: &mut Vec<&'static str>, hint: &'static str) {
    if !hints.contains(&hint) {
        hints.push(hint);
    }
}

/// 提示收集：根据错误文本提取常见修复建议（启发式）。
pub fn collect_hints(es: &str) -> Vec<&'static str> {
    let mut hints: Vec<&'static str> = Vec::new();
    let lower = es.to_lowercase();
    if lower.contains("not exists")
        || lower.contains("filesource")
        || lower.contains("missing 'path'")
        || lower.contains("file source missing 'path'")
    {
        push_hint_once(
            &mut hints,
            "生成输入数据: 'wpgen conf init && wpgen rule -n 1000'，默认写入 ./data/in_dat/gen.dat；若启用并行则生成 ./data/in_dat/gen-r*.dat",
        );
        push_hint_once(
            &mut hints,
            "确认工作目录是否正确，必要时使用 --work_root 指定",
        );
        push_hint_once(
            &mut hints,
            "文件源示例: [[sources]] key='file_1' connect='file_main' enable=true params_override={ base='./data/in_dat', file='gen*.dat', encode='text' }",
        );
    }
    if lower.contains("requires feature 'kafka'")
        || (lower.contains("kafka") && lower.contains("feature"))
    {
        push_hint_once(
            &mut hints,
            "Kafka 源需要启用 'kafka' 特性：如 'cargo build --features kafka --bins' 或启用 'community'",
        );
    }
    if lower.contains("duplicate source key") {
        push_hint_once(
            &mut hints,
            "sources 中存在重复 key；请确保每个源的 key 唯一",
        );
    }
    if lower.contains("unknown source kind")
        || lower.contains("no builder registered for source kind")
    {
        push_hint_once(
            &mut hints,
            "type 取值必须是 'file'/'syslog'/'tcp'/'kafka'（kafka 需启用 'kafka' 特性）",
        );
    }
    if lower.contains("toml parse error")
        && (lower.contains("defaults.toml")
            || lower.contains("load sink defaults")
            || lower.contains("expected `defaults`"))
    {
        push_hint_once(
            &mut hints,
            "检查 sinks/defaults.toml 结构：应使用 [defaults] 表，不要填写 route 的 version/sink_group 字段",
        );
    } else if lower.contains("toml parse error")
        && (lower.contains("topology/sinks")
            || lower.contains("business.d")
            || lower.contains("infra.d")
            || lower.contains("sink_group")
            || lower.contains("load infra sink routes")
            || lower.contains("load business sink routes")
            || lower.contains("清理 sinks"))
    {
        push_hint_once(
            &mut hints,
            "检查 sink route TOML 结构：使用 [sink_group] 与 [[sink_group.sinks]]，确认文件内容是合法 TOML",
        );
    } else if lower.contains("failed to parse unified [[sources]] config")
        || (lower.contains("toml parse error")
            && (lower.contains("wpsrc.toml")
                || lower.contains("[[sources]]")
                || lower.contains("source key")
                || lower.contains("load wpsrc")
                || lower.contains("parse wpsrc")))
    {
        push_hint_once(
            &mut hints,
            "检查 wpsrc.toml 结构：使用 [[sources]]，字段包含 key/type/enable/tags/path 等",
        );
    }
    if lower.contains("no data sources configured") || lower.contains("sources is empty") {
        push_hint_once(&mut hints, "确保至少有一个源 enable=true，并填写必需参数");
    }
    if lower.contains("invalid protocol") && lower.contains("syslog") {
        push_hint_once(
            &mut hints,
            "Syslog 协议仅支持 UDP/TCP：protocol='UDP' 或 'TCP'",
        );
    }
    if lower.contains("expect: ratio/tol cannot be combined with min/max") {
        push_hint_once(
            &mut hints,
            "sink.expect: 二选一使用 'ratio/tol' 或 'min/max'，不要混用",
        );
        push_hint_once(&mut hints, "示例1: [sink.expect] ratio=0.02 tol=0.01");
        push_hint_once(&mut hints, "示例2: [sink.expect] min=0.98 max=1.0");
    }
    if lower.contains("output.connect") && lower.contains("source connector") {
        push_hint_once(
            &mut hints,
            "wpgen 的 [output].connect 必须填写 sink connector id，例如 'tcp_sink'、'file_raw_sink'；不要填写 'tcp_src' 这类 source connector id",
        );
    }
    hints
}

/// 计算退出码（供 CLI 使用），与历史映射保持一致
pub fn exit_code_for(reason: &RunReason) -> i32 {
    match reason {
        RunReason::Dist(_) => 1,
        RunReason::Source(_) => 2,
        RunReason::Uvs(u) => u.error_code(),
    }
}

struct DiagnosticPrint<'a> {
    app: &'a str,
    reason: &'a str,
    detail: Option<String>,
    location: Option<String>,
    parse_excerpt: Option<String>,
    root_cause: Option<String>,
    hints: &'a [&'static str],
    exit_code: Option<i32>,
}

fn print_diagnostic(diag: DiagnosticPrint<'_>) {
    let title = format!("{} error", diag.app);
    let pretty_msg = pretty_reason(diag.reason);
    let detail_opt = diag.detail.filter(|d| {
        let pretty_detail = pretty_reason(d);
        !is_generic_detail(&pretty_detail) && pretty_detail.trim() != pretty_msg.trim()
    });

    eprintln!("{} {}", bg_red(" ERROR "), bold(&title));
    if let Some(d) = &detail_opt {
        eprintln!(
            "{} {}",
            red(pretty_msg.trim()),
            red(format!("- {}", pretty_reason(d).trim()))
        );
    } else {
        eprintln!("{}", red(pretty_msg.trim()));
    }
    if let Some(d) = detail_opt {
        eprintln!("{} {}", bold("detail:"), pretty_reason(&d).trim());
    }
    if let Some(location) = diag.location {
        let pretty_location = location
            .trim_start_matches('(')
            .replace(": ", "=")
            .trim()
            .to_string();
        let label = if looks_like_file_location(&location) {
            "file:"
        } else {
            "location:"
        };
        eprintln!("{} {}", bold(label), yellow(pretty_location));
    }
    if let Some(excerpt) = diag.parse_excerpt {
        eprintln!("{} {}", bold("parse:"), yellow(excerpt));
    }
    if let Some(root_cause) = diag.root_cause {
        eprintln!("{} {}", bold("cause:"), yellow(pretty_reason(&root_cause)));
    }
    if !diag.hints.is_empty() {
        eprintln!("{}", bold("hints:"));
        for h in diag.hints {
            eprintln!("  - {}", yellow(h));
        }
    }
    if let Some(code) = diag.exit_code {
        eprintln!("exit code: {}", code);
    }
}

/// 打印更友好的错误信息（含建议与上下文）。
pub fn print_run_error(app: &str, e: &RunError) {
    let summary = summarize_run_error(e);
    let report = e.report();
    let display_chain = e.display_chain();
    let hints = collect_report_hints(&report, &summary, &display_chain);
    let code = exit_code_for(e.reason());
    print_diagnostic(DiagnosticPrint {
        app,
        reason: &summary.triplet.reason,
        detail: summary.triplet.detail,
        location: summary.triplet.location,
        parse_excerpt: summary.parse_excerpt,
        root_cause: summary.root_cause,
        hints: &hints,
        exit_code: Some(code),
    });
}

/// 通用错误打印（不要求 RunError）。
/// - 仅基于字符串启发式提取 reason/detail/context 与 hints。
pub fn print_error(app: &str, err: &impl std::fmt::Display) {
    let raw = err.to_string();
    let triplet = derive_error_triplet(&raw);
    let parse_excerpt = extract_toml_parse_excerpt(&raw);
    let hints = collect_hints(&raw);
    print_diagnostic(DiagnosticPrint {
        app,
        reason: &triplet.reason,
        detail: triplet.detail,
        location: triplet.location,
        parse_excerpt,
        root_cause: None,
        hints: &hints,
        exit_code: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_error::ErrorMetadata;
    use wp_error::diagnostic_meta::MetaValue;
    #[test]
    fn test_hint_file_source() {
        let hs = collect_hints("File source missing 'path'");
        assert!(hs.iter().any(|h| h.contains("生成输入数据")));
    }

    #[test]
    fn test_extract_file_and_toml_parse_excerpt() {
        let raw = r#"[50041] configuration error << core config
  -> Source: [500] load sink defaults
  -> Context stack:
context 0:
target: load object from toml file with env
1. from path: /tmp/wp-use/topology/sinks/defaults.toml

context 1:
target: load sink defaults

Caused by:
  0: [500] load sink defaults
  1: [500] TOML parse error at line 1, column 1
       |
     1 | version = "2.0"
       | ^^^^^^^
     unknown field `version`, expected `defaults`"#;
        let triplet = derive_error_triplet(raw);
        let excerpt = extract_toml_parse_excerpt(raw).expect("toml parse excerpt");
        assert_eq!(
            triplet.location.as_deref(),
            Some("/tmp/wp-use/topology/sinks/defaults.toml")
        );
        assert!(excerpt.contains("line 1, column 1"));
        assert!(excerpt.contains("unknown field `version`, expected `defaults`"));
    }

    #[test]
    fn test_derive_triplet_reads_plain_detail_line() {
        let raw = "[50041] configuration error << core config - [100] validation error\ndetail: missing field 'sources'";
        let triplet = derive_error_triplet(raw);
        assert_eq!(
            triplet.reason,
            "[50041] configuration error << core config - [100] validation error"
        );
        assert_eq!(triplet.detail.as_deref(), Some("missing field 'sources'"));
    }

    #[test]
    fn test_pretty_reason_handles_dash_separator() {
        let raw = "[50041] configuration error << core config - [100] validation error";
        assert_eq!(pretty_reason(raw), "配置错误: 校验失败");
    }

    #[test]
    fn test_generic_detail_is_suppressed() {
        assert!(is_generic_detail("校验失败"));
        assert!(is_generic_detail("[100] validation error"));
        assert!(!is_generic_detail("missing field 'sources'"));
    }

    #[test]
    fn test_collect_hints_is_case_insensitive() {
        let hs = collect_hints("duplicate source key");
        assert!(hs.iter().any(|h| h.contains("重复 key")));
    }

    #[test]
    fn test_collect_hints_distinguishes_sink_route_toml_parse() {
        let raw = "配置错误 - TOML parse error at line 1, column 1\nlocation: 清理 sinks 输出失败 / /tmp/wp-use/topology/sinks/infra.d/miss.toml";
        let hs = collect_hints(raw);
        assert!(hs.iter().any(|h| h.contains("sink route TOML")));
        assert!(!hs.iter().any(|h| h.contains("wpsrc.toml")));
    }

    #[test]
    fn test_collect_hints_distinguishes_sink_defaults_toml_parse() {
        let raw = "TOML parse error at line 1, column 1\nunknown field `version`, expected `defaults`\n1. from path: /tmp/wp-use/topology/sinks/defaults.toml";
        let hs = collect_hints(raw);
        assert!(hs.iter().any(|h| h.contains("sinks/defaults.toml")));
        assert!(!hs.iter().any(|h| h.contains("sink route TOML")));
        assert!(!hs.iter().any(|h| h.contains("wpsrc.toml")));
    }

    #[test]
    fn test_collect_hints_keeps_wpsrc_toml_parse_hint() {
        let raw = "parse wpsrc config: TOML parse error at line 1, column 1: /tmp/wp-use/topology/sources/wpsrc.toml";
        let hs = collect_hints(raw);
        assert!(hs.iter().any(|h| h.contains("wpsrc.toml")));
    }

    #[test]
    fn test_collect_report_hints_prefers_structured_sink_defaults_metadata() {
        let report = ErrorReport {
            reason: "configuration error << core config".to_string(),
            detail: Some("TOML parse error at line 1, column 1".to_string()),
            position: None,
            want: None,
            path: None,
            context: Vec::new(),
            root_metadata: {
                let mut m = ErrorMetadata::new();
                m.insert(key::CONFIG_KIND, ConfigKind::SinkDefaults.as_str());
                m
            },
            source_frames: Vec::new(),
        };
        let summary = DiagnosticSummary {
            triplet: DiagnosticTriplet {
                reason: "配置错误".to_string(),
                detail: None,
                location: Some("/tmp/wp-use/topology/sinks/defaults.toml".to_string()),
            },
            parse_excerpt: Some("TOML parse error at line 1, column 1".to_string()),
            root_cause: None,
        };

        let hs = collect_report_hints(&report, &summary, "configuration error");
        assert!(hs.iter().any(|h| h.contains("sinks/defaults.toml")));
        assert!(!hs.iter().any(|h| h.contains("sink route TOML")));
    }

    #[test]
    fn test_collect_report_hints_prefers_structured_wpsrc_metadata() {
        let report = ErrorReport {
            reason: "configuration error << core config".to_string(),
            detail: Some("TOML parse error at line 1, column 1".to_string()),
            position: None,
            want: None,
            path: None,
            context: Vec::new(),
            root_metadata: {
                let mut m = ErrorMetadata::new();
                m.insert(key::CONFIG_KIND, ConfigKind::Wpsrc.as_str());
                m
            },
            source_frames: Vec::new(),
        };
        let summary = DiagnosticSummary {
            triplet: DiagnosticTriplet {
                reason: "配置错误".to_string(),
                detail: None,
                location: Some("/tmp/wp-use/topology/sources/wpsrc.toml".to_string()),
            },
            parse_excerpt: Some("TOML parse error at line 1, column 1".to_string()),
            root_cause: None,
        };

        let hs = collect_report_hints(&report, &summary, "configuration error");
        assert!(hs.iter().any(|h| h.contains("wpsrc.toml")));
        assert!(!hs.iter().any(|h| h.contains("sink route TOML")));
    }

    #[test]
    fn test_collect_report_hints_supports_raw_unknown_source_kind_hint() {
        let report = ErrorReport {
            reason: "configuration error << core config".to_string(),
            detail: Some("source factory not found for kind 'udp2'".to_string()),
            position: None,
            want: None,
            path: None,
            context: Vec::new(),
            root_metadata: {
                let mut m = ErrorMetadata::new();
                m.insert(key::HINT_CODE, "unknown_source_kind");
                m
            },
            source_frames: Vec::new(),
        };
        let summary = DiagnosticSummary {
            triplet: DiagnosticTriplet {
                reason: "配置错误".to_string(),
                detail: None,
                location: None,
            },
            parse_excerpt: None,
            root_cause: None,
        };

        let hs = collect_report_hints(&report, &summary, "configuration error");
        assert!(hs.iter().any(|h| h.contains("type 取值必须是")));
    }

    #[test]
    fn test_collect_report_hints_prefers_structured_duplicate_source_key_hint() {
        let report = ErrorReport {
            reason: "configuration error << core config".to_string(),
            detail: Some("duplicate source key: 's1'".to_string()),
            position: None,
            want: None,
            path: None,
            context: Vec::new(),
            root_metadata: {
                let mut m = ErrorMetadata::new();
                m.insert(key::HINT_CODE, HintCode::DuplicateSourceKey.as_str());
                m
            },
            source_frames: Vec::new(),
        };
        let summary = DiagnosticSummary {
            triplet: DiagnosticTriplet {
                reason: "配置错误".to_string(),
                detail: None,
                location: None,
            },
            parse_excerpt: None,
            root_cause: None,
        };

        let hs = collect_report_hints(&report, &summary, "configuration error");
        assert!(hs.iter().any(|h| h.contains("重复 key")));
    }

    #[test]
    fn test_collect_report_hints_prefers_structured_kafka_feature_hint() {
        let report = ErrorReport {
            reason: "configuration error << core config".to_string(),
            detail: Some("source factory not found for kind 'kafka'".to_string()),
            position: None,
            want: None,
            path: None,
            context: Vec::new(),
            root_metadata: {
                let mut m = ErrorMetadata::new();
                m.insert(key::HINT_CODE, HintCode::KafkaFeatureRequired.as_str());
                m
            },
            source_frames: Vec::new(),
        };
        let summary = DiagnosticSummary {
            triplet: DiagnosticTriplet {
                reason: "配置错误".to_string(),
                detail: None,
                location: None,
            },
            parse_excerpt: None,
            root_cause: None,
        };

        let hs = collect_report_hints(&report, &summary, "configuration error");
        assert!(
            hs.iter()
                .any(|h| h.contains("Kafka 源需要启用 'kafka' 特性"))
        );
    }

    #[test]
    fn test_derive_triplet_reads_group_location() {
        let raw = "[50041] configuration error << core config\n(group: source key: tcp_1)";
        let triplet = derive_error_triplet(raw);
        assert_eq!(
            triplet.location.as_deref(),
            Some("(group: source key: tcp_1)")
        );
    }

    #[test]
    fn test_frame_location_prefers_embedded_file_over_operation_path() {
        let frame = SourceFrame {
            index: 0,
            message: "configuration error".to_string(),
            display: Some("1. from path: /tmp/wp-use/topology/sinks/defaults.toml".to_string()),
            debug: String::new(),
            type_name: None,
            error_code: None,
            reason: None,
            want: Some("load infra sink routes for clean".to_string()),
            path: Some(
                "load infra sink routes for clean / load sink defaults / load object from toml file with env"
                    .to_string(),
            ),
            detail: None,
            metadata: ErrorMetadata::default(),
            is_root_cause: false,
        };
        assert_eq!(
            frame_location(&frame).as_deref(),
            Some("/tmp/wp-use/topology/sinks/defaults.toml")
        );
    }

    #[test]
    fn test_enrich_triplet_from_debug_fills_missing_detail() {
        let primary = DiagnosticTriplet {
            reason: "[50041] configuration error << core config".to_string(),
            detail: None,
            location: None,
        };
        let debug_raw = "RunError { reason: Uvs(ConfigError(CoreConf)), detail: Some(\"missing field 'sources'\") }";
        let (triplet, excerpt) = enrich_triplet_from_fallback(primary, debug_raw);
        assert_eq!(triplet.detail.as_deref(), Some("missing field 'sources'"));
        assert!(excerpt.is_none());
    }

    #[test]
    fn test_enrich_triplet_from_fallback_overrides_generic_detail() {
        let primary = DiagnosticTriplet {
            reason: "配置错误".to_string(),
            detail: Some("校验失败".to_string()),
            location: None,
        };
        let fallback_raw = "detail: missing field 'sources'";
        let (triplet, _) = enrich_triplet_from_fallback(primary, fallback_raw);
        assert_eq!(triplet.detail.as_deref(), Some("missing field 'sources'"));
    }

    #[test]
    fn test_has_effective_detail_filters_generic_reason_duplicate() {
        assert!(!has_effective_detail("配置错误", Some("配置错误")));
        assert!(!has_effective_detail("配置错误", Some("校验失败")));
        assert!(has_effective_detail("配置错误", Some("缺少变量: SEC_PWD")));
    }

    #[test]
    fn test_sanitize_detail_trims_debug_tail() {
        let raw = "override 'endpoint' not allowed (file: /tmp/monitor.toml)\"), position: None, context: [OperationContext { ... }]";
        assert_eq!(
            sanitize_detail(raw),
            "override 'endpoint' not allowed (file: /tmp/monitor.toml)"
        );
    }

    #[test]
    fn test_sanitize_detail_trims_structured_context_tail() {
        let raw = "数据清理存在失败项: sinks: 配置错误\n  -> Want: 清理 sinks 输出失败\n  -> Source: [500] TOML parse error";
        assert_eq!(sanitize_detail(raw), "数据清理存在失败项: sinks: 配置错误");
    }

    #[test]
    fn test_extract_toml_parse_excerpt_trims_structured_context_tail() {
        let raw = "detail\nTOML parse error at line 1, column 1\n  |\n1 | version = \"2.0\"\nunknown field `version`, expected `defaults`\n\n  -> Want: load infra sink routes for clean\n  -> Context stack:";
        let excerpt = extract_toml_parse_excerpt(raw).expect("parse excerpt");
        assert!(excerpt.contains("unknown field `version`, expected `defaults`"));
        assert!(!excerpt.contains("Context stack"));
        assert!(!excerpt.contains("load infra sink routes for clean"));
    }

    #[test]
    fn test_exit_code_mapping() {
        use orion_error::UvsReason;
        use wp_error::run_error::{DistFocus, SourceFocus};
        assert_eq!(exit_code_for(&RunReason::Dist(DistFocus::StgCtrl)), 1);
        assert_eq!(exit_code_for(&RunReason::Source(SourceFocus::NoData)), 2);
        let uv = UvsReason::core_conf();
        assert_eq!(exit_code_for(&RunReason::Uvs(uv)), 300);
    }
}
