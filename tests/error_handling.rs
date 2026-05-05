use orion_error::UnifiedReason as UvsReason;
use wp_engine::compat::LegacyOwe;
use wp_model_core::model::fmt_def::TextFmt;

use wp_conf::RunArgs;
use wp_engine::facade::kit::{WplCodePKG, wpl_workshop_parse};
use wp_engine::sinks::InfraSinkAgent;
use wp_engine::sinks::create_watch_out;
use wp_error::run_error::{RunReason, RunResult};
#[test]
fn should_handle_empty_input_gracefully() -> RunResult<()> {
    // Test case: Verify graceful handling of empty input data
    // This test ensures the parser can handle cases where input files contain no data

    let conf = r#"package /test_pkg {rule test {(ip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")} }"#;
    let in_path = "tests/err_test/sample.dat";
    let (_, _out) = create_watch_out(TextFmt::Kv);
    let args = RunArgs::for_test().expect("args");
    let pkg = WplCodePKG::from_code(conf).owe(RunReason::Uvs(UvsReason::rule_error()))?;
    wpl_workshop_parse(args, pkg, in_path, InfraSinkAgent::use_null())?;
    //assert_eq!(x.suc_cnt(), 0);
    Ok(())
}
