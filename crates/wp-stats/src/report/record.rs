use std::cmp::Ordering;

use crate::{
    SliceRecord,
    traits::{SlicesMetadata, SlicesType},
};

pub type StatRecord = SliceRecord<WpStatTag>;
impl Eq for SliceRecord<WpStatTag> {}
impl PartialEq<Self> for SliceRecord<WpStatTag> {
    fn eq(&self, other: &Self) -> bool {
        self.stat.total == other.stat.total
    }
}

impl PartialOrd<Self> for SliceRecord<WpStatTag> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for SliceRecord<WpStatTag> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.stat.total.cmp(&other.stat.total)
    }
}

#[derive(Clone, Default, Debug, Copy)]
pub struct WpStatTag {}

impl SlicesMetadata for WpStatTag {
    fn slices_type() -> SlicesType {
        SlicesType::Diy
    }
    fn slices_name() -> String {
        "diy".to_string()
    }
}
