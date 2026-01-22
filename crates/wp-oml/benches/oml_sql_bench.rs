use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use wp_knowledge::facade as kdb;
use wp_knowledge::mem::memdb::MemDB;

use wp_model_core::model::{DataField, DataRecord};
use wp_parser::Parser;

/// 初始化：
/// - 将知识库门面绑定到全局内存库
/// - 装载 example 表（tests/example.csv），供 SQL 评估路径基准使用
fn ensure_kdb() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = kdb::init_mem_provider(MemDB::global());
        let _ = MemDB::load_test();
    });
}

fn build_model(code: &str) -> ObjModel {
    let conf = code.to_string();
    oml_parse_raw
        .parse_next(&mut conf.as_str())
        .expect("parse OML model for bench")
}

fn bench_oml_sql(c: &mut Criterion) {
    ensure_kdb();

    // 仅 SQL 提取：常量条件（无命名参数）
    let mdl_no_params = build_model(
        r#"
        name : bench
        ---
        _,_ = select name,pinying from example where pinying = 'xiaolongnu' ;
        "#,
    );
    // 含 1 个命名参数（来自输入记录）
    let mdl_1_param = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(py) ;
        "#,
    );

    // 基础输入（命名参数 py）
    let src_with_param = DataRecord {
        items: vec![DataField::from_chars("py", "xiaolongnu")],
    };
    let empty = DataRecord { items: vec![] };

    let mut group = c.benchmark_group("oml_sql");

    // Cold：首次运行，SQL 会触发一次实际查询
    group.bench_function("no_params_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_no_params.transform(black_box(empty.clone()), &mut cache);
        })
    });

    group.bench_function("one_param_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_1_param.transform(black_box(src_with_param.clone()), &mut cache);
        })
    });

    // Hot：预热后重复运行，应以缓存为主（FieldQueryCache + 连接级 prepare_cached）
    group.bench_function("no_params_hot", |b| {
        let mut cache = FieldQueryCache::default();
        // 预热
        let _ = mdl_no_params.transform(empty.clone(), &mut cache);
        b.iter(|| {
            let _ = mdl_no_params.transform(black_box(empty.clone()), &mut cache);
        })
    });

    group.bench_function("one_param_hot", |b| {
        let mut cache = FieldQueryCache::default();
        // 预热
        let _ = mdl_1_param.transform(src_with_param.clone(), &mut cache);
        b.iter(|| {
            let _ = mdl_1_param.transform(black_box(src_with_param.clone()), &mut cache);
        })
    });

    // Miss：构造多个不同的 py 值，模拟缓存 miss（大键空间访问）
    group.bench_function("one_param_many_keys", |b| {
        let mut cache = FieldQueryCache::default();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            let key = format!("py_{}", i);
            let src = DataRecord {
                items: vec![DataField::from_chars("py", key.as_str())],
            };
            let _ = mdl_1_param.transform(black_box(src), &mut cache);
        })
    });

    // Zipf 热点分布：在有限键空间内（10 个既有 pinying 值）按 Zipf(α) 分布访问，观察缓存命中收益
    group.bench_function("one_param_zipf_hot_a1.2", |b| {
        let mut cache = FieldQueryCache::default();
        // 既有样例的 pinying 键（对应 example.csv）
        let keys: [&str; 10] = [
            "linghuchong",
            "renyingying",
            "yangguoyun",
            "xiaolongnu",
            "guojingyu",
            "huangronger",
            "zhangwuji",
            "zhaominmin",
            "zhouzhiruo",
            "yinlige",
        ];
        let alpha: f64 = std::env::var("OML_BENCH_ALPHA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.2);
        // 预计算 Zipf 累积分布
        let n = keys.len();
        let mut weights = vec![0f64; n];
        for (i, w) in weights.iter_mut().enumerate() {
            *w = 1.0 / ((i + 1) as f64).powf(alpha);
        }
        let z: f64 = weights.iter().sum();
        for w in &mut weights {
            *w /= z;
        }
        let mut cdf = vec![0f64; n];
        let mut acc = 0f64;
        for (i, w) in weights.iter().enumerate() {
            acc += *w;
            cdf[i] = acc;
        }
        // 简单 LCG 生成器，避免额外依赖
        let mut seed: u64 = 0x9E3779B97F4A7C15;
        // 预热：针对前几个热点先命中
        for k in keys.iter().take(n.min(3)) {
            let src = DataRecord {
                items: vec![DataField::from_chars("py", k)],
            };
            let _ = mdl_1_param.transform(src, &mut cache);
        }
        b.iter(|| {
            seed = seed
                .wrapping_mul(2862933555777941757)
                .wrapping_add(3037000493);
            let u = ((seed >> 11) as f64) / ((1u64 << 53) as f64);
            // 二分查找 CDF（这里线性扫描即可，n=10）
            let mut idx = 0;
            while idx + 1 < n && u > cdf[idx] {
                idx += 1;
            }
            let src = DataRecord {
                items: vec![DataField::from_chars("py", keys[idx])],
            };
            let _ = mdl_1_param.transform(black_box(src), &mut cache);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_oml_sql);
criterion_main!(benches);
