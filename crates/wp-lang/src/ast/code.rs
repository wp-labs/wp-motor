use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::ast::WplPackage;

use crate::parser::error::{WplCodeError, WplCodeReason, WplCodeResult, error_detail};
use crate::parser::wpl_pkg::wpl_package;
use crate::parser::wpl_rule::wpl_rule;
use crate::winnow::Parser;
use derive_getters::Getters;
use orion_error::{ErrorOwe, ErrorWith, ToStructError, UvsNotFoundFrom};
use wp_parser::comment::CommentParser;

#[derive(Debug, Clone, Getters)]
pub struct WplCode {
    path: PathBuf,
    code: String,
}

impl TryFrom<(PathBuf, String)> for WplCode {
    type Error = WplCodeError;
    fn try_from(v: (PathBuf, String)) -> WplCodeResult<Self> {
        Self::build(v.0, v.1.as_str())
    }
}

impl TryFrom<(PathBuf, &str)> for WplCode {
    type Error = WplCodeError;
    fn try_from(v: (PathBuf, &str)) -> WplCodeResult<Self> {
        Self::build(v.0, v.1)
    }
}
impl TryFrom<&str> for WplCode {
    type Error = WplCodeError;
    fn try_from(v: &str) -> WplCodeResult<Self> {
        Self::build(PathBuf::new(), v)
    }
}

impl WplCode {
    pub fn build(path: PathBuf, code: &str) -> WplCodeResult<Self> {
        let mut in_code = code;
        let pure_code = CommentParser::ignore_comment(&mut in_code).map_err(|e| {
            WplCodeError::from(WplCodeReason::Syntax(format!("comment proc error {} ", e)))
        })?;

        Ok(Self {
            path,
            code: pure_code.to_string(),
        })
    }
    pub fn get_code(&self) -> &String {
        &self.code
    }
    pub fn parse_pkg(&self) -> WplCodeResult<WplPackage> {
        let package = wpl_package
            .parse(self.code.as_str())
            .map_err(|err| WplCodeError::from(WplCodeReason::Syntax(error_detail(err))))?;
        Ok(package)
    }
    pub fn parse_rule(&self) -> WplCodeResult<WplPackage> {
        let rule = wpl_rule
            .parse(self.code.as_str())
            .map_err(|err| WplCodeError::from(WplCodeReason::Syntax(error_detail(err))))?;
        let mut target = WplPackage::default();
        target.rules.push_back(rule);
        Ok(target)
    }
    pub fn empty_ins() -> WplCodeResult<Self> {
        WplCode::try_from((PathBuf::new(), ""))
    }
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
    pub fn load<P: AsRef<Path> + Clone>(wpl_file: P) -> WplCodeResult<Self> {
        let mut buffer = Vec::with_capacity(10240);
        let mut f = File::open(wpl_file.clone())
            //.with_context(|| format!("conf file not found: {:?}", wpl_file))
            .owe_conf()
            .with(wpl_file.as_ref())?;
        //.owe_conf::<WPLCodeError>()?;
        f.read_to_end(&mut buffer).expect("read conf file error");
        let file_data = String::from_utf8(buffer).expect("conf file is not utf8");
        let code = WplCode::build(PathBuf::from(wpl_file.as_ref()), file_data.as_str())?;
        Ok(code)
    }

    pub fn mix_load<P: AsRef<Path> + Clone>(
        arg_file: Option<P>,
        src_rule: Option<String>,
    ) -> WplCodeResult<Self> {
        if let Some(rule) = src_rule {
            let code = format!("rule cli {{  {} }}", rule);
            return WplCode::try_from((PathBuf::from("src"), code.as_str()));
        }
        if let Some(rule_file) = arg_file {
            return WplCode::load(rule_file);
        }
        WplCodeReason::from_not_found("miss wpl file").err_result()
    }
}
