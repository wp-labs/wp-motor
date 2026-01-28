use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use winnow::Parser;
use wp_parser::atom::*;
use wp_parser::scope::ScopeEval;
use wp_parser::utils::{get_scope, peek_one};

/// Benchmark for variable name parsing (now returns &str, zero-copy)
fn bench_take_var_name(c: &mut Criterion) {
    let mut group = c.benchmark_group("take_var_name");

    let test_cases = vec![
        ("simple", "simple_var"),
        ("dotted", "user.profile.name"),
        ("complex", "data_source.connection_pool.max_size"),
        (
            "long",
            "very_long_variable_name_with_many_dots.field1.field2.field3.field4",
        ),
    ];

    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, &input| {
            b.iter(|| {
                let mut data = input;
                take_var_name.parse_next(&mut data).unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark for JSON path parsing (now returns &str)
fn bench_take_json_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("take_json_path");

    let test_cases = vec![
        ("simple", "field"),
        ("array_access", "items[0]"),
        ("nested", "data/items[5]/value"),
        ("complex", "response.data.users[10].profile.settings/theme"),
    ];

    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, &input| {
            b.iter(|| {
                let mut data = input;
                take_json_path.parse_next(&mut data).unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark for key-value pair parsing (now returns (&str, &str))
fn bench_take_key_pair(c: &mut Criterion) {
    let mut group = c.benchmark_group("take_key_pair");

    let test_cases = vec![
        ("simple", "key:value"),
        ("dotted", "user.name:john.doe"),
        (
            "long",
            "configuration.database.connection:pool_max_connections",
        ),
    ];

    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, &input| {
            b.iter(|| {
                let mut data = input;
                take_key_pair.parse_next(&mut data).unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark for parentheses value parsing (fixed nested bug)
fn bench_take_parentheses_val(c: &mut Criterion) {
    let mut group = c.benchmark_group("take_parentheses_val");

    let test_cases = vec![
        ("simple", "(hello)"),
        ("nested", "(outer(inner)value)"),
        ("deeply_nested", "(a(b(c(d)e)f)g)"),
        ("complex", "(function(arg1, arg2(nested)))"),
    ];

    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, &input| {
            b.iter(|| {
                let mut data = input;
                take_parentheses_val.parse_next(&mut data).unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark for scope evaluation (used by the fixed parentheses parser)
fn bench_scope_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("scope_eval");

    let test_cases = vec![
        ("simple", "(hello)", '(', ')'),
        ("nested", "(outer(inner))", '(', ')'),
        ("deeply_nested", "(a(b(c(d))))", '(', ')'),
        ("braces", "{key: {nested: value}}", '{', '}'),
        ("brackets", "[1, [2, [3, 4]]]", '[', ']'),
    ];

    for (name, input, beg, end) in test_cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(input, beg, end),
            |b, &(input, beg, end)| {
                b.iter(|| ScopeEval::len(black_box(input), black_box(beg), black_box(end)));
            },
        );
    }

    group.finish();
}

/// Benchmark for get_scope from utils (now returns &str, uses optimized char parser)
fn bench_get_scope(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_scope");

    let test_cases = vec![
        ("parentheses", "(content)", '(', ')'),
        ("braces", "{json: data}", '{', '}'),
        ("brackets", "[array, items]", '[', ']'),
        ("nested", "(outer(inner))", '(', ')'),
    ];

    for (name, input, beg, end) in test_cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(input, beg, end),
            |b, &(input, beg, end)| {
                b.iter(|| {
                    let mut data = input;
                    get_scope(&mut data, black_box(beg), black_box(end)).unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark for peek_one (now returns &str instead of String)
fn bench_peek_one(c: &mut Criterion) {
    c.bench_function("peek_one", |b| {
        b.iter(|| {
            let mut data = "test string";
            peek_one(black_box(&mut data)).unwrap()
        });
    });
}

/// Comprehensive benchmark simulating real-world parsing scenarios
fn bench_real_world_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    // Scenario 1: Parse configuration-like syntax
    group.bench_function("config_parsing", |b| {
        b.iter(|| {
            let mut data = "database.host:localhost";
            let _ = take_key_pair.parse_next(&mut data).unwrap();
        });
    });

    // Scenario 2: Parse function call with nested parentheses
    group.bench_function("function_call", |b| {
        b.iter(|| {
            let mut data = "(calculate(value1, value2))";
            let _ = take_parentheses_val.parse_next(&mut data).unwrap();
        });
    });

    // Scenario 3: Parse complex JSON path
    group.bench_function("json_path_lookup", |b| {
        b.iter(|| {
            let mut data = "response.data.users[0].profile.email";
            let _ = take_json_path.parse_next(&mut data).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_take_var_name,
    bench_take_json_path,
    bench_take_key_pair,
    bench_take_parentheses_val,
    bench_scope_eval,
    bench_get_scope,
    bench_peek_one,
    bench_real_world_parsing,
);

criterion_main!(benches);
