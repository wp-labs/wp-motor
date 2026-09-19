use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mysql::prelude::Queryable;
use mysql::{Opts, Pool};
use oml::core::AsyncDataTransformer;
use oml::parser::oml_parse_raw;
use tokio::sync::Barrier;
use tokio::task::JoinSet;
use tokio_postgres::NoTls;
use wp_knowledge::cache::FieldQueryCache;
use wp_knowledge::facade as kdb;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

fn oml_perf_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

fn perf_env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn perf_concurrency_levels() -> Vec<usize> {
    std::env::var("WP_KDB_PERF_CONCURRENCY")
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|part| part.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .collect::<Vec<_>>()
        })
        .filter(|levels| !levels.is_empty())
        .unwrap_or_else(|| vec![1, 4, 16, 64])
}

#[derive(Debug, Clone)]
struct ThroughputResult {
    name: &'static str,
    elapsed: Duration,
    ops: usize,
}

impl ThroughputResult {
    fn qps(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs == 0.0 {
            self.ops as f64
        } else {
            self.ops as f64 / secs
        }
    }
}

#[derive(Debug, Clone)]
struct ConcurrentThroughputResult {
    name: &'static str,
    concurrency: usize,
    elapsed: Duration,
    ops: usize,
    p50_us: f64,
    p95_us: f64,
}

impl ConcurrentThroughputResult {
    fn qps(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs == 0.0 {
            self.ops as f64
        } else {
            self.ops as f64 / secs
        }
    }
}

async fn parse_model(code: &str) -> oml::language::ObjModel {
    let mut code_ref = code;
    oml_parse_raw(&mut code_ref).await.expect("parse OML model")
}

fn build_workload(ops: usize, hotset: usize) -> Vec<DataRecord> {
    (0..ops)
        .map(|idx| {
            let id = ((idx * 17) % hotset + 1) as i64;
            DataRecord::from(vec![FieldStorage::from_owned(DataField::from_int(
                "id", id,
            ))])
        })
        .collect()
}

fn shard_workload(workload: &[DataRecord], workers: usize) -> Vec<Vec<DataRecord>> {
    let worker_count = workers.max(1).min(workload.len().max(1));
    let mut shards = vec![Vec::new(); worker_count];
    for (idx, item) in workload.iter().cloned().enumerate() {
        shards[idx % worker_count].push(item);
    }
    shards
}

fn percentile_us(values: &[f64], numerator: usize, denominator: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("latency sort"));
    let idx = ((sorted.len() - 1) * numerator) / denominator;
    sorted[idx]
}

async fn run_async_model(
    model: &oml::language::ObjModel,
    workload: &[DataRecord],
) -> ThroughputResult {
    let mut cache = FieldQueryCache::with_capacity(workload.len().max(1));
    let started = Instant::now();
    for record in workload {
        let row = model.transform_async(record.clone(), &mut cache).await;
        std::hint::black_box(row);
    }
    ThroughputResult {
        name: "async",
        elapsed: started.elapsed(),
        ops: workload.len(),
    }
}

fn print_throughput_result(provider: &str, result: &ThroughputResult) {
    eprintln!(
        "[wp-oml][{provider}-async-throughput] scenario={} elapsed_ms={} qps={:.0}",
        result.name,
        result.elapsed.as_millis(),
        result.qps(),
    );
}

fn print_concurrent_throughput_result(provider: &str, result: &ConcurrentThroughputResult) {
    eprintln!(
        "[wp-oml][{provider}-async-cache-concurrency] scenario={} concurrency={} elapsed_ms={} qps={:.0} p50_us={:.2} p95_us={:.2}",
        result.name,
        result.concurrency,
        result.elapsed.as_millis(),
        result.qps(),
        result.p50_us,
        result.p95_us,
    );
}

async fn seed_postgres(url: &str, rows: usize) {
    let (client, connection) = tokio_postgres::connect(url, NoTls)
        .await
        .expect("connect postgres for OML perf");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute(
            r#"
CREATE TABLE IF NOT EXISTS wp_oml_pg_perf_lookup (
    id BIGINT PRIMARY KEY,
    value TEXT NOT NULL
);
TRUNCATE TABLE wp_oml_pg_perf_lookup;
"#,
        )
        .await
        .expect("prepare postgres OML perf table");
    let stmt = client
        .prepare("INSERT INTO wp_oml_pg_perf_lookup (id, value) VALUES ($1, $2)")
        .await
        .expect("prepare postgres OML perf insert");
    for id in 1..=rows as i64 {
        let value = format!("value_{id}");
        client
            .execute(&stmt, &[&id, &value])
            .await
            .expect("insert postgres OML perf row");
    }
}

