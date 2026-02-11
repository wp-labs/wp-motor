use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

fn build_model(code: &str) -> ObjModel {
    let mut code_ref = code;
    oml_parse_raw(&mut code_ref).expect("parse OML model for static bench")
}

fn bench_static_vs_temp(c: &mut Criterion) {
    let static_oml = r#"
name : bench_static
---
static {
    tpl = object {
        id = chars(E1);
        tpl = chars('tpl text')
    };
}

target = match read(Content) {
    starts_with('foo') => tpl;
    _ => tpl;
};
EventId = read(target) | get(id);
EventTemplate = read(target) | get(tpl);
"#;

    let temp_oml = r#"
name : bench_temp
---
__E1 = object {
    id = chars(E1);
    tpl = chars('tpl text')
};

target = match read(Content) {
    starts_with('foo') => read(__E1);
    _ => read(__E1);
};
EventId = read(target) | get(id);
EventTemplate = read(target) | get(tpl);
"#;

    let mdl_static = build_model(static_oml);
    let mdl_temp = build_model(temp_oml);

    let input = DataRecord::from(vec![DataField::from_chars("Content", "foo message")]);

    let mut group = c.benchmark_group("oml_static_vs_temp");

    group.bench_function("static_block", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("temp_field", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_temp.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });
}

fn bench_multi_stage(c: &mut Criterion) {
    // Correct scenario: Static variables defined once, referenced directly in each stage
    // This tests Arc sharing across multiple transform stages

    // WITH STATIC: Define constants once, reference directly in all stages
    let stage1_static = r#"
name : stage1
---
static {
    HOST = chars("prod-server-01");
    ENV = chars("production");
    REGION = chars("us-west-2");
    VERSION = chars("v2.5.1");
}
host = HOST;
environment = ENV;
region = REGION;
version = VERSION;
timestamp = read(ts);
message = read(msg);
"#;

    // Stage 2-4 continue to reference the same static symbols (Arc shared)
    let stage2_static = r#"
name : stage2
---
static {
    HOST = chars("prod-server-01");
    ENV = chars("production");
    REGION = chars("us-west-2");
    VERSION = chars("v2.5.1");
}
host = HOST;
environment = ENV;
region = REGION;
version = VERSION;
timestamp = read(timestamp);
message = read(message);
level = chars("info");
"#;

    let stage3_static = r#"
name : stage3
---
static {
    HOST = chars("prod-server-01");
    ENV = chars("production");
    REGION = chars("us-west-2");
    VERSION = chars("v2.5.1");
}
host = HOST;
environment = ENV;
region = REGION;
version = VERSION;
timestamp = read(timestamp);
message = read(message);
level = read(level);
category = chars("application");
"#;

    let stage4_static = r#"
name : stage4
---
static {
    HOST = chars("prod-server-01");
    ENV = chars("production");
    REGION = chars("us-west-2");
    VERSION = chars("v2.5.1");
}
host = HOST;
environment = ENV;
region = REGION;
version = VERSION;
timestamp = read(timestamp);
message = read(message);
level = read(level);
category = read(category);
"#;

    // WITHOUT STATIC: Re-create values in each stage (deep copy each time)
    let stage1_no_static = r#"
name : stage1
---
host = chars("prod-server-01");
environment = chars("production");
region = chars("us-west-2");
version = chars("v2.5.1");
timestamp = read(ts);
message = read(msg);
"#;

    let stage2_no_static = r#"
name : stage2
---
host = chars("prod-server-01");
environment = chars("production");
region = chars("us-west-2");
version = chars("v2.5.1");
timestamp = read(timestamp);
message = read(message);
level = chars("info");
"#;

    let stage3_no_static = r#"
name : stage3
---
host = chars("prod-server-01");
environment = chars("production");
region = chars("us-west-2");
version = chars("v2.5.1");
timestamp = read(timestamp);
message = read(message);
level = read(level);
category = chars("application");
"#;

    let stage4_no_static = r#"
name : stage4
---
host = chars("prod-server-01");
environment = chars("production");
region = chars("us-west-2");
version = chars("v2.5.1");
timestamp = read(timestamp);
message = read(message);
level = read(level);
category = read(category);
"#;

    let mdl_stage1 = build_model(stage1_static);
    let mdl_stage2 = build_model(stage2_static);
    let mdl_stage3 = build_model(stage3_static);
    let mdl_stage4 = build_model(stage4_static);

    let mdl_stage1_ns = build_model(stage1_no_static);
    let mdl_stage2_ns = build_model(stage2_no_static);
    let mdl_stage3_ns = build_model(stage3_no_static);
    let mdl_stage4_ns = build_model(stage4_no_static);

    let input = DataRecord::from(vec![
        DataField::from_chars("ts", "2024-01-01T10:00:00Z"),
        DataField::from_chars("msg", "Request processed successfully"),
    ]);

    let mut group = c.benchmark_group("multi_stage_pipeline");

    group.bench_function("4_stages_with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let s1 = mdl_stage1.transform(data, &mut cache);
                let s2 = mdl_stage2.transform(s1, &mut cache);
                let s3 = mdl_stage3.transform(s2, &mut cache);
                mdl_stage4.transform(s3, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("4_stages_without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let s1 = mdl_stage1_ns.transform(data, &mut cache);
                let s2 = mdl_stage2_ns.transform(s1, &mut cache);
                let s3 = mdl_stage3_ns.transform(s2, &mut cache);
                mdl_stage4_ns.transform(s3, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("2_stages_with_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let s1 = mdl_stage1.transform(data, &mut cache);
                mdl_stage2.transform(s1, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("2_stages_without_static", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| {
                let s1 = mdl_stage1_ns.transform(data, &mut cache);
                mdl_stage2_ns.transform(s1, &mut cache)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_static_vs_temp, bench_multi_stage);
criterion_main!(benches);
