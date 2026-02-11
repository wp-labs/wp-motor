use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

fn build_model(code: &str) -> ObjModel {
    let mut code_ref = code;
    oml_parse_raw(&mut code_ref).expect("parse OML model for batch bench")
}

fn create_test_records(count: usize) -> Vec<DataRecord> {
    (0..count)
        .map(|i| {
            DataRecord::from(vec![
                DataField::from_chars("ts", &format!("2024-01-01T10:00:{:02}Z", i % 60)),
                DataField::from_chars("msg", &format!("Request {} processed", i)),
                DataField::from_chars("level", if i % 3 == 0 { "error" } else { "info" }),
            ])
        })
        .collect()
}

fn bench_batch_vs_single(c: &mut Criterion) {
    // Test model with static variables (best case for batch processing)
    let model_oml = r#"
name : batch_test
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
level_value = read(level);
category = chars("application");
"#;

    let model = build_model(model_oml);

    let mut group = c.benchmark_group("batch_processing");

    // Benchmark single-record processing (baseline)
    group.bench_function("single_record", |b| {
        let mut cache = FieldQueryCache::default();
        let record = create_test_records(1).pop().unwrap();
        b.iter_batched(
            || record.clone(),
            |data| model.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });

    // Benchmark batch sizes
    for batch_size in [10, 100] {
        // Single-record loop (N times transform())
        group.bench_with_input(
            BenchmarkId::new("single_loop", batch_size),
            &batch_size,
            |b, &size| {
                let mut cache = FieldQueryCache::default();
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| {
                        data.into_iter()
                            .map(|record| model.transform(record, &mut cache))
                            .collect::<Vec<_>>()
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Batch processing (transform_batch())
        group.bench_with_input(
            BenchmarkId::new("batch_api", batch_size),
            &batch_size,
            |b, &size| {
                let mut cache = FieldQueryCache::default();
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| model.transform_batch(data, &mut cache),
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_batch_multi_stage(c: &mut Criterion) {
    // Multi-stage pipeline with batch processing
    let stage1 = r#"
name : stage1
---
static {
    HOST = chars("prod-server-01");
    ENV = chars("production");
}
host = HOST;
environment = ENV;
timestamp = read(ts);
message = read(msg);
"#;

    let stage2 = r#"
name : stage2
---
static {
    REGION = chars("us-west-2");
    VERSION = chars("v2.5.1");
}
host = read(host);
environment = read(environment);
region = REGION;
version = VERSION;
timestamp = read(timestamp);
message = read(message);
"#;

    let mdl_stage1 = build_model(stage1);
    let mdl_stage2 = build_model(stage2);

    let mut group = c.benchmark_group("batch_multi_stage");

    // Benchmark different batch sizes
    for batch_size in [10, 50, 100] {
        // Single-record pipeline
        group.bench_with_input(
            BenchmarkId::new("single_pipeline", batch_size),
            &batch_size,
            |b, &size| {
                let mut cache = FieldQueryCache::default();
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| {
                        data.into_iter()
                            .map(|record| {
                                let s1 = mdl_stage1.transform(record, &mut cache);
                                mdl_stage2.transform(s1, &mut cache)
                            })
                            .collect::<Vec<_>>()
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Batch pipeline
        group.bench_with_input(
            BenchmarkId::new("batch_pipeline", batch_size),
            &batch_size,
            |b, &size| {
                let mut cache = FieldQueryCache::default();
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| {
                        let s1 = mdl_stage1.transform_batch(data, &mut cache);
                        mdl_stage2.transform_batch(s1, &mut cache)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_batch_cache_benefit(c: &mut Criterion) {
    // Test scenario where cache reuse provides maximum benefit
    let model_oml = r#"
name : cache_test
---
static {
    DEFAULT_HOST = chars("default-host");
    DEFAULT_ENV = chars("production");
}

host = DEFAULT_HOST;
environment = DEFAULT_ENV;
timestamp = read(ts);
message = read(msg);
"#;

    let model = build_model(model_oml);

    let mut group = c.benchmark_group("batch_cache_benefit");

    // Test with different batch sizes
    for batch_size in [1, 10, 50, 100] {
        // Fresh cache per record (worst case)
        group.bench_with_input(
            BenchmarkId::new("fresh_cache", batch_size),
            &batch_size,
            |b, &size| {
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| {
                        data.into_iter()
                            .map(|record| {
                                let mut fresh_cache = FieldQueryCache::default();
                                model.transform(record, &mut fresh_cache)
                            })
                            .collect::<Vec<_>>()
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Shared cache (best case)
        group.bench_with_input(
            BenchmarkId::new("shared_cache", batch_size),
            &batch_size,
            |b, &size| {
                let mut cache = FieldQueryCache::default();
                let records = create_test_records(size);
                b.iter_batched(
                    || records.clone(),
                    |data| model.transform_batch(data, &mut cache),
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_vs_single,
    bench_batch_multi_stage,
    bench_batch_cache_benefit
);
criterion_main!(benches);
