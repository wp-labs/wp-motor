use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use wp_source_syslog::normalize;

fn sample_rfc5424() -> &'static str {
    // PRI=14 (facility=user, severity=info)
    "<14>1 2024-10-05T12:34:56Z host app 123 - - GET /index HTTP/1.1"
}

fn sample_rfc3164() -> &'static str {
    // PRI=34 (facility=auth, severity=crit)
    "<34>Oct 11 22:14:15 mymachine su: 'su root' failed"
}

fn sample_plain() -> &'static str {
    "just plaintext payload line without header"
}

fn build_bulk(n: usize) -> Vec<String> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let s = if i % 3 == 0 {
            sample_rfc5424()
        } else if i % 3 == 1 {
            sample_rfc3164()
        } else {
            sample_plain()
        };
        v.push(format!("{} {}", s, i));
    }
    v
}

fn bench_normalize_single(c: &mut Criterion) {
    let mut g = c.benchmark_group("normalize_single");
    for (name, line) in [
        ("rfc5424", sample_rfc5424()),
        ("rfc3164", sample_rfc3164()),
        ("plain", sample_plain()),
    ] {
        g.throughput(Throughput::Bytes(line.len() as u64));
        g.bench_function(name, |b| {
            b.iter(|| {
                let n = normalize::normalize(black_box(line));
                black_box(n.message);
            })
        });
    }
    g.finish();
}

fn bench_normalize_bulk(c: &mut Criterion) {
    let mut g = c.benchmark_group("normalize_bulk");
    let sizes = [1_000usize, 10_000usize];
    for &n in &sizes {
        g.throughput(Throughput::Elements(n as u64));
        g.bench_function(format!("bulk_{}", n), |b| {
            b.iter_batched(
                || build_bulk(n),
                |lines| {
                    let mut total = 0usize;
                    for l in lines {
                        let nn = normalize::normalize(&l);
                        total += nn.message.len();
                    }
                    black_box(total);
                },
                BatchSize::LargeInput,
            )
        });
    }
    g.finish();
}

criterion_group!(benches, bench_normalize_single, bench_normalize_bulk);
criterion_main!(benches);
