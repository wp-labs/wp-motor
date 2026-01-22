use crate::ast::debug::{DebugFormat, DepIndent};
use crate::ast::{DEFAULT_META_NAME, WplField};
use derive_getters::Getters;
use std::fmt::{Display, Formatter};
use std::io::Write;
use wp_model_core::model::fmt_def::TextFmt;

use super::WplSep;

impl DebugFormat for (&WplField, &Option<usize>, &Option<WplSep>) {
    fn write<W>(&self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write + DepIndent,
    {
        let field_conf = self.0;
        let base_group_sep = self.2;
        if let Some(cnt) = field_conf.continuous_cnt {
            write!(w, "{}", cnt)?;
        }

        if field_conf.continuous {
            write!(w, "*")?;

            if field_conf.meta_name != DEFAULT_META_NAME {
                write!(w, "{}", field_conf.meta_name)?;
            }
        } else {
            write!(w, "{}", field_conf.meta_name)?;
        }

        if let Some(content) = &field_conf.content {
            write!(w, "({})", content)?;
        }

        if let Some(sub_fileds) = &field_conf.sub_fields {
            sub_fileds.write(w)?;
        }

        if let Some(name) = &field_conf.name {
            write!(w, ":{}", name)?;
        }
        if let Some(len) = field_conf.length {
            write!(w, "[{}]", len)?;
        }
        (&field_conf.fmt_conf, base_group_sep).write(w)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Default, Getters, Serialize, Deserialize)]
pub struct WplFieldFmt {
    pub scope_beg: Option<String>,
    pub scope_end: Option<String>,
    pub field_cnt: Option<usize>,
    pub sub_fmt: Option<TextFmt>,
}

impl WplFieldFmt {
    pub fn sub_case_new(beg: &str, end: &str) -> Self {
        Self {
            //patten_first: Some(true),
            scope_beg: Some(beg.into()),
            scope_end: Some(end.into()),
            ..Default::default()
        }
    }
}

impl Display for WplFieldFmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (self, &None).fmt_string().unwrap_or_default())
    }
}

#[cfg(test)]
pub mod for_test {
    use super::*;
    use wp_model_core::model::{DataType, MetaErr};

    pub fn fdc2(meta: &str, sep: &str) -> Result<WplField, MetaErr> {
        let mut conf = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            name: Some(meta.into()),
            fmt_conf: WplFieldFmt {
                ..Default::default()
            },
            separator: Some(WplSep::field_sep(sep)),
            ..Default::default()
        };
        conf.setup();
        Ok(conf)
    }

    pub fn fdc3_1(meta: &str, content: &str, sep: &str) -> Result<WplField, MetaErr> {
        let mut conf = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            name: Some(meta.into()),
            content: Some(content.to_string()),
            separator: Some(WplSep::field_sep(sep)),
            fmt_conf: WplFieldFmt {
                ..Default::default()
            },
            ..Default::default()
        };
        conf.setup();
        Ok(conf)
    }

    pub fn fdc2_1(meta: &str, fmt: WplFieldFmt) -> Result<WplField, MetaErr> {
        let mut conf = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            fmt_conf: fmt,
            ..Default::default()
        };
        conf.setup();
        Ok(conf)
    }

    pub fn fdc3(meta: &str, sep: &str, continuous: bool) -> Result<WplField, MetaErr> {
        let mut conf = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            //name: Some(meta.to_string()),
            fmt_conf: WplFieldFmt {
                ..Default::default()
            },
            continuous,
            separator: Some(WplSep::field_sep(sep)),
            ..Default::default()
        };
        conf.setup();
        Ok(conf)
    }

    pub fn fdc4_1(
        meta: &str,
        sep: &str,
        continuous: bool,
        cnt: usize,
    ) -> Result<WplField, MetaErr> {
        let mut conf = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            fmt_conf: WplFieldFmt {
                ..Default::default()
            },
            continuous,
            continuous_cnt: Some(cnt),
            separator: Some(WplSep::field_sep(sep)),
            ..Default::default()
        };
        conf.setup();
        Ok(conf)
    }
}
