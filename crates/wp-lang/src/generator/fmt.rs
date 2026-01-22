use wp_model_core::model::{DataField, DataRecord, DataType};
use wp_parser::WResult as ModalResult;

use crate::{WplSep, ast::fld_fmt::WplFieldFmt};

pub struct JsonGenFmt<T>(pub T);
pub struct RAWGenFmt<T>(pub T);
pub struct CSVGenFmt<T>(pub T);
pub struct KVGenFmt<T>(pub T);
pub struct ProtoGenFmt<T>(pub T);

pub struct GenChannel {
    pub rng: rand::rngs::ThreadRng,
}

impl GenChannel {
    pub fn new() -> Self {
        Self { rng: rand::rng() }
    }
}
impl Default for GenChannel {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ParserValue<T> {
    fn parse_value(data: &mut &str) -> ModalResult<T>;
}

//pub type FmtField = (Meta, DataField, FieldFmt, WPLSep);
#[derive(Debug, Clone)]
pub struct FmtField {
    pub meta: DataType,
    pub data_field: DataField,
    pub field_fmt: WplFieldFmt,
    pub sep: WplSep,
}

impl FmtField {
    pub fn new(meta: DataType, data_field: DataField, field_fmt: WplFieldFmt, sep: WplSep) -> Self {
        Self {
            meta,
            data_field,
            field_fmt,
            sep,
        }
    }
}

pub type FmtFieldVec = Vec<FmtField>;

pub fn record_from_fmt_fields(fields: FmtFieldVec) -> DataRecord {
    let mut data_fields = Vec::new();

    for field in fields {
        data_fields.push(field.data_field);
    }
    DataRecord::from(data_fields)
}

mod field_vec_fmt {
    use std::fmt::{Display, Formatter};

    use super::{CSVGenFmt, JsonGenFmt, KVGenFmt, ProtoGenFmt, RAWGenFmt};
    use crate::ast::GenFmt;
    use crate::eval::vof;
    use crate::generator::{FmtFieldVec, record_from_fmt_fields};
    use wp_data_fmt::{Csv, DataFormat, FormatType, Json, KeyValue, ProtoTxt, Raw};
    use wp_model_core::model::DataType;

    impl Display for KVGenFmt<&FmtFieldVec> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let fmt = KeyValue::default();
            write!(
                f,
                "{}",
                fmt.format_record(&record_from_fmt_fields(self.0.clone()))
            )
        }
    }

    impl Display for RAWGenFmt<&FmtFieldVec> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let len = self.0.len();
            let kvfmt = KeyValue::default();
            let rawfmt = Raw;
            for (i, fmt_field) in self
                .0
                .iter()
                .filter(|fmt_f| *fmt_f.data_field.get_meta() != DataType::Ignore)
                .enumerate()
            {
                write!(f, "{}", vof(fmt_field.field_fmt.scope_beg.clone(), ""))?;
                if let Some(fmt) = &fmt_field.field_fmt.sub_fmt {
                    let formatter = FormatType::from(fmt);
                    write!(
                        f,
                        "{}",
                        formatter.fmt_value(fmt_field.data_field.get_value())
                    )?;
                } else {
                    match fmt_field.meta {
                        DataType::KV => {
                            write!(f, "{}", kvfmt.format_field(&fmt_field.data_field))?;
                        }
                        _ => {
                            write!(f, "{}", rawfmt.format_field(&fmt_field.data_field))?;
                        }
                    }
                }

                write!(f, "{}", vof(fmt_field.field_fmt.scope_end.clone(), ""))?;
                if i != len - 1 {
                    write!(f, "{}", GenFmt(&fmt_field.sep))?;
                }
            }
            Ok(())
        }
    }

    impl Display for CSVGenFmt<&FmtFieldVec> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let fmt = Csv::default();
            write!(
                f,
                "{}",
                fmt.format_record(&record_from_fmt_fields(self.0.clone()))
            )
        }
    }

    impl Display for JsonGenFmt<&FmtFieldVec> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let fmt = Json;
            write!(
                f,
                "{}",
                fmt.format_record(&record_from_fmt_fields(self.0.clone()))
            )
        }
    }

    impl Display for ProtoGenFmt<&FmtFieldVec> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let fmt = ProtoTxt;
            write!(
                f,
                "{}",
                fmt.format_record(&record_from_fmt_fields(self.0.clone()))
            )
        }
    }
}
