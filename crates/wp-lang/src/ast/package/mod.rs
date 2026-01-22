use crate::parser::error::{WPLCodeErrorTrait, WplCodeError};
use derive_getters::Getters;
use smol_str::SmolStr;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::io::Write;
use wp_parser::Parser;

use crate::ast::debug::{DebugFormat, DepIndent};
use crate::ast::{WplRule, WplRuleMeta, WplTag};
use crate::parser::MergeTags;
use crate::parser::wpl_pkg::wpl_package;

use super::AnnFun;

#[derive(Default, Clone, Getters, Debug)]
pub struct WplPackage {
    pub name: SmolStr,
    pub rules: VecDeque<WplRule>,
    pub tags: Option<AnnFun>,
}

impl WplPackage {
    pub(crate) fn append(&mut self, p0: Vec<WplRule>) {
        for i in p0 {
            self.rules.push_back(i);
        }
    }
}

impl WplPackage {
    pub fn export_tags(&self) -> Option<Vec<WplTag>> {
        self.tags.as_ref().map(|x| x.export_tags())
    }
}

#[derive(Serialize, Deserialize)]
pub struct WplPkgMeta {
    pub name: SmolStr,
    pub rules: Vec<WplRuleMeta>,
}

impl From<&WplPackage> for WplPkgMeta {
    fn from(value: &WplPackage) -> Self {
        let mut rules = Vec::new();
        let pkg_tags = value.export_tags();
        for i in &value.rules {
            let mut r_stat = WplRuleMeta::from(i);
            if let Some(mut x) = pkg_tags.clone() {
                r_stat.tags.append(&mut x);
            }
            rules.push(r_stat);
        }
        Self {
            name: value.name.clone(),
            rules,
        }
    }
}

impl DebugFormat for WplPackage {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        if let Some(tag) = &self.tags {
            tag.write(w)?;
        }

        write!(w, "package {} ", self.name)?;
        self.write_open_brace(w)?;
        self.write_new_line(w)?;

        for rule in &self.rules {
            rule.write(w)?;
            self.write_new_line(w)?;
        }
        self.write_close_brace(w)?;
        self.write_new_line(w)?;

        Ok(())
    }
}

impl Display for WplPackage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}

impl MergeTags for WplPackage {
    fn merge_tags(&mut self, _: &Option<AnnFun>) {
        self.rules.merge_tags(&self.tags);
    }
}

impl WplPackage {
    pub fn new<S: Into<SmolStr>>(name: S, rules: Vec<WplRule>) -> Self {
        let name = name.into();
        debug_assert!(!name.is_empty());

        Self {
            name,
            rules: VecDeque::from(rules),
            tags: None,
        }
    }

    pub fn parse(data: &mut &str, path: &str) -> Result<Self, WplCodeError> {
        let package = wpl_package
            .parse_next(data)
            .map_err(|e| WplCodeError::from_syntax(e, data, path))?;
        Ok(package)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}
