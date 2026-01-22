#![allow(dead_code)]

use super::SinkDispatcher;
use crate::resources::SinkResUnit;
use crate::sinks::builtin_factories;
use crate::sinks::prelude::PkgID;
use crate::sinks::routing::agent::InfraSinkAgent;
use crate::sinks::{ProcMeta, SinkBackendType, SinkRecUnit, SinkRuntime};
use oml::language::DataModel;
use oml::parser::oml_parse_raw;
use once_cell::sync::Lazy;
use orion_exp::{Expression, RustSymbol};
use orion_overload::append::Appendable;
use std::sync::Arc;
use wp_conf::TCondParser;
use wp_conf::structure::{FlexGroup, SinkGroupConf, SinkInstanceConf};
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::fmt_def::TextFmt;
use wp_model_core::model::{DataField, DataRecord, Value};
use wp_parse_api::RawData;
use wp_parser::Parser;
use wpl::{WplEvaluator, wpl_express};

const NGINX_SAMPLE: &str = include_str!("../../../../tests/sample/nginx/sample.dat");

/// Nginx 数据集批量性能场景：构造批量记录、SinkDispatcher 与 OML 资源，供 Criterion 基准复用。
pub struct OmlBatchPerfCase {
    dispatcher: SinkDispatcher,
    infra: InfraSinkAgent,
    cache: FieldQueryCache,
    rule: ProcMeta,
    records: Vec<PerfRecord>,
}

struct PerfRecord {
    pkg_id: PkgID,
    record: Arc<DataRecord>,
}

#[derive(Debug, Clone)]
struct NginxTemplate {
    ip: String,
    timestamp: String,
    method: String,
    path: String,
    protocol: String,
    referer: String,
    agent: String,
    status: i64,
    bytes: i64,
}

impl OmlBatchPerfCase {
    /// 使用给定批量大小构造性能场景（默认 rule: `/bench/nginx`）。
    pub fn new(batch_size: usize) -> Self {
        assert!(batch_size > 0, "batch size must be > 0");
        let rule = ProcMeta::Rule("/bench/nginx".to_string());
        let template = parse_nginx_template(NGINX_SAMPLE.trim());
        let records = build_records(batch_size, &template);
        let dispatcher = build_dispatcher();
        Self {
            dispatcher,
            infra: InfraSinkAgent::use_null(),
            cache: FieldQueryCache::default(),
            rule,
            records,
        }
    }

    /// 返回批量大小，便于基准透传吞吐量。
    pub fn batch_size(&self) -> usize {
        self.records.len()
    }

    fn prepare_batch(&self) -> Vec<SinkRecUnit> {
        self.records
            .iter()
            .map(|entry| {
                SinkRecUnit::with_record(entry.pkg_id, self.rule.clone(), Arc::clone(&entry.record))
            })
            .collect()
    }

    /// 执行一次 `oml_proc_batch` 并返回 fanout 后的记录数量汇总。
    pub fn run_once(&mut self) -> usize {
        let batch = self.prepare_batch();
        let per_sink = self
            .dispatcher
            .oml_proc_batch(batch, &self.infra, &mut self.cache, &self.rule)
            .expect("oml_proc_batch perf case failed");
        per_sink.iter().map(|units| units.len()).sum()
    }
}

fn build_dispatcher() -> SinkDispatcher {
    let mut sink_res = SinkResUnit::use_null();
    sink_res.push_model(build_nginx_model());

    let mut group = FlexGroup::default();
    group.name = "nginx_perf".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);

    dispatcher.append(make_sink("nginx_all", None, None, &["tier:all"]));
    dispatcher.append(make_sink(
        "nginx_errors",
        Some("$status >= digit(500)"),
        Some(true),
        &["tier:error"],
    ));
    dispatcher.append(make_sink(
        "nginx_warn",
        Some("$status >= digit(400) && $status < digit(500)"),
        Some(true),
        &["tier:warn"],
    ));
    dispatcher.append(make_sink(
        "nginx_assets",
        Some("$path =* chars(*.png)"),
        Some(true),
        &["tier:asset"],
    ));

    dispatcher
}

