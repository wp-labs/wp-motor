//! 批量处理逻辑

use super::types::{ParsedDatSet, ProcessResult};
use crate::core::parser::{ParseOption, WplEngine};
use crate::sinks::{ProcMeta, SinkPackage, SinkRecUnit};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use wp_connector_api::SourceEvent;
use wp_model_core::model::{DataField, DataRecord};
use wp_parse_api::RawData;
use wpl::WparseError;

impl WplEngine {
    /// 解析并分组处理后的数据
    pub fn batch_parse_package(
        &mut self,
        batch: Vec<SourceEvent>,
        setting: &ParseOption,
    ) -> Result<ParsedDatSet, WparseError> {
        let mut sink_groups: HashMap<String, SinkPackage> = HashMap::new();
        let mut residue_data = Vec::new();
        let mut miss_packets = Vec::new();

        debug_data!("Processing events: len={}", batch.len());
        // 处理每个数据包
        for data in batch {
            match self.pipelines.parse_event(&data, setting) {
                ProcessResult::Success { wpl_key, record } => {
                    // 完全成功解析
                    let record = enrich_record_with_tags(record, &data.tags);
                    let rec_unit = SinkRecUnit::new(data.event_id, ProcMeta::Null, record);
                    sink_groups.entry(wpl_key).or_default().push(rec_unit);
                }
                ProcessResult::Partial {
                    wpl_key,
                    record,
                    residue,
                } => {
                    // 部分成功，有残留数据
                    let record = enrich_record_with_tags(record, &data.tags);
                    let rec_unit = SinkRecUnit::new(data.event_id, ProcMeta::Null, record);
                    sink_groups
                        .entry(wpl_key.clone())
                        .or_default()
                        .push(rec_unit);
                    let residue_event = format!("wpl:{},residue:{}", wpl_key, residue);
                    residue_data.push((data.event_id, residue_event));
                }
                ProcessResult::Miss(fail_info) => {
                    if payload_is_whitespace(&data.payload) {
                        trace_data!("drop whitespace event {} without miss", data.event_id);
                        continue;
                    }
                    // 完全失败，记录深度最高的错误信息
                    warn_data!(
                        "wpls miss event_id :{} data:\n{}",
                        data.event_id,
                        data.payload
                    );
                    miss_packets.push((data, fail_info));
                }
            }
        }

        Ok(ParsedDatSet {
            sink_groups,
            residue_data,
            missed_packets: miss_packets,
        })
    }
}

pub(crate) fn enrich_record_with_tags(
    record: Arc<DataRecord>,
    tags: &wp_connector_api::Tags,
) -> Arc<DataRecord> {
    if tags.is_empty() {
        return record;
    }
    let pairs = materialize_tags(tags);
    if pairs.is_empty() {
        return record;
    }
    let mut pending = Vec::new();
    for (key, value) in pairs {
        if record.field(&key).is_none() {
            pending.push((key, value));
        }
    }
    if pending.is_empty() {
        return record;
    }
    // Avoid cloning when the Arc is unique
    let mut enriched = match Arc::try_unwrap(record) {
        Ok(inner) => inner,
        Err(shared) => (*shared).clone(),
    };
    for (key, value) in pending {
        debug_data!("enrich source tags {}:{}", key, value);
        enriched.append(DataField::from_chars(key, value));
    }
    Arc::new(enriched)
}

#[derive(Deserialize)]
struct TagsSnapshot {
    item: Vec<(String, String)>,
}

fn materialize_tags(tags: &wp_connector_api::Tags) -> Vec<(String, String)> {
    if tags.is_empty() {
        return Vec::new();
    }
    match serde_json::to_vec(tags)
        .ok()
        .and_then(|raw| serde_json::from_slice::<TagsSnapshot>(&raw).ok())
    {
        Some(snapshot) => snapshot.item,
        None => Vec::new(),
    }
}

fn payload_is_whitespace(payload: &RawData) -> bool {
    match payload {
        RawData::String(s) => s.trim().is_empty(),
        RawData::Bytes(bytes) => bytes_are_whitespace(bytes.as_ref()),
        RawData::ArcBytes(buffer) => bytes_are_whitespace(buffer.as_slice()),
    }
}

