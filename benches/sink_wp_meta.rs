#![cfg_attr(not(feature = "perf-ci"), allow(dead_code))]

#[cfg(not(feature = "perf-ci"))]
compile_error!("sink_wp_meta benchmark requires --features perf-ci");

#[cfg(feature = "perf-ci")]
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
#[cfg(feature = "perf-ci")]
use std::hint::black_box;
#[cfg(feature = "perf-ci")]
use wp_engine::sinks::{
    SinkWpMetaDisablePerfCase, SinkWpMetaOutputPerfCase, WpMetaDisableMode, WpMetaOutputMode,
};

#[cfg(feature = "perf-ci")]
fn bench_sink_wp_meta(c: &mut Criterion) {
    let package_size = std::env::var("WF_BENCH_LINES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4096);
    let batch_size = std::env::var("WF_BENCH_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1024)
        .min(package_size)
        .max(1);

    bench_output_meta(c, package_size, batch_size);
    bench_disable_meta(c, package_size, batch_size);
}

#[cfg(feature = "perf-ci")]
fn bench_output_meta(c: &mut Criterion, package_size: usize, batch_size: usize) {
    let mut group = c.benchmark_group("sink_wp_meta_output_blackhole");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(package_size as u64));

    group.bench_function(
        BenchmarkId::new(format!("enabled_bsz_{}", batch_size), package_size),
        |b| {
            b.iter_batched(
                || {
                    SinkWpMetaOutputPerfCase::new(
                        package_size,
                        batch_size,
                        WpMetaOutputMode::Enabled,
                    )
                },
                |mut case| {
                    let processed = case.run_once();
                    black_box(processed);
                },
                BatchSize::SmallInput,
            )
        },
    );

    group.bench_function(
        BenchmarkId::new(format!("disabled_bsz_{}", batch_size), package_size),
        |b| {
            b.iter_batched(
                || {
                    SinkWpMetaOutputPerfCase::new(
                        package_size,
                        batch_size,
                        WpMetaOutputMode::Disabled,
                    )
                },
                |mut case| {
                    let processed = case.run_once();
                    black_box(processed);
                },
                BatchSize::SmallInput,
            )
        },
    );

    group.finish();
}

#[cfg(feature = "perf-ci")]
fn bench_disable_meta(c: &mut Criterion, package_size: usize, batch_size: usize) {
    let mut group = c.benchmark_group("sink_wp_meta_disable_dispatcher");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.throughput(Throughput::Elements(package_size as u64));

    group.bench_function(BenchmarkId::new("none", package_size), |b| {
        b.iter_batched(
            || SinkWpMetaDisablePerfCase::new(package_size, batch_size, WpMetaDisableMode::None),
            |mut case| {
                let processed = case.run_once();
                black_box(processed);
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function(BenchmarkId::new("ignore_wp_fields", package_size), |b| {
        b.iter_batched(
            || {
                SinkWpMetaDisablePerfCase::new(
                    package_size,
                    batch_size,
                    WpMetaDisableMode::DisableWpFields,
                )
            },
            |mut case| {
                let processed = case.run_once();
                black_box(processed);
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

#[cfg(feature = "perf-ci")]
criterion_group!(benches, bench_sink_wp_meta);
#[cfg(feature = "perf-ci")]
criterion_main!(benches);
