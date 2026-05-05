//! 运行时错误的美化与提示收集，供各 CLI 共享使用。

use orion_error::{
    reason::{ErrorCode, ErrorIdentityProvider},
    runtime::source::SourceFrame,
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
    want: Option<String>,
    parse_excerpt: Option<String>,
    root_cause: Option<String>,
}

fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
}
fn is_english() -> bool {
    let raw = std::env::var("WP_LANG")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    raw.starts_with("en_") || raw.starts_with("C.") || raw == "C" || raw == "POSIX"
}
fn i18n(zh: &'static str, en: &'static str) -> &'static str {
    if is_english() { en } else { zh }
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
    trimmed.contains('/')
        || trimmed.ends_with(".toml")
        || trimmed.ends_with(".yaml")
        || trimmed.ends_with(".yml")
        || trimmed.ends_with(".json")
}

fn extract_toml_parse_excerpt(raw: &str) -> Option<String> {
    let anchor = raw.find("TOML parse error at line ")?;
    let excerpt = raw[anchor..].trim();
    Some(excerpt.to_string())
}

#[allow(dead_code)]
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
    frame
        .path
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| frame.display.as_deref().and_then(extract_location))
        .or_else(|| frame.detail.as_deref().and_then(extract_location))
        .or_else(|| extract_location(&frame.message))
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
        .filter(|detail: &String| has_effective_detail(reason, Some(detail.as_str())));
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

fn summarize_run_error(e: &RunError) -> DiagnosticSummary {
    let report = e.report();
    let reason = report.reason().to_string();
    let mut detail = report.detail().map(sanitize_detail);

    // Location: structured path first, then context metadata, then source frames
    let mut location = report.position().map(ToString::to_string);
    if location.is_none() {
        location = report.context().iter().find_map(|ctx| {
            ctx.metadata()
                .get_str("file.path")
                .or_else(|| ctx.metadata().get_str("config.path"))
                .or_else(|| ctx.metadata().get_str("path"))
                .map(ToString::to_string)
        });
    }

    let mut parse_excerpt = detail.as_deref().and_then(extract_toml_parse_excerpt);

    for frame in e.source_frames() {
        if let Some(frame_loc) = frame_location(frame)
            && (location
                .as_deref()
                .is_none_or(|current| !looks_like_file_location(current))
                || looks_like_file_location(&frame_loc))
        {
            location = Some(frame_loc);
        }
        if parse_excerpt.is_none() {
            parse_excerpt = frame_parse_excerpt(frame);
        }
        if !has_effective_detail(&reason, detail.as_deref()) {
            detail = frame_detail_candidate(frame, &reason).or(detail);
        }
    }

    // Fallback to display_chain string parsing only when structured data is insufficient
    if !has_effective_detail(&reason, detail.as_deref())
        || location.is_none()
        || parse_excerpt.is_none()
    {
        let display_chain = e.display_chain();
        let fallback = derive_error_triplet(&display_chain);
        let fallback_excerpt = extract_toml_parse_excerpt(&display_chain);
        if !has_effective_detail(&reason, detail.as_deref()) {
            detail = fallback.detail.or(detail);
        }
        location = location.or(fallback.location);
        parse_excerpt = parse_excerpt.or(fallback_excerpt);
    }

    let root_cause = e.root_cause_frame().and_then(|frame| {
        root_cause_candidate(frame, &reason, detail.as_deref(), parse_excerpt.as_deref())
    });

    DiagnosticSummary {
        triplet: DiagnosticTriplet {
            reason,
            detail,
            location,
        },
        want: None,
        parse_excerpt,
        root_cause,
    }
}

fn push_hint_once(hints: &mut Vec<&'static str>, hint: &'static str) {
    if !hints.contains(&hint) {
        hints.push(hint);
    }
}

