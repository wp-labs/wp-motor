use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use wp_data_model::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};

fn build_model(code: &str) -> ObjModel {
    let mut code_ref = code;
    oml_parse_raw(&mut code_ref).expect("parse OML model for static bench")
}

fn bench_static_vs_temp(c: &mut Criterion) {
    let static_oml = r#"
name : bench_static
---
static {
    tpl = object {
        id = chars(E1);
        tpl = chars('tpl text')
    };
}

target = match read(Content) {
    starts_with('foo') => tpl;
    _ => tpl;
};
EventId = read(target) | get(id);
EventTemplate = read(target) | get(tpl);
"#;

    let temp_oml = r#"
name : bench_temp
---
__E1 = object {
    id = chars(E1);
    tpl = chars('tpl text')
};

target = match read(Content) {
    starts_with('foo') => read(__E1);
    _ => read(__E1);
};
EventId = read(target) | get(id);
EventTemplate = read(target) | get(tpl);
"#;

    let mdl_static = build_model(static_oml);
    let mdl_temp = build_model(temp_oml);

    let input = DataRecord::from(vec![DataField::from_chars("Content", "foo message")]);

    let mut group = c.benchmark_group("oml_static_vs_temp");

    group.bench_function("static_block", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_static.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("temp_field", |b| {
        let mut cache = FieldQueryCache::default();
        b.iter_batched(
            || input.clone(),
            |data| mdl_temp.transform(data, &mut cache),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_static_vs_temp);
criterion_main!(benches);
