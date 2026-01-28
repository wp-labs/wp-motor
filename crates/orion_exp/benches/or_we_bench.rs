use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use orion_exp::{CmpOperator, Comparison, ConditionEvaluator, RustSymbol};
use std::collections::HashMap;

// Local newtype wrapper to satisfy orphan rules
struct Map<'a> {
    inner: HashMap<&'a str, String>,
}
impl<'a> orion_exp::ValueGetter<String> for Map<'a> {
    fn get_value(&self, var: &str) -> Option<&String> {
        self.inner.get(var)
    }
}

fn gen_values(n: usize) -> Vec<String> {
    // generate simple values with varying endings
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        if i % 2 == 0 {
            v.push(format!("he{}llo", i % 10));
        } else {
            v.push(format!("h{}el{}o", i % 7, i % 9));
        }
    }
    v
}

fn bench_wildmatch_direct(c: &mut Criterion) {
    let mut g = c.benchmark_group("we_direct");
    let pattern = "he*lo".to_string();
    let values = gen_values(50_000);

    // Baseline: per-call compile & match
    g.bench_function("per_call_compile", |b| {
        b.iter(|| {
            let mut cnt = 0usize;
            for val in &values {
                let ok = wildmatch::WildMatch::new(pattern.as_str()).matches(val.as_str());
                cnt += ok as usize;
            }
            black_box(cnt)
        })
    });

    // Cached compile: compile once, reuse
    g.bench_function("cached_compile", |b| {
        b.iter_batched(
            || wildmatch::WildMatch::new(pattern.as_str()),
            |wm| {
                let mut cnt = 0usize;
                for val in &values {
                    let ok = wm.matches(val.as_str());
                    cnt += ok as usize;
                }
                black_box(cnt)
            },
            BatchSize::PerIteration,
        )
    });

    g.finish();
}

fn bench_orion_we_eval(c: &mut Criterion) {
    let mut g = c.benchmark_group("we_orion_eval");
    let pattern = "he*lo".to_string();
    let expr: Comparison<String, RustSymbol> = Comparison::new(CmpOperator::We, "a", pattern);
    let values = gen_values(50_000);

    g.bench_function("eval_we_many_values", |b| {
        b.iter(|| {
            let mut cnt = 0usize;
            for val in &values {
                let mut inner = HashMap::new();
                inner.insert("a", val.clone());
                let m = Map { inner };
                if expr.evaluate(&m) {
                    cnt += 1;
                }
            }
            black_box(cnt)
        })
    });
    g.finish();
}

criterion_group!(benches, bench_wildmatch_direct, bench_orion_we_eval);
criterion_main!(benches);
