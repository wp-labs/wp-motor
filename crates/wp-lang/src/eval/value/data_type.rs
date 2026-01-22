use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use crate::eval::value::parser::physical::time::parse_time;
use wp_model_core::model::DataType;
use wp_model_core::model::FNameStr;
use wp_model_core::model::FValueStr;
use wp_model_core::model::data::Field;
use wp_model_core::model::error::ModelError;
use wp_model_core::model::{DateTimeValue, Maker, Value};

pub trait DataTypeParser {
    fn from_str<N: Into<FNameStr>, V: Into<FValueStr> + Display>(
        meta: DataType,
        name: N,
        value: V,
    ) -> Result<Self, ModelError>
    where
        Self: Sized;
}

impl<T> DataTypeParser for Field<T>
where
    T: Maker<FValueStr>,
    T: Maker<i64>,
    T: Maker<f64>,
    T: Maker<IpAddr>,
    T: Maker<bool>,
    T: Maker<DateTimeValue>,
    T: Maker<Value>,
{
    fn from_str<N: Into<FNameStr>, V: Into<FValueStr> + Display>(
        meta: DataType,
        name: N,
        value: V,
    ) -> Result<Self, ModelError> {
        match meta {
            DataType::Chars => Ok(Field::<T>::from_chars(name.into(), value.into())),
            DataType::Symbol => Ok(Field::<T>::from_symbol(name.into(), value.into())),
            DataType::Digit => {
                let value = value
                    .into()
                    .parse::<i64>()
                    .map_err(|e| ModelError::Parse(e.to_string()))?;
                Ok(Field::<T>::from_digit(name.into(), value))
            }
            DataType::Float => {
                let value = value
                    .into()
                    .parse::<f64>()
                    .map_err(|e| ModelError::Parse(e.to_string()))?;
                Ok(Field::<T>::from_float(name.into(), value))
            }
            DataType::IP => {
                let value = Ipv4Addr::from_str(value.into().as_str())
                    .map_err(|e| ModelError::Parse(e.to_string()))?;
                Ok(Field::<T>::from_ip(name.into(), IpAddr::V4(value)))
            }
            DataType::Bool => {
                let value = bool::from_str(value.into().as_str())
                    .map_err(|e| ModelError::Parse(e.to_string()))?;
                Ok(Field::<T>::from_bool(name.into(), value))
            }
            DataType::Time => {
                let code: FValueStr = value.into().clone();
                Ok(Field::<T>::from_time(
                    name.into(),
                    parse_time(&mut code.as_str())
                        .map_err(|_| ModelError::Parse(format!("{} parse error", "time")))?,
                ))
            }
            _ => Err(ModelError::Parse(format!(
                "TDOEnum::from_str not support meta:{}",
                meta
            ))),
        }
    }
}