async fn run_async_model_concurrent(
    name: &'static str,
    model: &oml::language::ObjModel,
    workload: &[DataRecord],
    concurrency: usize,
) -> ConcurrentThroughputResult {
    let worker_count = concurrency.max(1).min(workload.len().max(1));
    let barrier = std::sync::Arc::new(Barrier::new(worker_count + 1));
    let mut set = JoinSet::new();
    for shard in shard_workload(workload, worker_count) {
        let barrier = barrier.clone();
        let model = model.clone();
        set.spawn(async move {
            let mut cache = FieldQueryCache::with_capacity(shard.len().max(1));
            let mut samples_us = Vec::with_capacity(shard.len());
            barrier.wait().await;
            for record in shard {
                let op_started = Instant::now();
                let row = model.transform_async(record, &mut cache).await;
                std::hint::black_box(row);
                samples_us.push(op_started.elapsed().as_secs_f64() * 1_000_000.0);
            }
            samples_us
        });
    }

    let started = Instant::now();
    barrier.wait().await;
    let mut samples_us = Vec::with_capacity(workload.len());
    while let Some(joined) = set.join_next().await {
        samples_us.extend(joined.expect("join oml perf worker"));
    }

    ConcurrentThroughputResult {
        name,
        concurrency: worker_count,
        elapsed: started.elapsed(),
        ops: workload.len(),
        p50_us: percentile_us(&samples_us, 50, 100),
        p95_us: percentile_us(&samples_us, 95, 100),
    }
}

fn seed_mysql(url: &str, rows: usize) {
    let opts = Opts::from_url(url).expect("parse mysql OML perf url");
    let pool = Pool::new(opts).expect("connect mysql OML perf");
    let mut admin = pool.get_conn().expect("open mysql OML perf conn");
    admin
        .query_drop(
            r#"
CREATE TABLE IF NOT EXISTS wp_oml_mysql_perf_lookup (
    id BIGINT PRIMARY KEY,
    value TEXT NOT NULL
)
"#,
        )
        .expect("create mysql OML perf table");
    admin
        .query_drop("TRUNCATE TABLE wp_oml_mysql_perf_lookup")
        .expect("truncate mysql OML perf table");
    for id in 1..=rows as i64 {
        admin
            .exec_drop(
                "INSERT INTO wp_oml_mysql_perf_lookup (id, value) VALUES (?, ?)",
                (id, format!("value_{id}")),
            )
            .expect("insert mysql OML perf row");
    }
}

#[test]
#[ignore = "requires WP_KDB_TEST_POSTGRES_URL and a reachable PostgreSQL instance"]
fn oml_async_postgres_provider_throughput() {
    let _guard = oml_perf_guard().lock().expect("oml perf guard");
    let url = std::env::var("WP_KDB_TEST_POSTGRES_URL")
        .expect("WP_KDB_TEST_POSTGRES_URL must be set for OML postgres perf");
    let rows = perf_env_usize("WP_KDB_PERF_ROWS", 10_000).max(1);
    let ops = perf_env_usize("WP_KDB_PERF_OPS", 10_000).max(1);
    let hotset = perf_env_usize("WP_KDB_PERF_HOTSET", 128).clamp(1, rows);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for OML postgres perf");
    rt.block_on(async {
        seed_postgres(&url, rows).await;
        kdb::init_postgres_provider(&url, Some(8)).expect("init postgres provider for OML perf");

        let pure_model = parse_model(
            r#"
name : bench
---
V = read(id) ;
"#,
        )
        .await;
        let sql_model = parse_model(
            r#"
name : bench
---
V = select value from wp_oml_pg_perf_lookup where id = read(id) ;
"#,
        )
        .await;
        let workload = build_workload(ops, hotset);

        let pure = run_async_model(&pure_model, &workload).await;
        let with_provider = run_async_model(&sql_model, &workload).await;

        eprintln!(
            "[wp-oml][postgres-async-throughput] rows={} ops={} hotset={}",
            rows, ops, hotset
        );
        print_throughput_result("postgres", &pure);
        print_throughput_result("postgres", &with_provider);
        eprintln!(
            "[wp-oml][postgres-async-throughput] ratio provider_vs_pure={:.2}x",
            with_provider.qps() / pure.qps()
        );
    });
}

#[test]
#[ignore = "requires WP_KDB_TEST_MYSQL_URL and a reachable MySQL instance"]
fn oml_async_mysql_provider_throughput() {
    let _guard = oml_perf_guard().lock().expect("oml perf guard");
    let url = std::env::var("WP_KDB_TEST_MYSQL_URL")
        .expect("WP_KDB_TEST_MYSQL_URL must be set for OML mysql perf");
    let rows = perf_env_usize("WP_KDB_PERF_ROWS", 10_000).max(1);
    let ops = perf_env_usize("WP_KDB_PERF_OPS", 10_000).max(1);
    let hotset = perf_env_usize("WP_KDB_PERF_HOTSET", 128).clamp(1, rows);

    seed_mysql(&url, rows);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for OML mysql perf");
    rt.block_on(async {
        kdb::init_mysql_provider(&url, Some(8)).expect("init mysql provider for OML perf");

        let pure_model = parse_model(
            r#"
name : bench
---
V = read(id) ;
"#,
        )
        .await;
        let sql_model = parse_model(
            r#"
name : bench
---
V = select value from wp_oml_mysql_perf_lookup where id = read(id) ;
"#,
        )
        .await;
        let workload = build_workload(ops, hotset);

        let pure = run_async_model(&pure_model, &workload).await;
        let with_provider = run_async_model(&sql_model, &workload).await;

        eprintln!(
            "[wp-oml][mysql-async-throughput] rows={} ops={} hotset={}",
            rows, ops, hotset
        );
        print_throughput_result("mysql", &pure);
        print_throughput_result("mysql", &with_provider);
        eprintln!(
            "[wp-oml][mysql-async-throughput] ratio provider_vs_pure={:.2}x",
            with_provider.qps() / pure.qps()
        );
    });
}

