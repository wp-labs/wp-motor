use std::cmp::Ordering;
use std::net::IpAddr;

pub use bool::BoolP;
pub use chars::CharsP;
pub use ignore::IgnoreP;
pub use symbol::PeekSymbolP;
pub use symbol::SymbolP;
use wp_model_core::model::DigitValue;

mod bool;
mod chars;
pub mod digit;
pub mod hex;
mod ignore;
mod symbol;

#[derive(Eq, PartialEq, Debug, Default)]
#[allow(dead_code)]
pub struct DigitRange {
    pub beg: DigitValue,
    pub end: DigitValue,
}

impl PartialOrd<Self> for DigitRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DigitRange {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.beg.cmp(&other.beg)
    }
}

#[derive(Eq, PartialEq, Debug)]
#[allow(dead_code)]
pub struct IpRange {
    pub beg: IpAddr,
    pub end: IpAddr,
}

impl PartialOrd<Self> for IpRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IpRange {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.beg.cmp(&other.beg)
    }
}
