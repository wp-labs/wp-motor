use std::fmt::{Display, Formatter};
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ModelName(pub(crate) String);

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RuleKey(pub String);
impl Display for RuleKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for RuleKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for ModelName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for SinkID {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
impl Display for SinkID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&String> for SinkID {
    fn from(value: &String) -> Self {
        Self(value.to_string())
    }
}

impl From<&String> for RuleKey {
    fn from(value: &String) -> Self {
        Self(value.to_string())
    }
}

impl Display for ModelName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&String> for ModelName {
    fn from(value: &String) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SinkID(pub(crate) String);