#[test]
#[ignore = "requires WP_KDB_TEST_POSTGRES_URL and a reachable PostgreSQL instance"]
fn oml_async_postgres_provider_cache_concurrency() {
    let _guard = oml_perf_guard().lock().expect("oml perf guard");
    let url = std::env::var("WP_KDB_TEST_POSTGRES_URL")
        .expect("WP_KDB_TEST_POSTGRES_URL must be set for OML postgres perf");
    let rows = perf_env_usize("WP_KDB_PERF_ROWS", 10_000).max(1);
    let ops = perf_env_usize("WP_KDB_PERF_OPS", 20_000).max(1);
    let hotset = perf_env_usize("WP_KDB_PERF_HOTSET", 128).clamp(1, rows);
    let concurrencies = perf_concurrency_levels();
    let worker_threads = concurrencies
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .clamp(2, 16);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .expect("build tokio runtime for OML postgres concurrency perf");
    rt.block_on(async {
        seed_postgres(&url, rows).await;
        kdb::init_postgres_provider(&url, Some(8)).expect("init postgres provider for OML perf");

        let pure_model = parse_model(
            r#"
name : bench
---
V = read(id) ;
"#,
        )
        .await;
        let sql_model = parse_model(
            r#"
name : bench
---
V = select value from wp_oml_pg_perf_lookup where id = read(id) ;
"#,
        )
        .await;
        let workload = build_workload(ops, hotset);

        eprintln!(
            "[wp-oml][postgres-async-cache-concurrency] rows={} ops={} hotset={} concurrencies={:?}",
            rows, ops, hotset, concurrencies
        );

        for concurrency in concurrencies {
            let pure =
                run_async_model_concurrent("pure_async", &pure_model, &workload, concurrency)
                    .await;
            let with_provider = run_async_model_concurrent(
                "async_provider_cache",
                &sql_model,
                &workload,
                concurrency,
            )
            .await;

            print_concurrent_throughput_result("postgres", &pure);
            print_concurrent_throughput_result("postgres", &with_provider);
            eprintln!(
                "[wp-oml][postgres-async-cache-concurrency] concurrency={} ratio provider_vs_pure={:.2}x",
                concurrency,
                with_provider.qps() / pure.qps()
            );
        }
    });
}

#[test]
#[ignore = "requires WP_KDB_TEST_MYSQL_URL and a reachable MySQL instance"]
fn oml_async_mysql_provider_cache_concurrency() {
    let _guard = oml_perf_guard().lock().expect("oml perf guard");
    let url = std::env::var("WP_KDB_TEST_MYSQL_URL")
        .expect("WP_KDB_TEST_MYSQL_URL must be set for OML mysql perf");
    let rows = perf_env_usize("WP_KDB_PERF_ROWS", 10_000).max(1);
    let ops = perf_env_usize("WP_KDB_PERF_OPS", 20_000).max(1);
    let hotset = perf_env_usize("WP_KDB_PERF_HOTSET", 128).clamp(1, rows);
    let concurrencies = perf_concurrency_levels();
    let worker_threads = concurrencies
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .clamp(2, 16);

    seed_mysql(&url, rows);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .expect("build tokio runtime for OML mysql concurrency perf");
    rt.block_on(async {
        kdb::init_mysql_provider(&url, Some(8)).expect("init mysql provider for OML perf");

        let pure_model = parse_model(
            r#"
name : bench
---
V = read(id) ;
"#,
        )
        .await;
        let sql_model = parse_model(
            r#"
name : bench
---
V = select value from wp_oml_mysql_perf_lookup where id = read(id) ;
"#,
        )
        .await;
        let workload = build_workload(ops, hotset);

        eprintln!(
            "[wp-oml][mysql-async-cache-concurrency] rows={} ops={} hotset={} concurrencies={:?}",
            rows, ops, hotset, concurrencies
        );

        for concurrency in concurrencies {
            let pure =
                run_async_model_concurrent("pure_async", &pure_model, &workload, concurrency)
                    .await;
            let with_provider = run_async_model_concurrent(
                "async_provider_cache",
                &sql_model,
                &workload,
                concurrency,
            )
            .await;

            print_concurrent_throughput_result("mysql", &pure);
            print_concurrent_throughput_result("mysql", &with_provider);
            eprintln!(
                "[wp-oml][mysql-async-cache-concurrency] concurrency={} ratio provider_vs_pure={:.2}x",
                concurrency,
                with_provider.qps() / pure.qps()
            );
        }
    });
}
