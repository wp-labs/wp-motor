use crate::language::prelude::*;

pub const PIPE_IP4_TO_INT: &str = "ip4_to_int";

#[derive(Clone, Debug, Default)]
pub struct Ip4ToInt {}

impl Display for Ip4ToInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_IP4_TO_INT)
    }
}
