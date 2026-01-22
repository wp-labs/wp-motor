use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::net::Ipv4Addr;

use orion_error::{ContextRecord, ErrorOwe, ErrorWith, WithContext};
use wp_model_core::model::FNameStr;

use crate::parser::error::WplCodeResult;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct FieldsGenRule {
    pub items: NamedFieldGF,
}

impl FieldsGenRule {
    pub fn load(path: &str) -> WplCodeResult<Self> {
        let mut ctx = WithContext::want("load gen rule");
        ctx.record("path", path);

        let content = std::fs::read_to_string(path).owe_conf().with(&ctx)?;
        let res: Self = toml::from_str(&content).owe_conf().with(&ctx)?;
        Ok(res)
    }
    pub fn new() -> Self {
        Self {
            items: NamedFieldGF::default(),
        }
    }
    pub fn add(&mut self, key: &str, value: FieldGenConf) {
        self.items.insert(key.into(), value);
    }
}

impl Default for FieldsGenRule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct FieldGenConf {
    pub scope: Option<GenScopeEnum>,
    pub gen_type: String,
    pub gen_fmt: Option<String>,
}

impl Default for FieldGenConf {
    fn default() -> Self {
        Self {
            scope: Some(GenScopeEnum::Digit(GenScope { beg: 0, end: 100 })),
            gen_type: "digit".to_string(),
            gen_fmt: Some("SN-{val}".to_string()),
        }
    }
}

impl FieldGenConf {
    /// Create a digit generator with a half-open range [beg, end).
    /// Example: FieldGenConf::digit_of(100, 4096)
    pub fn digit_of(beg: i64, end: i64) -> Self {
        Self {
            scope: Some(GenScopeEnum::Digit(GenScope { beg, end })),
            gen_type: "digit".to_string(),
            gen_fmt: None,
        }
    }

    /// Create a float generator with a half-open range [beg, end).
    pub fn float_of(beg: f64, end: f64) -> Self {
        Self {
            scope: Some(GenScopeEnum::Float(GenScope { beg, end })),
            gen_type: "float".to_string(),
            gen_fmt: None,
        }
    }

    /// Create an IPv4 range generator.
    pub fn ip_of(beg: Ipv4Addr, end: Ipv4Addr) -> Self {
        Self {
            scope: Some(GenScopeEnum::Ip(GenScope { beg, end })),
            gen_type: "ip".to_string(),
            gen_fmt: None,
        }
    }

    /// Create a chars generator that randomly picks from a fixed set of values.
    pub fn chars_from_list(values: Vec<String>) -> Self {
        Self {
            scope: Some(GenScopeEnum::Chars(values)),
            gen_type: "chars".to_string(),
            gen_fmt: None,
        }
    }

    /// Convenience helper for chars_from_list from &str slices.
    pub fn chars_from(values: &[&str]) -> Self {
        Self::chars_from_list(values.iter().map(|s| s.to_string()).collect())
    }

    /// Attach a render format (e.g., "SN-{val}") to the current generator.
    pub fn with_fmt(mut self, fmt: impl Into<String>) -> Self {
        self.gen_fmt = Some(fmt.into());
        self
    }
    pub fn example_1() -> Self {
        Self {
            scope: Some(GenScopeEnum::Digit(GenScope { beg: 100, end: 200 })),
            gen_type: "digit".to_string(),
            gen_fmt: None,
        }
    }
    pub fn example_2() -> Self {
        Self {
            scope: Some(GenScopeEnum::Digit(GenScope { beg: 0, end: 100 })),
            gen_type: "chars".to_string(),
            gen_fmt: Some("SN-{val}".to_string()),
        }
    }
    pub fn example_3() -> Self {
        Self {
            scope: Some(GenScopeEnum::Ip(GenScope {
                beg: Ipv4Addr::new(10, 0, 10, 0),
                end: Ipv4Addr::new(10, 0, 100, 255),
            })),
            gen_type: "ip".to_string(),
            gen_fmt: None,
        }
    }
}

impl Display for FieldGenConf {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.gen_type)?;
        if let Some(scope) = &self.scope {
            write!(f, ":rand{}", scope)?;
        }
        if let Some(fmt) = &self.gen_fmt {
            write!(f, " > {}", fmt)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GenScope<T>
where
    T: Clone + Debug + PartialEq + Display,
{
    pub beg: T,
    pub end: T,
}

impl<T> Display for GenScope<T>
where
    T: Clone + Debug + PartialEq + Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{})", self.beg, self.end)
    }
}

impl Display for GenScopeEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GenScopeEnum::Ip(scope) => write!(f, "{}", scope),
            GenScopeEnum::Digit(scope) => write!(f, "{}", scope),
            GenScopeEnum::Float(scope) => write!(f, "{}", scope),
            GenScopeEnum::Chars(scope) => write!(f, "{:?}", scope),
        }
    }
}

pub type NamedFieldGF = HashMap<FNameStr, FieldGenConf>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum GenScopeEnum {
    #[serde(rename = "ip")]
    Ip(GenScope<Ipv4Addr>),
    #[serde(rename = "digit")]
    Digit(GenScope<i64>),
    #[serde(rename = "float")]
    Float(GenScope<f64>),
    #[serde(rename = "chars")]
    Chars(Vec<String>),
}

/// Builder for NamedFieldGF: provides a friendly, chainable API to assemble field generators.
#[derive(Default, Debug, Clone)]
pub struct FieldGenBuilder {
    items: NamedFieldGF,
}

impl FieldGenBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self {
            items: NamedFieldGF::default(),
        }
    }

    /// Insert a raw config for `name`.
    pub fn insert_conf(mut self, name: impl Into<FNameStr>, conf: FieldGenConf) -> Self {
        self.items.insert(name.into(), conf);
        self
    }

    /// Add a digit field in range [beg, end).
    pub fn digit(self, name: impl Into<FNameStr>, beg: i64, end: i64) -> Self {
        self.insert_conf(name, FieldGenConf::digit_of(beg, end))
    }

    /// Add a float field in range [beg, end).
    pub fn float(self, name: impl Into<FNameStr>, beg: f64, end: f64) -> Self {
        self.insert_conf(name, FieldGenConf::float_of(beg, end))
    }

    /// Add an IPv4 field with [beg, end) range.
    pub fn ip(self, name: impl Into<FNameStr>, beg: Ipv4Addr, end: Ipv4Addr) -> Self {
        self.insert_conf(name, FieldGenConf::ip_of(beg, end))
    }

    /// Add a chars field with no predefined scope (free text generator).
    pub fn chars(self, name: impl Into<FNameStr>) -> Self {
        self.insert_conf(
            name,
            FieldGenConf {
                scope: None,
                gen_type: "chars".to_string(),
                gen_fmt: None,
            },
        )
    }

    /// Add a chars field that randomly picks from the provided set.
    pub fn chars_from(self, name: impl Into<FNameStr>, values: &[&str]) -> Self {
        self.insert_conf(name, FieldGenConf::chars_from(values))
    }

    /// Attach a format string for an existing field (no-op if the field does not exist).
    pub fn with_fmt(mut self, name: &str, fmt: impl Into<String>) -> Self {
        if let Some(conf) = self.items.get_mut(name) {
            conf.gen_fmt = Some(fmt.into());
        }
        self
    }

    /// Finalize and get the underlying map.
    pub fn build(self) -> NamedFieldGF {
        self.items
    }
}

impl From<FieldGenBuilder> for NamedFieldGF {
    fn from(b: FieldGenBuilder) -> Self {
        b.items
    }
}
