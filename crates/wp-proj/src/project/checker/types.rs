use serde::Serialize;

/// 单个组件的检查结果单元格
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Cell {
    /// 检查是否通过
    pub ok: bool,
    /// 错误消息（如果检查失败）
    pub msg: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ok: true, // 默认为成功状态，未检查的组件不应显示为错误
            msg: None,
        }
    }
}

impl Cell {
    /// 创建成功的检查结果
    pub fn success() -> Self {
        Self {
            ok: true,
            msg: None,
        }
    }

    /// 创建失败的检查结果
    pub fn failure(msg: String) -> Self {
        Self {
            ok: false,
            msg: Some(msg),
        }
    }

    /// 从 Result 转换
    pub fn from_result(result: Result<(), String>) -> Self {
        match result {
            Ok(_) => Self::success(),
            Err(e) => Self::failure(e),
        }
    }

    /// 创建成功但有消息的检查结果
    pub fn success_with_message(msg: String) -> Self {
        Self {
            ok: true,
            msg: Some(msg),
        }
    }

    /// 创建一个显式跳过的检查结果
    pub fn skipped() -> Self {
        Self::success_with_message("skipped".to_string())
    }
}

/// 单个项目的完整检查结果行
#[derive(Clone, Debug, Default, Serialize)]
pub struct Row {
    /// 项目路径
    pub path: String,
    /// 配置检查结果
    pub conf: Cell,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conf_detail: Option<String>,
    /// 连接器检查结果
    pub connectors: Cell,
    /// 连接器额外统计
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_counts: Option<ConnectorCounts>,
    /// 输入源检查结果
    pub sources: Cell,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_checks: Option<SourceBreakdown>,
    /// 输出接收器检查结果
    pub sinks: Cell,
    /// WPL 检查结果
    pub wpl: Cell,
    /// OML 检查结果
    pub oml: Cell,
}

impl Row {
    /// 创建新的检查结果行
    pub fn new(path: String) -> Self {
        Self {
            path,
            connector_counts: None,
            conf_detail: None,
            source_checks: None,
            ..Default::default()
        }
    }

    /// 获取失败的检查项数量
    #[allow(dead_code)]
    pub fn count_failures(&self) -> usize {
        let mut count = 0;
        if !self.conf.ok {
            count += 1;
        }
        if !self.connectors.ok {
            count += 1;
        }
        if !self.sources.ok {
            count += 1;
        }
        if !self.sinks.ok {
            count += 1;
        }
        if !self.wpl.ok {
            count += 1;
        }
        if !self.oml.ok {
            count += 1;
        }
        count
    }

    /// 获取通过的检查项数量
    #[allow(dead_code)]
    pub fn count_successes(&self) -> usize {
        let mut count = 0;
        if self.conf.ok {
            count += 1;
        }
        if self.connectors.ok {
            count += 1;
        }
        if self.sources.ok {
            count += 1;
        }
        if self.sinks.ok {
            count += 1;
        }
        if self.wpl.ok {
            count += 1;
        }
        if self.oml.ok {
            count += 1;
        }
        count
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ConnectorCounts {
    pub source_defs: usize,
    pub source_refs: usize,
    pub sink_defs: usize,
    pub sink_routes: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceBreakdown {
    pub syntax: Cell,
    pub runtime: Cell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_from_result_reflects_error_state() {
        let ok = Cell::from_result(Ok(()));
        assert!(ok.ok);
        let err = Cell::from_result(Err("boom".into()));
        assert!(!err.ok);
        assert_eq!(err.msg.as_deref(), Some("boom"));
    }

    #[test]
    fn row_counts_failures_and_successes() {
        let mut row = Row::new("/tmp".into());
        row.sources = Cell::failure("bad".into());
        row.oml = Cell::failure("boom".into());
        assert_eq!(row.count_failures(), 2);
        assert_eq!(row.count_successes(), 4);
    }
}
