use wp_model_core::model::fmt_def::TextFmt;

use wp_conf::RunArgs;
use wp_engine::facade::kit::{WplCodePKG, wpl_workshop_parse};
use wp_engine::sinks::InfraSinkAgent;
use wp_engine::sinks::create_watch_out;
use wp_engine::types::AnyResult;

#[test]
fn should_handle_empty_input_gracefully() -> AnyResult<()> {
    // Test case: Verify graceful handling of empty input data
    // This test ensures the parser can handle cases where input files contain no data

    let conf = r#"package /test_pkg {rule test {(ip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")} }"#;
    let in_path = "tests/err_test/sample.dat";
    let (_, _out) = create_watch_out(TextFmt::Kv);
    let args = RunArgs::for_test().expect("args");
    wpl_workshop_parse(
        args,
        WplCodePKG::from_code(conf)?,
        in_path,
        InfraSinkAgent::use_null(),
    )?;
    //assert_eq!(x.suc_cnt(), 0);
    Ok(())
}
