// Note: StatStage is referenced in docs/tests; avoid importing here to prevent unused warnings

/// 新版统计配置（非兼容重构）：
/// - 顶层 [stat] 仅包含全局窗口设定（window_sec）
/// - 分阶段条目：[[stat.pick]] / [[stat.parse]] / [[stat.sink]]
/// - 字段对齐：name→key，collect→fields，max→top_n，target: "*" 表示全量
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatConf {
    /// 统计窗口（单位秒）；None 表示使用默认（60）
    #[serde(default)]
    pub window_sec: Option<u64>,
    /// 采集阶段（Pick）统计项
    #[serde(default)]
    pub pick: Vec<StatItem>,
    /// 解析阶段（Parse）统计项
    #[serde(default)]
    pub parse: Vec<StatItem>,
    /// 下游阶段（Sink）统计项
    #[serde(default)]
    pub sink: Vec<StatItem>,
}
/*
// [stat]

[[stat.pick]]
key = "pick_stat"
target = "*"

[[stat.parse]]
key = "parse_stat"
target = "*"

[[stat.sink]]
key = "sink_stat"
target = "*"
*/
impl Default for StatConf {
    fn default() -> Self {
        Self {
            window_sec: Default::default(),
            pick: [StatItem {
                key: "pick_stat".into(),
                target: "*".into(),
                fields: Vec::new(),
                top_n: None,
            }]
            .to_vec(),
            parse: [StatItem {
                key: "parse_stat".into(),
                target: "*".into(),
                fields: Vec::new(),
                top_n: None,
            }]
            .to_vec(),
            sink: [StatItem {
                key: "sink_stat".into(),
                target: "*".into(),
                fields: Vec::new(),
                top_n: None,
            }]
            .to_vec(),
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
pub struct StatItem {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatItemBuilder {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub top_n: Option<usize>,
}

impl Default for StatItemBuilder {
    fn default() -> Self {
        Self {
            key: "item".to_string(),
            target: "*".to_string(),
            fields: Vec::new(),
            top_n: Some(10),
        }
    }
}

impl StatItemBuilder {
    pub fn key<S: Into<String>>(mut self, key: S) -> Self {
        self.key = key.into();
        self
    }

    pub fn target<S: Into<String>>(mut self, target: S) -> Self {
        self.target = target.into();
        self
    }

    pub fn fields<S: Into<String>>(mut self, fields: Vec<S>) -> Self {
        self.fields = fields.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn top_n(mut self, top_n: Option<usize>) -> Self {
        self.top_n = top_n;
        self
    }

    pub fn build(self) -> StatItem {
        StatItem {
            key: self.key,
            target: self.target,
            fields: self.fields,
            top_n: self.top_n,
        }
    }
}

impl StatConf {
    pub fn gen_default() -> Self {
        // 提供默认的生成器统计配置
        Self {
            window_sec: Some(60),
            pick: Vec::new(),  // 生成器没有 pick 阶段
            parse: Vec::new(), // 生成器没有 parse 阶段
            sink: vec![StatItem {
                key: "gen_direct".to_string(),
                target: "*".to_string(),
                fields: Vec::new(), // 不收集特定字段，只统计基础指标
                top_n: Some(20),
            }],
        }
    }
}
