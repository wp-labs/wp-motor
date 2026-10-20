//! 全局 tag：实例级、启动时加载的 record 字段集合。
//!
//! 与 `wp_event_id` 等事件 meta 不同，全局 tag 不依赖单条事件的内容，也不是
//! `wparse.toml` 的实例配置——它由每个实例自己的环境变量 `WP_GLOBAL_TAGS`
//! （或 work root 下的 `.env`）提供，启动后只读。

use std::collections::BTreeMap;
use std::sync::Arc;

use smol_str::SmolStr;
use wp_model_core::model::{DataField, DataRecord};

/// 每个 wparse 实例独立设置全局 tag 的环境变量。
pub const GLOBAL_TAGS_ENV: &str = "WP_GLOBAL_TAGS";

/// 系统 meta 字段前缀，全局 tag 不允许占用。
const RESERVED_PREFIX: &str = "wp_";

/// 启动时构建、之后只读的全局 tag 集合。
#[derive(Clone, Debug, Default)]
pub struct GlobalTags {
    tags: Arc<BTreeMap<SmolStr, SmolStr>>,
}

impl GlobalTags {
    /// 从 `WP_GLOBAL_TAGS` 读取。
    ///
    /// 格式：
    /// - `owner_id=team-a,env=prod`
    /// - 或 JSON：`{"owner_id":"team-a","note":"a,b=c"}`
    pub fn from_env() -> Result<Self, String> {
        Self::from_env_var(GLOBAL_TAGS_ENV)
    }

    fn from_env_var(name: &str) -> Result<Self, String> {
        match std::env::var(name) {
            Ok(raw) => Ok(Self::from_pairs(parse_global_tags(&raw)?)),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(err) => Err(format!("read {name} failed: {err}")),
        }
    }

    /// 直接由已知键值对构建。
    ///
    /// 不执行 `WP_GLOBAL_TAGS` 的格式与保留前缀校验；外部输入请走
    /// [`GlobalTags::from_env`]。重复 key 以后出现的值为准。
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        let tags = pairs
            .into_iter()
            .map(|(key, value)| (SmolStr::from(key), SmolStr::from(value)))
            .collect();
        Self {
            tags: Arc::new(tags),
        }
    }

    /// 遍历当前 tag，按 key 排序。
    pub fn iter(&self) -> impl Iterator<Item = (&SmolStr, &SmolStr)> {
        self.tags.iter()
    }

    pub fn to_map(&self) -> BTreeMap<String, String> {
        self.tags
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}

/// 解析 `WP_GLOBAL_TAGS`。
///
/// 简单场景用 `k=v,k2=v2`；值里包含 `,` 或 `=` 时请使用 JSON 对象。
pub fn parse_global_tags(raw: &str) -> Result<Vec<(String, String)>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    if raw.starts_with('{') {
        let map: BTreeMap<String, String> = serde_json::from_str(raw)
            .map_err(|err| format!("{GLOBAL_TAGS_ENV} JSON parse failed: {err}"))?;
        return validate_pairs(map);
    }

    let mut pairs = Vec::new();
    for item in raw.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| format!("{GLOBAL_TAGS_ENV} item '{item}' must be in key=value form"))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err(format!("{GLOBAL_TAGS_ENV} contains an empty key"));
        }
        pairs.push((key.to_string(), value.to_string()));
    }
    validate_pairs(pairs)
}

fn validate_pairs(
    pairs: impl IntoIterator<Item = (String, String)>,
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (key, value) in pairs {
        if key.is_empty() {
            return Err(format!("{GLOBAL_TAGS_ENV} contains an empty key"));
        }
        if key.starts_with(RESERVED_PREFIX) {
            return Err(format!(
                "{GLOBAL_TAGS_ENV} key '{key}' uses reserved prefix '{RESERVED_PREFIX}'"
            ));
        }
        out.push((key, value));
    }
    Ok(out)
}

