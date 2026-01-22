use crate::sinks::{prelude::*, utils::formatter::gen_fmt_dat};
use wp_model_core::model::fmt_def::TextFmt;
use wp_parse_api::RawData;

use crate::types::AnyResult;

use super::utils::formatter::fds_fmt_proc;

pub trait TDMDataAble {
    fn cov_data(&self, tdo: DataRecord) -> AnyResult<RawData>;
    fn gen_data(&self, data: FmtFieldVec) -> AnyResult<RawData>;
}

impl TDMDataAble for TextFmt {
    fn cov_data(&self, tdo: DataRecord) -> AnyResult<RawData> {
        fds_fmt_proc(*self, tdo)
    }
    fn gen_data(&self, data: FmtFieldVec) -> AnyResult<RawData> {
        gen_fmt_dat(*self, data)
    }
}
