use criterion::{Criterion, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use std::hint::black_box;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};
use wp_parser::Parser;

fn build_model(code: &str) -> ObjModel {
    let conf = code.to_string();
    oml_parse_raw
        .parse_next(&mut conf.as_str())
        .expect("parse OML model for bench")
}

// ---------------------------------------------------------------------------
// 测试语料
// ---------------------------------------------------------------------------
const EN_SHORT: &str = "database connection failed";
const EN_MEDIUM: &str = "Server failed to connect database after retry";
const EN_LONG: &str = "The application server failed to establish a persistent connection to the primary database \
     after 3 retry attempts, request processing timeout occurred";

const CN_SHORT: &str = "数据库连接失败";
const CN_MEDIUM: &str = "服务器连接数据库超时，正在重试";
const CN_LONG: &str =
    "应用服务器在尝试连接主数据库时发生超时异常，已重试3次，请求处理失败，系统将自动恢复连接";

// ---------------------------------------------------------------------------
// extract_main_word 基准
// ---------------------------------------------------------------------------
fn bench_extract_main_word(c: &mut Criterion) {
    let mdl = build_model(
        r#"
        name : bench_emw
        ---
        X = pipe read(msg) | extract_main_word ;
        "#,
    );

    let cases: &[(&str, &str)] = &[
        ("en_short", EN_SHORT),
        ("en_medium", EN_MEDIUM),
        ("en_long", EN_LONG),
        ("cn_short", CN_SHORT),
        ("cn_medium", CN_MEDIUM),
        ("cn_long", CN_LONG),
    ];

    let mut group = c.benchmark_group("extract_main_word");
    for &(label, text) in cases {
        let src = DataRecord::from(vec![DataField::from_chars("msg", text)]);
        group.bench_function(label, |b| {
            b.iter(|| {
                let mut cache = FieldQueryCache::default();
                let _ = mdl.transform(black_box(src.clone()), &mut cache);
            })
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// extract_subject_object 基准
// ---------------------------------------------------------------------------
fn bench_extract_subject_object(c: &mut Criterion) {
    let mdl = build_model(
        r#"
        name : bench_eso
        ---
        X = pipe read(msg) | extract_subject_object ;
        "#,
    );

    let cases: &[(&str, &str)] = &[
        ("en_short", EN_SHORT),
        ("en_medium", EN_MEDIUM),
        ("en_long", EN_LONG),
        ("cn_short", CN_SHORT),
        ("cn_medium", CN_MEDIUM),
        ("cn_long", CN_LONG),
    ];

    let mut group = c.benchmark_group("extract_subject_object");
    for &(label, text) in cases {
        let src = DataRecord::from(vec![DataField::from_chars("msg", text)]);
        group.bench_function(label, |b| {
            b.iter(|| {
                let mut cache = FieldQueryCache::default();
                let _ = mdl.transform(black_box(src.clone()), &mut cache);
            })
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// extract_subject_object + get 组合基准（实际使用场景）
// ---------------------------------------------------------------------------
fn bench_extract_subject_object_get(c: &mut Criterion) {
    let mdl = build_model(
        r#"
        name : bench_eso_get
        ---
        info   = pipe read(msg) | extract_subject_object ;
        sub    = pipe read(info) | get(subject) ;
        act    = pipe read(info) | get(action) ;
        obj    = pipe read(info) | get(object) ;
        stat   = pipe read(info) | get(status) ;
        "#,
    );

    let cases: &[(&str, &str)] = &[
        ("en_short", EN_SHORT),
        ("en_medium", EN_MEDIUM),
        ("en_long", EN_LONG),
        ("cn_short", CN_SHORT),
        ("cn_medium", CN_MEDIUM),
        ("cn_long", CN_LONG),
    ];

    let mut group = c.benchmark_group("extract_subject_object_get");
    for &(label, text) in cases {
        let src = DataRecord::from(vec![DataField::from_chars("msg", text)]);
        group.bench_function(label, |b| {
            b.iter(|| {
                let mut cache = FieldQueryCache::default();
                let _ = mdl.transform(black_box(src.clone()), &mut cache);
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_extract_main_word,
    bench_extract_subject_object,
    bench_extract_subject_object_get,
);
criterion_main!(benches);
