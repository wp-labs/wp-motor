//! Facade for enrichment API
//!
//! 通过最小导出将富化能力从核心实现中解耦，供应用/插件按需引用。
//! 典型使用方式：`use crate::facade::enrich::*;`。

// 最小必要接口：与 `wp-enrich-api` 对齐，只做转发不引入额外耦合。
pub use wp_enrich_api::{EnrichLibAble, EnrichRegistry, Enricher, EnrichingAble};
