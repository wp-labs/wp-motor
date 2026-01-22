use crate::core::sinks::sync_sink::RecSyncSink;
use crate::orchestrator::engine::definition::WplCodePKG;
use crate::{
    core::prelude::*,
    sinks::{BlackHoleSink, ProcMeta, SinkRecUnit},
};
use std::sync::Arc;
use wp_model_core::model::DataRecord;

use derive_getters::Getters;
use orion_error::ErrStrategy;
use std::collections::HashSet;
use wpl::WplPackage;

#[derive(Clone, Default)]
pub struct WplRepository {
    pub packages: Vec<WplPackage>,
}

#[derive(Default, Debug, Clone, Getters)]
pub struct SpaceIndex {
    rule_key: HashSet<String>,
    pkg_key: HashSet<String>,
}

impl From<&WplRepository> for SpaceIndex {
    fn from(value: &WplRepository) -> Self {
        let mut pkg_path_vec = HashSet::new();
        let mut rule_path_vec = HashSet::new();
        for pkg in &value.packages {
            pkg_path_vec.insert(pkg.name().to_string());
            for rule in &pkg.rules {
                rule_path_vec.insert(format!("{}/{}", pkg.name(), rule.name()));
            }
        }
        Self {
            pkg_key: pkg_path_vec,
            rule_key: rule_path_vec,
        }
    }
}

impl WplRepository {
    fn from_wpl_impl(
        value: WplCodePKG,
        err_sink: Option<&impl RecSyncSink>,
    ) -> WplCodeResult<Self> {
        let mut rules = Vec::new();

        for wpl_code in value.code_vec() {
            let mut code = wpl_code.get_code().as_str();
            let path = wpl_code.path().to_str().unwrap_or("unknown").to_string();
            let rule = WplPackage::parse(&mut code, path.as_str()); //.owe_rule()?;
            match rule {
                Ok(rule_ok) => {
                    info_ctrl!("success load & parse code : {:?}", wpl_code.path());
                    rules.push(rule_ok);
                }
                Err(e) => {
                    let info = format!("WPL load failed!, path: {}", path);
                    error_ctrl!("{}", info);
                    match current_error_policy().err4_load_wpl(&e) {
                        ErrStrategy::Retry => {}
                        ErrStrategy::Ignore => {
                            let mut report = ErrReport::new_wpl(info);
                            report.add_code(wpl_code.get_code());
                            report.add_error(e);
                            if let Some(sink) = err_sink {
                                // 创建一个临时的 SinkRecUnit 用于错误报告
                                let record = DataRecord::default();
                                sink.send_to_sink(SinkRecUnit::new(
                                    0,
                                    ProcMeta::Null,
                                    Arc::new(record),
                                ))
                                .owe_rule()?;
                            }
                        }
                        ErrStrategy::Throw => {
                            return Err(e).with(path);
                        }
                    }
                }
            }
        }

        Ok(Self { packages: rules })
    }
    pub fn from_wpl_tolerant(value: WplCodePKG, error: &impl RecSyncSink) -> WplCodeResult<Self> {
        Self::from_wpl_impl(value, Some(error))
    }
    pub fn from_wpl_strict(value: WplCodePKG) -> WplCodeResult<Self> {
        Self::from_wpl_impl(value, None::<&BlackHoleSink>)
    }

    pub fn get_rule_names(&self) -> HashSet<String> {
        let mut rule_names = HashSet::new();
        for package in &self.packages {
            for rule in &package.rules {
                rule_names.insert(format!("{}/{}", package.name, rule.name));
            }
        }
        rule_names
    }
}
