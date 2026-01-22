use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use wp_connector_api::{ConnectorDef, ParamMap};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectorTomlFile {
    #[serde(default)]
    pub connectors: Vec<ConnectorDef>,
}

pub fn param_value_from_toml(value: &toml::Value) -> JsonValue {
    match value {
        toml::Value::String(s) => JsonValue::String(s.clone()),
        toml::Value::Integer(i) => JsonValue::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        toml::Value::Boolean(b) => JsonValue::Bool(*b),
        toml::Value::Datetime(dt) => JsonValue::String(dt.to_string()),
        toml::Value::Array(arr) => {
            JsonValue::Array(arr.iter().map(param_value_from_toml).collect())
        }
        toml::Value::Table(tab) => JsonValue::Object(
            tab.iter()
                .map(|(k, v)| (k.clone(), param_value_from_toml(v)))
                .collect::<serde_json::Map<_, _>>(),
        ),
    }
}

pub fn param_map_from_table_ref(table: &toml::value::Table) -> ParamMap {
    let mut map = ParamMap::new();
    for (k, v) in table.iter() {
        map.insert(k.clone(), param_value_from_toml(v));
    }
    map
}

pub fn param_map_to_table(map: &ParamMap) -> toml::value::Table {
    fn conv(value: &JsonValue) -> toml::Value {
        match value {
            JsonValue::Null => toml::Value::String("null".into()),
            JsonValue::Bool(b) => toml::Value::Boolean(*b),
            JsonValue::Number(num) => {
                if let Some(i) = num.as_i64() {
                    toml::Value::Integer(i)
                } else if let Some(u) = num.as_u64() {
                    if u <= i64::MAX as u64 {
                        toml::Value::Integer(u as i64)
                    } else {
                        toml::Value::Float(u as f64)
                    }
                } else if let Some(f) = num.as_f64() {
                    toml::Value::Float(f)
                } else {
                    toml::Value::Float(0.0)
                }
            }
            JsonValue::String(s) => toml::Value::String(s.clone()),
            JsonValue::Array(arr) => toml::Value::Array(arr.iter().map(conv).collect()),
            JsonValue::Object(obj) => {
                let mut table = toml::value::Table::new();
                for (k, v) in obj.iter() {
                    table.insert(k.clone(), conv(v));
                }
                toml::Value::Table(table)
            }
        }
    }
    let mut table = toml::value::Table::new();
    for (k, v) in map.iter() {
        table.insert(k.clone(), conv(v));
    }
    table
}
