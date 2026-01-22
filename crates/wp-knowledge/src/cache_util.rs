use wp_data_model::cache::CacheAble;
use wp_error::KnowledgeResult;
use wp_log::warn_kdb;
use wp_model_core::model::DataField;

use crate::mem::RowData;

/// Generic cache wrapper: fetch from cache by `c_params`, otherwise run `query_fn` and save.
pub fn cache_query_impl<const N: usize>(
    c_params: &[DataField; N],
    cache: &mut impl CacheAble<DataField, RowData, N>,
    query_fn: impl FnOnce() -> KnowledgeResult<RowData>,
) -> RowData {
    if let Some(hit) = cache.fetch(c_params) {
        return hit.clone();
    }
    match query_fn() {
        Ok(rows) => {
            cache.save(c_params, rows.clone());
            rows
        }
        Err(e) => {
            warn_kdb!("[kdb] query error: {}", e);
            Vec::new()
        }
    }
}
