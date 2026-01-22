use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, black_box, criterion_group, criterion_main,
};
use rand::Rng;
use rand::distributions::{Distribution, WeightedIndex};
use rand::seq::SliceRandom;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use wp_knowledge::DBQuery;
use wp_knowledge::facade as kdb;
use wp_knowledge::mem::memdb::{MDBEnum, MemDB, cache_query};
use wp_knowledge::mem::thread_clone::ThreadClonedMDB;
use wp_model_core::cache::FieldQueryCache;
use wp_model_core::model::DataField;

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default)
}
fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}
fn cfg_n_rows() -> usize {
    env_usize("WP_BENCH_ROWS", 10_000)
}
fn cfg_reads() -> usize {
    env_usize("WP_BENCH_READS", 50_000)
}
fn cfg_threads() -> usize {
    env_usize("WP_BENCH_THREADS", 4)
}
fn cfg_zipf_alpha() -> f64 {
    env_f64("WP_BENCH_ALPHA", 1.2)
}

fn prepare_kv(db: &MemDB) {
    let _ = db.execute("DROP TABLE IF EXISTS kv");
    db.execute("CREATE TABLE kv (k TEXT PRIMARY KEY, v INTEGER)")
        .unwrap();
    // bulk insert within a transaction for speed
    db.execute("BEGIN IMMEDIATE").unwrap();
    {
        let conn = db.clone();
        // Prepare once and reuse (fast load path)
        // Reuse the table_load logic is cumbersome here since we generate synthetic data
        for i in 0..cfg_n_rows() {
            let sql = "INSERT INTO kv (k, v) VALUES (?1, ?2)";
            let key = format!("key_{}", i);
            let _ = conn
                .query_row_params(
                    sql,
                    [&key as &dyn rusqlite::ToSql, &i as &dyn rusqlite::ToSql],
                )
                .ok();
        }
    }
    db.execute("COMMIT").unwrap();
}

fn bench_read_baseline(c: &mut Criterion) {
    let db = MemDB::instance();
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();

    let mut group = c.benchmark_group("knowledge_read_baseline");
    group.sampling_mode(SamplingMode::Auto);
    group.throughput(Throughput::Elements(cfg_reads() as u64));
    group.bench_function("select_by_key", |b| {
        let mut rng = rand::rng();
        b.iter(|| {
            for _ in 0..cfg_reads() {
                let k = keys.choose(&mut rng).unwrap();
                let p = [(":k", k.as_str())];
                let row = db
                    .query_row_params("SELECT v FROM kv WHERE k=:k", &p)
                    .unwrap();
                black_box(&row);
            }
        })
    });
    group.finish();
}

fn bench_read_with_cache(c: &mut Criterion) {
    let db = MemDB::instance();
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let mdb = MDBEnum::Use(db.clone());
    let mut cache = FieldQueryCache::with_capacity(cfg_reads());

    // prewarm cache (one full scan)
    for k in &keys {
        let params = [DataField::from_chars(":k", k.as_str())];
        let row = cache_query(
            &mdb,
            "SELECT v FROM kv WHERE k=:k",
            &params,
            &[(":k", k.as_str())],
            &mut cache,
        );
        assert!(!row.is_empty());
    }

    let mut group = c.benchmark_group("knowledge_read_cache");
    group.throughput(Throughput::Elements(cfg_reads() as u64));
    group.bench_function("select_by_key_cached", |b| {
        let mut rng = rand::rng();
        b.iter(|| {
            for _ in 0..cfg_reads() {
                let k = &keys[rng.gen_range(0..cfg_n_rows())];
                let params = [DataField::from_chars(":k", k.as_str())];
                let row = cache_query(
                    &mdb,
                    "SELECT v FROM kv WHERE k=:k",
                    &params,
                    &[(":k", k.as_str())],
                    &mut cache,
                );
                black_box(&row);
            }
        })
    });
    group.finish();
}

