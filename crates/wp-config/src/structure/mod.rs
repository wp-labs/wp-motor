// Note: structure module only re-exports base traits/types.
// Keep it dependency-light and implementation-free.
// V1 MDBConf removed; KnowDB v2 uses directory layout + SQL files

mod framework;
mod group;
mod io;
mod sink;
mod source;

pub use group::{
    Basis, ExpectMode, FixedGroup, FlexGroup, GroupExpectSpec, SinkGroupConf, extend_matches,
};
pub use io::{FileSinkConf, SyslogSinkConf, SyslogSourceConf};
pub use sink::{SinkExpectOverride, SinkInstanceConf, SinkRouteConf};
pub use source::SourceInstanceConf;

pub use bool_de::de_opt_bool_onoff;
/// 稳定别名：对外重导出基础接口与工具。
/// 建议新代码可直接依赖 `wp-conf-base`，也可以继续通过 `wp-conf` 引用这些接口。
pub use wp_conf_base::structure::{ConfStdOperation, Protocol, Validate, de_bool_onoff};
// Re-export orion_error ErrorOwe so downstream can `use crate::structure::ErrorOwe` for `.owe_conf()` etc.
pub use orion_error::ErrorOwe;

// No ConfStdOperation impl for MDBConf in V2

/// 兼容性函数：`wp-conf-base` 尚未发布 `de_opt_bool_onoff`，临时在此补充。
/// 下游依赖 `wp-conf` 的代码可以保持 `wp_conf::structure::de_opt_bool_onoff` 的调用方式，避免反复调整。
mod bool_de {
    use serde::Deserialize;

    pub fn de_opt_bool_onoff<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum In {
            B(bool),
            S(String),
            I(i64),
        }

        let opt: Option<In> = Option::deserialize(deserializer)?;
        Ok(match opt {
            None => None,
            Some(In::B(b)) => Some(b),
            Some(In::I(i)) => Some(i != 0),
            Some(In::S(s)) => {
                let v = s.trim().to_ascii_lowercase();
                match v.as_str() {
                    "on" | "true" | "1" | "yes" | "y" => Some(true),
                    "off" | "false" | "0" | "no" | "n" => Some(false),
                    _ => None,
                }
            }
        })
    }
}
