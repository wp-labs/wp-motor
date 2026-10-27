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
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::fmt_def::TextFmt;
use wp_model_core::model::{DataRecord, Value};
use wp_stat::{ReportVariant, StatReq, StatStage, StatTarget};

#[tokio::test(flavor = "current_thread")]
async fn test_tags_injection_into_record() {
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
    let rule = crate::sinks::ProcMeta::WplName("/test/rule".to_string());
    let fds = Arc::new(DataRecord::default());

    let batch = vec![SinkRecUnit::with_record(
        pkg_id,
        rule.clone(),
        Arc::clone(&fds),
    )];
    let out = disp
        .oml_proc_batch_async(batch, &infra, &mut cache, &rule)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].len(), 1);
    let rec = out[0][0].data();
    // Expect keys injected as top-level fields
    assert!(rec.items.len() >= 3);
    // Check that the expected values are present
    let values: Vec<&Value> = rec.items.iter().map(|f| f.get_value()).collect();
    assert!(values.contains(&&Value::from("v1"))); // k1 value
    assert!(values.contains(&&Value::from("v2"))); // k2 value
    assert!(values.contains(&&Value::from("true"))); // flag value
}

#[tokio::test(flavor = "current_thread")]
async fn filter_expect_true_routes_on_true() {
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
    let rule = crate::sinks::ProcMeta::WplName("/r".to_string());
    let mut rec = DataRecord::default();
    rec.append(wp_model_core::model::DataField::from_chars("flag", "yes"));
    let batch = vec![SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec))];
    let out = disp
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn filter_expect_false_routes_on_false() {
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
    let rule = crate::sinks::ProcMeta::WplName("/r".to_string());
    let mut rec = DataRecord::default();
    rec.append(wp_model_core::model::DataField::from_chars("flag", "no"));
    let batch = vec![SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec))];
    let out = disp
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn fast_path_handles_multiple_sinks_without_transform() {
    use wp_model_core::model::DataField;

    let sink_conf1 = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Raw, None);
    let sink_rt1 = SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf1.clone(),
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let mut sink_conf2 = SinkInstanceConf::null_new("s2".to_string(), TextFmt::Raw, None);
    sink_conf2.set_tags(vec!["tag_key: val".to_string()]);
    let sink_rt2 = SinkRuntime::new(
        "./rescue".to_string(),
        "s2".to_string(),
        sink_conf2.clone(),
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    );

    let group = FlexGroup::build_conf("g", vec![sink_conf1, sink_conf2]);
    let mut disp = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    disp.append(sink_rt1);
    disp.append(sink_rt2);

    let mut record = DataRecord::default();
    record.append(DataField::from_chars("k", "v"));
    let shared = Arc::new(record);
    let rule = crate::sinks::ProcMeta::WplName("/fast".to_string());
    let mut cache = FieldQueryCache::default();
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::clone(&shared),
    )];
    let outputs = disp
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 1);
    assert_eq!(outputs[1].len(), 1);
    assert!(Arc::ptr_eq(&shared, outputs[0][0].data()));
    // 第二条 sink 需要 tags，应获得新的实例。
    assert!(!Arc::ptr_eq(&shared, outputs[1][0].data()));
    assert!(outputs[1][0].data().items.len() > shared.items.len());
}

#[tokio::test(flavor = "current_thread")]
async fn batch_fast_path_replicates_records_with_tags() {
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
    let rule = crate::sinks::ProcMeta::WplName("/fast".to_string());
    let batch = vec![
        SinkRecUnit::with_record(1, rule.clone(), Arc::new(record1)),
        SinkRecUnit::with_record(2, rule.clone(), Arc::new(record2)),
    ];

    let mut cache = FieldQueryCache::default();
    let outputs = disp
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 2);
    assert_eq!(outputs[1].len(), 2);
    assert_eq!(outputs[0][0].data().items.len(), base_len);
    // 第二个 sink 注入 tag，应多出一列
    assert_eq!(outputs[1][0].data().items.len(), base_len + 1);
}

#[tokio::test(flavor = "current_thread")]
async fn fast_path_global_tags_override_sink_tags() {
    use crate::core::parser::global_tags::GlobalTags;
    use wp_model_core::model::DataField;

    let mut group = FlexGroup::default();
    group.name = "fast_global".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "tenant_id".to_string(),
        "alice".to_string(),
    )])));

    let mut sink_conf = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    sink_conf.set_tags(vec![
        "env: dev".to_string(),
        "tenant_id: from-sink".to_string(),
    ]);
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    ));

    let mut rec = DataRecord::default();
    rec.append(DataField::from_chars("k", "v"));
    // 模拟解析阶段已盖上的全局 tag，随后叠加同名 sink tag
    rec.append(DataField::from_chars("tenant_id", "alice"));
    let rule = crate::sinks::ProcMeta::WplName("/fast".to_string());
    let batch = vec![SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec))];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].len(), 1);
    let record = outputs[0][0].data();
    assert_eq!(
        record
            .items
            .iter()
            .filter(|f| f.get_name() == "tenant_id")
            .count(),
        1
    );
    assert!(matches!(record.get_value("tenant_id"), Some(Value::Chars(v)) if v == "alice"));
    assert!(matches!(record.get_value("env"), Some(Value::Chars(v)) if v == "dev"));
}

