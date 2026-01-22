use crate::DataTypeParser;
use crate::parser::utils::take_meta_name;
use smol_str::SmolStr;
use std::fmt::Display;
use winnow::ascii::multispace0;
use winnow::combinator::fail;
use winnow::error::StrContext;
use winnow::stream::Stream;
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataField, DataType};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::{take_parentheses_val, take_var_name};
use wp_parser::symbol::ctx_desc;

pub fn take_datatype(data: &mut &str) -> WResult<DataType> {
    take_datatype_impl
        .context(StrContext::Label("<datatype>"))
        .parse_next(data)
}
pub fn take_datatype_impl(data: &mut &str) -> WResult<DataType> {
    let _ = multispace0.parse_next(data)?;
    let cp = data.checkpoint();
    let meta_str = take_meta_name.parse_next(data)?;
    if let Ok(meta) = DataType::from(meta_str) {
        Ok(meta)
    } else {
        data.reset(&cp);
        fail.context(ctx_desc("DataType from str fail"))
            .parse_next(&mut "")
    }
}

pub fn field_ins<N: Into<FNameStr>, V: Into<SmolStr> + Display>(
    meta: DataType,
    name: N,
    val: V,
) -> WResult<DataField> {
    if let Ok(tdo) = DataField::from_str(meta, name, val) {
        Ok(tdo)
    } else {
        fail.context(ctx_desc("DataField from str fail"))
            .parse_next(&mut "")
    }
}

pub fn take_field(data: &mut &str) -> WResult<DataField> {
    // 解析类型化字面量：ip(...)/digit(...)/chars(...)
    // 语义上这是“无名值”，字段名应为空字符串，类型由 meta 承担。
    // 过去实现将标识符作为 name（如 "ip"），与下游比较语义不一致。
    // 此处修正为：name = ""，只保留 meta+value。
    let key = take_var_name.parse_next(data)?; // e.g. "ip"
    let value = take_parentheses_val.parse_next(data)?; // e.g. "127.0.0.1"
    let mut key_for_meta = key; // 用副本喂给 take_datatype
    let meta = take_datatype(&mut key_for_meta)?; // DataType::IP
    let target = field_ins(meta, "", &value)?; // typed literal has no field name
    Ok(target)
}
