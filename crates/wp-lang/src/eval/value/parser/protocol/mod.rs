use crate::ast::WplSep;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::parser::utils::{quot_r_str, quot_str, window_path};
use winnow::ascii::multispace0;
use winnow::combinator::alt;
use wp_model_core::model::DataField;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

pub mod array;
pub mod base64;
pub mod json;
pub mod json_exact;
mod json_impl;
pub mod keyval;
pub mod kvarr;
pub mod proto_text;

pub fn take_sub_tdo(
    fpu: &FieldEvalUnit,
    upper_sep: &WplSep,
    data: &mut &str,
    key: &str,
    out: &mut Vec<DataField>,
) -> ModalResult<()> {
    multispace0.parse_next(data)?;
    let str_val_r = alt((quot_r_str, quot_str, window_path)).parse_next(data);
    match str_val_r {
        Ok(mut str_val) => {
            if let Some(sub_fpu) = fpu.get_sub_fpu(key) {
                let mut prefer_sep = fpu.conf().resolve_sep(upper_sep);
                prefer_sep.apply_default(WplSep::inherited_sep("\\0"));
                let run_key = sub_fpu.conf().run_key(key);
                Ok(sub_fpu.parse(&prefer_sep, &mut str_val, run_key, out)?)
            } else {
                out.push(DataField::from_chars(key, str_val));
                Ok(())
            }
        }
        Err(_) => {
            if let Some(sub_fpu) = fpu.get_sub_fpu(key) {
                let mut prefer_sep = fpu.conf().resolve_sep(upper_sep);
                prefer_sep.apply_default(WplSep::inherited_sep("\\0"));
                let run_key = sub_fpu.conf().run_key(key);
                Ok(sub_fpu.parse(&prefer_sep, data, run_key, out)?)
            } else {
                let sep = fpu.conf().resolve_sep(upper_sep);
                let val = sep.read_until_sep(data)?;
                let trim_val = val.trim();
                out.push(DataField::from_chars(key, trim_val));
                Ok(())
            }
        }
    }
}