#[tokio::test(flavor = "current_thread")]
async fn batch_routing_respects_sink_conditions() {
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
    let rule = crate::sinks::ProcMeta::WplName("/batch/cond".to_string());
    let batch = vec![
        SinkRecUnit::with_record(10, rule.clone(), Arc::new(rec_yes)),
        SinkRecUnit::with_record(11, rule.clone(), Arc::new(rec_no)),
    ];

    let mut cache = FieldQueryCache::default();
    let outputs = disp
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 1);
    assert_eq!(outputs[0][0].id(), &10);
    assert_eq!(outputs[1].len(), 1);
    assert_eq!(outputs[1][0].id(), &11);
}

#[tokio::test(flavor = "current_thread")]
async fn batch_oml_transforms_records_for_all_sinks() {
    use wp_model_core::model::DataField;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : batch_oml_model
rule :
    /batch/oml
---
converted : chars = chars(done) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
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

    let rule = crate::sinks::ProcMeta::WplName("/batch/oml".to_string());
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
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    for (idx, units) in outputs.iter().enumerate() {
        assert_eq!(units.len(), 2);
        for unit in units {
            assert!(matches!(
                unit.meta(),
                crate::sinks::ProcMeta::OmlName(tag) if tag == "batch_oml_model"
            ));
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

#[tokio::test(flavor = "current_thread")]
async fn batch_oml_transforms_records_for_all_sinks_async() {
    use wp_model_core::model::DataField;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : batch_oml_model_async
rule :
    /batch/oml_async
---
converted : chars = chars(done) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "oml_async".to_string();
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

    let rule = crate::sinks::ProcMeta::WplName("/batch/oml_async".to_string());
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
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
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

#[tokio::test(flavor = "current_thread")]
async fn oml_output_reapplies_global_tags_and_overrides_model_field() {
    use crate::core::parser::global_tags::GlobalTags;
    use wp_model_core::model::DataField;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : global_tag_model
rule :
    /batch/global_tag
---
tenant_id : chars = chars(model_tenant) ;
log_type : chars = chars(raw_log) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "global_tag".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![
        ("tenant_id".to_string(), "alice".to_string()),
        ("wist-gw_id".to_string(), "gw-local".to_string()),
    ])));

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
    // sink 自带 tag 不得覆盖全局 tag：同名时以全局值为准，且只保留一个键。
    sink_conf_b.set_tags(vec![
        "env: dev".to_string(),
        "tenant_id: from-sink".to_string(),
    ]);
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

    let rule = crate::sinks::ProcMeta::WplName("/batch/global_tag".to_string());
    let mut rec = DataRecord::default();
    rec.append(DataField::from_chars("src", "alpha"));
    let batch = vec![SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec))];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    for (idx, units) in outputs.iter().enumerate() {
        assert_eq!(units.len(), 1);
        let record = units[0].data();
        // 全局 tag 覆盖模型里写死的同名字段，且不产生重复键
        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == "tenant_id")
                .count(),
            1
        );
        assert!(matches!(record.get_value("tenant_id"), Some(Value::Chars(v)) if v == "alice"));
        assert!(matches!(record.get_value("wist-gw_id"), Some(Value::Chars(v)) if v == "gw-local"));
        // 模型自身的输出字段保留
        assert!(matches!(record.get_value("log_type"), Some(Value::Chars(v)) if v == "raw_log"));
        // sink 自带 tag 仍然存在（仅第二个 sink 配置了 env）
        assert_eq!(record.get_value("env").is_some(), idx == 1);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn global_tags_are_visible_to_sink_conditions() {
    use crate::core::parser::global_tags::GlobalTags;

    let mut group = FlexGroup::default();
    group.name = "global_tag_cond".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "tier".to_string(),
        "gold".to_string(),
    )])));

    let mut hit_conf = SinkInstanceConf::null_new("hit".to_string(), TextFmt::Json, None);
    hit_conf.set_filter_expect(true);
    let hit_cond = TCondParser::exp(&mut "$tier == chars(gold)".to_string().as_str()).unwrap();
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "hit".to_string(),
        hit_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(hit_cond),
        Vec::new(),
    ));

    let mut miss_conf = SinkInstanceConf::null_new("miss".to_string(), TextFmt::Json, None);
    miss_conf.set_filter_expect(true);
    let miss_cond = TCondParser::exp(&mut "$tier == chars(silver)".to_string().as_str()).unwrap();
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "miss".to_string(),
        miss_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(miss_cond),
        Vec::new(),
    ));

    let rule = crate::sinks::ProcMeta::WplName("/r".to_string());
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::new(DataRecord::default()),
    )];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].len(), 1, "cond 应能看到补盖的全局 tag");
    assert_eq!(outputs[1].len(), 0, "不匹配的 cond 不应被投递");

    let record = outputs[0][0].data();
    assert!(matches!(record.get_value("tier"), Some(Value::Chars(v)) if v == "gold"));
}

