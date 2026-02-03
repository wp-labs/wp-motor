use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::generator::{FieldGenConf, GenChannel};
use crate::types::AnyResult;
use crate::winnow::Parser;
use winnow::combinator::alt;
use winnow::stream::Stream as _;
use winnow::token::take;
use wp_model_core::model::DataField;
use wp_model_core::model::FNameStr;
use wp_parser::WResult;

#[derive(Default)]
pub struct TimeStampPSR {}

impl PatternParser for TimeStampPSR {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _: &FieldEvalUnit,
        _: &crate::ast::WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> WResult<()> {
        let dt = alt((parse_timestamp_us, parse_timestamp_ms, parse_timestamp)).parse_next(data)?;
        out.push(DataField::from_time(name, dt.naive_local()));
        Ok(())
    }
    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &crate::ast::WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        super::gen_time(gnc, f_conf, g_conf)
    }
}

// ----- helpers moved from old time.rs -----
fn parse_timestamp(data: &mut &str) -> WResult<chrono::DateTime<chrono::Utc>> {
    let ts_s = take(10usize).parse_next(data)?;
    if let Ok(Some(dt)) = ts_s.parse().map(|x| chrono::DateTime::from_timestamp(x, 0)) {
        Ok(dt)
    } else {
        let cp = (*data).checkpoint();
        Err(winnow::error::ErrMode::Backtrack(
            wp_parser::utils::context_error(data, &cp, "timestamp fail"),
        ))
    }
}
fn parse_timestamp_ms(data: &mut &str) -> WResult<chrono::DateTime<chrono::Utc>> {
    let ts_ms = take(13usize).parse_next(data)?;
    if let Ok(Some(dt)) = ts_ms.parse().map(chrono::DateTime::from_timestamp_millis) {
        Ok(dt)
    } else {
        let cp = (*data).checkpoint();
        Err(winnow::error::ErrMode::Backtrack(
            wp_parser::utils::context_error(data, &cp, "timestamp_millis fail"),
        ))
    }
}
fn parse_timestamp_us(data: &mut &str) -> WResult<chrono::DateTime<chrono::Utc>> {
    let ts_us = take(16usize).parse_next(data)?;
    if let Ok(Some(dt)) = ts_us.parse().map(chrono::DateTime::from_timestamp_micros) {
        Ok(dt)
    } else {
        let cp = (*data).checkpoint();
        Err(winnow::error::ErrMode::Backtrack(
            wp_parser::utils::context_error(data, &cp, "timestamp_micros fail"),
        ))
    }
}
