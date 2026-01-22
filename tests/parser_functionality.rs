//use wp_log::conf::{log_init, LogConf};
use std::io::Cursor;

use wp_conf::RunArgs;
use wp_engine::facade::test_helpers::SinkTerminal;
use wp_engine::types::{AnyResult, SafeH};
use wp_model_core::model::fmt_def::TextFmt;

use wp_engine::facade::kit::{WplCodePKG, engine_proc_file, wpl_workshop_parse};
use wp_engine::sinks::InfraSinkAgent;
use wp_engine::sinks::create_watch_out;

// 更直观的断言工具，避免在测试体内出现复杂分支
fn read_buffer(buffer: SafeH<Cursor<Vec<u8>>>) -> String {
    let vec = buffer
        .read()
        .expect("buffer read failed")
        .clone()
        .into_inner();
    String::from_utf8(vec).expect("buffer to utf8")
}

fn assert_buffer_eq(buffer: SafeH<Cursor<Vec<u8>>>, expect: &str) {
    let actual = read_buffer(buffer);
    assert_eq!(expect.trim(), actual.trim());
}

fn assert_buffer_starts_with(buffer: SafeH<Cursor<Vec<u8>>>, expect: &str, prefix_len: usize) {
    let actual = read_buffer(buffer);
    let a = actual;
    let e = expect;
    assert!(
        a.len() >= prefix_len && e.len() >= prefix_len,
        "prefix too long for compare: need {}, actual_len={}, expect_len={}",
        prefix_len,
        a.len(),
        e.len()
    );
    assert_eq!(&e[..prefix_len], &a[..prefix_len]);
}

#[test]
fn should_parse_nginx_access_log() -> AnyResult<()> {
    // Test case: Verify normal nginx access log parsing functionality
    // This test validates parsing of standard nginx access logs with HTTP request/response data

    let conf = r#"package /pkg {rule test {(ip,2*_,time<[,]>,http/request",http/status,digit:length,chars",http/agent",_")}}"#;
    let in_path = "tests/sample/nginx/sample.dat";
    let (buffer, out) = create_watch_out(TextFmt::Kv);
    let args = RunArgs::for_test().expect("args");
    let infra = InfraSinkAgent::only_default(SinkTerminal::Storage(out));
    wpl_workshop_parse(args, WplCodePKG::from_code(conf)?, in_path, infra)?;
    let expect = r#"ip: 192.168.1.2, time: 2019-08-06 12:12:19, http/request: "GET /nginx-logo.png HTTP/1.1", http/status: 200, length: 368, chars: "http://119.122.1.4/", http/agent: "Mozilla/5.0(Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36 ""#;
    assert_buffer_eq(buffer, expect);
    Ok(())
}

#[test]
fn should_parse_json_with_escaped_quotes() -> AnyResult<()> {
    // Test case: Verify JSON quota escaping functionality
    // This test validates parsing of JSON data with escaped quotes and special characters

    let expect = r#"{"a":1,"b":" 中国 "}"#;
    let conf = r#"package pkg { rule esc_1 {|unquote/unescape|(json)} }"#;
    let in_path = "tests/sample/format/quota.dat";
    let (buffer, _out) = create_watch_out(TextFmt::Json);

    let mut args = RunArgs::for_test().expect("args");
    args.line_max = Some(1);
    let infra = InfraSinkAgent::only_default(SinkTerminal::Storage(_out));
    wpl_workshop_parse(args, WplCodePKG::from_code(conf)?, in_path, infra)?;
    assert_buffer_eq(buffer, expect);
    Ok(())
}

#[test]
#[ignore = "format dismatch"]
fn should_parse_huawei_zhuru_traffic_prefix() -> AnyResult<()> {
    // Test case: Verify Huawei ZhuRu network traffic probe parsing
    // This test validates parsing of Huawei firewall network traffic logs with base64 data

    let expect = "id: 190, time: 2023-05-15 07:09:12, sn: KM-KJY-DC-USG12004-B02, chars: %%01POLICY/6/POLICYPERMIT(l), CID: 0x814f041e, vsys: CSG_Security, protocol: 6, source-ip: 10.111.117.49, source-port: 34616, destination-ip: 10.111.48.230, dport: 60051, time: 2023/5/15 15:09:12, _src-zone: untrust, _dst-zone: trust, application-name: , line-name: HO202212060377705-1.\0,";
    let rule = r#"package /pkg { rule/network/traffic_probe { |decode/base64|(digit:id<<,>>,time,sn,chars\:),opt(kv\;), (*kv\,)}}"#;

    let in_path = "tests/sample/huawei_zhuru/sample.dat";
    let (buffer, _out) = create_watch_out(TextFmt::Kv);
    let infra = InfraSinkAgent::only_default(SinkTerminal::Storage(_out));
    engine_proc_file(WplCodePKG::from_code(rule)?, in_path, infra, 1)?;
    assert_buffer_starts_with(buffer, expect, 200);
    Ok(())
}