#[tokio::test(flavor = "current_thread")]
async fn global_tag_override_decides_cond_routing() {
    use crate::core::parser::global_tags::GlobalTags;

    // 模型固定输出 env=prod；全局 tag 把 env 改成 dev 后再参与 cond 判断
    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : env_override_model
rule :
    /batch/env_override
---
env : chars = chars(prod) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "env_override".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "env".to_string(),
        "dev".to_string(),
    )])));

    let mut sink_conf = SinkInstanceConf::null_new("dev_sink".to_string(), TextFmt::Json, None);
    sink_conf.set_filter_expect(true);
    let cond = TCondParser::exp(&mut "$env == chars(dev)".to_string().as_str()).unwrap();
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "dev_sink".to_string(),
        sink_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        Some(cond),
        Vec::new(),
    ));

    let rule = crate::sinks::ProcMeta::WplName("/batch/env_override".to_string());
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::new(DataRecord::default()),
    )];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    // 若 cond 在覆盖前求值，模型输出的 prod 不会命中 dev
    assert_eq!(outputs[0].len(), 1, "cond 应在全局 tag 覆盖之后求值");
    assert!(matches!(
        outputs[0][0].data().get_value("env"),
        Some(Value::Chars(v)) if v == "dev"
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn oml_failure_record_gets_global_tags() {
    use crate::core::parser::global_tags::GlobalTags;
    use crate::facade::test_helpers::SinkTerminal;
    use crate::sinks::SinkGroupAgent;
    use wp_model_core::model::DataField;

    let mut sink_res = SinkResUnit::use_null();
    // 读取输入中不存在的字段 -> 模型输出为空 -> 进失败（error sink）分支
    let mut code = r#"
name : empty_oml_model
rule :
    /batch/empty
---
missing : chars = read(does_not_exist) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "empty".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "tenant_id".to_string(),
        "alice".to_string(),
    )])));

    let sink_conf_a = SinkInstanceConf::null_new("sink_a".to_string(), TextFmt::Json, None);
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "sink_a".to_string(),
        sink_conf_a,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    ));

    // 用一个真实 channel 充当 infra error sink，捕获失败记录
    let (err_tx, mut err_rx) = tokio::sync::mpsc::channel(4);
    let infra = crate::sinks::InfraSinkAgent {
        default: SinkGroupAgent::null(),
        miss: SinkGroupAgent::null(),
        residue: SinkGroupAgent::null(),
        moni: SinkGroupAgent::null(),
        error: SinkGroupAgent::new(
            SinkGroupConf::Flexi(FlexGroup::default()),
            SinkTerminal::Channel(err_tx),
        ),
    };

    let rule = crate::sinks::ProcMeta::WplName("/batch/empty".to_string());
    let mut rec = DataRecord::default();
    rec.append(DataField::from_chars("src", "alpha"));
    let batch = vec![SinkRecUnit::with_record(1, rule.clone(), Arc::new(rec))];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &infra, &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].is_empty(), "空输出不应下发到业务 sink");

    let package = err_rx.try_recv().expect("error sink 应收到失败记录");
    assert_eq!(package.len(), 1);
    let record = package[0].data();
    assert_eq!(
        record
            .items
            .iter()
            .filter(|f| f.get_name() == "tenant_id")
            .count(),
        1
    );
    assert!(matches!(record.get_value("tenant_id"), Some(Value::Chars(v)) if v == "alice"));
    assert!(matches!(
        record.get_value("__err_kind"),
        Some(Value::Chars(v)) if v == "oml_transform_empty"
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn empty_global_tags_do_not_change_oml_output() {
    use crate::core::parser::global_tags::GlobalTags;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : empty_tags_model
rule :
    /batch/empty_tags
---
converted : chars = chars(done) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "empty_tags".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);
    // 配了 Some(empty) 也不应该多出字段
    dispatcher.set_global_tags(Arc::new(GlobalTags::default()));

    let sink_conf = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    ));

    let rule = crate::sinks::ProcMeta::WplName("/batch/empty_tags".to_string());
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::new(DataRecord::default()),
    )];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].len(), 1);
    assert_eq!(outputs[0][0].data().items.len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn fast_path_keeps_zero_copy_when_no_sink_tags() {
    use crate::core::parser::global_tags::GlobalTags;
    use wp_model_core::model::DataField;

    let mut group = FlexGroup::default();
    group.name = "fast_zero_copy".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "tenant_id".to_string(),
        "alice".to_string(),
    )])));

    // 无 sink tags：应继续共享 base Arc，不因全局 tag 而多一次 clone
    let sink_conf = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    ));

    let mut rec = DataRecord::default();
    rec.append(DataField::from_chars("k", "v"));
    rec.append(DataField::from_chars("tenant_id", "alice"));
    let shared = Arc::new(rec);
    let rule = crate::sinks::ProcMeta::WplName("/fast".to_string());
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::clone(&shared),
    )];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs[0].len(), 1);
    assert!(Arc::ptr_eq(&shared, outputs[0][0].data()));
}

