use criterion::{Criterion, criterion_group, criterion_main};
use orion_error::TestAssert;
use std::fmt::Write as FmtWrite;
use wp_parse_api::RawData;
use wp_parser::Parser;
use wpl::{WplEvaluator, wpl_express};

fn wpl_parse(lpp: &WplEvaluator, data: &RawData) {
    for _ in 0..500 {
        let _ = lpp.proc(data.clone(), 0);
    }
}

fn build_flat_json_chars(n: usize, val: &str) -> String {
    let mut s = String::with_capacity(n * (val.len() + 10));
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "\"k{}\":\"{}\"", i, val);
    }
    s.push('}');
    s
}

fn build_pipes_f_chars_has(m: usize, n: usize, val: &str) -> String {
    // 构造 m 个 f_chars_has，目标落在 [0, n) 中，平均分布
    let mut s = String::new();
    for i in 0..m {
        if i > 0 {
            s.push_str(" |");
        } else {
            s.push('|');
        }
        let target = format!("k{}", (i * (n / m + 1)) % n);
        let _ = write!(s, " f_chars_has({}, {})", target, val);
    }
    s
}

fn criterion_pipe_f_chars_has_1k_1p(c: &mut Criterion) {
    let n_fields = 1000;
    let data = build_flat_json_chars(n_fields, "v");
    let wpl = format!("(json {} )", build_pipes_f_chars_has(1, n_fields, "v"));
    let express = wpl_express.parse(&wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data.clone());
    c.bench_function("pipe_f_chars_has_1k_1p", |b| {
        b.iter(|| wpl_parse(&lpp, &raw))
    });
}

fn criterion_pipe_f_chars_has_1k_10p(c: &mut Criterion) {
    let n_fields = 1000;
    let data = build_flat_json_chars(n_fields, "v");
    let wpl = format!("(json {} )", build_pipes_f_chars_has(10, n_fields, "v"));
    let express = wpl_express.parse(&wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data.clone());
    c.bench_function("pipe_f_chars_has_1k_10p", |b| {
        b.iter(|| wpl_parse(&lpp, &raw))
    });
}

fn build_pipes_f_chars_in(m: usize, n: usize, val: &str) -> String {
    let mut s = String::new();
    for i in 0..m {
        if i > 0 {
            s.push_str(" |");
        } else {
            s.push('|');
        }
        let target = format!("k{}", (i * (n / m + 1)) % n);
        // values 数组含目标 val 与若干噪声
        let _ = write!(s, " f_chars_in({}, [x,y,{},z])", target, val);
    }
    s
}

fn criterion_pipe_f_chars_in_1k_10p(c: &mut Criterion) {
    let n_fields = 1000;
    let data = build_flat_json_chars(n_fields, "v");
    let wpl = format!("(json {} )", build_pipes_f_chars_in(10, n_fields, "v"));
    let express = wpl_express.parse(&wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    c.bench_function("pipe_f_chars_in_1k_10p", |b| {
        b.iter(|| wpl_parse(&lpp, &data))
    });
}

criterion_group!(
    benches,
    criterion_pipe_f_chars_has_1k_1p,
    criterion_pipe_f_chars_has_1k_10p,
    criterion_pipe_f_chars_in_1k_10p,
);
criterion_main!(benches);