/// 提示收集：基于 stable_code 提供修复建议。
pub fn collect_hints(stable_code: &str, detail: Option<&str>) -> Vec<&'static str> {
    let mut hints: Vec<&'static str> = Vec::new();

    match stable_code {
        // ── 配置错误 ──
        "conf.core_invalid" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查配置文件语法和必需字段",
                    "Check configuration file syntax and required fields",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "conf.feature_invalid" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查是否启用了所需特性（feature flag），或使用了不支持的配置项",
                    "Check whether required feature flags are enabled, or unsupported config fields are used",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "conf.dynamic_invalid" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "动态配置加载失败，检查配置源是否可访问、格式是否正确",
                    "Dynamic config load failed — check whether config source is accessible and has valid format",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }

        // ── 系统 / IO ──
        "sys.io_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查文件系统状态和文件权限",
                    "Check filesystem state and file permissions",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "sys.network_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查网络连接和服务可达性",
                    "Check network connectivity and service reachability",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "sys.timeout" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "操作超时，可稍后重试或检查下游服务延迟",
                    "Operation timed out — retry later or inspect downstream service latency",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "sys.resource_exhausted" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "资源不足，检查磁盘空间和内存使用",
                    "Resource exhausted — check disk space and memory usage",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "sys.data_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "数据处理失败，检查输入数据格式和内容完整性",
                    "Data processing failed — check input format and content integrity",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "sys.external_service_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "外部服务调用失败，检查服务是否可用以及认证信息是否正确",
                    "External service call failed — check service availability and authentication",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }

        // ── 业务逻辑 ──
        "biz.validation_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查输入数据格式和字段值是否符合要求",
                    "Check whether input data format and field values meet requirements",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "biz.business_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "业务规则校验未通过，检查输入是否满足业务约束",
                    "Business rule validation failed — check whether input meets business constraints",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "biz.not_found" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "确认资源路径和标识符是否正确，目标文件或目录是否存在",
                    "Verify resource path and identifier — check whether the target file or directory exists",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "biz.permission_denied" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "检查文件或目录的读写权限，必要时使用 chmod 或切换用户",
                    "Check file/directory read/write permissions — use chmod or switch user if needed",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "biz.run_rule_error" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "规则执行失败，检查规则文件语法和数据格式是否匹配",
                    "Rule execution failed — check rule file syntax and whether data format matches",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }

        // ── 分发层 ──
        "biz.dist" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "数据分发失败，检查 sink 配置和下游服务状态",
                    "Data distribution failed — check sink configuration and downstream service status",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }
        "biz.source" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "数据源访问失败，检查 source 配置、文件是否存在、网络是否可达",
                    "Data source access failed — check source config, file existence, and network reachability",
                ),
            );
            hint_by_detail(&mut hints, detail);
        }

        // ── 内部逻辑异常 ──
        "logic.internal_invariant_broken" => {
            push_hint_once(
                &mut hints,
                i18n(
                    "内部逻辑错误，请联系开发者并提供复现步骤",
                    "Internal logic error — please contact the developer and provide reproduction steps",
                ),
            );
        }

        _ => {
            hint_by_detail(&mut hints, detail);
        }
    }

    hints
}