/// 把全局 tag 盖到 record 上；同名规则字段会被全局值覆盖。
///
/// 主 record 与 `copy_event_parse` 旁路 record 都走同一入口。
pub fn apply_global_tags(record: &mut DataRecord, tags: &GlobalTags) {
    for (key, value) in tags.iter() {
        while record.remove_field(key.as_str()) {}
        record.append(DataField::from_chars(key.as_str(), value.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kv_pairs() {
        assert_eq!(
            parse_global_tags(" owner_id = team-a , env=prod ").unwrap(),
            vec![
                ("owner_id".to_string(), "team-a".to_string()),
                ("env".to_string(), "prod".to_string()),
            ]
        );
    }

    #[test]
    fn parse_json_pairs() {
        assert_eq!(
            parse_global_tags(r#"{"owner_id":"team-a","note":"a,b=c"}"#).unwrap(),
            vec![
                ("note".to_string(), "a,b=c".to_string()),
                ("owner_id".to_string(), "team-a".to_string()),
            ]
        );
    }

    #[test]
    fn parse_empty_and_blank() {
        assert!(parse_global_tags("").unwrap().is_empty());
        assert!(parse_global_tags("  , , ").unwrap().is_empty());
    }

    #[test]
    fn parse_rejects_malformed_items() {
        assert!(parse_global_tags("owner_id").is_err());
        assert!(parse_global_tags("=team-a").is_err());
        assert!(parse_global_tags(r#"{"owner_id":1}"#).is_err());
    }

    #[test]
    fn parse_keeps_equals_in_value() {
        assert_eq!(
            parse_global_tags("token=a=b=c").unwrap(),
            vec![("token".to_string(), "a=b=c".to_string())]
        );
    }

    #[test]
    fn parse_duplicate_keys_last_wins() {
        let tags = GlobalTags::from_pairs(parse_global_tags("owner_id=a,owner_id=b").unwrap());
        assert_eq!(tags.to_map().get("owner_id").map(String::as_str), Some("b"));
    }

    #[test]
    fn parse_rejects_reserved_wp_prefix() {
        assert!(parse_global_tags("wp_event_id=1").is_err());
        assert!(parse_global_tags("owner_id=1,wp_src_key=x").is_err());
        assert!(parse_global_tags(r#"{"wp_event_md5":"x"}"#).is_err());
    }

    #[test]
    fn from_env_var_reads_and_parses() {
        let name = "WPARSE_TEST_GLOBAL_TAGS_FROM_ENV";
        unsafe { std::env::remove_var(name) };
        assert!(GlobalTags::from_env_var(name).unwrap().is_empty());

        unsafe { std::env::set_var(name, "owner_id=team-a") };
        let tags = GlobalTags::from_env_var(name).unwrap();
        assert_eq!(
            tags.to_map(),
            BTreeMap::from([("owner_id".to_string(), "team-a".to_string())])
        );

        unsafe { std::env::set_var(name, "owner_id") };
        assert!(GlobalTags::from_env_var(name).is_err());

        unsafe { std::env::remove_var(name) };
    }

    #[test]
    fn from_pairs_keeps_sorted_map() {
        let tags = GlobalTags::from_pairs(vec![
            ("owner_id".into(), "team-a".into()),
            ("env".into(), "prod".into()),
        ]);
        assert_eq!(
            tags.to_map(),
            BTreeMap::from([
                ("env".to_string(), "prod".to_string()),
                ("owner_id".to_string(), "team-a".to_string()),
            ])
        );
        assert!(!tags.is_empty());
    }

    #[test]
    fn apply_overrides_existing_field_without_duplicates() {
        let tags = GlobalTags::from_pairs(vec![("owner_id".into(), "team-a".into())]);
        let mut record = DataRecord::from(vec![
            DataField::from_chars("owner_id", "from-rule"),
            DataField::from_chars("msg", "hello"),
        ]);

        apply_global_tags(&mut record, &tags);

        assert_eq!(
            record.field("owner_id").unwrap().get_value().as_str(),
            Some("team-a")
        );
        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == "owner_id")
                .count(),
            1,
            "global tag must replace, not duplicate"
        );
        assert_eq!(
            record.field("msg").unwrap().get_value().as_str(),
            Some("hello")
        );
    }

    #[test]
    fn apply_is_noop_for_empty_tags() {
        let tags = GlobalTags::default();
        let mut record = DataRecord::from(vec![DataField::from_chars("msg", "hello")]);
        apply_global_tags(&mut record, &tags);
        assert_eq!(record.len(), 1);
    }

    #[test]
    fn apply_removes_all_duplicate_rule_fields() {
        let tags = GlobalTags::from_pairs(vec![("owner_id".into(), "team-a".into())]);
        let mut record = DataRecord::from(vec![
            DataField::from_chars("owner_id", "first"),
            DataField::from_chars("owner_id", "second"),
            DataField::from_chars("msg", "hello"),
        ]);

        apply_global_tags(&mut record, &tags);

        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == "owner_id")
                .count(),
            1
        );
        assert_eq!(
            record.field("owner_id").unwrap().get_value().as_str(),
            Some("team-a")
        );
    }

    #[test]
    fn apply_multiple_tags_in_sorted_order() {
        let tags = GlobalTags::from_pairs(vec![
            ("owner_id".into(), "team-a".into()),
            ("env".into(), "prod".into()),
        ]);
        let mut record = DataRecord::default();

        apply_global_tags(&mut record, &tags);

        let names: Vec<&str> = record.items.iter().map(|f| f.get_name()).collect();
        assert_eq!(names, vec!["env", "owner_id"]);
    }
}
