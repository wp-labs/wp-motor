//! Workshop 处理结果相关类型

use crate::sinks::SinkPackage;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, fmt::Display};
use wp_connector_api::SourceEvent;
use wp_model_core::model::DataRecord;
use wpl::{PkgID, WparseError};

/// 解析失败信息
#[derive(Debug, Clone)]
pub struct ParseFailInfo {
    /// 匹配度最高的 wpl 规则名称
    pub best_wpl: String,
    /// 解析失败的具体错误
    pub best_error: WparseError,
    /// 解析深度（字符位置）
    pub depth: usize,
    /// 发生时间
    pub timestamp: DateTime<Utc>,
}

impl ParseFailInfo {
    pub fn new(best_wpl: String, best_error: WparseError, depth: usize) -> Self {
        Self {
            best_wpl,
            best_error,
            depth,
            timestamp: Utc::now(),
        }
    }

    /// 格式化错误信息
    pub fn format_error(&self) -> String {
        format!(
            "[{}] target wpl: {} (depth: {})\nError: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
            self.best_wpl,
            self.depth,
            self.best_error
        )
    }
}
impl Display for ParseFailInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] target wpl: {} (depth: {})\nError: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
            self.best_wpl,
            self.depth,
            self.best_error
        )
    }
}

/// 处理结果枚举
#[derive(Debug)]
pub enum ProcessResult {
    /// 完全成功解析
    Success {
        wpl_key: String,
        record: std::sync::Arc<DataRecord>,
    },
    /// 部分成功（有残留数据）
    Partial {
        wpl_key: String,
        record: std::sync::Arc<DataRecord>,
        residue: String,
    },
    /// 完全失败
    Miss(ParseFailInfo),
}

/// 批量处理后的数据结构
pub struct ParsedDatSet {
    pub sink_groups: HashMap<String, SinkPackage>,
    pub residue_data: Vec<(PkgID, String)>,
    pub missed_packets: Vec<(SourceEvent, ParseFailInfo)>,
}