fn make_sink(
    name: &str,
    cond_expr: Option<&str>,
    expect_true: Option<bool>,
    tags: &[&str],
) -> SinkRuntime {
    let mut conf = SinkInstanceConf::null_new(name.to_string(), TextFmt::Json, None);
    if let Some(expect) = expect_true {
        conf.set_filter_expect(expect);
    }
    if !tags.is_empty() {
        conf.set_tags(tags.iter().map(|t| t.to_string()).collect());
    }
    let cond = cond_expr.map(parse_condition);
    SinkRuntime::new(
        "./rescue".to_string(),
        name.to_string(),
        conf,
        SinkBackendType::Proxy(builtin_factories::make_blackhole_sink()),
        cond,
        Vec::new(),
    )
}

fn parse_condition(expr: &str) -> Expression<DataField, RustSymbol> {
    let owned = expr.to_string();
    let mut view = owned.as_str();
    TCondParser::exp(&mut view).expect("invalid sink condition expression")
}

fn build_nginx_model() -> DataModel {
    let mut code = r#"
name : nginx_perf_batch
rule :
    /bench/nginx
---
size : digit = take(size);
status : digit = take(status);
str_status = match read(option:[status]) {
    digit(500) => chars(Internal Server Error);
    digit(404) => chars(Not Found);
};
match_chars = match read(option:[wp_src_ip]) {
    ip(127.0.0.1) => chars(localhost);
    !ip(127.0.0.1) => chars(attack_ip);
};
* : auto = read();
"#;
    let model = oml_parse_raw(&mut code).expect("parse nginx perf oml");
    DataModel::Object(model)
}

fn build_records(batch_size: usize, template: &NginxTemplate) -> Vec<PerfRecord> {
    let mut records = Vec::with_capacity(batch_size);
    for idx in 0..batch_size {
        let pkg_id = (idx as PkgID).saturating_add(1);
        let record = Arc::new(build_record(template, idx));
        records.push(PerfRecord { pkg_id, record });
    }
    records
}

fn build_record(template: &NginxTemplate, idx: usize) -> DataRecord {
    let mut record = DataRecord::default();
    append_chars(&mut record, "client_ip", variant_ip(&template.ip, idx));
    append_chars(
        &mut record,
        "timestamp",
        variant_timestamp(&template.timestamp, idx),
    );
    append_chars(&mut record, "method", template.method.clone());
    append_chars(&mut record, "path", variant_path(&template.path, idx));
    append_chars(&mut record, "protocol", template.protocol.clone());
    let status = variant_status(idx, template.status);
    record.append(DataField::from_digit("status", status));
    let bytes = variant_bytes(idx, template.bytes);
    record.append(DataField::from_digit("bytes", bytes));
    append_chars(
        &mut record,
        "referer",
        variant_referer(&template.referer, idx),
    );
    append_chars(&mut record, "user_agent", template.agent.clone());
    append_chars(&mut record, "site", format!("nginx-site-{}", idx % 4));
    record
}

fn parse_nginx_template(line: &str) -> NginxTemplate {
    static NGX_EVAL: Lazy<WplEvaluator> = Lazy::new(|| {
        const EXPR: &str = r#"(ip:client_ip,2*_,time/clf:timestamp<[,]>,http/request:request",http/status:status,digit:bytes,chars:referer",http/agent:agent",_")"#;
        let express = wpl_express
            .parse(EXPR)
            .expect("parse nginx benchmark expression");
        WplEvaluator::from(&express, None).expect("build nginx evaluator")
    });

    if let Ok((record, _)) = NGX_EVAL.proc(RawData::from_string(line.to_string()), 0)
        && let Some(template) = template_from_record(&record)
    {
        return template;
    }

    fallback_parse(line)
}

