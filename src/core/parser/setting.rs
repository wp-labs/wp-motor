use derive_getters::Getters;
use wp_stat::StatReq;

#[derive(Default, Getters)]
pub struct ParseOption {
    gen_msg_id: bool,
    gen_event_md5: bool,
    stat_req: Vec<StatReq>,
}

impl ParseOption {
    pub fn new(gen_msg_id: bool, gen_event_md5: bool, stat_req: Vec<StatReq>) -> Self {
        Self {
            gen_msg_id,
            gen_event_md5,
            stat_req,
        }
    }
}
