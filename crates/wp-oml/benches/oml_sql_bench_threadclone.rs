use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oml::core::DataTransformer;
use oml::language::ObjModel;
use oml::parser::oml_parse_raw;
use orion_variate::EnvDict;
use std::fs;
use std::path::PathBuf;
use wp_conf::test_support::ForTest;
use wp_knowledge::facade as kdb;
use wp_model_core::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord};
use wp_parser::Parser;

const BENCH_KEYS: [&str; 10] = [
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

fn init_threadclone_provider() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        // 构建临时 V2 knowdb 目录，加载 example 表
        let root = PathBuf::from("./wp_bench_knowdb");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("conf")).unwrap();
        fs::create_dir_all(root.join("models/knowledge/example")).unwrap();
        // 写 knowdb.toml
        let conf_toml = r#"version = 2
base_dir = "./models/knowledge"

[default]
transaction = true
batch_size  = 2000
on_error    = "fail"

[csv]
has_header = true
delimiter  = ","
encoding   = "utf-8"
trim       = true

[[tables]]
name = "example"
dir  = "example"
columns.by_header = ["name", "pinying"]
[tables.expected_rows]
min = 1
max = 100
enabled = true
"#;
        fs::write(root.join("conf/knowdb.toml"), conf_toml).unwrap();
        // 写 example SQL 与 CSV（复用 wp-knowledge 的样例 CSV）
        let create_sql = "CREATE TABLE IF NOT EXISTS {table} (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  pinying TEXT NOT NULL\n);\n";
        let insert_sql = "INSERT INTO {table} (name, pinying) VALUES (?1, ?2);\n";
        fs::write(root.join("models/knowledge/example/create.sql"), create_sql).unwrap();
        fs::write(root.join("models/knowledge/example/insert.sql"), insert_sql).unwrap();
        let csv_src = format!(
            "{}/../wp-knowledge/src/mem/dict/example.csv",
            env!("CARGO_MANIFEST_DIR")
        );
        fs::copy(csv_src, root.join("models/knowledge/example/data.csv")).unwrap();

        let auth_uri = format!(
            "file:{}/wp_bench_authority.sqlite?mode=rwc&uri=true",
            root.display()
        );
        kdb::init_thread_cloned_from_knowdb(&root, &root.join("conf/knowdb.toml"), &auth_uri, &EnvDict::test_default())
            .expect("init thread-cloned provider");
    });
}

fn build_model(code: &str) -> ObjModel {
    let conf = code.to_string();
    oml_parse_raw
        .parse_next(&mut conf.as_str())
        .expect("parse OML model for bench")
}

