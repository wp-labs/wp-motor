use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use wp_stat::{Mergeable, StatCollector, StatRecorder, StatTarget, model::request::StatReq};

fn bench_record_task(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_task");
    for &n in &[100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(n as u64));
        // Prebuild keys to avoid measuring string allocation inside loop too much
        let keys: Vec<String> = (0..n).map(|i| format!("key{i}")).collect();

        group.bench_with_input(BenchmarkId::new("All", n), &n, |b, &_| {
            b.iter(|| {
                let mut collector = StatCollector::new(
                    "/".to_string(),
                    StatReq::simple_test(StatTarget::All, Vec::new(), n),
                );
                // Record each key once
                for k in &keys {
                    collector.record_task("/", black_box(k.as_str()));
                }
                // Ensure optimizer doesn't drop the work
                let report = collector.collect_stat();
                black_box(report);
            });
        });

        group.bench_with_input(BenchmarkId::new("Item", n), &n, |b, &_| {
            b.iter(|| {
                let mut collector = StatCollector::new(
                    "rule1".to_string(),
                    StatReq::simple_test(StatTarget::Item("rule1".to_string()), Vec::new(), n),
                );
                for k in &keys {
                    collector.record_task("rule1", black_box(k.as_str()));
                }
                let report = collector.collect_stat();
                black_box(report);
            });
        });
    }
    group.finish();
}

fn bench_collect_top_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("collect_top_n");
    // (max, total_keys)
    for &(max_keep, total) in &[(10usize, 100usize), (100, 10_000)] {
        group.throughput(Throughput::Elements(total as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("max{max_keep}_n{total}")),
            &total,
            |b, &_n| {
                // Prebuild keys
                let keys: Vec<String> = (0..total).map(|i| format!("k{i}")).collect();
                b.iter(|| {
                    // cache size honors max_keep
                    let mut collector = StatCollector::new(
                        "/".to_string(),
                        StatReq::simple_test(StatTarget::All, Vec::new(), max_keep),
                    );
                    // Push many keys; many will be evicted by LRU but we measure path
                    for k in &keys {
                        collector.record_task("/", black_box(k.as_str()));
                    }
                    let report = collector.collect_stat();
                    black_box(report);
                });
            },
        );
    }
    group.finish();
}

fn bench_merge_reports(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_reports");
    for &(max_keep, n) in &[(50usize, 1_000usize), (200, 5_000)] {
        group.throughput(Throughput::Elements((n * 2) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("max{max_keep}_n{n}")),
            &n,
            |b, &_n| {
                // Prebuild keys and split between two collectors to create overlap
                let keys: Vec<String> = (0..n).map(|i| format!("m{i}")).collect();
                b.iter(|| {
                    let mut c1 = StatCollector::new(
                        "/".to_string(),
                        StatReq::simple_test(StatTarget::All, Vec::new(), max_keep),
                    );
                    let mut c2 = StatCollector::new(
                        "/".to_string(),
                        StatReq::simple_test(StatTarget::All, Vec::new(), max_keep),
                    );
                    // First half into c1 (with repeats to create higher totals)
                    for (i, k) in keys.iter().enumerate() {
                        if i % 2 == 0 {
                            c1.record_task("/", black_box(k.as_str()));
                            c1.record_task("/", black_box(k.as_str()));
                        } else {
                            c2.record_task("/", black_box(k.as_str()));
                        }
                    }
                    let mut r1 = c1.collect_stat();
                    let r2 = c2.collect_stat();
                    r1.merge(r2);
                    black_box(r1);
                });
            },
        );
    }
    group.finish();
}

// Explore effect of LRU capacity when the working set is much larger.
fn bench_lru_capacity(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_capacity");
    let total = 10_000usize;
    // Prebuild a large set of unique keys
    let keys: Vec<String> = (0..total).map(|i| format!("c{i}")).collect();
    group.throughput(Throughput::Elements(total as u64));
    for &cap in &[16usize, 64, 256, 1024] {
        group.bench_with_input(BenchmarkId::from_parameter(cap), &cap, |b, &cap| {
            b.iter(|| {
                let mut collector = StatCollector::new(
                    "/".to_string(),
                    StatReq::simple_test(StatTarget::All, Vec::new(), cap),
                );
                for k in &keys {
                    collector.record_task("/", black_box(k.as_str()));
                }
                let report = collector.collect_stat();
                black_box(report);
            });
        });
    }
    group.finish();
}

// Explore effect of hit-rate by mixing accesses to a small hot set and a large cold set.
fn bench_lru_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_hit_rate");
    let cap = 256usize;
    let hot_n = 64usize; // hot set size
    let cold_n = 4096usize; // cold set size
    let iters = 50_000usize; // total operations per iteration
    // Prebuild keys and access sequences for ratios 1/10, 5/10, 9/10 hot accesses
    let hot_keys: Vec<String> = (0..hot_n).map(|i| format!("h{i}")).collect();
    let cold_keys: Vec<String> = (0..cold_n).map(|i| format!("x{i}")).collect();

    for &hot_per_10 in &[1usize, 5, 9] {
        // Prebuild a sequence of &str with controlled mix to avoid measuring selection overhead
        let mut seq: Vec<&str> = Vec::with_capacity(iters);
        for i in 0..iters {
            if i % 10 < hot_per_10 {
                seq.push(hot_keys[i % hot_n].as_str());
            } else {
                seq.push(cold_keys[i % cold_n].as_str());
            }
        }
        group.throughput(Throughput::Elements(iters as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("cap{cap}_hot{}/10", hot_per_10)),
            &hot_per_10,
            |b, &_hp10| {
                b.iter(|| {
                    let mut collector = StatCollector::new(
                        "/".to_string(),
                        StatReq::simple_test(StatTarget::All, Vec::new(), cap),
                    );
                    for key in &seq {
                        collector.record_task("/", black_box(*key));
                    }
                    let report = collector.collect_stat();
                    black_box(report);
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = bench_record_task, bench_collect_top_n, bench_merge_reports, bench_lru_capacity, bench_lru_hit_rate
}
criterion_main!(benches);
