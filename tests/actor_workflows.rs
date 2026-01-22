//! Core logic integration tests
//!
//! This test suite validates the core functionality of the warp-parse system,
//! including actor workflows, rule generation, and data processing pipelines.

use async_broadcast::broadcast;
use std::collections::HashMap;
use tokio::sync::mpsc;
use wp_connector_api::Tags;
use wp_engine::facade::generator::rule_gen_run;
use wp_engine::facade::kit::engine_check;
use wp_engine::facade::test_helpers::ActorCtrlCmd;
use wp_engine::facade::test_helpers::GenRuleUnit;
use wp_engine::facade::test_helpers::ParseOption;
use wp_engine::facade::test_helpers::SinkTerminal;
use wp_engine::facade::test_helpers::{
    ActParser, ActorWork, ParseWorkerSender, RuleGRA, async_test_prepare, read_data,
};
use wp_model_core::model::fmt_def::TextFmt;

use wp_engine::facade::kit::WplCodePKG;
use wp_engine::facade::test_helpers::ShutdownCmd;

use wp_engine::sinks::BufferMonitor;
use wp_engine::sinks::FormatAdapter;
use wp_engine::sinks::InfraSinkAgent;
use wp_engine::sinks::build_file_sink;
use wp_engine::sinks::create_watch_out;
use wp_engine::sinks::{SinkBackendType, SinkEndpoint};
use wp_engine::sinks::{SinkRegistry, SinkRouteAgent};

use wp_conf::RunArgs;
use wp_conf::structure::SinkInstanceConf;
use wp_engine::types::AnyResult;
use wpl::WplCode;

//=============================================================================
// Actor Workflow Tests
//=============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn actor_workflows_process_http_logs_with_base64_decoding() -> AnyResult<()> {
    // Test configuration: Parse H3C WAF logs with base64 decoding
    let rule =
        r#"package /test_pkg { rule test {|decode/base64|(digit:id<<,>>,time,sn,time,*_)} }"#;
    let sample_data_path = "tests/sample/h3c_waf/sample.dat";

    // Prepare test environment and input data
    let (mut args, mut input_io) = async_test_prepare(sample_data_path)
        .await
        .expect("Failed to prepare test environment");
    args.line_max = Some(1);

    // Setup communication channels
    let (data_sender, data_receiver) = mpsc::channel(32);
    let (command_sender, command_receiver) = broadcast(1000);
    let (monitor_sender, _monitor_receiver) = mpsc::channel(1000);

    // Configure data subscription
    let subscription_channel = ParseWorkerSender::new(data_sender);
    let source_key = "/test_pkg/test";

    // Read and process input data
    let read_result = read_data(
        &mut input_io,
        source_key.to_string(),
        Tags::default(),
        command_sender.clone(),
        subscription_channel,
        args.line_max,
    )
    .await;
    println!("Read data result: {:?}", read_result);

    // Setup data processing pipeline
    let mut processing_pipeline: FormatAdapter<BufferMonitor> = FormatAdapter::new(TextFmt::Json);
    let buffer_monitor = BufferMonitor::new();
    let output_buffer = buffer_monitor.buffer.clone();
    processing_pipeline.next_pipe(buffer_monitor);

    // Configure sink routing
    let sink_router = SinkRouteAgent::null();
    let sink_infrastructure = InfraSinkAgent::only_default(SinkTerminal::Storage(
        SinkEndpoint::Buffer(processing_pipeline),
    ));

    // Parse and build service definition
    let service_code = WplCodePKG::from_code(rule)?;

    // Initialize actor processor
    let mut actor_processor = ActorWork::new(
        "actor-workflow-http-base64-test",
        data_receiver,
        command_receiver,
        monitor_sender,
        ActParser::from_normal(service_code, sink_router, sink_infrastructure, Vec::new())
            .expect("Failed to build language processor"),
    );

    // Configure parsing settings
    let parsing_settings = ParseOption::default();

    // Execute processing with timeout shutdown
    command_sender
        .broadcast(ActorCtrlCmd::Stop(ShutdownCmd::Timeout(100)))
        .await?;

    let processing_result = actor_processor.proc(parsing_settings).await;
    println!("Processing result: {:?}", processing_result);
    processing_result.expect("Actor processing failed!");

    // Validate output
    let output_data = output_buffer
        .read()
        .expect("Failed to read output buffer")
        .clone()
        .into_inner();

    println!("Output buffer length: {}", output_data.len());
    let output_string =
        String::from_utf8(output_data.clone()).expect("Failed to convert output data to string");
    let actual_output = output_string.trim();
    println!("Actual output: '{}'", actual_output);

    let expected_output = r#"{"id":188,"time":"2026-05-17 08:54:06","sn":"KMDLDLH3CW2020G2WAFA01","time":"2023-05-17 08:54:06"}"#;
    println!("Expected output: '{}'", expected_output);

    assert_eq!(
        expected_output, actual_output,
        "Parsed output should match expected JSON format"
    );

    println!("✅ Actor workflow successfully processed HTTP logs with base64 decoding");
    Ok(())
}

//=============================================================================
// Rule Generation Tests
//=============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn rule_generator_end_to_end_processing() -> AnyResult<()> {
    // Test configuration: Generate complex log patterns with multiple fields
    let generation_rule = "package /test_pkg { rule test { (digit,time,sn,chars,time,kv,sn,chars,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request,http/agent)\\,} }";

    // Configure rule generation arguments
    let generation_args = RuleGRA {
        ..Default::default()
    };

    // Setup file sink for generated data
    let sink_configuration = SinkInstanceConf::file_new(
        "rule-generator-test".to_string(),
        TextFmt::Csv,
        "/tmp/rule_generator_test.log",
        None,
    );

    // Create output target
    let output_target = SinkBackendType::Proxy(Box::new(
        build_file_sink(&sink_configuration, "/tmp/rule_generator_test.log").await?,
    ));

    // Parse and compile generation rules
    let language_rules = WplCode::try_from(generation_rule)?.parse_pkg()?;
    let rule_unit = GenRuleUnit::new(language_rules.clone(), HashMap::new());

    // Execute rule generation
    rule_gen_run(generation_args.clone(), vec![rule_unit], output_target)
        .await
        .expect("Rule generation failed");

    // Setup validation arguments
    let validation_args = RunArgs {
        line_max: Some(10),
        check: 1,
        ..Default::default()
    };

    // Create output watcher for validation
    let (_, validation_output) = create_watch_out(TextFmt::Kv);

    // Execute end-to-end validation
    let generated_data_path = "/tmp/rule_generator_test.log";
    engine_check(
        validation_args,
        WplCodePKG::from_code(generation_rule)?,
        generated_data_path,
        SinkRegistry::for_test(validation_output),
    )
    .expect("End-to-end data validation failed!");

    println!("✅ Rule generator end-to-end processing completed successfully");
    Ok(())
}
