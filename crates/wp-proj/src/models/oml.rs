use crate::compat::{ErrorOweBase, UvsFrom};
use futures_lite::future::block_on;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use orion_conf::ErrorWith;
use orion_error::conversion::ToStructError;
use orion_variate::EnvDict;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wp_conf::{engine::EngineConfig, utils::find_conf_files};
use wp_engine::facade::config::WPARSE_OML_FILE;
use wp_error::run_error::{RunReason, RunResult};
use wp_log::warn_ctrl;

use crate::traits::{Checkable, Component, ComponentBase, ComponentLifecycle, HasExamples};
use crate::types::CheckStatus;
use crate::utils::{TemplateInitializer, error_handler::ErrorHandler};

#[derive(Clone)]
pub struct Oml {
    base: ComponentBase,
}

// Deref to ComponentBase for seamless access to base methods
impl std::ops::Deref for Oml {
    type Target = ComponentBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for Oml {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Oml {
    pub fn new<P: AsRef<Path>>(work_root: P, eng_conf: Arc<EngineConfig>) -> Self {
        Self {
            base: ComponentBase::new(work_root, eng_conf),
        }
    }

    fn oml_root(&self) -> PathBuf {
        self.resolve_path(self.eng_conf().oml_root())
    }

    /// Initialize OML with example content for the specified project directory
    pub fn init_with_examples(&self) -> RunResult<()> {
        let work_root = self.work_root();
        let example_oml_content = include_str!("../example/oml/nginx.oml");
        if !example_oml_content.contains("name") || !example_oml_content.contains("rule") {
            return ErrorHandler::config_error("example OML content is missing essential fields");
        }

        self.create_example_files(work_root)?;

        println!("OML initialized successfully with example content");
        Ok(())
    }

    /// Create example OML files in the specified project directory
    fn create_example_files(&self, _work_root: &Path) -> RunResult<()> {
        let oml_dir = self.oml_root();
        let initializer = TemplateInitializer::new(oml_dir.clone());

        let example_oml_content = include_str!("../example/oml/nginx.oml");
        initializer.write_files(&[("example.oml", example_oml_content)])?;

        println!("Created example OML files:");
        println!("  - {:?}", oml_dir.join("example.oml"));

        Ok(())
    }

    pub fn check(&self, _dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        let oml_root = self.oml_root();
        if !oml_root.exists() {
            return Ok(CheckStatus::Miss);
        }
        let root_str = oml_root.to_str().ok_or_else(|| {
            RunReason::from_conf()
                .to_err()
                .with_detail("oml root path is not valid UTF-8")
        })?;
        let oml_files = find_conf_files(root_str, WPARSE_OML_FILE)
            .owe(RunReason::from_conf())
            .with_context(root_str)
            .doing("find oml files")?;
        if oml_files.is_empty() {
            return Ok(CheckStatus::Miss);
        }
        for f in &oml_files {
            ErrorHandler::check_file_not_empty(f, "OML")?;
        }

        // Full syntax + semantic validation for every OML file.
        let diagnostics = validate_oml_files(&oml_files);

        for warning in &diagnostics.warnings {
            warn_ctrl!("OML check: {}", warning);
        }

        if !diagnostics.errors.is_empty() {
            return Err(RunReason::from_rule()
                .to_err()
                .with_detail(diagnostics.errors.join("\n")));
        }
        Ok(CheckStatus::Suc)
    }
}

/// Aggregated result of the OML component validation.
struct OmlDiagnostics {
    /// Blocking problems: the check must fail and report these.
    errors: Vec<String>,
    /// Non-blocking problems that are surfaced but do not fail the check.
    warnings: Vec<String>,
}

/// Validate every discovered OML file:
/// - run the official OML parser over the whole file (after comment removal),
///   so any unrecognized or trailing content is rejected (errors);
/// - then apply non-blocking semantic checks such as model-name uniqueness
///   and missing rule patterns (warnings).
fn validate_oml_files(oml_files: &[PathBuf]) -> OmlDiagnostics {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut names: BTreeMap<String, String> = BTreeMap::new(); // name -> defining file

    for fp in oml_files {
        match parse_oml_file(fp) {
            Ok(model) => {
                let name = model.name().to_string();

                if let Some(prev_file) = names.get(&name) {
                    warnings.push(format!(
                        "{}: duplicate OML model name '{}' (already defined in {})",
                        fp.display(),
                        name,
                        prev_file
                    ));
                } else {
                    names.insert(name.clone(), fp.display().to_string());
                }

                if model.rules().is_empty() {
                    warnings.push(format!(
                        "{}: OML model '{}' declares no rule pattern and will never match a WPL rule",
                        fp.display(),
                        name
                    ));
                }
            }
            Err(e) => errors.push(e),
        }
    }

    OmlDiagnostics { errors, warnings }
}

/// Parse a single OML file with the official parser.
///
/// Comments are masked out (matching the runtime `OMLCode` comment pipeline),
/// then the *entire* remaining content is parsed with `oml_parse_raw`. The
/// parser rejects most trailing content it cannot consume, and any content it
/// returns `Ok` while leaving behind (e.g. after a second `---` separator) is
/// rejected explicitly, so unknown syntax such as `def label_for(...)` at the
/// end of a file fails validation instead of being silently ignored.
///
/// On failure the returned message contains the file path, the 1-based
/// line/column of the failure, the parser reason, and the offending snippet.
fn parse_oml_file(path: &Path) -> Result<ObjModel, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: failed to read OML file: {}", path.display(), e))?;

    let masked = mask_comments(&raw);
    let mut data: &str = masked.as_str();

    let parsed = block_on(oml_parse_raw(&mut data));

    // `oml_parse_raw` can succeed while leaving unparsed input behind (for
    // example when a second `---` separator appears mid-file). Any non-blank
    // remainder means the file was not fully validated.
    if parsed.is_ok() && !data.trim().is_empty() {
        let consumed = masked.len().saturating_sub(data.len());
        let (line, col) = line_col(&masked, consumed);
        let near = snippet_at(&masked, consumed);
        return Err(format!(
            "{}:{}:{}: OML syntax error: unexpected trailing content{}",
            path.display(),
            line,
            col,
            near
        ));
    }

    match parsed {
        Ok(model) => Ok(model),
        Err(err) => {
            let consumed = masked.len().saturating_sub(data.len());
            let (line, col) = line_col(&masked, consumed);
            let near = snippet_at(&masked, consumed);
            Err(format!(
                "{}:{}:{}: OML syntax error: {}{}",
                path.display(),
                line,
                col,
                err,
                near
            ))
        }
    }
}

/// Mask `//` line comments and `/* ... */` block comments with spaces.
///
/// The output has exactly the same byte length as the input and preserves every
/// newline, so an offset computed against the masked text is still valid for the
/// original file. Comment markers are only recognized at the start of a token
/// and `//` runs to the end of the line, mirroring the `CommentParser` used by
/// the OML runtime pipeline.
fn mask_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let n = src.len();
    let mut i = 0usize;
    let mut in_block = false;