fn template_from_record(record: &DataRecord) -> Option<NginxTemplate> {
    let ip = value_as_string(record, "client_ip")?;
    let timestamp = value_as_string(record, "timestamp")?;
    let request = value_as_string(record, "request")?;
    let status = value_as_digit(record, "status")?;
    let bytes = value_as_digit(record, "bytes")?;
    let referer = value_as_string(record, "referer").unwrap_or_default();
    let agent = value_as_string(record, "agent").unwrap_or_default();
    let (method, path, protocol) = split_request(&request);
    Some(NginxTemplate {
        ip,
        timestamp,
        method,
        path,
        protocol,
        referer,
        agent,
        status,
        bytes,
    })
}

fn split_request(request: &str) -> (String, String, String) {
    let mut iter = request.split_whitespace();
    let method = iter.next().unwrap_or("").to_string();
    let path = iter.next().unwrap_or("").to_string();
    let protocol = iter.next().unwrap_or("").to_string();
    (method, path, protocol)
}

fn value_as_string(record: &DataRecord, key: &str) -> Option<String> {
    record.get_value(key).map(|val| val.to_string())
}

fn value_as_digit(record: &DataRecord, key: &str) -> Option<i64> {
    match record.get_value(key)? {
        Value::Digit(d) => Some(*d),
        other => other.to_string().parse().ok(),
    }
}

fn fallback_parse(line: &str) -> NginxTemplate {
    let parts: Vec<&str> = line.split('"').collect();
    //let head = parts.get(0).copied().unwrap_or("");
    let head = parts.first().copied().unwrap_or("");
    let request = parts.get(1).copied().unwrap_or("");
    let status_chunk = parts.get(2).copied().unwrap_or("");
    let referer = parts.get(3).copied().unwrap_or("");
    let agent = parts.get(5).copied().unwrap_or("");

    let ip = head.split_whitespace().next().unwrap_or("").to_string();
    let timestamp = head
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .unwrap_or("")
        .trim()
        .to_string();

    let mut req_parts = request.split_whitespace();
    let method = req_parts.next().unwrap_or("").to_string();
    let path = req_parts.next().unwrap_or("").to_string();
    let protocol = req_parts.next().unwrap_or("").to_string();

    let mut status_parts = status_chunk.split_whitespace();
    let status = status_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let bytes = status_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);

    NginxTemplate {
        ip,
        timestamp,
        method,
        path,
        protocol,
        referer: referer.to_string(),
        agent: agent.to_string(),
        status,
        bytes,
    }
}

fn variant_ip(base: &str, idx: usize) -> String {
    if let Some((prefix, _last)) = base.rsplit_once('.') {
        format!("{}.{}", prefix, (idx % 200) + 1)
    } else {
        base.to_string()
    }
}

fn variant_path(base: &str, idx: usize) -> String {
    format!("{}?v={}", base, idx % 1024)
}

fn variant_referer(base: &str, idx: usize) -> String {
    format!("{}?ref={}", base, idx % 256)
}

fn variant_timestamp(base: &str, idx: usize) -> String {
    if let Some((stamp, tz)) = base.rsplit_once(' ')
        && let Some((head, sec)) = stamp.rsplit_once(':')
        && let Ok(sec_val) = sec.parse::<u32>()
    {
        let rotated = (sec_val + (idx as u32 % 60)) % 60;
        return format!("{}:{:02} {}", head, rotated, tz);
    }
    base.to_string()
}

fn variant_status(idx: usize, base: i64) -> i64 {
    match idx % 10 {
        0 => 502,
        1 => 404,
        2 => 499,
        3 => 418,
        _ => base,
    }
}

fn variant_bytes(idx: usize, base: i64) -> i64 {
    base + (idx as i64 % 400)
}

fn append_chars(record: &mut DataRecord, name: &str, value: String) {
    record.append(DataField::from_chars(name.to_string(), value));
}
