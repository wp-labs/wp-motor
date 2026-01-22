use std::path::Path;

use orion_conf::ErrorOwe;
use wp_engine::facade::kit::WplCodePKG;
use wp_engine::sinks::ViewOuter;
use wp_engine::sinks::{SinkEndpoint, SinkRegistry};
use wp_error::RunResult;
use wpl::WplCode;

pub fn simple_ins_run_res<P: AsRef<Path> + Clone>(
    rule_file: Option<P>,
    rule_expr: Option<String>,
) -> RunResult<(WplCodePKG, SinkRegistry)> {
    let wpl_code = WplCode::mix_load(rule_file, rule_expr).owe_rule()?;
    let view = ViewOuter::new(wp_engine::sinks::DebugViewer::default());
    let view_miss = ViewOuter::new(wp_engine::sinks::DebugViewer::default());
    let sinks = SinkRegistry::new_sample(SinkEndpoint::View(view), SinkEndpoint::View(view_miss));
    Ok((WplCodePKG::from_model(vec![wpl_code]), sinks))
}
