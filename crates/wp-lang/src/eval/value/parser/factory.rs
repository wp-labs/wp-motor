use orion_error::{ContextRecord, ErrorOwe, ErrorWith, WithContext};

use crate::eval::value::parse_def::{Hold, ParserHold};
use crate::eval::value::parser::base::digit::{DigitP, FloatP};
use crate::eval::value::parser::base::hex::HexDigitP;
use crate::eval::value::parser::base::*;
use crate::eval::value::parser::compute::device::SnP;
use crate::eval::value::parser::network::http;
use crate::eval::value::parser::network::net::{IpNetP, IpPSR};
use crate::eval::value::parser::physical::time::{
    TimeCLF, TimeISOP, TimeP, TimeRFC2822, TimeRFC3339, TimeStampPSR,
};
use crate::eval::value::parser::protocol::array::ArrayP;
use crate::eval::value::parser::protocol::base64::Base64P;
use crate::eval::value::parser::protocol::json::JsonP;
use crate::eval::value::parser::protocol::json_exact::ExactJsonP;
use crate::eval::value::parser::protocol::keyval::KeyValP;
use crate::eval::value::parser::protocol::kvarr::KvArrP;
use crate::eval::value::parser::protocol::proto_text::ProtoTextP;
use crate::parser::error::{WplCodeError, WplCodeReason, WplCodeResult};
use wp_model_core::model::DataType;

use super::auto::CombinedParser;
#[derive(Default)]
pub struct ParserFactory {}

impl ParserFactory {
    pub fn crate_auto() -> WplCodeResult<ParserHold> {
        let mut parse = CombinedParser::new();
        parse.ps.push(ParserFactory::create(&DataType::Json)?);
        parse.ps.push(ParserFactory::create(&DataType::Time)?);
        parse.ps.push(ParserFactory::create(&DataType::IP)?);
        parse.ps.push(ParserFactory::create(&DataType::KV)?);
        parse.ps.push(ParserFactory::create(&DataType::Float)?);
        parse.ps.push(ParserFactory::create(&DataType::Digit)?);
        parse.ps.push(ParserFactory::create(&DataType::Hex)?);
        parse.ps.push(ParserFactory::create(&DataType::Chars)?);
        Ok(Hold::new(parse))
    }

    pub fn create_simple(meta: &DataType) -> Option<ParserHold> {
        match *meta {
            DataType::Bool => Some(Hold::new(BoolP::default())),
            DataType::Chars => Some(Hold::new(CharsP::default())),
            DataType::Symbol => Some(Hold::new(SymbolP::default())),
            DataType::PeekSymbol => Some(Hold::new(PeekSymbolP::default())),
            DataType::Digit => Some(Hold::new(DigitP::default())),
            DataType::Float => Some(Hold::new(FloatP::default())),
            DataType::Ignore => Some(Hold::new(IgnoreP::default())),
            DataType::Time => Some(Hold::new(TimeP::default())),
            DataType::TimeCLF => Some(Hold::new(TimeCLF::default())),
            DataType::TimeISO => Some(Hold::new(TimeISOP::default())),
            DataType::TimeRFC3339 => Some(Hold::new(TimeRFC3339::default())),
            DataType::TimeRFC2822 => Some(Hold::new(TimeRFC2822::default())),
            DataType::TimeTIMESTAMP => Some(Hold::new(TimeStampPSR::default())),
            DataType::IP => Some(Hold::new(IpPSR::default())),
            DataType::IpNet => Some(Hold::new(IpNetP::default())),
            DataType::Port => Some(Hold::new(DigitP::default())),
            DataType::SN => Some(Hold::new(SnP::default())),
            DataType::Hex => Some(Hold::new(HexDigitP::default())),
            DataType::Base64 => Some(Hold::new(Base64P::default())),
            DataType::KvArr => Some(Hold::new(KvArrP::default())),
            DataType::KV => Some(Hold::new(KeyValP::default())),
            DataType::Json => Some(Hold::new(JsonP::default())),
            DataType::ExactJson => Some(Hold::new(ExactJsonP::default())),
            DataType::HttpRequest => Some(Hold::new(http::RequestP::default())),
            DataType::HttpStatus => Some(Hold::new(http::StatusP::default())),
            DataType::HttpAgent => Some(Hold::new(http::AgentP::default())),
            DataType::HttpMethod => Some(Hold::new(http::MethodP::default())),
            DataType::ProtoText => Some(Hold::new(ProtoTextP::default())),
            _ => None,
        }
    }

    fn create_l1(meta: &DataType) -> WplCodeResult<ParserHold> {
        let mut ctx = WithContext::want("create parser");
        ctx.record("meta", meta.to_string());
        if let Some(hold) = Self::create_simple(meta) {
            return Ok(hold);
        } else if DataType::Auto == *meta {
            return Self::crate_auto();
        }
        Err(WplCodeError::from(WplCodeReason::UnSupport(
            meta.to_string(),
        )))
        .with(&ctx)
    }

    pub fn create(meta: &DataType) -> WplCodeResult<ParserHold> {
        let mut ctx = WithContext::want("create parser");
        ctx.record("meta", meta.to_string());
        if let DataType::Array(next_name) = meta {
            let sub_meta = DataType::from(next_name.as_str())
                .owe(WplCodeReason::UnSupport(next_name.into()))
                .with(&ctx)?;
            match Self::create(&sub_meta) {
                Ok(_) => Ok(Hold::new(ArrayP::new())),
                Err(e) => Err(e),
            }
        } else {
            Self::create_l1(meta)
        }
    }
}
