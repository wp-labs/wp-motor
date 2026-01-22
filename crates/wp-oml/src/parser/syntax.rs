use crate::language::EvaluationTarget;
use crate::parser::oml_acq::oml_gens_acq;
use crate::parser::oml_aggregate;
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt};
use wp_model_core::model::DataField;
use wp_parser::Parser;
use wp_parser::WResult;
use wpl::parser::datatype::{field_ins, take_datatype};

use crate::language::{DCT_GET, DCT_OPTION, GenericBinding, OML_CRATE_IN};
use wp_parser::atom::{take_json_path, take_key_pair, take_parentheses_val};
use wp_parser::symbol::{symbol_colon, symbol_comma, symbol_under_line};
use wp_parser::utils::get_scope;

pub fn oml_default(data: &mut &str) -> WResult<GenericBinding> {
    (symbol_under_line, symbol_colon).parse_next(data)?;
    let value = oml_gens_acq.parse_next(data)?;
    Ok(GenericBinding::new(EvaluationTarget::auto_default(), value))
}
#[allow(dead_code)]
pub fn oml_string(data: &mut &str) -> WResult<String> {
    multispace0.parse_next(data)?;
    let value = get_scope(data, '"', '"')?;
    Ok(value.to_string())
}

pub fn oml_args(data: &mut &str) -> WResult<(String, String)> {
    //let mut key = None;
    let x = alt((
        oml_aggregate::oml_args_option.map(|x| (DCT_OPTION.to_string(), x.clone())),
        oml_aggregate::oml_args_in.map(|x| (OML_CRATE_IN.to_string(), x.clone())),
        take_key_pair.map(|(k, v)| (k.to_string(), v.to_string())),
        take_json_path.map(|x| (DCT_GET.to_string(), x.to_string())),
    ))
    .parse_next(data)?;
    opt(symbol_comma).parse_next(data)?;
    Ok(x)
}

pub fn oml_value(data: &mut &str) -> WResult<DataField> {
    let meta = take_datatype.parse_next(data)?;
    let val = take_parentheses_val.parse_next(data)?;
    let tdo = field_ins(meta, "", val.as_str())?;
    Ok(tdo)
}
