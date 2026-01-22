#![cfg_attr(not(feature = "perf-ci"), allow(dead_code))]

#[cfg(not(feature = "perf-ci"))]
compile_error!("oml_proc_batch_nginx 基准需要启用 --features perf-ci");

#[cfg(feature = "perf-ci")]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "perf-ci")]
use std::hint::black_box;
#[cfg(feature = "perf-ci")]
use wp_engine::sinks::OmlBatchPerfCase;

#[cfg(feature = "perf-ci")]
fn bench_oml_proc_batch_nginx(c: &mut Criterion) {
    let batch_size = std::env::var("WF_BENCH_LINES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4096);
    let mut case = OmlBatchPerfCase::new(batch_size);
    let mut group = c.benchmark_group("oml_proc_batch_nginx");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(case.batch_size() as u64));
    group.bench_function(BenchmarkId::from_parameter(case.batch_size()), |b| {
        b.iter(|| {
            let processed = case.run_once();
            black_box(processed);
        })
    });
    group.finish();
}

#[cfg(feature = "perf-ci")]
criterion_group!(benches, bench_oml_proc_batch_nginx);
#[cfg(feature = "perf-ci")]
criterion_main!(benches);
