use std::{
    collections::VecDeque,
    fmt::{Display, Formatter},
    io::Write,
};

use contracts::debug_requires;
use derive_getters::Getters;
use orion_overload::new::New1;
use smol_str::SmolStr;

use crate::{
    ast::{
        AnnFun, WplField,
        debug::{DebugFormat, DepIndent},
        group::WplGroup,
    },
    parser::MergeTags,
};

#[derive(Default, Clone, Getters, Debug)]
pub struct WplRule {
    pub name: SmolStr,
    pub statement: WplStatementType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum WplStatementType {
    Express(WplExpress),
}

impl DebugFormat for WplStatementType {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        match self {
            WplStatementType::Express(define) => define.write(w),
        }
    }
}

impl Display for WplStatementType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}

impl Default for WplStatementType {
    fn default() -> Self {
        WplStatementType::Express(WplExpress::default())
    }
}

/*
impl From<Vec<WPLField>> for WPLStatement {
    fn from(fields: Vec<WPLField>) -> Self {
        WPLStatement::Express(STMExpress::new(fields))
    }
}

 */

impl WplStatementType {
    pub fn first_group(&self) -> Option<&WplGroup> {
        match self {
            WplStatementType::Express(rule) => rule.group.first(),
        }
    }
    pub fn first_field(&self) -> Option<&WplField> {
        match self {
            WplStatementType::Express(rule) => rule.group.first().and_then(|x| x.first()),
        }
    }

    pub fn tags(&self) -> &Option<AnnFun> {
        match self {
            WplStatementType::Express(rule) => &rule.tags,
        }
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
pub struct WplExpress {
    // 管道预处理
    pub pipe_process: Vec<SmolStr>,
    pub group: Vec<WplGroup>,
    pub tags: Option<AnnFun>,
}

impl MergeTags for WplExpress {
    fn merge_tags(&mut self, other_tags: &Option<AnnFun>) {
        if let Some(tags) = &mut self.tags {
            tags.merge_tags(other_tags)
        } else {
            self.tags = other_tags.clone()
        }
    }
}

impl Display for WplExpress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}

impl DebugFormat for WplExpress {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let depth = w.add_indent();
        for (index, pipe) in self.pipe_process.iter().enumerate() {
            if index == 0 {
                self.write_indent(w, depth)?;
                write!(w, "|")?;
            }
            write!(w, "{}|", pipe)?;
        }

        for (index, field) in self.group.iter().enumerate() {
            if index != 0 {
                write!(w, ",")?;
                self.write_new_line(w)?;
            }
            self.write_indent(w, depth)?;
            field.write(w)?;
        }
        w.sub_indent();
        Ok(())
    }
}

impl New1<Vec<WplGroup>> for WplExpress {
    fn new(group: Vec<WplGroup>) -> Self {
        WplExpress {
            pipe_process: Vec::new(),
            group,
            tags: None,
        }
    }
}

impl New1<Vec<WplField>> for WplExpress {
    fn new(fields: Vec<WplField>) -> Self {
        let group = vec![WplGroup::new(fields)];
        WplExpress {
            pipe_process: Vec::new(),
            group,
            tags: None,
        }
    }
}

impl WplRule {
    pub fn add_tags(mut self, tags: Option<AnnFun>) -> Self {
        match self.statement {
            WplStatementType::Express(mut define) => {
                define.tags = tags;
                self.statement = WplStatementType::Express(define);
                self
            }
        }
    }
}

impl DebugFormat for WplRule {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let depth = w.add_indent();

        if let Some(tags) = &self.statement.tags() {
            self.write_indent(w, depth)?;
            tags.write(w)?;
        }
        self.write_indent(w, depth)?;

        write!(w, "rule {} ", &self.name)?;
        self.write_open_brace(w)?;
        self.write_new_line(w)?;
        self.statement.write(w)?;
        self.write_new_line(w)?;
        self.write_indent(w, depth)?;
        self.write_close_brace(w)?;
        w.sub_indent();
        Ok(())
    }
}

impl Display for WplRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}

impl PartialEq for WplRule {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.statement == other.statement
    }
}

impl WplRule {
    #[debug_requires(! name.is_empty(), "lang rule name is empty")]
    pub fn new(name: String, rule: WplStatementType) -> Self {
        WplRule {
            name: SmolStr::from(name),
            statement: rule,
        }
    }

    pub fn get_name(&self) -> &SmolStr {
        &self.name
    }
    pub fn path(&self, pkg_name: &str) -> String {
        format!("{}/{}", pkg_name, self.get_name())
    }
}

impl MergeTags for VecDeque<WplRule> {
    fn merge_tags(&mut self, other_tags: &Option<AnnFun>) {
        for r in self.iter_mut() {
            match &mut r.statement {
                WplStatementType::Express(define) => {
                    if let Some(tags) = &mut define.tags {
                        tags.merge_tags(other_tags);
                    } else {
                        define.tags = other_tags.clone()
                    }
                }
            }
        }
    }
}

#[test]
fn test_lang_rule() {
    let rule = WplRule {
        statement: WplStatementType::Express(WplExpress {
            pipe_process: vec![SmolStr::from("decode/base64"), SmolStr::from("zip")],
            group: vec![],
            tags: None,
        }),
        name: SmolStr::from("hello"),
    };
    assert_eq!(
        rule.to_string(),
        "  rule hello {\n    |decode/base64|zip|\n  }"
    );
}
