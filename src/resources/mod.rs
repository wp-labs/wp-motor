pub mod core;
pub mod indexing;
pub mod sinks;
pub mod utils;
pub use core::types::RuleKey;
pub use core::types::{ModelName, SinkID};
pub use indexing::model_index::SinkModelIndex;
pub use indexing::rule_index::SinkRuleRegistry;
pub use sinks::null_resources::AssignRes;
pub use sinks::sink_resources::SinkResUnit;

pub use core::manager::OmlRepository;
pub use core::manager::ResManager;
