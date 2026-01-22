use super::SinkDispatcher;
use crate::resources::SinkResUnit;
use crate::sinks::SinkBackendType;
use crate::sinks::SinkRecUnit;
use crate::sinks::SinkRuntime;
use crate::sinks::routing::agent::InfraSinkAgent;
// use crate::stat::{MonRecv, MonSend};
// use crate::types::AnyResult;
use oml::language::DataModel;
use oml::parser::oml_parse_raw;
use orion_overload::append::Appendable;
use std::sync::Arc;
use wp_conf::TCondParser;
use wp_conf::structure::SinkInstanceConf;
use wp_conf::structure::{FlexGroup, SinkGroupConf};
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::fmt_def::TextFmt;
use wp_model_core::model::{DataRecord, Value};

#[test]
fn test_tags_injection_into_record() {
    // Build a sink conf with tags
    let mut sconf = SinkInstanceConf::null_new("t1".to_string(), TextFmt::Json, None);
    sconf.set_tags(vec![
        "k1: v1".to_string(),
        "flag".to_string(),
        "k2=v2".to_string(),
    ]);
    // Sink runtime with no-op backend (no IO)
    let sink_rt = SinkRuntime::new(
        "./rescue".to_string(),
        "t1".to_string(),
        sconf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );
    // Dispatcher with empty resources
    let mut g = FlexGroup::default();
    g.name = "group".to_string();
    let mut disp = SinkDispatcher::new(SinkGroupConf::Flexi(g), SinkResUnit::use_null());
    disp.append(sink_rt);

    // Prepare inputs
    let pkg_id: wpl::PkgID = 1;
    let infra = InfraSinkAgent::use_null();
    let mut cache = FieldQueryCache::default();
    let rule = crate::sinks::ProcMeta::Rule("/test/rule".to_string());
    let fds = Arc::new(DataRecord::default());

    // Call the path where tags injection happens
    let out = disp
        .oml_proc(pkg_id, &infra, &mut cache, &rule, fds)
        .unwrap();
    assert_eq!(out.len(), 1);
    let (_rt, rec) = &out[0];
    // Expect keys injected as top-level fields
    // Since rec is now Record<Value>, we access values by index
    let rec = rec.as_ref();
    assert!(rec.items.len() >= 3);
    // Check that the expected values are present
    let values: Vec<&Value> = rec.items.iter().map(|f| f.get_value()).collect();
    assert!(values.contains(&&Value::from("v1"))); // k1 value
    assert!(values.contains(&&Value::from("v2"))); // k2 value
    assert!(values.contains(&&Value::from("true"))); // flag value
}

#[test]
fn filter_expect_true_routes_on_true() {
    // Build sink with cond: $flag == chars(yes), expect true
    let mut sconf =
        wp_conf::structure::SinkInstanceConf::null_new("t2".to_string(), TextFmt::Json, None);
    sconf.set_filter_expect(true);
    let cond = TCondParser::exp(&mut "$flag == chars(yes)".to_string().as_str()).unwrap();
    let sink_rt = SinkRuntime::new(
        "./rescue".to_string(),
        "t2".to_string(),
        sconf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(cond),
        Vec::new(),
    );
    let mut g = wp_conf::structure::FlexGroup::default();
    g.name = "g".to_string();
    let mut disp = SinkDispatcher::new(
        wp_conf::structure::SinkGroupConf::Flexi(g),
        crate::resources::SinkResUnit::use_null(),
    );
    disp.append(sink_rt);

    let mut cache = FieldQueryCache::default();
    let rule = crate::sinks::ProcMeta::Rule("/r".to_string());
    let mut rec = DataRecord::default();
    rec.append(wp_model_core::model::DataField::from_chars("flag", "yes"));
    let out = disp
        .oml_proc(
            1,
            &InfraSinkAgent::use_null(),
            &mut cache,
            &rule,
            rec.into(),
        )
        .unwrap();
    assert_eq!(out.len(), 1);
}

