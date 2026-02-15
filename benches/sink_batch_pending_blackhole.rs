#![cfg_attr(not(feature = "perf-ci"), allow(dead_code))]

#[cfg(not(feature = "perf-ci"))]
compile_error!("sink_batch_pending_blackhole 基准需要启用 --features perf-ci");

#[cfg(feature = "perf-ci")]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "perf-ci")]
use std::hint::black_box;
#[cfg(feature = "perf-ci")]
use wp_engine::sinks::SinkBatchBufferPerfCase;

#[cfg(feature = "perf-ci")]
fn bench_sink_batch_pending_blackhole(c: &mut Criterion) {
    let package_size = std::env::var("WF_BENCH_LINES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4096);
    let base_batch_size = std::env::var("WF_BENCH_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1024);

    // buffered 路径：batch_size 严格大于 package_size，避免自动直通
    let pending_batch_size = package_size
        .saturating_add(1)
        .max(base_batch_size.saturating_add(1));
    // 直通路径：batch_size 不大于 package_size，触发自动绕过 pending
    let bypass_batch_size = base_batch_size.min(package_size).max(1);

    let mut pending_case = SinkBatchBufferPerfCase::new(package_size, pending_batch_size);
    let mut bypass_case = SinkBatchBufferPerfCase::new(package_size, bypass_batch_size);

    let mut group = c.benchmark_group("sink_batch_pending_blackhole");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(package_size as u64));

    group.bench_function(
        BenchmarkId::new(
            format!("buffered_path_bsz_{}", pending_case.batch_size()),
            pending_case.package_size(),
        ),
        |b| {
            b.iter(|| {
                let processed = pending_case.run_once();
                black_box(processed);
            })
        },
    );

    group.bench_function(
        BenchmarkId::new(
            format!("bypass_path_bsz_{}", bypass_case.batch_size()),
            bypass_case.package_size(),
        ),
        |b| {
            b.iter(|| {
                let processed = bypass_case.run_once();
                black_box(processed);
            })
        },
    );

    group.finish();
}

#[cfg(feature = "perf-ci")]
criterion_group!(benches, bench_sink_batch_pending_blackhole);
#[cfg(feature = "perf-ci")]
criterion_main!(benches);