fn bench_threadclone_sql(c: &mut Criterion) {
    init_threadclone_provider();

    // 单参与多参（2~5）
    let mdl_1 = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(p1) ;
        "#,
    );

    let mdl_2 = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(p1) or pinying = read(p2) ;
        "#,
    );
    let mdl_3 = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(p1) or pinying = read(p2) or pinying = read(p3) ;
        "#,
    );
    let mdl_4 = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(p1) or pinying = read(p2) or pinying = read(p3) or pinying = read(p4) ;
        "#,
    );
    let mdl_5 = build_model(
        r#"
        name : bench
        ---
        A,B = select name,pinying from example where pinying = read(p1) or pinying = read(p2) or pinying = read(p3) or pinying = read(p4) or pinying = read(p5) ;
        "#,
    );

    let src1 = DataRecord {
        items: vec![DataField::from_chars("p1", "xiaolongnu")],
    };
    let src2 = DataRecord {
        items: vec![
            DataField::from_chars("p1", "xiaolongnu"),
            DataField::from_chars("p2", "guojing"),
        ],
    };
    let src3 = DataRecord {
        items: vec![
            DataField::from_chars("p1", "xiaolongnu"),
            DataField::from_chars("p2", "guojing"),
            DataField::from_chars("p3", "yangguo"),
        ],
    };
    let src4 = DataRecord {
        items: vec![
            DataField::from_chars("p1", "xiaolongnu"),
            DataField::from_chars("p2", "guojing"),
            DataField::from_chars("p3", "yangguo"),
            DataField::from_chars("p4", "huangrong"),
        ],
    };
    let src5 = DataRecord {
        items: vec![
            DataField::from_chars("p1", "xiaolongnu"),
            DataField::from_chars("p2", "guojing"),
            DataField::from_chars("p3", "yangguo"),
            DataField::from_chars("p4", "huangrong"),
            DataField::from_chars("p5", "zhoubo"),
        ],
    };

    let mut group = c.benchmark_group("oml_sql_threadclone");
    // 冷路径（每次新 cache）
    group.bench_function("1param_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_1.transform(black_box(src1.clone()), &mut cache);
        })
    });
    group.bench_function("2params_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_2.transform(black_box(src2.clone()), &mut cache);
        })
    });
    group.bench_function("3params_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_3.transform(black_box(src3.clone()), &mut cache);
        })
    });
    group.bench_function("4params_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_4.transform(black_box(src4.clone()), &mut cache);
        })
    });
    group.bench_function("5params_cold", |b| {
        b.iter(|| {
            let mut cache = FieldQueryCache::default();
            let _ = mdl_5.transform(black_box(src5.clone()), &mut cache);
        })
    });

    // 热路径（预热 + 复用 cache）
    group.bench_function("1param_hot", |b| {
        let mut cache = FieldQueryCache::default();
        let _ = mdl_1.transform(src1.clone(), &mut cache);
        b.iter(|| {
            let _ = mdl_1.transform(black_box(src1.clone()), &mut cache);
        })
    });
    group.bench_function("5params_hot", |b| {
        let mut cache = FieldQueryCache::default();
        let _ = mdl_5.transform(src5.clone(), &mut cache);
        b.iter(|| {
            let _ = mdl_5.transform(black_box(src5.clone()), &mut cache);
        })
    });

    // 并发吞吐（热路径）：4/8 线程，各线程循环多次以减小 spawn 开销占比
    fn run_concurrent(model: &ObjModel, src: &DataRecord, threads: usize, loops_per_thread: usize) {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let m = model.clone();
            let s = src.clone();
            handles.push(std::thread::spawn(move || {
                let mut cache = FieldQueryCache::default();
                // 预热
                let _ = m.transform(s.clone(), &mut cache);
                for _ in 0..loops_per_thread {
                    let _ = m.transform(s.clone(), &mut cache);
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    }

    group.bench_function("concurrent_4t_hot", |b| {
        b.iter(|| run_concurrent(&mdl_1, &src1, 4, 100))
    });
    group.bench_function("concurrent_8t_hot", |b| {
        b.iter(|| run_concurrent(&mdl_1, &src1, 8, 100))
    });

    // 2/3/4/5 参数并发热路径
    group.bench_function("concurrent_4t_hot_p2", |b| {
        b.iter(|| run_concurrent(&mdl_2, &src2, 4, 100))
    });
    group.bench_function("concurrent_8t_hot_p2", |b| {
        b.iter(|| run_concurrent(&mdl_2, &src2, 8, 100))
    });
    group.bench_function("concurrent_4t_hot_p3", |b| {
        b.iter(|| run_concurrent(&mdl_3, &src3, 4, 100))
    });
    group.bench_function("concurrent_8t_hot_p3", |b| {
        b.iter(|| run_concurrent(&mdl_3, &src3, 8, 100))
    });
    group.bench_function("concurrent_4t_hot_p4", |b| {
        b.iter(|| run_concurrent(&mdl_4, &src4, 4, 100))
    });
    group.bench_function("concurrent_8t_hot_p4", |b| {
        b.iter(|| run_concurrent(&mdl_4, &src4, 8, 100))
    });
    group.bench_function("concurrent_4t_hot_p5", |b| {
        b.iter(|| run_concurrent(&mdl_5, &src5, 4, 100))
    });
    group.bench_function("concurrent_8t_hot_p5", |b| {
        b.iter(|| run_concurrent(&mdl_5, &src5, 8, 100))
    });

    // Zipf 热点分布（threadclone 版本，单线程）
    group.bench_function("zipf_hot_a1.2", |b| {
        let mut cache = FieldQueryCache::default();
        let keys = &BENCH_KEYS;
        let alpha: f64 = std::env::var("OML_BENCH_ALPHA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.2);
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
        let mut seed: u64 = 0x9E3779B97F4A7C15;
        // 预热
        for k in keys.iter().take(n.min(3)) {
            let src = DataRecord {
                items: vec![DataField::from_chars("p1", k)],
            };
            let _ = mdl_1.transform(src, &mut cache);
        }
        b.iter(|| {
            seed = seed
                .wrapping_mul(2862933555777941757)
                .wrapping_add(3037000493);
            let u = ((seed >> 11) as f64) / ((1u64 << 53) as f64);
            let mut idx = 0;
            while idx + 1 < n && u > cdf[idx] {
                idx += 1;
            }
            let src = DataRecord {
                items: vec![DataField::from_chars("p1", keys[idx])],
            };
            let _ = mdl_1.transform(black_box(src), &mut cache);
        })
    });

    // 多线程 Zipf(α) 热路径：4/8 线程
    fn run_zipf_concurrent(
        model: &ObjModel,
        keys: &'static [&'static str],
        alpha: f64,
        threads: usize,
        loops_per_thread: usize,
    ) {
        let n = keys.len();
        let mut handles = Vec::with_capacity(threads);
        for t in 0..threads {
            let m = model.clone();
            // 复制 keys 到每个线程，避免共享借用（'static 保证可在线程中使用）
            let ks: Vec<&'static str> = keys.to_vec();
            handles.push(std::thread::spawn(move || {
                let mut cache = FieldQueryCache::default();
                // 构建 Zipf CDF
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
                // 线程种子不同（与线程索引相关）
                let mut seed: u64 = 0x9E3779B97F4A7C15u64.wrapping_add((t as u64) << 1);
                // 预热热点
                for k in ks.iter().take(n.min(3)) {
                    let src = DataRecord {
                        items: vec![DataField::from_chars("p1", k)],
                    };
                    let _ = m.transform(src, &mut cache);
                }
                for _ in 0..loops_per_thread {
                    seed = seed
                        .wrapping_mul(2862933555777941757)
                        .wrapping_add(3037000493);
                    let u = ((seed >> 11) as f64) / ((1u64 << 53) as f64);
                    let mut idx = 0;
                    while idx + 1 < n && u > cdf[idx] {
                        idx += 1;
                    }
                    let src = DataRecord {
                        items: vec![DataField::from_chars("p1", ks[idx])],
                    };
                    let _ = m.transform(src, &mut cache);
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    }

    group.bench_function("concurrent_4t_zipf_a1.2", |b| {
        b.iter(|| {
            let alpha: f64 = std::env::var("OML_BENCH_ALPHA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.2);
            run_zipf_concurrent(&mdl_1, &BENCH_KEYS, alpha, 4, 100);
        })
    });
    group.bench_function("concurrent_8t_zipf_a1.2", |b| {
        b.iter(|| {
            let alpha: f64 = std::env::var("OML_BENCH_ALPHA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.2);
            run_zipf_concurrent(&mdl_1, &BENCH_KEYS, alpha, 8, 100);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_threadclone_sql);
criterion_main!(benches);