#[test]
fn filter_expect_false_routes_on_false() {
    // Build sink with cond: $flag == chars(yes), expect false => deliver when flag != yes
    let mut sconf =
        wp_conf::structure::SinkInstanceConf::null_new("t3".to_string(), TextFmt::Json, None);
    sconf.set_filter_expect(false);
    let cond = TCondParser::exp(&mut "$flag == chars(yes)".to_string().as_str()).unwrap();
    let sink_rt = SinkRuntime::new(
        "./rescue".to_string(),
        "t3".to_string(),
        sconf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(cond),
        Vec::new(),
    );
    let mut g = wp_conf::structure::FlexGroup::default();
    g.name = "g".to_string();
    let mut disp = SinkDispatcher::new(
        wp_conf::structure::SinkGroupConf::Flexi(g),
        crate::resources::SinkResUnit::use_null(),
    );
    disp.append(sink_rt);

    let mut cache = FieldQueryCache::default();
    let rule = crate::sinks::ProcMeta::Rule("/r".to_string());
    let mut rec = DataRecord::default();
    rec.append(wp_model_core::model::DataField::from_chars("flag", "no"));
    let out = disp
        .oml_proc(
            1,
            &InfraSinkAgent::use_null(),
            &mut cache,
            &rule,
            rec.into(),
        )
        .unwrap();
    assert_eq!(out.len(), 1);
}

