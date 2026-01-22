use orion_overload::new::New1;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::str::FromStr;

use crate::ast::debug::{DebugFormat, DepIndent};
use crate::ast::syntax::wpl_sep::WplSep;
use crate::ast::{WplField, WplFmt};

#[derive(Debug, PartialEq, Clone)]
pub enum WplGroupType {
    Opt(GroupOpt),
    Seq(GroupSeq),
    Alt(GroupAlt),
    SomeOf(GroupSomeOf),
}
impl Default for WplGroupType {
    fn default() -> Self {
        WplGroupType::Seq(GroupSeq)
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
pub struct GroupSeq;
#[derive(Default, Debug, PartialEq, Clone)]
pub struct GroupOpt;

#[derive(Default, Debug, PartialEq, Clone)]
pub struct GroupAlt;

#[derive(Default, Debug, PartialEq, Clone)]
pub struct GroupSomeOf;

impl Display for WplGroupType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WplGroupType::Opt(_) => {
                write!(f, "opt")?;
            }
            WplGroupType::Seq(_) => {
                write!(f, "seq")?;
            }
            WplGroupType::Alt(_) => {
                write!(f, "alt")?;
            }
            WplGroupType::SomeOf(_) => {
                write!(f, "some_of")?;
            }
        }
        Ok(())
    }
}
impl FromStr for WplGroupType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "opt" => Ok(WplGroupType::Opt(GroupOpt)),
            "order" | "seq" => Ok(WplGroupType::Seq(GroupSeq)),
            "alt" => Ok(WplGroupType::Alt(GroupAlt)),
            "some_of" => Ok(WplGroupType::SomeOf(GroupSomeOf)),
            _ => Err(()),
        }
    }
}
#[derive(Default, Debug, PartialEq, Clone)]
pub struct WplGroup {
    pub meta: WplGroupType,
    pub fields: Vec<WplField>,
    pub base_group_sep: Option<WplSep>,
    pub base_group_len: Option<usize>,
}

impl New1<Vec<WplField>> for WplGroup {
    fn new(fields: Vec<WplField>) -> Self {
        Self {
            fields,
            ..Default::default()
        }
    }
}

impl WplGroup {
    pub fn first(&self) -> Option<&WplField> {
        self.fields.first()
    }
    pub fn meta_from(&mut self, meta_str: Option<&str>) {
        if let Some(data) = meta_str
            && let Ok(meta) = WplGroupType::from_str(data)
        {
            self.meta = meta;
            return;
        }
        self.meta = WplGroupType::Seq(GroupSeq);
    }
    pub fn resolve_sep(&self, ups: &WplSep) -> WplSep {
        if let Some(cur) = &self.base_group_sep {
            let mut combo = cur.clone();
            combo.override_with(ups);
            combo
        } else {
            ups.clone()
        }
    }
}

impl DebugFormat for WplGroup {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        if self.meta != WplGroupType::Seq(GroupSeq) {
            write!(w, "{}", self.meta)?;
        }
        self.write_open_parenthesis(w)?;
        self.write_new_line(w)?;
        let depth = w.add_indent();
        for (index, field) in self.fields.iter().enumerate() {
            if index != 0 {
                write!(w, ",")?;
                self.write_new_line(w)?;
            }
            self.write_indent(w, depth)?;
            (field, &self.base_group_len, &self.base_group_sep).write(w)?;
        }
        self.write_new_line(w)?;
        let depth = w.sub_indent();
        self.write_indent(w, depth)?;
        self.write_close_parenthesis(w)?;
        if let Some(len) = self.base_group_len {
            write!(w, "[{}]", len)?;
        }

        if let Some(sep) = &self.base_group_sep {
            write!(w, "{}", WplFmt(sep))?;
        }

        Ok(())
    }
}

impl Display for WplGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}
