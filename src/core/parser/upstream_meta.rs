//! 接力（A 解析完 → B 再解析 A 的输出）时，上游已带的 `wp_*` meta 是否保留。
//!
//! 三个开关都从进程环境读取，启动时加载的 `.env` 同样生效
//! （与 `WP_INS_ID` / `WP_GLOBAL_TAGS` / `UPSTREAM_OML_NAME_KEEP` 一致）。
//!
//! 默认值等于「保留上游」，即不配置时行为与引入开关前一致。

use std::sync::OnceLock;

/// 上游已带 `wp_event_id` 时是否沿用。
///
/// - `true`（默认）：沿用上游 ID，跨阶段可关联
/// - `false`：本实例重新发号，上游值被覆盖（每阶段独立标识）
pub const UPSTREAM_EVENT_ID_KEEP_ENV: &str = "UPSTREAM_EVENT_ID_KEEP";

/// 上游已带 `wp_src_key` / `wp_src_ip` 时是否沿用。
///
/// - `true`（默认）：沿用（保留最初入口）
/// - `false`：用本实例的 source key / 对端 IP 覆盖，便于按本阶段做看板
pub const UPSTREAM_SRC_KEEP_ENV: &str = "UPSTREAM_SRC_KEEP";

/// 上游已带 `wp_event_md5` 时是否沿用（仅 `gen_event_md5 = true` 时生效）。
///
/// - `true`（默认）：沿用上游指纹（原始 payload 的 md5）
/// - `false`：用本实例收到的 payload 重算并覆盖
pub const UPSTREAM_EVENT_MD5_KEEP_ENV: &str = "UPSTREAM_EVENT_MD5_KEEP";

/// 三个开关的生效值；进程内只解析一次。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UpstreamKeep {
    pub(crate) event_id: bool,
    pub(crate) src: bool,
    pub(crate) event_md5: bool,
}

impl UpstreamKeep {
    pub(crate) fn load() -> Self {
        static FLAGS: OnceLock<UpstreamKeep> = OnceLock::new();
        *FLAGS.get_or_init(|| Self {
            event_id: flag(UPSTREAM_EVENT_ID_KEEP_ENV, true),
            src: flag(UPSTREAM_SRC_KEEP_ENV, true),
            event_md5: flag(UPSTREAM_EVENT_MD5_KEEP_ENV, true),
        })
    }
}

/// 读取布尔开关；未设置或无法识别时取 `default`。
fn flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|raw| parse_flag(&raw))
        .unwrap_or(default)
}

/// 解析布尔字面量；无法识别返回 `None`。
pub(crate) fn parse_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" | "" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flag_accepts_common_spellings() {
        for raw in ["1", "true", "TRUE", " yes ", "on"] {
            assert_eq!(parse_flag(raw), Some(true), "raw={raw:?}");
        }
        for raw in ["0", "false", "FALSE", " no ", "off", ""] {
            assert_eq!(parse_flag(raw), Some(false), "raw={raw:?}");
        }
    }

    #[test]
    fn parse_flag_rejects_garbage() {
        assert_eq!(parse_flag("maybe"), None);
        assert_eq!(parse_flag("2"), None);
    }

    #[test]
    fn flag_falls_back_to_default_when_unset() {
        let missing = "WPARSE_TEST_DEFINITELY_UNSET_FLAG";
        assert!(flag(missing, true));
        assert!(!flag(missing, false));
    }

    #[test]
    fn load_is_cached_and_stable() {
        // OnceLock 只解析一次，重复调用结果必须一致。
        assert_eq!(UpstreamKeep::load(), UpstreamKeep::load());
    }
}
