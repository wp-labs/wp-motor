use std::{fmt::Display, io::Write};

use crate::{
    WplSep,
    ast::{
        DEFAULT_FIELD_KEY, WplField, WplFieldFmt,
        debug::{DebugFormat, DepIndent},
    },
};

use super::types::WplFieldSet;

impl DebugFormat for WplFieldSet {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let item_count = self.conf_items().len();

        for (index, x) in self.conf_items().exact_iter().enumerate() {
            if item_count == 1 && index == 0 {
                self.write_open_parenthesis(w)?;
                let set = x.clone();
                FieldConfSetWrap(set.0, set.1).write(w)?;
                self.write_close_parenthesis(w)?;
                return Ok(());
            }

            if index == 0 {
                self.write_open_parenthesis(w)?;
            }
            let set = x.clone();
            FieldConfSetWrap(set.0, set.1).write(w)?;
            write!(w, ",")?;

            if index == self.conf_items().len() - 1 {
                self.write_close_parenthesis(w)?;
            }
        }
        for (index, x) in self.conf_items().wild_iter().enumerate() {
            if item_count == 1 && index == 0 {
                self.write_open_parenthesis(w)?;
                let set = x.clone();
                FieldConfSetWrap(set.0, set.2).write(w)?;
                self.write_close_parenthesis(w)?;
                return Ok(());
            }

            if index == 0 {
                self.write_open_parenthesis(w)?;
            }
            let set = x.clone();
            FieldConfSetWrap(set.0, set.2).write(w)?;
            write!(w, ",")?;

            if index == self.conf_items().len() - 1 {
                self.write_close_parenthesis(w)?;
            }
        }

        Ok(())
    }
}

impl DebugFormat for (&WplFieldFmt, &Option<WplSep>) {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let conf = self.0;
        if let (Some(beg), Some(end)) = (&conf.scope_beg, &conf.scope_end) {
            if beg == end && beg == "\"" {
                write!(w, "\"")?;
            } else {
                write!(w, "<")?;
                for c in beg.chars() {
                    if c == ',' || c == '\\' {
                        write!(w, "\\{}", c)?;
                    } else {
                        write!(w, "{}", c)?;
                    }
                }

                write!(w, ",")?;

                for c in end.chars() {
                    if c == ',' || c == '\\' {
                        write!(w, "\\{}", c)?;
                    } else {
                        write!(w, "{}", c)?;
                    }
                }
                write!(w, ">")?;
            }
        }

        /*
        if conf.patten_first.is_some() {
            write!(w, "~")?;
        }
         */

        if let Some(cnt) = conf.field_cnt {
            write!(w, "^{}", cnt)?;
        }

        //write!(w, "{}", WPLFmt(&self.1))?;

        Ok(())
    }
}

pub struct FieldConfSetWrap(pub String, pub WplField);
impl DebugFormat for FieldConfSetWrap {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let key = &self.0;
        let conf = &self.1;
        if conf.meta_name != "chars" {
            write!(w, "{}", conf.meta_name)?;
        }

        if key != DEFAULT_FIELD_KEY {
            write!(w, "@{}", key)?;
        }

        if let Some(name) = &conf.name {
            write!(w, ":{}", name)?;
        }

        write!(w, "{}", conf.fmt_conf)?;

        Ok(())
    }
}

impl Display for FieldConfSetWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or("".to_string()))
    }
}
