use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use wpl::{SepPattern, WplSep, build_pattern};

// ── Helpers ──────────────────────────────────────────────────────────

fn build(raw: &str) -> SepPattern {
    build_pattern(raw).unwrap()
}

// ── build_pattern parsing benchmarks ─────────────────────────────────

fn bench_build_literal(c: &mut Criterion) {
    c.bench_function("build_pattern/literal", |b| {
        b.iter(|| build_pattern(black_box("abc")))
    });
}

fn bench_build_glob(c: &mut Criterion) {
    c.bench_function("build_pattern/glob_star", |b| {
        b.iter(|| build_pattern(black_box("*\\s(key=)")))
    });
}

fn bench_build_complex(c: &mut Criterion) {
    c.bench_function("build_pattern/complex", |b| {
        b.iter(|| build_pattern(black_box("field?:\\h=\\h*\\n")))
    });
}

// ── SepPattern::find benchmarks ──────────────────────────────────────

fn bench_find_literal_short(c: &mut Criterion) {
    let pat = build("abc");
    let haystack = "xyzabcdef";
    c.bench_function("find/literal_short", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_literal_long(c: &mut Criterion) {
    let pat = build(",");
    // 200 chars haystack with comma at position 150
    let haystack: String = "x".repeat(150) + "," + &"y".repeat(49);
    c.bench_function("find/literal_long", |b| {
        b.iter(|| pat.find(black_box(&haystack)))
    });
}

fn bench_find_star_non_greedy(c: &mut Criterion) {
    let pat = build("*=");
    let haystack = "key=value=extra=more";
    c.bench_function("find/star_non_greedy", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_whitespace(c: &mut Criterion) {
    let pat = build("\\s=");
    let haystack = "key   =value";
    c.bench_function("find/whitespace_eq", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_preserve(c: &mut Criterion) {
    let pat = build("*\\s(key=)");
    let haystack = "hello world  key=value";
    c.bench_function("find/preserve", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_field_any(c: &mut Criterion) {
    let pat = build("field?:");
    let haystack = "fieldA:value";
    c.bench_function("find/field_any", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_no_match(c: &mut Criterion) {
    let pat = build("\\s=");
    let haystack = "no_whitespace_equals_here";
    c.bench_function("find/no_match", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

// ── match_at_start benchmarks ────────────────────────────────────────

fn bench_match_at_start_literal(c: &mut Criterion) {
    let pat = build(",");
    c.bench_function("match_start/literal", |b| {
        b.iter(|| pat.match_at_start(black_box(",rest")))
    });
}

fn bench_match_at_start_glob(c: &mut Criterion) {
    let pat = build("\\s=");
    c.bench_function("match_start/glob", |b| {
        b.iter(|| pat.match_at_start(black_box("  =value")))
    });
}

// ── read_until_sep integration benchmarks ────────────────────────────

fn bench_read_until_sep_str(c: &mut Criterion) {
    // Baseline: existing Str separator
    let sep = WplSep::field_sep(",");
    let data_str = "hello,world,foo,bar";
    c.bench_function("read_until_sep/str", |b| {
        b.iter(|| {
            let mut data = black_box(data_str);
            let _ = sep.read_until_sep(&mut data);
        })
    });
}

fn bench_read_until_sep_pattern_literal(c: &mut Criterion) {
    // Pattern literal (should be comparable to Str)
    let pat = build(",");
    let sep = WplSep::field_sep_pattern(pat);
    let data_str = "hello,world,foo,bar";
    c.bench_function("read_until_sep/pattern_literal", |b| {
        b.iter(|| {
            let mut data = black_box(data_str);
            let _ = sep.read_until_sep(&mut data);
        })
    });
}

fn bench_read_until_sep_pattern_glob(c: &mut Criterion) {
    let pat = build("*=");
    let sep = WplSep::field_sep_pattern(pat);
    let data_str = "key=value=extra";
    c.bench_function("read_until_sep/pattern_glob", |b| {
        b.iter(|| {
            let mut data = black_box(data_str);
            let _ = sep.read_until_sep(&mut data);
        })
    });
}

fn bench_read_until_sep_pattern_ws(c: &mut Criterion) {
    let pat = build("\\s=");
    let sep = WplSep::field_sep_pattern(pat);
    let data_str = "some_key  =value";
    c.bench_function("read_until_sep/pattern_ws", |b| {
        b.iter(|| {
            let mut data = black_box(data_str);
            let _ = sep.read_until_sep(&mut data);
        })
    });
}

// ── Stress: long input with star ─────────────────────────────────────

fn bench_find_star_long_input(c: &mut Criterion) {
    let pat = build("*\\n");
    // 10KB line
    let haystack: String = "x".repeat(10_000) + "\n";
    c.bench_function("find/star_long_10k", |b| {
        b.iter(|| pat.find(black_box(&haystack)))
    });
}

// ── \S / \H benchmarks ──────────────────────────────────────────────

fn bench_build_non_whitespace(c: &mut Criterion) {
    c.bench_function("build_pattern/non_ws", |b| {
        b.iter(|| build_pattern(black_box("\\s(\\S=)")))
    });
}

fn bench_find_non_whitespace_kvarr(c: &mut Criterion) {
    // Real-world kvarr pattern: \s consumed, \S= preserved
    let pat = build("\\s(\\S=)");
    let haystack = "msg=Test message externalId=0";
    c.bench_function("find/non_ws_kvarr", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_non_whitespace_long(c: &mut Criterion) {
    // Long kvarr-style input
    let pat = build("\\s(\\S=)");
    let haystack = "msg=This is a very long message with many words externalId=12345 severity=high source=firewall action=allow proto=tcp srcip=192.168.1.1 dstip=10.0.0.1";
    c.bench_function("find/non_ws_kvarr_long", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_non_horizontal_whitespace(c: &mut Criterion) {
    let pat = build("\\H=");
    let haystack = "key\t:\tval\texternalId=0";
    c.bench_function("find/non_h_ws", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_non_whitespace_backtrack(c: &mut Criterion) {
    // Pattern where \S must backtrack: \S consumes greedily then shrinks
    let pat = build("\\s\\S=");
    let haystack = "msg=Test message externalId=0";
    c.bench_function("find/non_ws_backtrack", |b| {
        b.iter(|| pat.find(black_box(haystack)))
    });
}

fn bench_find_literal_vs_pattern(c: &mut Criterion) {
    // Compare raw str::find vs Pattern literal on same data
    let data = "field1,field2,field3,field4,field5,field6,field7,field8";

    let pat = build(",");
    let sep_pattern = WplSep::field_sep_pattern(pat);
    let sep_str = WplSep::field_sep(",");

    let mut group = c.benchmark_group("literal_vs_pattern");
    group.bench_function("str_sep", |b| {
        b.iter(|| {
            let mut d = black_box(data);
            let _ = sep_str.read_until_sep(&mut d);
        })
    });
    group.bench_function("pattern_literal_sep", |b| {
        b.iter(|| {
            let mut d = black_box(data);
            let _ = sep_pattern.read_until_sep(&mut d);
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_build_literal,
    bench_build_glob,
    bench_build_complex,
    bench_build_non_whitespace,
    bench_find_literal_short,
    bench_find_literal_long,
    bench_find_star_non_greedy,
    bench_find_whitespace,
    bench_find_preserve,
    bench_find_field_any,
    bench_find_no_match,
    bench_find_non_whitespace_kvarr,
    bench_find_non_whitespace_long,
    bench_find_non_horizontal_whitespace,
    bench_find_non_whitespace_backtrack,
    bench_match_at_start_literal,
    bench_match_at_start_glob,
    bench_read_until_sep_str,
    bench_read_until_sep_pattern_literal,
    bench_read_until_sep_pattern_glob,
    bench_read_until_sep_pattern_ws,
    bench_find_star_long_input,
    bench_find_literal_vs_pattern,
);
criterion_main!(benches);