fn bytes_are_whitespace(bytes: &[u8]) -> bool {
    bytes.is_empty() || bytes.iter().all(|b| b.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::wpl_engine::parser::MultiParser;
    use crate::core::parser::wpl_engine::pipeline::WplPipeline;
    use crate::sinks::{InfraSinkAgent, SinkGroupAgent};
    use std::sync::Arc;
    use wp_connector_api::{SourceEvent, Tags};
    use wp_parse_api::RawData;
    use wpl::{WplEvaluator, gen_pkg_id};

    fn build_event(payload: &str) -> SourceEvent {
        SourceEvent::new(
            gen_pkg_id(),
            "test-src",
            RawData::String(payload.to_string()),
            Arc::new(Tags::new()),
        )
    }

    fn build_event_with_tags(payload: &str, tag_pairs: &[(&str, &str)]) -> SourceEvent {
        let mut tags = Tags::new();
        for (key, value) in tag_pairs {
            tags.set(*key, *value);
        }
        SourceEvent::new(
            gen_pkg_id(),
            "test-src",
            RawData::String(payload.to_string()),
            Arc::new(tags),
        )
    }

    fn assert_chars_field(record: &DataRecord, key: &str, expected: &str) {
        use wp_model_core::model::Value;
        let field = record
            .field(key)
            .unwrap_or_else(|| panic!("missing field {key}"));
        match field.get_value() {
            Value::Chars(actual) => assert_eq!(actual, expected),
            other => panic!("field {key} expected chars, got {:?}", other),
        }
    }

    fn build_real_engine(rules: &[(&str, &str)]) -> WplEngine {
        let mut pipelines = Vec::new();
        for (idx, (key, code)) in rules.iter().enumerate() {
            let evaluator = WplEvaluator::from_code(code).expect("build evaluator");
            let pipeline = WplPipeline::new(
                idx,
                key.to_string(),
                Vec::new(),
                evaluator,
                vec![SinkGroupAgent::null()],
                Vec::new(),
            );
            pipelines.push(pipeline);
        }
        WplEngine {
            pipelines: MultiParser::new(pipelines),
            infra_agent: InfraSinkAgent::use_null(),
        }
    }

    #[test]
    fn batch_parse_package_groups_sink_packages_and_residue() {
        let mut engine =
            build_real_engine(&[("nginx_access", NGINX_RULE), ("json_payload", JSON_RULE)]);
        let option = ParseOption::default();

        let event_a = build_event(NGINX_SAMPLE);
        let id_a = event_a.event_id;
        let event_b = build_event(JSON_SAMPLE);
        let id_b = event_b.event_id;
        let event_c = build_event(&format!("{}TAIL", NGINX_SAMPLE));
        let id_c = event_c.event_id;

        let parsed = engine
            .batch_parse_package(vec![event_a, event_b, event_c], &option)
            .expect("parse batch");

        let ParsedDatSet {
            sink_groups,
            residue_data,
            missed_packets,
        } = parsed;

        assert!(missed_packets.is_empty());
        assert_eq!(
            residue_data,
            vec![(id_c, "wpl:nginx_access,residue:TAIL".to_string())]
        );

        let alpha_pkg = sink_groups
            .get("nginx_access")
            .expect("nginx group missing");
        let alpha_ids: Vec<_> = alpha_pkg.iter().map(|unit| *unit.id()).collect();
        assert_eq!(alpha_ids, vec![id_a, id_c]);

        let beta_pkg = sink_groups.get("json_payload").expect("json group missing");
        let beta_ids: Vec<_> = beta_pkg.iter().map(|unit| *unit.id()).collect();
        assert_eq!(beta_ids, vec![id_b]);
    }

    #[test]
    fn batch_parse_package_tracks_missed_packets() {
        let mut engine =
            build_real_engine(&[("nginx_access", NGINX_RULE), ("json_payload", JSON_RULE)]);
        let option = ParseOption::default();
        let miss_event = build_event("NOTHING-VALID");
        let miss_id = miss_event.event_id;

        let parsed = engine
            .batch_parse_package(vec![miss_event], &option)
            .expect("parse batch");

        let ParsedDatSet {
            sink_groups,
            residue_data,
            missed_packets,
        } = parsed;

        assert!(residue_data.is_empty());
        assert_eq!(missed_packets.len(), 1);
        assert_eq!(missed_packets[0].0.event_id, miss_id);
        assert!(sink_groups.is_empty());
    }

    const NGINX_RULE: &str = r#"
rule nginx_access {
  (ip,2*_,time/clf<[,]>,http/request",http/status,digit,chars",http/agent",_")
}
"#;

    const JSON_RULE: &str = r#"
rule json_payload {
  (json(chars@data))
}
"#;

    // 日志与规则样例来源于 crates/wp-lang 的 bench/test 数据
    const NGINX_SAMPLE: &str = r#"222.133.52.20 - - [06/Aug/2019:12:12:19 +0800] "GET /nginx-logo.png HTTP/1.1" 200 368 "http://119.122.1.4/" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36" "-""#;
    const JSON_SAMPLE: &str = r#"{ "data": "192.168.1.1" }"#;

    #[test]
    fn batch_parse_package_handles_real_multi_rules() {
        let mut engine =
            build_real_engine(&[("nginx_access", NGINX_RULE), ("json_payload", JSON_RULE)]);
        let events = vec![build_event(NGINX_SAMPLE), build_event(JSON_SAMPLE)];
        let parsed = engine
            .batch_parse_package(events, &ParseOption::default())
            .expect("parse real data");

        assert!(parsed.residue_data.is_empty());
        assert!(parsed.missed_packets.is_empty());

        let nginx_pkg = parsed
            .sink_groups
            .get("nginx_access")
            .expect("missing nginx group");
        assert_eq!(nginx_pkg.len(), 1);

        let json_pkg = parsed
            .sink_groups
            .get("json_payload")
            .expect("missing json group");
        assert_eq!(json_pkg.len(), 1);
    }

    #[test]
    fn batch_parse_package_enriches_records_with_tags() {
        let mut engine = build_real_engine(&[("nginx_access", NGINX_RULE)]);
        let option = ParseOption::default();
        let event = build_event_with_tags(
            NGINX_SAMPLE,
            &[
                ("env", "test"),
                ("dev_src_ip", "10.0.0.1"),
                ("access_source", "custom"),
            ],
        );

        let parsed = engine
            .batch_parse_package(vec![event], &option)
            .expect("parse with tags");

        let nginx_pkg = parsed
            .sink_groups
            .get("nginx_access")
            .expect("missing nginx group");
        assert_eq!(nginx_pkg.len(), 1);
        let record = nginx_pkg.first().expect("missing record").data();

        assert_chars_field(record, "env", "test");
        assert_chars_field(record, "dev_src_ip", "10.0.0.1");
        assert_chars_field(record, "access_source", "custom");
    }

    #[test]
    fn enrich_record_with_tags_skips_when_all_present() {
        let mut tags = Tags::new();
        tags.set("env", "prod");
        let record = DataRecord::from(vec![DataField::from_chars("env", "prod")]);
        let arc = Arc::new(record);
        let enriched = enrich_record_with_tags(Arc::clone(&arc), &tags);
        assert!(Arc::ptr_eq(&arc, &enriched));
    }

    #[test]
    fn enrich_record_with_tags_appends_missing_keys() {
        let mut tags = Tags::new();
        tags.set("env", "test");
        let record = Arc::new(DataRecord::from(vec![DataField::from_chars("foo", "bar")]));
        let enriched = enrich_record_with_tags(record, &tags);
        assert_chars_field(&enriched, "env", "test");
        assert_chars_field(&enriched, "foo", "bar");
    }

    const MID_FAIL_RULE: &str = r#"
rule mid_fail {
  (symbol(CONTROL)), alt(symbol(-ALPHA),symbol(-BETA)),(digit,digit,chars)
}
"#;

    const SHORT_FAIL_RULE: &str = r#"
rule short_fail {
  (symbol(CONTROL),digit)
}
"#;

    const DEEP_FAIL_RULE: &str = r#"
rule deep_fail {
    (symbol(CONTROL)), alt(symbol(-ALPHA),symbol(-BETA)),(digit,chars,bool)
}
"#;

    #[test]
    fn batch_parse_package_prefers_deepest_rule_on_miss() {
        const CONTROLLED_MISS_SAMPLE: &str = "CONTROL-ALPHA 1024 warpparse 64";
        let mut engine = build_real_engine(&[
            ("short_fail", SHORT_FAIL_RULE),
            ("mid_fail", MID_FAIL_RULE),
            ("deep_fail", DEEP_FAIL_RULE),
        ]);
        let miss_event = build_event(CONTROLLED_MISS_SAMPLE);
        let miss_id = miss_event.event_id;

        let parsed = engine
            .batch_parse_package(vec![miss_event], &ParseOption::default())
            .expect("parse broken data");

        dbg!(parsed.sink_groups.keys().collect::<Vec<_>>());
        assert!(parsed.sink_groups.is_empty());
        assert!(parsed.residue_data.is_empty());
        assert_eq!(parsed.missed_packets.len(), 1);

        let (event, fail) = &parsed.missed_packets[0];
        assert_eq!(event.event_id, miss_id);
        assert_eq!(fail.best_wpl, "deep_fail");
        assert!(fail.depth > 0, "expected recorded depth from parser");
    }

    #[test]
    fn batch_parse_package_skips_whitespace_miss() {
        let mut engine = build_real_engine(&[("nginx_access", NGINX_RULE)]);
        let blank_event = build_event("   \n\t");
        let parsed = engine
            .batch_parse_package(vec![blank_event], &ParseOption::default())
            .expect("parse blank");
        assert!(parsed.sink_groups.is_empty());
        assert!(parsed.missed_packets.is_empty());
    }
}
