pub(crate) mod indexing;
pub(crate) mod setting;
//pub(crate) mod plg_pipes;
pub(crate) mod wpl_engine;

pub(crate) use indexing::ResourceIndexer;
pub(crate) use setting::ParseOption;
// Re-export local implementations to keep public API aligned with internal types
pub(crate) use self::wpl_engine::WplEngine;
pub(crate) use self::wpl_engine::pipeline::WplPipeline;
pub(crate) use self::wpl_engine::repo::{SpaceIndex, WplRepository};
