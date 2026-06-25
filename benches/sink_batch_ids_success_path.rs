#![cfg_attr(not(feature = "perf-ci"), allow(dead_code))]

#[cfg(not(feature = "perf-ci"))]
compile_error!("sink_batch_ids_success_path benchmark requires --features perf-ci");

#[cfg(feature = "perf-ci")]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "perf-ci")]
use std::hint::black_box;
#[cfg(feature = "perf-ci")]
use std::sync::Arc;
#[cfg(feature = "perf-ci")]
use wp_engine::sinks::{ProcMeta, SinkPackage, SinkRecUnit};
#[cfg(feature = "perf-ci")]
use wp_model_core::model::{DataField, DataRecord};

#[cfg(feature = "perf-ci")]
fn build_package(count: usize) -> SinkPackage {
    let units = (0..count).map(|idx| {
        let mut record = DataRecord::default();
        record.append(DataField::from_chars("k", format!("v{}", idx)));
        SinkRecUnit::new(
            idx as u64,
            ProcMeta::Rule("/bench/sink_ids".to_string()),
            Arc::new(record),
        )
    });
    SinkPackage::from_units(units)
}

#[cfg(feature = "perf-ci")]
fn collect_records_with_ids(package: &SinkPackage) -> usize {
    let mut ids = Vec::with_capacity(package.len());
    let records: Vec<Arc<DataRecord>> = package
        .iter()
        .map(|unit| {
            ids.push(*unit.id());
            unit.data().clone()
        })
        .collect();
    black_box(&ids);
    black_box(&records);
    records.len()
}

#[cfg(feature = "perf-ci")]
fn collect_records_without_ids(package: &SinkPackage) -> usize {
    let records: Vec<Arc<DataRecord>> = package.iter().map(|unit| unit.data().clone()).collect();
    black_box(&records);
    records.len()
}

#[cfg(feature = "perf-ci")]
fn build_records(count: usize) -> Vec<Arc<DataRecord>> {
    (0..count)
        .map(|idx| {
            let mut record = DataRecord::default();
            record.append(DataField::from_chars("k", format!("v{}", idx)));
            Arc::new(record)
        })
        .collect()
}

#[cfg(feature = "perf-ci")]
fn clone_records_with_synthetic_ids(records: &[Arc<DataRecord>]) -> usize {
    let ids: Vec<u64> = (0..records.len() as u64).collect();
    let sent = records.to_vec();
    black_box(&ids);
    black_box(&sent);
    sent.len()
}

#[cfg(feature = "perf-ci")]
fn clone_records_without_synthetic_ids(records: &[Arc<DataRecord>]) -> usize {
    let sent = records.to_vec();
    black_box(&sent);
    sent.len()
}

#[cfg(feature = "perf-ci")]
fn bench_sink_batch_ids_success_path(c: &mut Criterion) {
    let sizes = std::env::var("WF_BENCH_LINES_LIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|item| item.trim().parse::<usize>().ok())
                .filter(|size| *size > 0)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![256, 1024, 4096]);

    let mut group = c.benchmark_group("sink_batch_ids_success_path_package_collect");
    group.measurement_time(std::time::Duration::from_secs(3));

    for &size in &sizes {
        let package = build_package(size);
        group.throughput(Throughput::Elements(size as u64));

        group.bench_function(BenchmarkId::new("with_ids", size), |b| {
            b.iter(|| {
                let processed = collect_records_with_ids(black_box(&package));
                black_box(processed);
            })
        });

        group.bench_function(BenchmarkId::new("without_ids", size), |b| {
            b.iter(|| {
                let processed = collect_records_without_ids(black_box(&package));
                black_box(processed);
            })
        });
    }

    group.finish();

    let mut group = c.benchmark_group("sink_batch_ids_success_path_current_batch");
    group.measurement_time(std::time::Duration::from_secs(3));

    for size in sizes {
        let records = build_records(size);
        group.throughput(Throughput::Elements(size as u64));

        group.bench_function(BenchmarkId::new("with_ids", size), |b| {
            b.iter(|| {
                let processed = clone_records_with_synthetic_ids(black_box(&records));
                black_box(processed);
            })
        });

        group.bench_function(BenchmarkId::new("without_ids", size), |b| {
            b.iter(|| {
                let processed = clone_records_without_synthetic_ids(black_box(&records));
                black_box(processed);
            })
        });
    }

    group.finish();
}

#[cfg(feature = "perf-ci")]
criterion_group!(benches, bench_sink_batch_ids_success_path);
#[cfg(feature = "perf-ci")]
criterion_main!(benches);
