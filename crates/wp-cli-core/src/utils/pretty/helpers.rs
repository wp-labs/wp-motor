use std::path::Path;
use wp_conf::structure::Basis;

pub fn fmt_f(v: f64) -> String {
    // render up to 3 decimals; strip trailing zeros and dot
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}

pub fn basis_cn(b: &Basis) -> &'static str {
    match b {
        Basis::TotalInput => "Total Input",
        Basis::GroupInput => "Group Input",
        Basis::Model { .. } => "Model",
    }
}

pub fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
}

pub fn colorize(s: &str, code: &str) -> String {
    if no_color() {
        s.to_string()
    } else {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    }
}

pub fn color_warn<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "33")
}
pub fn color_err<S: AsRef<str>>(s: S) -> String {
    colorize(s.as_ref(), "31")
}
pub fn color_bg<S: AsRef<str>>(s: S, code: &str) -> String {
    colorize(s.as_ref(), code)
}
pub fn bg_pass<S: AsRef<str>>(s: S) -> String {
    color_bg(s, "42")
}
pub fn bg_fail<S: AsRef<str>>(s: S) -> String {
    color_bg(s, "41")
}
pub fn bg_warn<S: AsRef<str>>(s: S) -> String {
    color_bg(s, "43")
}

pub fn short_display_path(path: &Path, root: Option<&Path>, levels: usize) -> String {
    let display = if let Some(root) = root {
        path.strip_prefix(root)
            .map(|rel| rel.display().to_string())
            .unwrap_or_else(|_| path.display().to_string())
    } else {
        path.display().to_string()
    };
    truncate_path_tail(&display, levels)
}

fn truncate_path_tail(path: &str, levels: usize) -> String {
    if levels == 0 {
        return path.to_string();
    }

    let path_ref = Path::new(path);
    let components: Vec<_> = path_ref.components().collect();
    if components.len() <= levels {
        return path.to_string();
    }

    let start = components.len() - levels;
    let tail = components[start..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    format!(".../{}", tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn fmt_f_compacts_decimals() {
        assert_eq!(fmt_f(0.0), "0");
        assert_eq!(fmt_f(1.0), "1");
        assert_eq!(fmt_f(1.2), "1.2");
        assert_eq!(fmt_f(0.333_33), "0.333");
    }

    #[test]
    fn basis_pretty_names() {
        assert_eq!(basis_cn(&Basis::TotalInput), "Total Input");
        assert_eq!(basis_cn(&Basis::GroupInput), "Group Input");
        assert_eq!(basis_cn(&Basis::Model { mdl: "x".into() }), "Model");
    }

    #[test]
    fn short_display_path_keeps_short_relative_path() {
        assert_eq!(
            short_display_path(Path::new("data/in_dat/gen.dat"), None, 3),
            "data/in_dat/gen.dat"
        );
    }

    #[test]
    fn short_display_path_truncates_long_absolute_path() {
        assert_eq!(
            short_display_path(
                Path::new(
                    "/Users/wp/devspace/wp-labs/warp-parse/my_example/data/out_dat/demo.json"
                ),
                None,
                3,
            ),
            ".../data/out_dat/demo.json"
        );
    }

    #[test]
    fn short_display_path_prefers_relative_to_root_before_truncation() {
        assert_eq!(
            short_display_path(
                Path::new(
                    "/tmp/work/wp-examples/extensions/pg_knowledge/models/knowledge/semantic_dict.toml"
                ),
                Some(Path::new("/tmp/work")),
                3,
            ),
            ".../models/knowledge/semantic_dict.toml"
        );
    }
}
