use crate::language::prelude::*;

pub const OML_CRATE_IN: &str = "in";
pub const OML_CRATE_OPTION: &str = "option";
#[derive(Default, Builder, Debug, Clone, PartialEq, Getters)]
pub struct FieldTake {
    #[builder(default)]
    pub get: Option<String>,
    #[builder(default)]
    pub option: Vec<String>,
    #[builder(default)]
    pub collect: Vec<String>,
    #[builder(default)]
    pub default_acq: Option<GenericAccessor>,
}

impl VarAccess for FieldTake {
    fn field_name(&self) -> &Option<String> {
        self.get()
    }
}

impl Display for FieldTake {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "take( ")?;
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
