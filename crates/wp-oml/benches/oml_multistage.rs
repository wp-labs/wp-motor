use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

fn build_model(code: &str) -> ObjModel {
    let mut code_ref = code;
    oml_parse_raw(&mut code_ref).expect("parse OML model")
}

/// Single stage: Input → Transform → Output
fn bench_single_stage(c: &mut Criterion) {
    // OML with 40% static fields (4 out of 10)
    let static_oml = r#"
name : single_stage_static
---
static {
    HOST = chars("192.168.1.1");
    PORT = digit(8080);
    STATUS_OK = chars("success");
    LEVEL_INFO = chars("info");
}

host = HOST;
port = PORT;
status = STATUS_OK;
level = LEVEL_INFO;
timestamp = read(ts);
user = read(uid);
action = read(act);
duration = read(dur);
bytes = read(size);
result = read(res);
"#;

    let no_static_oml = r#"
name : single_stage_no_static
---
host = chars("192.168.1.1");
port = digit(8080);
status = chars("success");
level = chars("info");
timestamp = read(ts);
user = read(uid);
action = read(act);
duration = read(dur);
bytes = read(size);
result = read(res);
"#;

    let mdl_static = build_model(static_oml);
    let mdl_no_static = build_model(no_static_oml);

    let input = DataRecord::from(vec![
        DataField::from_chars("ts", "2024-01-01 10:00:00"),
        DataField::from_chars("uid", "user123"),
        DataField::from_chars("act", "login"),
        DataField::from_digit("dur", 150),
        DataField::from_digit("size", 1024),
        DataField::from_chars("res", "ok"),
    ]);

    let mut group = c.benchmark_group("single_stage");

    group.bench_function("with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_no_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });
}

/// Multi-stage: Input → Stage1 → Stage2 → Stage3 → Stage4 → Output
fn bench_multi_stage(c: &mut Criterion) {
    // Stage 1: Parse and normalize
    let stage1_static = r#"
name : stage1_parse
---
static {
    HOST_LOCAL = chars("localhost");
    PORT_DEFAULT = digit(80);
    STATUS_OK = chars("ok");
    LEVEL_INFO = chars("info");
}

host = HOST_LOCAL;
port = PORT_DEFAULT;
status = STATUS_OK;
level = LEVEL_INFO;
raw_ts = read(timestamp);
raw_user = read(user);
"#;

    // Stage 2: Enrich
    let stage2_static = r#"
name : stage2_enrich
---
static {
    REGION_US = chars("us-west");
    TIER_FREE = chars("free");
    VERSION_V1 = chars("v1.0");
}

host = read(host);
port = read(port);
status = read(status);
level = read(level);
raw_ts = read(raw_ts);
raw_user = read(raw_user);
region = REGION_US;
tier = TIER_FREE;
version = VERSION_V1;
"#;

    // Stage 3: Filter
    let stage3_static = r#"
name : stage3_filter
---
static {
    FILTERED_STATUS = chars("filtered");
}

host = read(host);
port = read(port);
status = read(status);
level = read(level);
raw_ts = read(raw_ts);
raw_user = read(raw_user);
region = read(region);
tier = read(tier);
version = read(version);
filter_status = FILTERED_STATUS;
"#;

    // Stage 4: Aggregate
    let stage4_static = r#"
name : stage4_aggregate
---
static {
    AGG_TYPE = chars("count");
    METRIC_NAME = chars("requests");
}

host = read(host);
port = read(port);
status = read(status);
level = read(level);
region = read(region);
tier = read(tier);
version = read(version);
agg_type = AGG_TYPE;
metric = METRIC_NAME;
"#;

    let mdl_stage1 = build_model(stage1_static);
    let mdl_stage2 = build_model(stage2_static);
    let mdl_stage3 = build_model(stage3_static);
    let mdl_stage4 = build_model(stage4_static);

    // No-static versions (for comparison)
    let stage1_no_static = r#"
name : stage1_parse
---
host = chars("localhost");
port = digit(80);
status = chars("ok");
level = chars("info");
raw_ts = read(timestamp);
raw_user = read(user);
"#;

    let stage2_no_static = r#"
name : stage2_enrich
---
host = read(host);
port = read(port);
status = read(status);
level = read(level);
raw_ts = read(raw_ts);
raw_user = read(raw_user);
region = chars("us-west");
tier = chars("free");
version = chars("v1.0");
"#;

    let stage3_no_static = r#"
name : stage3_filter
---
host = read(host);
port = read(port);
status = read(status);
level = read(level);
raw_ts = read(raw_ts);
raw_user = read(raw_user);
region = read(region);
tier = read(tier);
version = read(version);
filter_status = chars("filtered");
"#;

    let stage4_no_static = r#"
name : stage4_aggregate
---
host = read(host);
port = read(port);
status = read(status);
level = read(level);
region = read(region);
tier = read(tier);
version = read(version);
agg_type = chars("count");
metric = chars("requests");
"#;

    let mdl_stage1_ns = build_model(stage1_no_static);
    let mdl_stage2_ns = build_model(stage2_no_static);
    let mdl_stage3_ns = build_model(stage3_no_static);
    let mdl_stage4_ns = build_model(stage4_no_static);

    let input = DataRecord::from(vec![
        DataField::from_chars("timestamp", "2024-01-01 10:00:00"),
        DataField::from_chars("user", "user123"),
    ]);

    let mut group = c.benchmark_group("multi_stage");

    // 2-stage pipeline
    group.bench_function("2stage_with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let stage1 = mdl_stage1.transform(data, &mut cache);
                mdl_stage2.transform(stage1, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("2stage_without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let stage1 = mdl_stage1_ns.transform(data, &mut cache);
                mdl_stage2_ns.transform(stage1, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    // 4-stage pipeline
    group.bench_function("4stage_with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let stage1 = mdl_stage1.transform(data, &mut cache);
                let stage2 = mdl_stage2.transform(stage1, &mut cache);
                let stage3 = mdl_stage3.transform(stage2, &mut cache);
                mdl_stage4.transform(stage3, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("4stage_without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let stage1 = mdl_stage1_ns.transform(data, &mut cache);
                let stage2 = mdl_stage2_ns.transform(stage1, &mut cache);
                let stage3 = mdl_stage3_ns.transform(stage2, &mut cache);
                mdl_stage4_ns.transform(stage3, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });
}

/// Stress test: 10 million references to same static value
fn bench_high_reuse(c: &mut Criterion) {
    let static_oml = r#"
name : high_reuse
---
static {
    CONST_VAL = chars("constant_value_that_is_reused_many_times");
}

v1 = CONST_VAL;
v2 = CONST_VAL;
v3 = CONST_VAL;
v4 = CONST_VAL;
v5 = CONST_VAL;
v6 = CONST_VAL;
v7 = CONST_VAL;
v8 = CONST_VAL;
v9 = CONST_VAL;
v10 = CONST_VAL;
"#;

    let no_static_oml = r#"
name : high_reuse_no_static
---
v1 = chars("constant_value_that_is_reused_many_times");
v2 = chars("constant_value_that_is_reused_many_times");
v3 = chars("constant_value_that_is_reused_many_times");
v4 = chars("constant_value_that_is_reused_many_times");
v5 = chars("constant_value_that_is_reused_many_times");
v6 = chars("constant_value_that_is_reused_many_times");
v7 = chars("constant_value_that_is_reused_many_times");
v8 = chars("constant_value_that_is_reused_many_times");
v9 = chars("constant_value_that_is_reused_many_times");
v10 = chars("constant_value_that_is_reused_many_times");
"#;

    let mdl_static = build_model(static_oml);
    let mdl_no_static = build_model(no_static_oml);

    let input = DataRecord::default();

    let mut group = c.benchmark_group("high_reuse");

    group.bench_function("with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_no_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_single_stage,
    bench_multi_stage,
    bench_high_reuse
);
criterion_main!(benches);
