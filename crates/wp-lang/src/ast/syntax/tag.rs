use std::collections::BTreeMap;
use std::io::Write;

use smol_str::SmolStr;
use winnow::stream::Accumulate;

use crate::ast::WplTag;
use crate::ast::debug::{DebugFormat, DepIndent};
use crate::parser::MergeTags;

pub type TagKvs = BTreeMap<SmolStr, SmolStr>;
pub type CopyRaw = (SmolStr, SmolStr);

#[derive(Debug, PartialEq, Clone)]
pub enum AnnEnum {
    Tags(TagKvs),
    Copy(CopyRaw),
}
#[derive(Debug, PartialEq, Default, Clone)]
pub struct AnnFun {
    pub tags: TagKvs,
    pub copy_raw: Option<CopyRaw>,
}

impl MergeTags for AnnFun {
    fn merge_tags(&mut self, other_tags: &Option<AnnFun>) {
        if let Some(atags) = other_tags {
            for (other_k, other_v) in &atags.tags {
                if self.tags.get_mut(other_k).is_none() {
                    self.tags.insert(other_k.clone(), other_v.clone());
                }
            }

            if self.copy_raw.is_none() {
                self.copy_raw = atags.copy_raw.clone()
            }
        }
    }
}

impl AnnFun {
    pub fn export_tags(&self) -> Vec<WplTag> {
        let mut tags = Vec::new();
        for (k, v) in &self.tags {
            tags.push(WplTag::new(k.clone(), v.clone()))
        }
        tags
    }
}

impl DebugFormat for AnnFun {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        write!(w, "#[tag")?;
        let tag_count = self.tags.len();

        for (index, t) in self.tags.iter().enumerate() {
            if tag_count == 1 && index == 0 {
                self.write_open_parenthesis(w)?;
                write!(w, "{}:\"{}\"", t.0, t.1)?;
                self.write_close_parenthesis(w)?;

                write!(w, "]")?;
                self.write_new_line(w)?;
                return Ok(());
            }
            if index == 0 {
                self.write_open_parenthesis(w)?;
            }
            write!(w, "{}:\"{}\"", t.0, t.1)?;

            if index == tag_count - 1 {
                self.write_close_parenthesis(w)?;
            } else {
                write!(w, ", ")?;
            }
        }

        if let Some((ck, cv)) = &self.copy_raw {
            write!(w, ", copy_raw")?;
            self.write_open_parenthesis(w)?;
            write!(w, "{}:\"{}\"", ck, cv)?;
            self.write_close_parenthesis(w)?;
        }
        write!(w, "]")?;
        self.write_new_line(w)?;
        Ok(())
    }
}

impl Accumulate<()> for AnnFun {
    fn initial(_: Option<usize>) -> Self {
        AnnFun::default()
    }

    fn accumulate(&mut self, _acc: ()) {}
}