fn bench_read_prepared_once(c: &mut Criterion) {
    let db = MemDB::instance();
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();

    let mut group = c.benchmark_group("knowledge_read_prepared");
    group.throughput(Throughput::Elements(cfg_reads() as u64));
    const OPS_PER_SAMPLE: usize = 10_000;
    group.bench_function("select_by_key_prepared_once", |b| {
        b.iter_custom(|iters| {
            db.with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT v FROM kv WHERE k=?1")?;
                let mut rng = rand::rng();
                let start = Instant::now();
                for _ in 0..iters {
                    for _ in 0..OPS_PER_SAMPLE.min(cfg_reads()) {
                        let k = black_box(keys.choose(&mut rng).unwrap());
                        let _: i64 = stmt.query_row([k.as_str()], |row| row.get(0)).unwrap();
                    }
                }
                Ok(start.elapsed())
            })
            .unwrap()
        })
    });
    group.finish();
}

fn bench_read_hotspot_zipf(c: &mut Criterion) {
    let db = MemDB::instance();
    prepare_kv(&db);
    let mdb = MDBEnum::Use(db.clone());
    let mut cache = FieldQueryCache::with_capacity(cfg_reads());

    // Prewarm all keys (ensures缓存命中不会触发首次 miss 额外开销)
    for i in 0..cfg_n_rows() {
        let k = format!("key_{}", i);
        let params = [DataField::from_chars(":k", &k)];
        let _ = cache_query(
            &mdb,
            "SELECT v FROM kv WHERE k=:k",
            &params,
            &[(":k", k.as_str())],
            &mut cache,
        );
    }

    // 近似 Zipf(α) 热点分布：用加权采样近似（权重 w_k = 1/k^α）
    let s = cfg_zipf_alpha();
    let weights: Vec<f64> = (1..=cfg_n_rows()).map(|k| (k as f64).powf(-s)).collect();
    let dist = WeightedIndex::new(&weights).unwrap();
    let mut indices: Vec<usize> = Vec::with_capacity(cfg_reads());
    let mut rng = rand::rng();
    for _ in 0..cfg_reads() {
        let idx = dist.sample(&mut rng);
        indices.push(idx);
    }

    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();

    let mut group = c.benchmark_group("knowledge_read_hotspot_zipf");
    group.throughput(Throughput::Elements(cfg_reads() as u64));
    group.bench_function(BenchmarkId::from_parameter("zipf_a1.2"), |b| {
        b.iter(|| {
            for &idx in &indices {
                let k = &keys[idx];
                let params = [DataField::from_chars(":k", k.as_str())];
                let row = cache_query(
                    &mdb,
                    "SELECT v FROM kv WHERE k=:k",
                    &params,
                    &[(":k", k.as_str())],
                    &mut cache,
                );
                black_box(&row);
            }
        })
    });
    group.finish();
}