#[test]
fn fast_path_handles_multiple_sinks_without_transform() {
    use wp_model_core::model::DataField;

    let sink_conf1 = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    let sink_rt1 = SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf1,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut sink_conf2 = SinkInstanceConf::null_new("s2".to_string(), TextFmt::Json, None);
    sink_conf2.set_tags(vec!["tag_key: val".to_string()]);
    let sink_rt2 = SinkRuntime::new(
        "./rescue".to_string(),
        "s2".to_string(),
        sink_conf2,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut group = FlexGroup::default();
    group.name = "g".to_string();
    let mut disp = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    disp.append(sink_rt1);
    disp.append(sink_rt2);

    let mut record = DataRecord::default();
    record.append(DataField::from_chars("k", "v"));
    let shared = Arc::new(record);
    let rule = crate::sinks::ProcMeta::Rule("/fast".to_string());
    let mut cache = FieldQueryCache::default();
    let outputs = disp
        .oml_proc(
            1,
            &InfraSinkAgent::use_null(),
            &mut cache,
            &rule,
            Arc::clone(&shared),
        )
        .unwrap();

    assert_eq!(outputs.len(), 2);
    // 第一条 sink 直接复用输入 Arc
    assert!(Arc::ptr_eq(&shared, &outputs[0].1));
    // 第二条 sink 需要 tags，应获得新的实例
    assert!(!Arc::ptr_eq(&shared, &outputs[1].1));
    assert!(outputs[1].1.items.len() > shared.items.len());
}

#[test]
fn batch_fast_path_replicates_records_with_tags() {
    use wp_model_core::model::DataField;

    let sink_conf1 = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    let sink_rt1 = SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf1,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut sink_conf2 = SinkInstanceConf::null_new("s2".to_string(), TextFmt::Json, None);
    sink_conf2.set_tags(vec!["cluster: a".to_string()]);
    let sink_rt2 = SinkRuntime::new(
        "./rescue".to_string(),
        "s2".to_string(),
        sink_conf2,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut group = FlexGroup::default();
    group.name = "g".to_string();
    let mut disp = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    disp.append(sink_rt1);
    disp.append(sink_rt2);

    let mut record1 = DataRecord::default();
    record1.append(DataField::from_chars("k", "v"));
    let base_len = record1.items.len();
    let record2 = record1.clone();
    let rule = crate::sinks::ProcMeta::Rule("/fast".to_string());
    let batch = vec![
        SinkRecUnit::with_record(1, rule.clone(), Arc::new(record1)),
        SinkRecUnit::with_record(2, rule.clone(), Arc::new(record2)),
    ];

    let mut cache = FieldQueryCache::default();
    let outputs = disp
        .oml_proc_batch(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 2);
    assert_eq!(outputs[1].len(), 2);
    assert_eq!(outputs[0][0].data().items.len(), base_len);
    // 第二个 sink 注入 tag，应多出一列
    assert_eq!(outputs[1][0].data().items.len(), base_len + 1);
}

#[test]
fn batch_routing_respects_sink_conditions() {
    use wp_model_core::model::DataField;

    let mut sconf_true = SinkInstanceConf::null_new("s_true".to_string(), TextFmt::Json, None);
    sconf_true.set_filter_expect(true);
    let cond = TCondParser::exp(&mut "$flag == chars(yes)".to_string().as_str()).unwrap();
    let sink_true = SinkRuntime::new(
        "./rescue".to_string(),
        "s_true".to_string(),
        sconf_true,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(cond.clone()),
        Vec::new(),
    );

    let mut sconf_false = SinkInstanceConf::null_new("s_false".to_string(), TextFmt::Json, None);
    sconf_false.set_filter_expect(false);
    let sink_false = SinkRuntime::new(
        "./rescue".to_string(),
        "s_false".to_string(),
        sconf_false,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(cond),
        Vec::new(),
    );

    let mut group = FlexGroup::default();
    group.name = "cond".to_string();
    let mut disp = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    disp.append(sink_true);
    disp.append(sink_false);

    let mut rec_yes = DataRecord::default();
    rec_yes.append(DataField::from_chars("flag", "yes"));
    let mut rec_no = DataRecord::default();
    rec_no.append(DataField::from_chars("flag", "no"));
    let rule = crate::sinks::ProcMeta::Rule("/batch/cond".to_string());
    let batch = vec![
        SinkRecUnit::with_record(10, rule.clone(), Arc::new(rec_yes)),
        SinkRecUnit::with_record(11, rule.clone(), Arc::new(rec_no)),
    ];

    let mut cache = FieldQueryCache::default();
    let outputs = disp
        .oml_proc_batch(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 1);
    assert_eq!(outputs[0][0].id(), &10);
    assert_eq!(outputs[1].len(), 1);
    assert_eq!(outputs[1][0].id(), &11);
}

#[test]
fn batch_oml_transforms_records_for_all_sinks() {
    use wp_model_core::model::DataField;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : batch_oml_model
rule :
    /batch/oml
---
converted : chars = chars(done) ;
"#;
    let model = oml_parse_raw(&mut code).expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "oml".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);

    let sink_conf_a = SinkInstanceConf::null_new("sink_a".to_string(), TextFmt::Json, None);
    let sink_a = SinkRuntime::new(
        "./rescue".to_string(),
        "sink_a".to_string(),
        sink_conf_a,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut sink_conf_b = SinkInstanceConf::null_new("sink_b".to_string(), TextFmt::Json, None);
    sink_conf_b.set_tags(vec!["cluster: b".to_string()]);
    let sink_b = SinkRuntime::new(
        "./rescue".to_string(),
        "sink_b".to_string(),
        sink_conf_b,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    dispatcher.append(sink_a);
    dispatcher.append(sink_b);

    let rule = crate::sinks::ProcMeta::Rule("/batch/oml".to_string());
    let mut rec1 = DataRecord::default();
    rec1.append(DataField::from_chars("src", "alpha"));
    let mut rec2 = DataRecord::default();
    rec2.append(DataField::from_chars("src", "beta"));
    let batch = vec![
        SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec1)),
        SinkRecUnit::with_record(2, rule.clone(), Arc::new(rec2)),
    ];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .unwrap();

    assert_eq!(outputs.len(), 2);
    for (idx, units) in outputs.iter().enumerate() {
        assert_eq!(units.len(), 2);
        for unit in units {
            let record = unit.data();
            let converted = record.get_value("converted");
            assert!(matches!(converted, Some(Value::Chars(v)) if v == "done"));
            if idx == 0 {
                assert_eq!(record.items.len(), 1);
            } else {
                assert_eq!(record.items.len(), 2);
            }
        }
    }
}

// 隐私相关逻辑与字段已移除：对应行为测试一并删除
