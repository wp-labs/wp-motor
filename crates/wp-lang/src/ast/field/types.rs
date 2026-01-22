use crate::ast::WplPipe;
use crate::ast::debug::DebugFormat;
use crate::ast::fld_fmt::WplFieldFmt;
use crate::ast::syntax::wpl_sep::WplSep;
use crate::parser::wpl_field::wpl_field;
use crate::types::WildMap;
use derive_getters::Getters;
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataType, MetaErr};
use wp_parser::Parser;
use wp_parser::WResult;

pub const DEFAULT_FIELD_KEY: &str = "*";
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct WplField {
    pub meta_type: DataType,
    pub meta_name: FNameStr,
    pub name: Option<FNameStr>,
    pub content: Option<String>,
    pub fmt_conf: WplFieldFmt,
    pub continuous: bool,
    pub continuous_cnt: Option<usize>,
    pub length: Option<usize>,
    pub sub_fields: Option<WplFieldSet>,
    pub enriches: Vec<EnrichConf>,
    pub desc: String,
    pub pipe: Vec<WplPipe>,
    pub is_opt: bool,
    pub take_sep: bool,
    pub separator: Option<WplSep>,
}
impl WplField {
    pub fn scope_conf(&self) -> (&Option<String>, &Option<String>) {
        (&self.fmt_conf.scope_beg, &self.fmt_conf.scope_end)
    }
    pub fn safe_name(&self) -> FNameStr {
        self.name.clone().unwrap_or_else(|| self.meta_name.clone())
    }
    pub fn field_cnt(&self) -> Option<usize> {
        self.fmt_conf.field_cnt
    }
    pub fn have_scope(&self) -> bool {
        self.fmt_conf.scope_beg.is_some() && self.fmt_conf.scope_end.is_some()
    }
    pub fn run_key(&self, key: &str) -> Option<FNameStr> {
        self.name().clone().or(Some(FNameStr::from(key)))
    }
    pub fn run_key_str(&self, key: &str) -> FNameStr {
        self.name().clone().unwrap_or_else(|| FNameStr::from(key))
    }
    /*
    pub fn use_sep(&mut self, sep: PrioSep) {
        self.fmt_conf.use_sep(sep.clone());
        if let Some(sub_fields) = &mut self.sub_fields {
            for (_, ref mut sub) in sub_fields.conf_items.exact_iter_mut() {
                sub.use_sep(sep.clone())
            }
            for (_, _, ref mut sub) in sub_fields.conf_items.wild_iter_mut() {
                sub.use_sep(sep.clone())
            }
        }
    }

     */
    pub fn resolve_sep(&self, ups: &WplSep) -> WplSep {
        if let Some(cur) = &self.separator {
            let mut combo = cur.clone();
            combo.override_with(ups);
            combo
        } else {
            ups.clone()
        }
    }

    pub fn resolve_sep_ref<'a>(&'a self, ups: &'a WplSep) -> Cow<'a, WplSep> {
        if self.separator.is_some() {
            Cow::Owned(self.resolve_sep(ups))
        } else {
            Cow::Borrowed(ups)
        }
    }
}

impl Display for WplField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            (self, &None, &None).fmt_string().unwrap_or("".to_string())
        )
    }
}

pub const DEFAULT_META_NAME: &str = "auto";

impl Default for WplField {
    fn default() -> Self {
        WplField {
            meta_type: DataType::Auto,
            meta_name: DEFAULT_META_NAME.into(),
            fmt_conf: WplFieldFmt::default(),
            name: None,
            content: None,
            continuous: false,
            continuous_cnt: None,
            length: None,
            sub_fields: None,
            enriches: Vec::new(),
            desc: String::new(),
            pipe: Vec::new(),
            is_opt: false,
            take_sep: true,
            separator: None,
        }
    }
}

impl WplField {
    /*
    #[inline]
    pub fn sep_tag(&self) -> &str {
        if let Some(sep) = &self.fmt_conf().separator {
            sep.val()
        } else {
            " "
        }
    }
     */

    pub fn new(meta: &str) -> Result<Self, MetaErr> {
        let ins = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            ..Default::default()
        };
        ins.validate();
        Ok(ins)
    }

    pub fn sub_for_arr(meta: &str) -> Result<Self, MetaErr> {
        let ins = WplField {
            meta_type: DataType::from(meta)?,
            meta_name: meta.into(),
            fmt_conf: WplFieldFmt::default(),
            take_sep: false,
            ..Default::default()
        };
        ins.validate();
        Ok(ins)
    }
    pub fn name_default(name: &str) -> Self {
        Self {
            name: Some(name.into()),
            ..Default::default()
        }
    }

    pub fn try_parse(conf: &str) -> WResult<Self> {
        let (_, mut ins) = wpl_field.parse_peek(conf)?;
        ins.setup();
        Ok(ins)
    }
    pub fn setup(&mut self) {
        self.validate();
        self.build_desc();
    }
    fn validate(&self) {
        match &self.meta_type {
            DataType::Chars | DataType::Ignore => {}
            DataType::Digit => {
                if self.fmt_conf.field_cnt.is_some() {
                    panic!("meta type {:?} can not have field_cnt", self.meta_type);
                }
            }
            _ => {
                if self.fmt_conf.field_cnt.is_some() {
                    //TODO fix
                    panic!("meta type {:?} can not have field_cnt", self.meta_type);
                }
            }
        }
    }
    pub fn build_desc(&mut self) {
        self.desc = self.to_string();
    }
    pub fn build(mut self) -> Self {
        self.build_desc();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default, Getters)]
pub struct WplFieldSet {
    conf_items: WildMap<WplField>,
}
impl From<WildMap<WplField>> for WplFieldSet {
    fn from(value: WildMap<WplField>) -> Self {
        Self { conf_items: value }
    }
}

impl WplFieldSet {
    pub fn get(&self, path: &str) -> Option<&WplField> {
        self.conf_items.get(path)
    }
    pub fn add<S: Into<String>>(&mut self, path: S, conf: WplField) {
        self.conf_items.insert(path.into(), conf);
    }
}

impl Display for WplFieldSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fmt_string().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{WplField, WplFieldSet};
    use crate::types::AnyResult;

    #[test]
    fn test_get_conf() -> AnyResult<()> {
        let mut confs = WplFieldSet::default();
        confs.add("a", WplField::name_default("1"));
        confs.add("b", WplField::name_default("2"));
        confs.add("x/*", WplField::name_default("3"));
        confs.add("x/a", WplField::name_default("4"));
        confs.add("y/z/*", WplField::name_default("5"));
        confs.add("y/z/1/2", WplField::name_default("6"));

        assert_eq!(confs.get("a"), Some(&WplField::name_default("1")));
        assert_ne!(confs.get("a"), Some(&WplField::name_default("2")));
        assert_eq!(confs.get("x/"), Some(&WplField::name_default("3")));
        assert_eq!(confs.get("x/b"), Some(&WplField::name_default("3")));
        assert_eq!(confs.get("x/a"), Some(&WplField::name_default("4")));
        assert_eq!(confs.get("y/z/1"), Some(&WplField::name_default("5")));
        assert_eq!(confs.get("y/z/1/2"), Some(&WplField::name_default("6")));
        assert_eq!(confs.get("y/x/1"), None);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EnrichConf {
    pub key: String,
    pub dict: String,
    pub adds: Vec<(String, String)>,
}
