pub type AnyResult<T> = anyhow::Result<T>;
mod wild_map;

// Keep WildMap as language-internal
pub use wild_map::WildMap;

// WildArray 已迁移到 wp-specs；本 crate 不再重导出，调用方请改为 `use wp_specs::WildArray`。