fn bench_read_concurrent_baseline(c: &mut Criterion) {
    let db = MemDB::global();
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let threads = cfg_threads();
    let per_thread = cfg_reads() / threads;

    let mut group = c.benchmark_group("knowledge_read_concurrent");
    group.throughput(Throughput::Elements((threads * per_thread) as u64));
    group.bench_function(BenchmarkId::from_parameter(format!("{}t", threads)), |b| {
        b.iter_custom(|iters| {
            // we use min(iters, 1) to keep a single run predictable
            let iters = iters.max(1);
            let start = Instant::now();
            for _ in 0..iters {
                let mut handles = Vec::with_capacity(threads);
                let keys_arc = Arc::new(keys.clone());
                for _ in 0..threads {
                    let keys_cl = keys_arc.clone();
                    handles.push(thread::spawn(move || {
                        let mdb = MemDB::global();
                        let mut rng = rand::rng();
                        for _ in 0..per_thread {
                            let k = keys_cl.choose(&mut rng).unwrap();
                            let p = [(":k", k.as_str())];
                            let _ = mdb
                                .query_row_params("SELECT v FROM kv WHERE k=:k", &p)
                                .unwrap();
                        }
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_read_concurrent_prepared(c: &mut Criterion) {
    let db = MemDB::global();
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let threads = cfg_threads();
    let per_thread = cfg_reads() / threads;

    let mut group = c.benchmark_group("knowledge_read_concurrent_prepared");
    group.throughput(Throughput::Elements((threads * per_thread) as u64));
    group.bench_function(BenchmarkId::from_parameter(format!("{}t", threads)), |b| {
        b.iter_custom(|iters| {
            let iters = iters.max(1);
            let start = Instant::now();
            for _ in 0..iters {
                let mut handles = Vec::with_capacity(threads);
                let keys_arc = Arc::new(keys.clone());
                for _ in 0..threads {
                    let keys_cl = keys_arc.clone();
                    handles.push(thread::spawn(move || {
                        let dbt = MemDB::global();
                        // hold a connection while running this thread's batch
                        dbt.with_conn(|conn| {
                            let mut stmt = conn.prepare("SELECT v FROM kv WHERE k=?1")?;
                            let mut rng = rand::rng();
                            for _ in 0..per_thread {
                                let k = keys_cl.choose(&mut rng).unwrap();
                                let _: i64 =
                                    stmt.query_row([k.as_str()], |row| row.get(0)).unwrap();
                            }
                            Ok::<_, anyhow::Error>(())
                        })
                        .unwrap();
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_read_concurrent_shared_prepared(c: &mut Criterion) {
    // Experimental: shared in-memory URI + pool>1 for higher concurrent throughput
    let threads = cfg_threads();
    let db = MemDB::shared_pool(threads as u32).expect("shared pool init");
    prepare_kv(&db);
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let per_thread = cfg_reads() / threads;

    let mut group = c.benchmark_group("knowledge_read_concurrent_shared_prepared");
    group.throughput(Throughput::Elements((threads * per_thread) as u64));
    group.bench_function(BenchmarkId::from_parameter(format!("{}t", threads)), |b| {
        b.iter_custom(|iters| {
            let iters = iters.max(1);
            let start = Instant::now();
            for _ in 0..iters {
                let mut handles = Vec::with_capacity(threads);
                let keys_arc = Arc::new(keys.clone());
                let db_arc = Arc::new(db.clone());
                for _ in 0..threads {
                    let keys_cl = keys_arc.clone();
                    let db_cl = db_arc.clone();
                    handles.push(thread::spawn(move || {
                        db_cl
                            .with_conn(|conn| {
                                let mut stmt = conn.prepare("SELECT v FROM kv WHERE k=?1")?;
                                let mut rng = rand::rng();
                                for _ in 0..per_thread {
                                    let k = keys_cl.choose(&mut rng).unwrap();
                                    let _: i64 =
                                        stmt.query_row([k.as_str()], |row| row.get(0)).unwrap();
                                }
                                Ok::<_, anyhow::Error>(())
                            })
                            .unwrap();
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn prepare_kv_authority(path: &str) {
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
        | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
        | rusqlite::OpenFlags::SQLITE_OPEN_URI;
    let db = MemDB::new_file(path, 1, flags).expect("create authority file db");
    db.execute("DROP TABLE IF EXISTS kv").unwrap();
    db.execute("CREATE TABLE kv (k TEXT PRIMARY KEY, v INTEGER)")
        .unwrap();
    db.execute("BEGIN IMMEDIATE").unwrap();
    for i in 0..cfg_n_rows() {
        let sql = "INSERT INTO kv (k, v) VALUES (?1, ?2)";
        let key = format!("key_{}", i);
        let _ = db
            .query_row_params(
                sql,
                [&key as &dyn rusqlite::ToSql, &i as &dyn rusqlite::ToSql],
            )
            .ok();
    }
    db.execute("COMMIT").unwrap();
}

fn bench_read_threadclone_prepared(c: &mut Criterion) {
    // Build authority file DB once
    let auth_path = "file:./wp_bench_authority.sqlite?mode=rwc&uri=true";
    prepare_kv_authority(auth_path);
    let tc = ThreadClonedMDB::from_authority("file:./wp_bench_authority.sqlite?mode=ro&uri=true");
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let threads = cfg_threads();
    let per_thread = cfg_reads() / threads;

    let mut group = c.benchmark_group("knowledge_read_threadclone_prepared");
    group.throughput(Throughput::Elements((threads * per_thread) as u64));
    group.bench_function(BenchmarkId::from_parameter(format!("{}t", threads)), |b| {
        b.iter_custom(|iters| {
            let iters = iters.max(1);
            let start = Instant::now();
            for _ in 0..iters {
                let mut handles = Vec::with_capacity(threads);
                let keys_arc = Arc::new(keys.clone());
                let tc_arc = Arc::new(tc.clone());
                for _ in 0..threads {
                    let keys_cl = keys_arc.clone();
                    let tc_cl = tc_arc.clone();
                    handles.push(thread::spawn(move || {
                        // Use the domain-specific result type expected by with_tls_conn
                        tc_cl
                            .with_tls_conn(|conn| {
                                // In benchmarks, unwrap is acceptable to keep the hot path clear
                                let mut stmt = conn
                                    .prepare("SELECT v FROM kv WHERE k=?1")
                                    .expect("prepare");
                                let mut rng = rand::rng();
                                for _ in 0..per_thread {
                                    let k = keys_cl.choose(&mut rng).unwrap();
                                    let _: i64 =
                                        stmt.query_row([k.as_str()], |row| row.get(0)).unwrap();
                                }
                                Ok(())
                            })
                            .unwrap();
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_read_walpool_prepared(c: &mut Criterion) {
    // Build authority (WAL) and create read-only pool
    let auth_path = "file:./wp_bench_authority.sqlite?mode=rwc&uri=true";
    prepare_kv_authority(auth_path);
    // enable WAL
    let _ = kdb::init_wal_pool_from_authority(
        "file:./wp_bench_authority.sqlite?mode=ro&uri=true",
        cfg_threads() as u32,
    );

    // We create a dedicated MemDB for benchmark to control pool
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI;
    let db = MemDB::new_file(
        "file:./wp_bench_authority.sqlite?mode=ro&uri=true",
        cfg_threads() as u32,
        flags,
    )
    .expect("wal pool");
    let keys: Vec<String> = (0..cfg_n_rows()).map(|i| format!("key_{}", i)).collect();
    let threads = cfg_threads();
    let per_thread = cfg_reads() / threads;

    let mut group = c.benchmark_group("knowledge_read_walpool_prepared");
    group.throughput(Throughput::Elements((threads * per_thread) as u64));
    group.bench_function(BenchmarkId::from_parameter(format!("{}t", threads)), |b| {
        b.iter_custom(|iters| {
            let iters = iters.max(1);
            let start = Instant::now();
            for _ in 0..iters {
                let mut handles = Vec::with_capacity(threads);
                let keys_arc = Arc::new(keys.clone());
                let db_arc = Arc::new(db.clone());
                for _ in 0..threads {
                    let keys_cl = keys_arc.clone();
                    let db_cl = db_arc.clone();
                    handles.push(thread::spawn(move || {
                        db_cl
                            .with_conn(|conn| {
                                let mut stmt =
                                    conn.prepare_cached("SELECT v FROM kv WHERE k=?1")?;
                                let mut rng = rand::rng();
                                for _ in 0..per_thread {
                                    let k = keys_cl.choose(&mut rng).unwrap();
                                    let _: i64 =
                                        stmt.query_row([k.as_str()], |row| row.get(0)).unwrap();
                                }
                                Ok::<_, anyhow::Error>(())
                            })
                            .unwrap();
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            }
            start.elapsed()
        })
    });
    group.finish();
}
criterion_group!(
    benches,
    bench_read_baseline,
    bench_read_with_cache,
    bench_read_prepared_once,
    bench_read_hotspot_zipf,
    bench_read_concurrent_baseline,
    bench_read_concurrent_prepared,
    bench_read_concurrent_shared_prepared,
    bench_read_threadclone_prepared,
    bench_read_walpool_prepared
);
criterion_main!(benches);
