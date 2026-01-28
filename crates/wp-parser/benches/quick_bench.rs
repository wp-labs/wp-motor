use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use winnow::Parser;
use wp_parser::atom::*;
use wp_parser::scope::ScopeEval;
use wp_parser::utils::get_scope;

fn quick_benchmarks(c: &mut Criterion) {
    // Benchmark: Variable name parsing (zero-copy optimization)
    c.bench_function("take_var_name_optimized", |b| {
        b.iter(|| {
            let mut data = "user.profile.name_field";
            take_var_name.parse_next(black_box(&mut data)).unwrap()
        });
    });

    // Benchmark: JSON path parsing (zero-copy optimization)
    c.bench_function("take_json_path_optimized", |b| {
        b.iter(|| {
            let mut data = "response.data.users[10].email";
            take_json_path.parse_next(black_box(&mut data)).unwrap()
        });
    });

    // Benchmark: Key-value pair parsing (zero-copy optimization)
    c.bench_function("take_key_pair_optimized", |b| {
        b.iter(|| {
            let mut data = "database.host:localhost";
            take_key_pair.parse_next(black_box(&mut data)).unwrap()
        });
    });

    // Benchmark: Nested parentheses (bug fix validation)
    c.bench_function("take_parentheses_nested", |b| {
        b.iter(|| {
            let mut data = "(outer(inner(deep)))";
            take_parentheses_val
                .parse_next(black_box(&mut data))
                .unwrap()
        });
    });

    // Benchmark: Scope evaluation (core algorithm)
    c.bench_function("scope_eval_nested", |b| {
        b.iter(|| ScopeEval::len(black_box("(a(b(c)))"), black_box('('), black_box(')')));
    });

    // Benchmark: get_scope with optimized char parser
    c.bench_function("get_scope_optimized", |b| {
        b.iter(|| {
            let mut data = "(nested(content))";
            get_scope(black_box(&mut data), black_box('('), black_box(')')).unwrap()
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(50);
    targets = quick_benchmarks
}

criterion_main!(benches);
