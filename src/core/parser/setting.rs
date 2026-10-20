use std::sync::Arc;

use derive_getters::Getters;
use wp_stat::StatReq;

use crate::core::parser::global_tags::GlobalTags;

#[derive(Default, Getters, Clone)]
pub struct ParseOption {
    gen_msg_id: bool,
    gen_event_md5: bool,
    stat_req: Vec<StatReq>,
    /// 实例级全局 tag；启动时来自 `WP_GLOBAL_TAGS`（或 `.env`），启动后只读。
    global_tags: Option<Arc<GlobalTags>>,
}

impl ParseOption {
    pub fn new(gen_msg_id: bool, gen_event_md5: bool, stat_req: Vec<StatReq>) -> Self {
        Self {
            gen_msg_id,
            gen_event_md5,
            stat_req,
            global_tags: None,
        }
    }

    pub fn with_global_tags(mut self, global_tags: Arc<GlobalTags>) -> Self {
        self.global_tags = Some(global_tags);
        self
    }
}
