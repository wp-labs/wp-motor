use crate::common::paths::OUT_FILE_PATH;
use crate::structure::SinkInstanceConf;
use crate::structure::group::default_flush_interval_ms;
use crate::structure::{Basis, ExpectMode, FixedGroup, FlexGroup, GroupExpectSpec};
use wp_model_core::model::fmt_def::TextFmt;
use wp_specs::WildArray;

impl FixedGroup {
    pub fn default_ins() -> Self {
        FixedGroup {
            name: "default".to_string(),
            expect: None,
            sinks: vec![SinkInstanceConf::file_new(
                "default_sink".to_string(),
                TextFmt::ProtoText,
                format!("{}/default.dat", OUT_FILE_PATH),
                None,
            )],
            parallel: 1,
            flush_interval_ms: default_flush_interval_ms(),
        }
    }

    pub fn miss_ins() -> Self {
        FixedGroup {
            name: "miss".to_string(),
            expect: None,
            sinks: vec![SinkInstanceConf::file_new(
                "miss_sink".to_string(),
                TextFmt::Raw,
                format!("{}/miss.dat", OUT_FILE_PATH),
                None,
            )],
            parallel: 1,
            flush_interval_ms: default_flush_interval_ms(),
        }
    }

    pub fn residue_ins() -> Self {
        FixedGroup {
            name: "residue".to_string(),
            expect: None,
            sinks: vec![SinkInstanceConf::file_new(
                "residue_sink".to_string(),
                TextFmt::Raw,
                format!("{}/residue.dat", OUT_FILE_PATH),
                None,
            )],
            parallel: 1,
            flush_interval_ms: default_flush_interval_ms(),
        }
    }
    // intercept_ins removed: intercept 组已废弃

    pub fn error_ins() -> Self {
        FixedGroup {
            name: "error".to_string(),
            expect: None,
            sinks: vec![SinkInstanceConf::file_new(
                "err_sink".to_string(),
                TextFmt::Raw,
                format!("{}/error.dat", OUT_FILE_PATH),
                None,
            )],
            parallel: 1,
            flush_interval_ms: default_flush_interval_ms(),
        }
    }
}
impl FlexGroup {
    pub fn monitor_ins() -> Self {
        FlexGroup {
            name: "monitor".to_string(),
            parallel: 1,
            rule: WildArray::default(),
            oml: WildArray::default(),
            tags: Vec::new(),
            filter: None,
            // 生成的 framework.toml 中默认包含 expect 字段，便于后续校验/观测
            expect: Some(GroupExpectSpec {
                basis: Basis::GroupInput,
                window: None,
                min_samples: Some(100),
                mode: ExpectMode::Warn,
                sum_tol: None,
                others_max: None,
            }),
            flush_interval_ms: default_flush_interval_ms(),
            sinks: vec![SinkInstanceConf::file_new(
                "monitor_sink".to_string(),
                TextFmt::ProtoText,
                format!("{}/monitor.dat", OUT_FILE_PATH),
                None,
            )],
        }
    }
}
