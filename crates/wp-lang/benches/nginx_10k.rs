use chrono::DateTime;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use orion_error::TestAssert;
use std::hint::black_box;
use wp_parse_api::RawData;
use wp_parser::Parser;
use wpl::{WplEvaluator, wpl_express};

// 构造 10k 行 nginx 样本（复用仓库自带样例）
fn build_nginx_lines(n: usize) -> Vec<String> {
    let sample = "222.133.52.20 - - [06/Aug/2019:12:12:19 +0800] \"GET /nginx-logo.png HTTP/1.1\" 200 368 \"http://119.122.1.4/\" \"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36\" \"-\"";
    std::iter::repeat_n(sample.to_string(), n).collect()
}

// 预处理：将 [dd/Mon/yyyy:HH:MM:SS +ZZZZ] 转换为 [<epoch_sec>]
fn to_epoch_lines(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for s in lines {
        // 找到第一对方括号
        if let Some(l) = s.find('[') {
            if let Some(rel) = s[l..].find(']') {
                let r = l + rel;
                let inner = &s[l + 1..r];
                // 解析成 epoch（只在预处理阶段，解析失败则保留原行，避免 bench 崩）
                let ts = DateTime::parse_from_str(inner, "%d/%b/%Y:%H:%M:%S %z")
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);
                let mut new_s = String::with_capacity(s.len());
                new_s.push_str(&s[..=l]); // 包含左括号
                new_s.push_str(&ts.to_string());
                new_s.push_str(&s[r..]); // 包含右括号
                out.push(new_s);
            } else {
                out.push(s.clone());
            }
        } else {
            out.push(s.clone());
        }
    }
    out
}

fn run_parse_all(evaluator: &WplEvaluator, lines: &[String]) {
    for s in lines {
        let raw = RawData::from_string(s.clone());
        let _ = black_box(evaluator.proc(raw, 0));
    }
}

pub fn bench_nginx_10k(c: &mut Criterion) {
    let n = std::env::var("WF_BENCH_LINES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000);
    let lines = build_nginx_lines(n);
    let lines_epoch = to_epoch_lines(&lines);

    // 与 usecase/benchmark_case/tcp2file_test/models/wpl/nginx/parse.wpl 中 rule 等价的表达式
    let wpl_full =
        r#"(ip:sip,2*_,time<[,]>,http/request\",http/status,digit,chars\",http/agent\",_\")"#;
    let wpl_full_clf =
        r#"(ip:sip,2*_,time/clf<[,]>,http/request\",http/status,digit,chars\",http/agent\",_\")"#;
    // 去掉 time，粗略估算 chrono 路径的占比（两者差值）
    let wpl_no_time =
        r#"(ip:sip,2*_,chars,http/request\",http/status,digit,chars\",http/agent\",_\")"#;
    // 时间换成 timestamp 快路径，粗测“fast path”潜力
    let wpl_epoch =
        r#"(ip:sip,2*_,time/timestamp,http/request\",http/status,digit,chars\",http/agent\",_\")"#;

    let express_full = wpl_express.parse(wpl_full).assert();
    let express_full_clf = wpl_express.parse(wpl_full_clf).assert();
    let express_no_time = wpl_express.parse(wpl_no_time).assert();
    let express_epoch = wpl_express.parse(wpl_epoch).assert();

    let eval_full = WplEvaluator::from(&express_full, None).assert();
    let eval_full_clf = WplEvaluator::from(&express_full_clf, None).assert();
    let eval_no_time = WplEvaluator::from(&express_no_time, None).assert();
    let eval_epoch = WplEvaluator::from(&express_epoch, None).assert();

    let mut group = c.benchmark_group("nginx_10k");
    // 增加测量时间，避免超快用例导致 0ms 量化误差
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(30);
    group.throughput(Throughput::Elements(lines.len() as u64));

    group.bench_function(BenchmarkId::new("full", lines.len()), |b| {
        b.iter(|| run_parse_all(&eval_full, &lines))
    });
    group.bench_function(BenchmarkId::new("full_clf", lines.len()), |b| {
        b.iter(|| run_parse_all(&eval_full_clf, &lines))
    });
    group.bench_function(BenchmarkId::new("no_time", lines.len()), |b| {
        b.iter(|| run_parse_all(&eval_no_time, &lines))
    });
    group.bench_function(BenchmarkId::new("epoch", lines.len()), |b| {
        b.iter(|| run_parse_all(&eval_epoch, &lines_epoch))
    });

    group.finish();
}

criterion_group!(benches, bench_nginx_10k);
criterion_main!(benches);
