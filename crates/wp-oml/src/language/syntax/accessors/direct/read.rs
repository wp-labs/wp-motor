use crate::language::GenericAccessor;
use crate::language::prelude::*;
use std::default::Default;
use wildmatch::WildMatch;

pub const OML_CRATE_IN: &str = "in";
pub const OML_CRATE_OPTION: &str = "option";

#[derive(Default, Builder, Debug, Clone, PartialEq, Getters)]
pub struct ReadOption {
    #[builder(default)]
    pub get: Option<String>,
    #[builder(default)]
    pub option: Vec<String>,
    #[builder(default)]
    pub collect: Vec<String>,
    #[builder(default)]
    pub default_acq: Option<GenericAccessor>,
}

#[derive(Default, Debug, Clone, PartialEq, Getters)]
pub struct FieldRead {
    //#[builder(default)]
    pub get: Option<String>,
    //#[builder(default)]
    pub option: Vec<String>,
    //#[builder(default)]
    pub collect: Vec<String>,
    //#[builder(default)]
    pub default_acq: Option<GenericAccessor>,
    collect_wild: Vec<WildMatch>,
}
impl From<ReadOption> for FieldRead {
    fn from(value: ReadOption) -> Self {
        let mut collect_wild = Vec::new();
        for collect in &value.collect {
            collect_wild.push(WildMatch::new(collect.as_str()));
        }
        Self {
            get: value.get,
            option: value.option,
            collect: value.collect,
            default_acq: value.default_acq,
            collect_wild,
        }
    }
}

impl FieldRead {
    pub fn new(name: String) -> Self {
        Self {
            get: Some(name),
            ..Default::default()
        }
    }
}
impl VarAccess for FieldRead {
    fn field_name(&self) -> &Option<String> {
        self.get()
    }
}

impl Display for FieldRead {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "read( ")?;
        if let Some(g) = &self.get {
            write!(f, "{}", g)?;
        }
        let len = self.option.len();
        for (i, x) in self.option.iter().enumerate() {
            if i == 0 {
                write!(f, "{}: [ ", OML_CRATE_OPTION)?;
                write!(f, "{}", x)?;
            } else {
                write!(f, ", {}", x)?;
            }
            if i == len - 1 {
                write!(f, " ]",)?;
            }
        }
        let len = self.collect.len();
        for (i, x) in self.collect.iter().enumerate() {
            if i == 0 {
                write!(f, "{}: [ ", OML_CRATE_IN)?;
                write!(f, "{}", x)?;
            } else {
                write!(f, ", {}", x)?;
            }
            if i == len - 1 {
                write!(f, " ]",)?;
            }
        }
        write!(f, " )")?;
        if let Some(default_x) = &self.default_acq {
            writeln!(f, "{{")?;
            writeln!(f, "_ :  {}", default_x)?;
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}
