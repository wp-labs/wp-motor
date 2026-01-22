pub mod mergeable;
pub mod recorder;

/// Trait for merging two instances of the same type
pub trait Mergeable<T> {
    /// Merges another instance into this one
    fn merge(&mut self, other: T);
}

/// Trait for tracking metrics on sliced data
pub trait SliceMetrics {
    /// Returns the unique key for this slice
    fn slices_key(&self) -> &str;

    /// Adds metrics from another slice to this one
    fn add(&mut self, other: &Self);

    /// Records an incoming event
    fn rec_in(&mut self);

    /// Records a successful completion
    fn rec_suc(&mut self);

    /// Records the end of an event
    fn rec_end(&mut self);

    /// Returns the total count of events
    fn get_total(&self) -> u64;
}

/// Types of statistical slices available in the system
pub enum SlicesType {
    Lib = 1,
    Pick = 2,
    Parse = 3,
    Sink = 4,
    SGroup = 5,
    Rule = 6,
    Diy = 7,
}
impl AsRef<str> for SlicesType {
    fn as_ref(&self) -> &str {
        match self {
            SlicesType::Lib => "lib",
            SlicesType::Pick => "pick",
            SlicesType::Parse => "parse",
            SlicesType::Sink => "sink",
            SlicesType::SGroup => "sgroup",
            SlicesType::Rule => "gen",
            SlicesType::Diy => "diy",
        }
    }
}

/// Metadata trait for slices, providing type and naming information
pub trait SlicesMetadata {
    /// Returns the type of this slice
    fn slices_type() -> SlicesType;

    /// Returns the name of this slice
    fn slices_name() -> String;

    /// Optional first tag name for additional categorization
    fn tag1_name() -> Option<String> {
        None
    }

    /// Optional second tag name for additional categorization
    fn tag2_name() -> Option<String> {
        None
    }
}