    while i < n {
        if in_block {
            // Skip leading whitespace, then a `*/` token closes the block.
            while i < n {
                let ch = src[i..].chars().next().unwrap();
                if ch.is_whitespace() {
                    push_masked(&mut out, ch);
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            if i >= n {
                break;
            }
            if src[i..].starts_with("*/") {
                out.push_str("  ");
                i += 2;
                in_block = false;
                continue;
            }
            i = mask_to_line_end(&mut out, src, i);
        } else {
            // Keep leading whitespace verbatim.
            while i < n {
                let ch = src[i..].chars().next().unwrap();
                if ch.is_whitespace() {
                    out.push(ch);
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            if i >= n {
                break;
            }
            if src[i..].starts_with("/*") {
                out.push_str("  ");
                i += 2;
                in_block = true;
                continue;
            }
            if src[i..].starts_with("//") {
                i = mask_to_line_end(&mut out, src, i);
                continue;
            }
            // Keep the rest of the line verbatim.
            while i < n {
                let ch = src[i..].chars().next().unwrap();
                if ch == '\n' {
                    out.push('\n');
                    i += 1;
                    break;
                }
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    out
}

/// Push a single masked character, preserving newlines and byte length.
fn push_masked(out: &mut String, ch: char) {
    if ch == '\n' {
        out.push('\n');
    } else {
        out.extend(std::iter::repeat_n(' ', ch.len_utf8()));
    }
}

/// Mask characters up to (but not including) the end of the current line.
/// Returns the index of the trailing newline (or the end of the input).
fn mask_to_line_end(out: &mut String, src: &str, mut i: usize) -> usize {
    let n = src.len();
    while i < n {
        let ch = src[i..].chars().next().unwrap();
        if ch == '\n' {
            break;
        }
        out.extend(std::iter::repeat_n(' ', ch.len_utf8()));
        i += ch.len_utf8();
    }
    i
}

/// Compute the 1-based line/column of a byte offset.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Build a short single-line snippet starting at the given offset.
fn snippet_at(text: &str, offset: usize) -> String {
    let offset = offset.min(text.len());
    let snippet: String = text[offset..]
        .chars()
        .take(30)
        .collect::<String>()
        .replace('\n', "\\n");
    if snippet.trim().is_empty() {
        String::new()
    } else {
        format!(" (near: '{}')", snippet)
    }
}

// Trait implementations for unified component interface
impl Component for Oml {
    fn component_name(&self) -> &'static str {
        "OML"
    }
}

impl Checkable for Oml {
    fn check(&self, dict: &orion_variate::EnvDict) -> RunResult<CheckStatus> {
        // Delegate to the existing check implementation
        Oml::check(self, dict)
    }
}

impl HasExamples for Oml {
    fn init_with_examples(&self) -> RunResult<()> {
        // Delegate to the existing init_with_examples implementation
        Oml::init_with_examples(self)
    }
}

impl ComponentLifecycle for Oml {
    fn init(&self, _dict: &EnvDict) -> RunResult<()> {
        // OML initialization uses examples by default
        self.init_with_examples()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{temp_workdir, write_file};
    use tempfile::TempDir;

    fn oml_with_files(files: &[(&str, &str)]) -> (TempDir, Oml) {
        let temp = temp_workdir();
        let root = temp.path().to_str().unwrap();
        let eng = Arc::new(EngineConfig::init(root).conf_absolutize(root));
        let oml = Oml::new(root, eng);
        for (name, body) in files {
            write_file(temp.path(), &format!("models/oml/{}", name), body);
        }
        (temp, oml)
    }

    fn check_oml(oml: &Oml) -> Result<CheckStatus, String> {
        oml.check(&EnvDict::default()).map_err(|e| e.to_string())
    }

    #[test]
    fn initialize_examples_creates_valid_files() {
        let temp = temp_workdir();
        let root = temp.path().to_str().unwrap();
        let eng = Arc::new(EngineConfig::init(root).conf_absolutize(root));
        let oml = Oml::new(root, eng);
        oml.init_with_examples().expect("init examples");

        let example_file = temp.path().join("models/oml/example.oml");
        assert!(example_file.exists());
        assert!(
            !temp.path().join("models/oml/knowdb.toml").exists(),
            "knowdb.toml should not be generated under models/oml"
        );
        assert!(!temp.path().join("models/oml/*.oml").exists());
        // The generated example is legitimate OML and must pass validation.
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn valid_oml_passes() {
        let (_temp, oml) = oml_with_files(&[(
            "nginx.oml",
            "name : nginx\nrule : /nginx/*\n---\n* : auto = take() ;\n",
        )]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn comments_and_static_blocks_pass() {
        let (_temp, oml) = oml_with_files(&[(
            "commented.oml",
            r#"name : commented
rule : /commented/*
---
// a line comment
/* a block comment
   spanning lines
*/
tag : chars = chars(hello) ;
* : auto = take() ;
"#,
        )]);
        let result = check_oml(&oml);
        assert!(
            matches!(result, Ok(CheckStatus::Suc)),
            "comment file should pass: {result:?}"
        );
    }

    #[test]
    fn trailing_def_garbage_fails_with_location() {
        let body = "name : sample\nrule : /sample/*\n---\nvalue = read(source/value);\n\ndef label_for(name: str):\n    return name\n";
        let (_temp, oml) = oml_with_files(&[("sample.oml", body)]);
        let err = check_oml(&oml).expect_err("trailing def should fail validation");
        assert!(
            err.contains("sample.oml"),
            "error should name the file: {err}"
        );
        assert!(
            err.contains("OML syntax error"),
            "error should mention syntax: {err}"
        );
        // Line/column should be reported (format `path:line:col:`); the
        // offending `def label_for(...)` starts on line 6.
        assert!(
            err.contains("sample.oml:6:"),
            "error should carry the failure line: {err}"
        );
    }

    #[test]
    fn trailing_unknown_text_fails() {
        let body = "name : sample\nrule : /sample/*\n---\nvalue = read(source/value);\nthis is not oml at all\n";
        let (_temp, oml) = oml_with_files(&[("sample.oml", body)]);
        let err = check_oml(&oml).expect_err("trailing text should fail validation");
        assert!(
            err.contains("sample.oml"),
            "error should name the file: {err}"
        );
        assert!(err.contains("OML syntax error"), "error: {err}");
    }

    #[test]
    fn missing_separator_fails() {
        let (_temp, oml) = oml_with_files(&[(
            "bad_head.oml",
            "name : bad_head\nrule : /bad/*\ndescription : nope\n",
        )]);
        let err = check_oml(&oml).expect_err("invalid head should fail validation");
        assert!(
            err.contains("bad_head.oml"),
            "error should name the file: {err}"
        );
    }

    #[test]
    fn duplicate_model_names_warn_but_pass() {
        let body = "name : dup\nrule : /dup/*\n---\n* : auto = take() ;\n";
        let (_temp, oml) = oml_with_files(&[("a.oml", body), ("b.oml", body)]);
        // Duplicate names are reported as warnings, not blocking errors.
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn model_without_rules_warns_but_passes() {
        let (_temp, oml) =
            oml_with_files(&[("norule.oml", "name : norule\n---\n* : auto = take() ;\n")]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn multiple_valid_models_pass() {
        let (_temp, oml) = oml_with_files(&[
            ("a.oml", "name : a\nrule : /a/*\n---\n* : auto = take() ;\n"),
            ("b.oml", "name : b\nrule : /b/*\n---\n* : auto = take() ;\n"),
        ]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn empty_file_fails() {
        let (_temp, oml) = oml_with_files(&[("empty.oml", "   \n\n")]);
        let err = check_oml(&oml).expect_err("empty file should fail validation");
        assert!(
            err.contains("empty.oml"),
            "error should name the file: {err}"
        );
    }

    #[test]
    fn comment_in_head_passes() {
        let (_temp, oml) = oml_with_files(&[(
            "head_comment.oml",
            "name : head_comment\n// comment between head fields\nrule : /hc/*\n---\n* : auto = take() ;\n",
        )]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn trailing_comment_passes() {
        let (_temp, oml) = oml_with_files(&[(
            "trailing_comment.oml",
            "name : tc\nrule : /tc/*\n---\n* : auto = take() ;\n// trailing comment\n",
        )]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn double_slash_inside_string_is_not_a_comment() {
        let (_temp, oml) = oml_with_files(&[(
            "url.oml",
            "name : url\nrule : /url/*\n---\nlink : chars = chars('a//b') ;\n",
        )]);
        assert!(
            matches!(check_oml(&oml), Ok(CheckStatus::Suc)),
            "a `//` inside a string must not be treated as a comment: {:?}",
            check_oml(&oml)
        );
    }

    #[test]
    fn enable_false_model_passes() {
        let (_temp, oml) = oml_with_files(&[(
            "disabled.oml",
            "name : disabled\nrule : /d/*\nenable : false\n---\n* : auto = take() ;\n",
        )]);
        assert!(matches!(check_oml(&oml), Ok(CheckStatus::Suc)));
    }

    #[test]
    fn syntax_error_reports_near_snippet() {
        let body = "name : sample\nrule : /sample/*\n---\nvalue = read(source/value);\n\ndef label_for(name: str):\n    return name\n";
        let (_temp, oml) = oml_with_files(&[("sample.oml", body)]);
        let err = check_oml(&oml).expect_err("trailing def should fail validation");
        assert!(
            err.contains("near:"),
            "error should include an offending snippet: {err}"
        );
        assert!(
            err.contains("label_for"),
            "snippet should point at the offending token: {err}"
        );
    }

    #[test]
    fn second_separator_with_leftover_fails() {
        // `oml_parse_raw` returns Ok after consuming a second `---` while leaving
        // the following aggregate unparsed; the check must reject the leftover.
        let body = "name : two\nrule : /two/*\n---\n* : auto = take() ;\n---\nleaked = read(x);\n";
        let (_temp, oml) = oml_with_files(&[("two.oml", body)]);
        let err = check_oml(&oml).expect_err("leftover after second separator should fail");
        assert!(err.contains("two.oml"), "error should name the file: {err}");
        assert!(
            err.contains("trailing content"),
            "error should mention trailing content: {err}"
        );
    }

    // --- mask_comments unit checks ---------------------------------------------

    fn assert_mask_preserves(src: &str) -> String {
        let masked = mask_comments(src);
        assert_eq!(
            masked.len(),
            src.len(),
            "mask must preserve byte length for {src:?}"
        );
        assert_eq!(
            masked.matches('\n').count(),
            src.matches('\n').count(),
            "mask must preserve newline count for {src:?}"
        );
        masked
    }

    #[test]
    fn mask_leaves_rule_glob_untouched() {
        // `/nginx/*` contains `/*` mid-token and must NOT open a block comment.
        let src = "rule : /nginx/*\n---\n* : auto = take() ;\n";
        assert_eq!(mask_comments(src), src);
    }

    #[test]
    fn mask_removes_line_comment() {
        let src = "// comment\nname : x\n";
        let masked = assert_mask_preserves(src);
        assert!(!masked.contains("comment"));
        assert!(masked.contains("name : x"));
        assert!(masked.starts_with("  "));
    }

    #[test]
    fn mask_removes_block_comment() {
        let src = "/* zzz\n yyy\n*/\nname : x\n";
        let masked = assert_mask_preserves(src);
        assert!(!masked.contains("zzz"));
        assert!(!masked.contains("yyy"));
        assert!(masked.contains("name : x"));
    }

    #[test]
    fn mask_preserves_length_for_unicode_comment() {
        let src = "// 中文注释\nname : x\n";
        let masked = assert_mask_preserves(src);
        assert!(!masked.contains("中文"));
        assert!(masked.contains("name : x"));
    }

    // --- line_col unit checks ---------------------------------------------------

    #[test]
    fn line_col_reports_one_based_positions() {
        assert_eq!(line_col("abc", 0), (1, 1));
        assert_eq!(line_col("abc", 2), (1, 3));
        assert_eq!(line_col("ab\ncd", 3), (2, 1));
        assert_eq!(line_col("ab\ncd", 4), (2, 2));
    }
}