/// 基于 detail 文本的补充提示（仅保留 stable_code 无法区分的少量关键场景）。
fn hint_by_detail(hints: &mut Vec<&'static str>, detail: Option<&str>) {
    let d = detail.unwrap_or("").to_lowercase();
    if d.is_empty() {
        return;
    }

    // ── 样本 / 规则 ──
    if d.contains("no sample.dat") || d.contains("no rule file") {
        push_hint_once(
            hints,
            i18n(
                "在规则目录下放置 sample.dat 和对应的 .wpl 文件，或使用 'wpgen rule -n 1000' 生成样本数据",
                "Place sample.dat and matching .wpl files in the rule directory, or run 'wpgen rule -n 1000' to generate sample data",
            ),
        );
        push_hint_once(
            hints,
            i18n(
                "规则目录默认位于 <work_root>/rule/，sample.dat 和 parse.wpl 需在同一目录下",
                "Rule directory defaults to <work_root>/rule/ — sample.dat and parse.wpl must be in the same directory",
            ),
        );
    }

    // ── 特性 / feature ──
    if d.contains("requires feature") || (d.contains("kafka") && d.contains("feature")) {
        push_hint_once(
            hints,
            i18n(
                "缺少编译特性，使用 'cargo build --features kafka --bins' 或启用 'community' 特性",
                "Missing compile feature — use 'cargo build --features kafka --bins' or enable the 'community' feature",
            ),
        );
    }

    // ── 废弃字段 ──
    if d.contains("unknown field `mode`") {
        push_hint_once(
            hints,
            i18n(
                "\"mode\" 字段已废弃，请从 wpgen.toml 中删除该字段",
                "The \"mode\" field is deprecated — remove it from wpgen.toml",
            ),
        );
    }
    if d.contains("unknown field `duration_secs`") {
        push_hint_once(
            hints,
            i18n(
                "\"duration_secs\" 字段已废弃，请从 wpgen.toml 中删除该字段",
                "The \"duration_secs\" field is deprecated — remove it from wpgen.toml",
            ),
        );
    }
}

/// 基于文本的提示收集（用于无 RunError 的通用错误）。
pub fn collect_hints_from_text(text: &str) -> Vec<&'static str> {
    let mut hints: Vec<&'static str> = Vec::new();
    hint_by_detail(&mut hints, Some(text));
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
    want: Option<String>,
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
    eprintln!("{}", red(pretty_msg.trim()));
    if let Some(d) = &detail_opt {
        eprintln!("{} {}", bold("detail:"), pretty_reason(d).trim());
    }
    if let Some(want) = diag.want {
        eprintln!("{} {}", bold("doing:"), yellow(want));
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
    let hints = collect_hints(e.stable_code(), summary.triplet.detail.as_deref());
    let code = exit_code_for(e.reason());
    print_diagnostic(DiagnosticPrint {
        app,
        reason: &summary.triplet.reason,
        detail: summary.triplet.detail,
        want: summary.want,
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
    let hints = collect_hints_from_text(&raw);
    print_diagnostic(DiagnosticPrint {
        app,
        reason: &triplet.reason,
        detail: triplet.detail,
        want: None,
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
    #[test]
    fn test_hint_file_source() {
        let hs = collect_hints("biz.source", Some("missing 'path'"));
        assert!(
            hs.iter()
                .any(|h| h.contains("Data source access failed") || h.contains("数据源访问失败"))
        );
    }

    #[test]
    fn test_collect_hints_by_stable_code_conf() {
        let hs = collect_hints("conf.core_invalid", Some("missing field 'sources'"));
        assert!(
            hs.iter()
                .any(|h| h.contains("Check configuration file") || h.contains("检查配置文件语法"))
        );
    }

    #[test]
    fn test_collect_hints_by_stable_code_io() {
        let hs = collect_hints("sys.io_error", None);
        assert!(
            hs.iter()
                .any(|h| h.contains("filesystem") || h.contains("文件系统"))
        );
    }

    #[test]
    fn test_collect_hints_by_stable_code_with_detail() {
        let hs = collect_hints(
            "conf.core_invalid",
            Some("no sample.dat with matching .wpl found"),
        );
        assert!(hs.iter().any(|h| h.contains("wpgen rule")));
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
        let hs = collect_hints_from_text("No Sample.dat with matching .wpl found");
        assert!(
            hs.iter()
                .any(|h| h.contains("wpgen rule") || h.contains("sample.dat"))
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
    fn test_exit_code_mapping() {
        use orion_error::UnifiedReason;
        use wp_error::run_error::{DistFocus, SourceFocus};
        assert_eq!(exit_code_for(&RunReason::Dist(DistFocus::StgCtrl)), 1);
        assert_eq!(exit_code_for(&RunReason::Source(SourceFocus::NoData)), 2);
        let uv = UnifiedReason::core_conf();
        assert_eq!(exit_code_for(&RunReason::Uvs(uv)), 300);
    }
}
