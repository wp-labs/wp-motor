//! 运行时错误的美化与提示收集，供各 CLI 共享使用。

use orion_error::ErrorCode;
use wp_error::run_error::{RunError, RunReason};

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

/// 从长串的嵌套错误中提取三元要素：原因、细节、上下文位置（若有）。
fn derive_error_triplet(raw: &str) -> (String, Option<String>, Option<String>) {
    let reason = if let Some(idx) = raw.find("StructError") {
        raw[..idx].trim_end().to_string()
    } else {
        raw.to_string()
    };
    let mut details = raw
        .find("Details:")
        .and_then(|pos| raw[pos + "Details:".len()..].lines().next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if details.is_none() {
        if let Some(pos) = raw.find("Core(\"") {
            let tail = &raw[pos + 6..];
            if let Some(end) = tail.find("\")") {
                let msg = &tail[..end];
                if !msg.is_empty() {
                    details = Some(msg.to_string());
                }
            }
        } else if let Some(pos) = raw.find("detail: Some(\"") {
            let tail = &raw[pos + 14..];
            if let Some(end) = tail.find("\")") {
                let msg = &tail[..end];
                if !msg.is_empty() {
                    details = Some(msg.to_string());
                }
            }
        }
    }
    let context = raw
        .rfind("(group:")
        .map(|start| raw[start..].trim().trim_end_matches(')').to_string());
    (reason, details, context)
}

/// 提示收集：根据错误文本提取常见修复建议（启发式）。
pub fn collect_hints(es: &str) -> Vec<&'static str> {
    let mut hints: Vec<&'static str> = Vec::new();
    if es.contains("not exists")
        || es.contains("FileSource")
        || es.contains("missing 'path'")
        || es.contains("File source missing 'path'")
    {
        hints.push(
            "生成输入数据: 'wpgen conf init && wpgen rule -n 1000' 或创建 ./data/in_dat/gen.dat",
        );
        hints.push("确认工作目录是否正确，必要时使用 --work_root 指定");
        hints.push("文件源示例: [[sources]] key='file_1' connect='file_main' enable=true params_override={ base='./data/in_dat', file='gen.dat', encode='text' }");
    }
    if es.contains("requires feature 'kafka'") || (es.contains("kafka") && es.contains("feature")) {
        hints.push("Kafka 源需要启用 'kafka' 特性：如 'cargo build --features kafka --bins' 或启用 'community'");
    }
    if es.contains("Duplicate source key") {
        hints.push("sources 中存在重复 key；请确保每个源的 key 唯一");
    }
    if es.contains("Unknown source kind") || es.contains("No builder registered for source kind") {
        hints.push("type 取值必须是 'file'/'syslog'/'tcp'/'kafka'（kafka 需启用 'kafka' 特性）");
    }
    if es.contains("Failed to parse unified [[sources]] config")
        || es.contains("Failed to parse TOML")
    {
        hints.push("检查 wpsrc.toml 结构：使用 [[sources]]，字段包含 key/type/enable/tags/path 等");
    }
    if es.contains("No data sources configured") || es.contains("sources is empty") {
        hints.push("确保至少有一个源 enable=true，并填写必需参数");
    }
    if es.contains("invalid protocol") && es.to_lowercase().contains("syslog") {
        hints.push("Syslog 协议仅支持 UDP/TCP：protocol='UDP' 或 'TCP'");
    }
    if es.contains("expect: ratio/tol cannot be combined with min/max") {
        hints.push("sink.expect: 二选一使用 'ratio/tol' 或 'min/max'，不要混用");
        hints.push("示例1: [sink.expect] ratio=0.02 tol=0.01");
        hints.push("示例2: [sink.expect] min=0.98 max=1.0");
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

/// 打印更友好的错误信息（含建议与上下文）。
pub fn print_run_error(app: &str, e: &RunError) {
    let title = format!("{} error", app);
    let raw = e.to_string();
    let (reason_str, details_opt, ctx_opt) = derive_error_triplet(&raw);
    let hints = collect_hints(&raw);

    eprintln!("{} {}", bg_red(" ERROR "), bold(&title));
    let pretty_reason = reason_str
        .replace(
            "[50041] configuration error << core config > ",
            "配置错误: ",
        )
        .replace("[100] validation error << ", "校验失败: ")
        .replace("syntax err:", "")
        .replace("sink validate error: ", "");
    if let Some(d) = &details_opt {
        eprintln!("{} {}", red(pretty_reason.trim()), red(format!("- {}", d)));
    } else {
        eprintln!("{}", red(pretty_reason.trim()));
    }
    if let Some(d) = details_opt {
        eprintln!("{} {}", bold("detail:"), d);
    }
    if let Some(c) = ctx_opt {
        let ctx = c.trim_start_matches('(').replace(": ", "=");
        eprintln!("{} {}", bold("location:"), yellow(ctx));
    }
    if !hints.is_empty() {
        eprintln!("{}", bold("hints:"));
        for h in hints {
            eprintln!("  - {}", yellow(h));
        }
    }
    let code = exit_code_for(e.reason());
    eprintln!("exit code: {}", code);
}

/// 通用错误打印（不要求 RunError）。
/// - 仅基于字符串启发式提取 reason/detail/context 与 hints。
pub fn print_error(app: &str, err: &impl std::fmt::Display) {
    let title = format!("{} error", app);
    let raw = err.to_string();
    let (reason_str, details_opt, ctx_opt) = derive_error_triplet(&raw);
    let hints = collect_hints(&raw);

    eprintln!("{} {}", bg_red(" ERROR "), bold(&title));
    let pretty_reason = reason_str
        .replace(
            "[50041] configuration error << core config > ",
            "配置错误: ",
        )
        .replace("[100] validation error << ", "校验失败: ")
        .replace("syntax err:", "")
        .replace("sink validate error: ", "");
    if let Some(d) = &details_opt {
        eprintln!("{} {}", red(pretty_reason.trim()), red(format!("- {}", d)));
    } else {
        eprintln!("{}", red(pretty_reason.trim()));
    }
    if let Some(d) = details_opt {
        eprintln!("{} {}", bold("detail:"), d);
    }
    if let Some(c) = ctx_opt {
        let ctx = c.trim_start_matches('(').replace(": ", "=");
        eprintln!("{} {}", bold("location:"), yellow(ctx));
    }
    if !hints.is_empty() {
        eprintln!("{}", bold("hints:"));
        for h in hints {
            eprintln!("  - {}", yellow(h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hint_file_source() {
        let hs = collect_hints("File source missing 'path'");
        assert!(hs.iter().any(|h| h.contains("生成输入数据")));
    }
    #[test]
    fn test_exit_code_mapping() {
        use orion_error::UvsReason;
        use wp_error::run_error::{DistFocus, SourceFocus};
        assert_eq!(exit_code_for(&RunReason::Dist(DistFocus::StgCtrl)), 1);
        assert_eq!(exit_code_for(&RunReason::Source(SourceFocus::NoData)), 2);
        let uv = UvsReason::core_conf("bad conf");
        assert_eq!(exit_code_for(&RunReason::Uvs(uv)), 300);
    }
}
