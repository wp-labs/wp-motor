use crate::sources::event_id::next_event_id;
use bytes::Bytes;
use std::sync::Arc;
use wp_connector_api::{SourceEvent, Tags};
use wp_parse_api::RawData;

/// 基础事件构建（附加 access_ip、ups_ip、复制 tags 为 Tags）
pub fn build_base_event(key: &str, tags: &Tags, client_ip: &str, payload: Bytes) -> SourceEvent {
    let mut stags = tags.clone();
    stags.set("access_ip", client_ip.to_string());
    let mut ev = SourceEvent::new(
        next_event_id(),
        key,
        RawData::Bytes(payload),
        Arc::new(stags),
    );
    if let Ok(ip) = client_ip.parse::<std::net::IpAddr>() {
        ev.ups_ip = Some(ip);
    }
    ev
}

/// 基于预构建的 Tags 构造事件：仅追加 access_ip，减少热点路径上的标签拷贝
pub fn build_event_from_stags(
    key: &str,
    base: &Tags,
    _client_ip: &str,
    payload: Bytes,
) -> SourceEvent {
    // 复用 base tags，client_ip 参数当前未使用但保留用于API兼容性
    let stags = base.clone();

    SourceEvent::new(
        next_event_id(),
        key,
        RawData::Bytes(payload),
        Arc::new(stags),
    )
}