#[tokio::test(flavor = "current_thread")]
async fn oml_global_tags_coexist_with_non_conflicting_sink_tags() {
    use crate::core::parser::global_tags::GlobalTags;

    let mut sink_res = SinkResUnit::use_null();
    let mut code = r#"
name : coexist_model
rule :
    /batch/coexist
---
log_type : chars = chars(raw_log) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml model");
    sink_res.push_model(DataModel::Object(model));

    let mut group = FlexGroup::default();
    group.name = "coexist".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), sink_res);
    dispatcher.set_global_tags(Arc::new(GlobalTags::from_pairs(vec![(
        "tenant_id".to_string(),
        "alice".to_string(),
    )])));

    let mut sink_conf = SinkInstanceConf::null_new("s1".to_string(), TextFmt::Json, None);
    sink_conf.set_tags(vec!["env: dev".to_string()]);
    dispatcher.append(SinkRuntime::new(
        "./rescue".to_string(),
        "s1".to_string(),
        sink_conf,
        SinkBackendType::Proxy(crate::sinks::builtin_factories::make_blackhole_sink()),
        None,
        Vec::new(),
    ));

    let rule = crate::sinks::ProcMeta::WplName("/batch/coexist".to_string());
    let batch = vec![SinkRecUnit::with_record(
        1,
        rule.clone(),
        Arc::new(DataRecord::default()),
    )];

    let mut cache = FieldQueryCache::default();
    let outputs = dispatcher
        .oml_proc_batch_async(batch, &InfraSinkAgent::use_null(), &mut cache, &rule)
        .await
        .unwrap();

    assert_eq!(outputs[0].len(), 1);
    let record = outputs[0][0].data();
    let names: Vec<&str> = record.items.iter().map(|f| f.get_name()).collect();
    // 无冲突时不重新 replace，字段顺序保持“模型输出 -> 全局 tag -> sink tag”
    assert_eq!(names, vec!["log_type", "tenant_id", "env"]);
    assert!(matches!(record.get_value("tenant_id"), Some(Value::Chars(v)) if v == "alice"));
    assert!(matches!(record.get_value("env"), Some(Value::Chars(v)) if v == "dev"));
}

#[test]
fn ingress_stat_records_group_recv_batch() {
    let mut group = FlexGroup::default();
    group.name = "all_static".to_string();
    let mut dispatcher = SinkDispatcher::new(SinkGroupConf::Flexi(group), SinkResUnit::use_null());
    dispatcher.set_ingress_stat_target(
        0,
        2,
        vec![StatReq {
            stage: StatStage::Sink,
            name: "sink_stat".to_string(),
            target: StatTarget::All,
            collect: Vec::new(),
            max: 16,
        }],
    );
    dispatcher.record_ingress_batch(7);

    let rt = tokio::runtime::Runtime::new().expect("build runtime");
    let (mon_tx, mut mon_rx) = tokio::sync::mpsc::channel(2);
    rt.block_on(async {
        dispatcher
            .send_ingress_stat(&mon_tx)
            .await
            .expect("send ingress stat");
    });
    let report = rt.block_on(async {
        match mon_rx.recv().await {
            Some(ReportVariant::Stat(report)) => report,
            None => panic!("missing ingress stat report"),
        }
    });

    assert_eq!(report.get_name(), "sink_stat");
    assert_eq!(report.target_display(), "all_static#0@recv");
    assert_eq!(report.get_data().len(), 1);
    assert_eq!(report.get_data()[0].stat.total, 7);
    assert_eq!(report.get_data()[0].stat.success, 7);
}

// 隐私相关逻辑与字段已移除：对应行为测试一并删除
