use criterion::{Criterion, criterion_group, criterion_main};
use orion_error::TestAssert;
use std::fmt::Write;
use wp_parse_api::RawData;
use wp_parser::Parser;
use wpl::{WplEvaluator, wpl_express};

fn wpl_parse(lpp: &WplEvaluator, data: &RawData) {
    for _ in 0..1000 {
        let _ = lpp.proc(0, data.clone(), 0);
    }
}

fn criterion_benchmark_suc(c: &mut Criterion) {
    let wpl = r#"(digit:id,digit:len,time,sn,chars:dev-name,time,kv,sn,chars:dev-name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
    "#;
    let data = r#"1407,509,2021-4-20 18:10:19,WCY7-ZT-QEAK-N6PD,ByHJpEtscumFff6FNLLjoFwMsOjVRWHMxxFT56NxfmktY1ASgo,2022-4-4 21:0:13,Tv7=9WxLPktFSMRBH4WRUCiBkmh2swZLod,DQGB-NL-RY2X-0SFD,cqIZXVT8FtAYrrlKI7q2CKL0D69Cg5jgbtnzzaJnUcUusZBIF5,2020-11-8 10:58:21,2022-4-13 14:27:12,111.237.105.120,TeG=ro1WpYpimAoG0n182NqwpkRvX2Xfod,q9gZeTkIxlCoGrAEUNqHhG17CT4OKebKXC0Ze5iXiyi2JYYnwc,hnB=FEdOhmFkM6SxBwiy3ATZePyBJBK5TT,YUC=X9JVE4p4WCNRwNjIdJ8mwnjLzs9fTY,Cmvp92V96paAHM8L60NzWl93AUHSR3WdxriwHmUDDxVohd8NcI,gtd=5srrDgB8YZMipedJ60jpl99HQg2SZR,8Ju=I1C1RzlgmX3IlS9Vp2hLsQWiudvZqz,uVAx1yArjlE1suY3887oCA44dWbm2MNZykeAqCwiq2KJbZlais,3ERd33ADEIKXISZLYWJx8juR455t753fybdcypXE2akn4KqITx,83.213.168.46,tzZ6oyqEA9ffm1e1Pi96344C6HVlw9zti4LWhBd0z9gStkFDuw,[GET /index  HTTP/1.1 ],"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.77 Safari/537.36""#;
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data.to_string());
    c.bench_function("bench_data_suc", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn criterion_benchmark_fail(c: &mut Criterion) {
    let wpl = r#"(digit:id,digit:len,time,sn,chars:dev-name,time,kv,sn,chars:dev-name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
    "#;
    let data = r#"1407,2021-4-20 18:10:19,WCY7-ZT-QEAK-N6PD,ByHJpEtscumFff6FNLLjoFwMsOjVRWHMxxFT56NxfmktY1ASgo,2022-4-4 21:0:13,Tv7=9WxLPktFSMRBH4WRUCiBkmh2swZLod,DQGB-NL-RY2X-0SFD,cqIZXVT8FtAYrrlKI7q2CKL0D69Cg5jgbtnzzaJnUcUusZBIF5,2020-11-8 10:58:21,2022-4-13 14:27:12,111.237.105.120,TeG=ro1WpYpimAoG0n182NqwpkRvX2Xfod,q9gZeTkIxlCoGrAEUNqHhG17CT4OKebKXC0Ze5iXiyi2JYYnwc,hnB=FEdOhmFkM6SxBwiy3ATZePyBJBK5TT,YUC=X9JVE4p4WCNRwNjIdJ8mwnjLzs9fTY,Cmvp92V96paAHM8L60NzWl93AUHSR3WdxriwHmUDDxVohd8NcI,gtd=5srrDgB8YZMipedJ60jpl99HQg2SZR,8Ju=I1C1RzlgmX3IlS9Vp2hLsQWiudvZqz,uVAx1yArjlE1suY3887oCA44dWbm2MNZykeAqCwiq2KJbZlais,3ERd33ADEIKXISZLYWJx8juR455t753fybdcypXE2akn4KqITx,83.213.168.46,tzZ6oyqEA9ffm1e1Pi96344C6HVlw9zti4LWhBd0z9gStkFDuw,[GET /index  HTTP/1.1 ],"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.77 Safari/537.36""#;
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data.to_string());
    c.bench_function("bench_data_fail", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn criterion_nginx(c: &mut Criterion) {
    let wpl = r#"(ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")"#;
    let data = r#"222.133.52.20 - - [06/Aug/2019:12:12:19 +0800] "GET /nginx-logo.png HTTP/1.1" 200 368 "http://119.122.1.4/" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36" "-""#;
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data.to_string());
    c.bench_function("nginx", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn build_deep_json(levels: usize, key_len: usize) -> String {
    // 生成形如 {"aaaa..": {"aaaa..": { ... 1 }}} 的深层 JSON
    let key = "a".repeat(key_len);
    let mut s = String::from("1");
    for _ in 0..levels {
        s = format!("{{\"{}\": {}}}", key, s);
    }
    s
}

fn build_array_json(key_len: usize, n: usize) -> String {
    let key = "k".repeat(key_len);
    let mut arr = String::with_capacity(n * 3);
    for i in 0..n {
        if i > 0 {
            arr.push(',');
        }
        arr.push_str(&i.to_string());
    }
    format!("{{\"{}\": {{ \"arr\": [{}] }} }}", key, arr)
}

fn criterion_json_deep(c: &mut Criterion) {
    // 深层 + 长 key，覆盖路径拼接热路径
    let wpl = r#"(json)"#;
    let data = build_deep_json(8, 64);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("json_deep_paths", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn criterion_json_array(c: &mut Criterion) {
    // 父级长 key + 中等规模数组
    let wpl = r#"(json)"#;
    let data = build_array_json(128, 64);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("json_large_array", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn build_flat_json(n: usize) -> String {
    let mut s = String::with_capacity(n * 12);
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "\"k{}\":{}", i, i);
    }
    s.push('}');
    s
}

fn build_wpl_subfields(n: usize) -> String {
    // 生成如 json(digit@k0, digit@k1, ...)
    let mut w = String::from("json(");
    for i in 0..n {
        if i > 0 {
            w.push(',');
        }
        let _ = write!(w, "digit@k{}", i);
    }
    w.push(')');
    w
}

fn criterion_json_flat_no_subs(c: &mut Criterion) {
    let wpl = r#"(json)"#;
    let data = build_flat_json(64);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("json_flat_no_subs", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn criterion_json_flat_with_subs(c: &mut Criterion) {
    // 20 个子字段，覆盖 sub_fpu 热路径
    let sub = build_wpl_subfields(20);
    let wpl = format!("({})", sub);
    let data = build_flat_json(64);
    let express = wpl_express.parse(&wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("json_flat_with_subs", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn build_escaped_text(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 16);
    s.push_str("{\"text\":\"");
    for i in 0..lines {
        let _ = write!(s, "L{}\\n", i);
    }
    s.push_str("\"}");
    s
}

fn criterion_json_decoded_pipe(c: &mut Criterion) {
    // 对比 decoded 管道开销
    let wpl = r#"(json(chars@text) | json_unescape())"#;
    let data = build_escaped_text(64);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("json_decoded_pipe", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn build_kv_bulk(n: usize) -> String {
    // 生成形如 k0=1 k1=2 ... 的 kv 文本
    let mut s = String::with_capacity(n * 8);
    for i in 0..n {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "k{}={}", i, i);
    }
    s
}

fn build_kv_wpl(n: usize) -> String {
    // 生成 (kv(@k0), kv(@k1), ...)
    let mut w = String::from("(");
    for i in 0..n {
        if i > 0 {
            w.push(',');
        }
        let _ = write!(w, "kv(@k{})", i);
    }
    w.push(')');
    w
}

fn criterion_kv_bulk(c: &mut Criterion) {
    let wpl = build_kv_wpl(16);
    let data = build_kv_bulk(32);
    let express = wpl_express.parse(&wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("kv_bulk", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn build_proto_deep(levels: usize) -> String {
    // a{b{c{...: "v"}}}
    let mut s = String::from("value: \"v\"");
    for i in (0..levels).rev() {
        let ch = ((b'a' + (i % 26) as u8) as char).to_string();
        s = format!("{} {{{}}}", ch, s);
    }
    s
}

fn build_proto_wide(n: usize) -> String {
    // obj { k0: "v" k1: "v" ... }
    let mut body = String::new();
    for i in 0..n {
        body.push_str(&format!("k{}: \"v\" ", i));
    }
    format!("obj {{{}}}", body)
}

fn criterion_proto_text_deep(c: &mut Criterion) {
    let wpl = r#"(proto_text)"#;
    let data = build_proto_deep(16);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("proto_text_deep", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

fn criterion_proto_text_wide(c: &mut Criterion) {
    let wpl = r#"(proto_text)"#;
    let data = build_proto_wide(128);
    let express = wpl_express.parse(wpl).assert();
    let lpp = WplEvaluator::from(&express, None).assert();
    let raw = RawData::from_string(data);
    c.bench_function("proto_text_wide", |b| b.iter(|| wpl_parse(&lpp, &raw)));
}

criterion_group!(
    benches,
    criterion_benchmark_suc,
    criterion_benchmark_fail,
    criterion_nginx,
    criterion_json_deep,
    criterion_json_array,
    criterion_json_flat_no_subs,
    criterion_json_flat_with_subs,
    criterion_json_decoded_pipe,
    criterion_kv_bulk,
    criterion_proto_text_deep,
    criterion_proto_text_wide
);
criterion_main!(benches);
