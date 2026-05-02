use once_cell::sync::Lazy;
use orion_conf::error::OrionConfResult;
use orion_error::{UvsFrom, conversion::ToStructError};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// 语义功能全局开关（默认关闭）
static SEMANTIC_ENABLED: AtomicBool = AtomicBool::new(false);

/// 设置语义功能开关（由主 crate 在启动时调用）
pub fn set_semantic_enabled(v: bool) {
    SEMANTIC_ENABLED.store(v, Ordering::Relaxed);
}

/// 查询语义功能是否启用
pub fn is_semantic_enabled() -> bool {
    SEMANTIC_ENABLED.load(Ordering::Relaxed)
}

/// 默认外部语义词典路径（按顺序尝试）
const DEFAULT_SEMANTIC_DICT_PATHS: &[&str] = &[
    "models/knowledge/semantic_dict.toml",
    "knowledge/semantic_dict.toml",
];

/// 语义词典路径覆盖（由主引擎按 work_root 注入）
static SEMANTIC_DICT_CONFIG_PATH: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// 设置语义词典配置文件路径（可选）
pub fn set_semantic_dict_config_path(path: Option<PathBuf>) {
    if let Ok(mut guard) = SEMANTIC_DICT_CONFIG_PATH.write() {
        *guard = path;
    }
}

/// 语义词典配置文件版本
const SUPPORTED_VERSION: u32 = 1;

/// 配置合并模式
#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MergeMode {
    /// 添加模式：外部配置添加到内置配置（默认）
    #[default]
    Add,
    /// 替换模式：外部配置完全替换内置配置
    Replace,
}

/// 外部语义词典配置
#[derive(Debug, Deserialize)]
pub struct SemanticDictConf {
    #[serde(default = "default_semantic_dict_enabled", alias = "enable")]
    pub enabled: bool,
    pub version: u32,
    #[serde(default)]
    pub mode: MergeMode,
    #[serde(default)]
    pub stop_words: Option<StopWordsConf>,
    #[serde(default)]
    pub domain_words: Option<DomainWordsConf>,
    #[serde(default)]
    pub status_words: Option<StatusWordsConf>,
    #[serde(default)]
    pub action_verbs: Option<ActionVerbsConf>,
    #[serde(default)]
    pub entity_nouns: Option<EntityNounsConf>,
}

fn default_semantic_dict_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct StopWordsConf {
    #[serde(default)]
    pub chinese: Vec<String>,
    #[serde(default)]
    pub english: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DomainWordsConf {
    #[serde(flatten)]
    pub categories: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct StatusWordsConf {
    #[serde(default)]
    pub english: Vec<String>,
    #[serde(default)]
    pub chinese: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActionVerbsConf {
    #[serde(default)]
    pub english: Vec<String>,
    #[serde(default)]
    pub chinese: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntityNounsConf {
    #[serde(default)]
    pub english: Vec<String>,
    #[serde(default)]
    pub chinese: Vec<String>,
}

/// 加载外部语义词典配置
pub fn load_semantic_dict(config_path: &Path) -> OrionConfResult<SemanticDictConf> {
    let content = fs::read_to_string(config_path).map_err(|e| {
        orion_conf::error::ConfIOReason::from_conf()
            .to_err()
            .with_detail(format!(
                "read semantic dict config '{}' failed",
                config_path.display()
            ))
            .with_source(e)
    })?;

    let conf: SemanticDictConf = toml::from_str(&content).map_err(|e| {
        orion_conf::error::ConfIOReason::from_conf()
            .to_err()
            .with_detail(format!(
                "parse semantic dict config '{}' failed",
                config_path.display()
            ))
            .with_source(e)
    })?;

    if conf.version != SUPPORTED_VERSION {
        return Err(orion_conf::error::ConfIOReason::from_validation()
            .to_err()
            .with_detail(format!(
                "unsupported semantic_dict version: {} (expected {})",
                conf.version, SUPPORTED_VERSION
            ))
            .with_context(config_path));
    }

    Ok(conf)
}

/// 从向量构建 HashSet（使用 Box::leak 转为 'static）
fn build_hashset_from_vec(words: &[String]) -> HashSet<&'static str> {
    words
        .iter()
        .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
        .collect()
}

/// 从字符串切片构建 HashSet
fn build_hashset_from_strs(words: &[&'static str]) -> HashSet<&'static str> {
    words.iter().copied().collect()
}

fn resolve_semantic_dict_path(config_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = config_path {
        return Some(path.to_path_buf());
    }

    if let Ok(guard) = SEMANTIC_DICT_CONFIG_PATH.read()
        && let Some(path) = guard.as_ref()
    {
        return Some(path.clone());
    }

    for rel in DEFAULT_SEMANTIC_DICT_PATHS.iter() {
        let path = PathBuf::from(rel);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// 全局语义词典（使用 Lazy 延迟加载）
pub static SEMANTIC_DICT: Lazy<SemanticDict> = Lazy::new(|| {
    // 创建内置默认词典
    let mut dict = SemanticDict::builtin();

    // 尝试加载外部配置（默认路径或显式设置的路径）
    if let Some(config_path) = resolve_semantic_dict_path(None) {
        match load_semantic_dict(&config_path) {
            Ok(conf) => {
                if conf.enabled {
                    dict.merge(conf);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load external semantic dict config: {}.",
                    e
                );
            }
        }
    }

    dict
});

/// 语义词典运行时结构
#[derive(Debug)]
pub struct SemanticDict {
    /// 核心词性标签（用于 extract_main_word）
    pub core_pos: HashSet<&'static str>,
    /// 停用词
    pub stop_words: HashSet<&'static str>,
    /// 日志领域关键词
    pub domain_words: HashSet<&'static str>,
    /// 状态词
    pub status_words: HashSet<&'static str>,
    /// 动作词
    pub action_verbs: HashSet<&'static str>,
    /// 实体名词
    pub entity_nouns: HashSet<&'static str>,
}

impl SemanticDict {
    /// 创建系统内置词典
    pub fn builtin() -> Self {
        Self {
            // 核心词性（硬编码，不可配置）
            core_pos: build_hashset_from_strs(&[
                "n", "nr", "ns", "nt", "nz", "ng", // 名词类
                "v", "vn", "vd", // 动词类
                "a", "ad", "an", // 形容词类
                "eng", "m", "x", "t", "i", // 英文、数词等
            ]),

            // 停用词
            stop_words: build_hashset_from_strs(&[
                // 中文停用词
                "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个",
                "上", "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好",
                "自己", "这", // 英文停用词
                "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "of", "at",
                "in", "to", "for", "and", "or", "but",
            ]),

            // 日志领域关键词
            domain_words: build_hashset_from_strs(&[
                // 日志级别
                "error",
                "warn",
                "info",
                "debug",
                "fatal",
                "trace",
                // 系统相关
                "exception",
                "failure",
                "timeout",
                "connection",
                "database",
                "server",
                "client",
                "request",
                "response",
                "login",
                "logout",
                "auth",
                "authentication",
                "permission",
                "access",
                // 网络相关
                "http",
                "https",
                "tcp",
                "udp",
                "ip",
                "port",
                "socket",
                // 安全相关
                "attack",
                "virus",
                "malware",
                "threat",
                "alert",
                "blocked",
                "denied",
            ]),

            // 状态词
            status_words: build_hashset_from_strs(&[
                // 英文
                "failed",
                "failure",
                "success",
                "succeeded",
                "timeout",
                "exception",
                "crashed",
                "disconnected",
                "stopped",
                "completed",
                "pending",
                "refused",
                "dropped",
                "rejected",
                "expired",
                "closed",
                // 中文
                "失败",
                "成功",
                "超时",
                "异常",
                "错误",
                "崩溃",
                "断开",
                "拒绝",
                "丢失",
            ]),

            // 动作词
            action_verbs: build_hashset_from_strs(&[
                // 英文
                "connect",
                "login",
                "logout",
                "respond",
                "start",
                "stop",
                "fail",
                "run",
                "process",
                "send",
                "receive",
                "read",
                "write",
                "open",
                "close",
                "bind",
                "listen",
                "authenticate",
                "authorize",
                "create",
                "delete",
                "update",
                "upload",
                "download",
                "retry",
                "handle",
                "load",
                "fetch",
                "parse",
                "resolve",
                "block",
                "deny",
                // 中文
                "连接",
                "登录",
                "登出",
                "请求",
                "响应",
                "启动",
                "停止",
                "处理",
                "发送",
                "接收",
                "读取",
                "写入",
                "认证",
                "访问",
                "创建",
                "删除",
                "更新",
                "下载",
                "上传",
                "重试",
            ]),

            // 实体名词（覆盖词缀规则）
            entity_nouns: build_hashset_from_strs(&[
                // 英文
                "connection",
                "transaction",
                "session",
                "application",
                "configuration",
                "permission",
                "operation",
                "exception",
                // 中文
                "连接",
                "会话",
                "事务",
                "应用",
                "配置",
                "权限",
            ]),
        }
    }

    /// 合并外部配置
    pub fn merge(&mut self, conf: SemanticDictConf) {
        let mode = conf.mode;

        // 处理停用词
        if let Some(stop_words) = conf.stop_words {
            let mut words = stop_words.chinese.clone();
            words.extend(stop_words.english);
            let new_set = build_hashset_from_vec(&words);

            match mode {
                MergeMode::Add => {
                    self.stop_words.extend(new_set);
                }
                MergeMode::Replace => {
                    self.stop_words = new_set;
                }
            }
        }

        // 处理领域词
        if let Some(domain_words) = conf.domain_words {
            let mut words = Vec::new();
            // 遍历所有分类，支持任意自定义分类名
            for (_category, word_list) in domain_words.categories {
                words.extend(word_list);
            }
            let new_set = build_hashset_from_vec(&words);

            match mode {
                MergeMode::Add => {
                    self.domain_words.extend(new_set);
                }
                MergeMode::Replace => {
                    self.domain_words = new_set;
                }
            }
        }

        // 处理状态词
        if let Some(status_words) = conf.status_words {
            let mut words = status_words.english.clone();
            words.extend(status_words.chinese);
            let new_set = build_hashset_from_vec(&words);

            match mode {
                MergeMode::Add => {
                    self.status_words.extend(new_set);
                }
                MergeMode::Replace => {
                    self.status_words = new_set;
                }
            }
        }

        // 处理动作词
        if let Some(action_verbs) = conf.action_verbs {
            let mut words = action_verbs.english.clone();
            words.extend(action_verbs.chinese);
            let new_set = build_hashset_from_vec(&words);

            match mode {
                MergeMode::Add => {
                    self.action_verbs.extend(new_set);
                }
                MergeMode::Replace => {
                    self.action_verbs = new_set;
                }
            }
        }

        // 处理实体名词
        if let Some(entity_nouns) = conf.entity_nouns {
            let mut words = entity_nouns.english.clone();
            words.extend(entity_nouns.chinese);
            let new_set = build_hashset_from_vec(&words);

            match mode {
                MergeMode::Add => {
                    self.entity_nouns.extend(new_set);
                }
                MergeMode::Replace => {
                    self.entity_nouns = new_set;
                }
            }
        }
    }
}

// ========== 公开的 API 方法 ==========

/// 校验词汇列表，返回 (空字符串数量, 重复词数量)
fn validate_word_list(words: &[String]) -> (usize, usize) {
    let mut seen = HashSet::new();
    let mut empties = 0;
    let mut dups = 0;
    for w in words {
        if w.is_empty() {
            empties += 1;
        } else if !seen.insert(w) {
            dups += 1;
        }
    }
    (empties, dups)
}

/// 检查语义词典配置文件是否有效
///
/// 用于 `wproj check` 命令验证配置文件
///
/// # 参数
/// - `config_path`: 配置文件路径（可选），如果为 None 则使用默认路径自动探测
///
/// # 返回
/// - Ok(Some(message)): 配置文件存在且有效，返回成功信息
/// - Ok(None): 未配置外部语义词典（使用默认内置词典）
/// - Err(message): 配置文件无效或加载失败
pub fn check_semantic_dict_config(config_path: Option<&Path>) -> Result<Option<String>, String> {
    check_semantic_dict_config_detailed(config_path).map(|result| result.map(|r| r.message))
}

/// 语义词典配置检查结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticDictCheckResult {
    pub message: String,
    pub warnings: Vec<String>,
}

/// 检查语义词典配置文件是否有效，并返回非阻断性告警。
pub fn check_semantic_dict_config_detailed(
    config_path: Option<&Path>,
) -> Result<Option<SemanticDictCheckResult>, String> {
    // 确定配置文件路径
    let path = resolve_semantic_dict_path(config_path);

    // 如果没有配置，返回 None（使用内置词典）
    let Some(path) = path else {
        return Ok(None);
    };

    // 检查文件是否存在
    if !path.exists() {
        if config_path.is_some() {
            return Err(format!("语义词典配置文件不存在: {}", path.display()));
        }
        return Ok(None);
    }

    // 尝试加载并验证配置
    match load_semantic_dict(&path) {
        Ok(conf) => {
            if !conf.enabled {
                return Ok(None);
            }
            // 统计配置的词汇数量
            let mut total_words = 0;
            if let Some(ref stop_words) = conf.stop_words {
                total_words += stop_words.chinese.len() + stop_words.english.len();
            }
            if let Some(ref domain_words) = conf.domain_words {
                for words in domain_words.categories.values() {
                    total_words += words.len();
                }
            }
            if let Some(ref status_words) = conf.status_words {
                total_words += status_words.english.len() + status_words.chinese.len();
            }
            if let Some(ref action_verbs) = conf.action_verbs {
                total_words += action_verbs.english.len() + action_verbs.chinese.len();
            }
            if let Some(ref entity_nouns) = conf.entity_nouns {
                total_words += entity_nouns.english.len() + entity_nouns.chinese.len();
            }

            // 校验词汇内容：检测空字符串、重复词、空类别。这些是非阻断性告警。
            let mut warnings: Vec<String> = Vec::new();
            if let Some(ref stop_words) = conf.stop_words {
                let (empties, dups) = validate_word_list(&stop_words.chinese);
                if empties > 0 {
                    warnings.push(format!("stop_words.chinese: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("stop_words.chinese: {} duplicate(s)", dups));
                }
                if stop_words.chinese.is_empty() {
                    warnings.push("stop_words.chinese: empty category".into());
                }
                let (empties, dups) = validate_word_list(&stop_words.english);
                if empties > 0 {
                    warnings.push(format!("stop_words.english: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("stop_words.english: {} duplicate(s)", dups));
                }
                if stop_words.english.is_empty() {
                    warnings.push("stop_words.english: empty category".into());
                }
            }
            if let Some(ref domain_words) = conf.domain_words {
                for (cat, words) in &domain_words.categories {
                    let (empties, dups) = validate_word_list(words);
                    if empties > 0 {
                        warnings.push(format!("domain_words.{}: {} empty word(s)", cat, empties));
                    }
                    if dups > 0 {
                        warnings.push(format!("domain_words.{}: {} duplicate(s)", cat, dups));
                    }
                    if words.is_empty() {
                        warnings.push(format!("domain_words.{}: empty category", cat));
                    }
                }
            }
            if let Some(ref status_words) = conf.status_words {
                let (empties, dups) = validate_word_list(&status_words.english);
                if empties > 0 {
                    warnings.push(format!("status_words.english: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("status_words.english: {} duplicate(s)", dups));
                }
                if status_words.english.is_empty() {
                    warnings.push("status_words.english: empty category".into());
                }
                let (empties, dups) = validate_word_list(&status_words.chinese);
                if empties > 0 {
                    warnings.push(format!("status_words.chinese: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("status_words.chinese: {} duplicate(s)", dups));
                }
                if status_words.chinese.is_empty() {
                    warnings.push("status_words.chinese: empty category".into());
                }
            }
            if let Some(ref action_verbs) = conf.action_verbs {
                let (empties, dups) = validate_word_list(&action_verbs.english);
                if empties > 0 {
                    warnings.push(format!("action_verbs.english: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("action_verbs.english: {} duplicate(s)", dups));
                }
                if action_verbs.english.is_empty() {
                    warnings.push("action_verbs.english: empty category".into());
                }
                let (empties, dups) = validate_word_list(&action_verbs.chinese);
                if empties > 0 {
                    warnings.push(format!("action_verbs.chinese: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("action_verbs.chinese: {} duplicate(s)", dups));
                }
                if action_verbs.chinese.is_empty() {
                    warnings.push("action_verbs.chinese: empty category".into());
                }
            }
            if let Some(ref entity_nouns) = conf.entity_nouns {
                let (empties, dups) = validate_word_list(&entity_nouns.english);
                if empties > 0 {
                    warnings.push(format!("entity_nouns.english: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("entity_nouns.english: {} duplicate(s)", dups));
                }
                if entity_nouns.english.is_empty() {
                    warnings.push("entity_nouns.english: empty category".into());
                }
                let (empties, dups) = validate_word_list(&entity_nouns.chinese);
                if empties > 0 {
                    warnings.push(format!("entity_nouns.chinese: {} empty word(s)", empties));
                }
                if dups > 0 {
                    warnings.push(format!("entity_nouns.chinese: {} duplicate(s)", dups));
                }
                if entity_nouns.chinese.is_empty() {
                    warnings.push("entity_nouns.chinese: empty category".into());
                }
            }

            let mode_str = match conf.mode {
                MergeMode::Add => "ADD（扩展内置词典）",
                MergeMode::Replace => "REPLACE（替换内置词典）",
            };

            Ok(Some(SemanticDictCheckResult {
                message: format!(
                    "语义词典配置有效: {} | 模式: {} | 词汇数: {}",
                    path.display(),
                    mode_str,
                    total_words
                ),
                warnings,
            }))
        }
        Err(e) => Err(format!("语义词典配置加载失败: {}", e)),
    }
}

/// 初始化语义词典，加载外部配置（如果存在）
///
/// 用于引擎启动时预加载语义词典
///
/// 此函数会触发 SEMANTIC_DICT 的延迟初始化，并返回加载结果信息
///
/// # 返回
/// - Ok(message): 加载成功的信息
/// - Err(message): 加载失败（但会回退到内置词典）
pub fn init_semantic_dict() -> Result<String, String> {
    // 触发 SEMANTIC_DICT 的延迟初始化
    let dict = &*SEMANTIC_DICT;

    // 检查是否有外部配置（默认路径或显式设置路径）
    if let Some(config_path) = resolve_semantic_dict_path(None) {
        if !config_path.exists() {
            return Ok(format!(
                "语义词典已加载 | 使用内置词典 | 词汇数: {} 个领域词, {} 个停用词",
                dict.domain_words.len(),
                dict.stop_words.len()
            ));
        }
        match load_semantic_dict(&config_path) {
            Ok(conf) => {
                if conf.enabled {
                    Ok(format!(
                        "语义词典已加载 | 配置: {} | 词汇数: {} 个领域词, {} 个停用词",
                        config_path.display(),
                        dict.domain_words.len(),
                        dict.stop_words.len()
                    ))
                } else {
                    Ok(format!(
                        "语义词典已加载 | 外部词典已禁用: {} | 使用内置词典 | 词汇数: {} 个领域词, {} 个停用词",
                        config_path.display(),
                        dict.domain_words.len(),
                        dict.stop_words.len()
                    ))
                }
            }
            Err(e) => Err(format!("语义词典配置加载失败: {}", e)),
        }
    } else {
        Ok(format!(
            "语义词典已加载 | 使用内置词典 | 词汇数: {} 个领域词, {} 个停用词",
            dict.domain_words.len(),
            dict.stop_words.len()
        ))
    }
}

/// 生成默认的语义词典配置文件示例
///
/// 用于 `wproj init` 命令创建配置文件模板
///
/// # 返回
/// 配置文件的 TOML 内容字符串
pub fn generate_default_semantic_dict_config() -> String {
    r#"# 语义词典外部配置（知识配置）
# 文件位置：models/knowledge/semantic_dict.toml
# 用于扩展或替换系统内置的语义词典

version = 1

# 外部词典开关（可选，默认 true）
enabled = true

# 配置模式：
#   - "add"：将下面的词汇添加到系统内置词典（默认，推荐）
#   - "replace"：用下面的词汇完全替换系统内置词典
mode = "add"

# 停用词（可选）
# [stop_words]
# chinese = ["额外停用词"]
# english = ["custom_stop_word"]

# 日志领域关键词（可选）
# 注意：支持任意自定义分类名，不局限于下面的例子
[domain_words]
# 常用通用分类示例
# log_level = ["critical"]
# system = ["cache", "queue"]
# network = ["websocket", "grpc"]
# security = ["firewall", "encryption"]

# 自定义分类示例（可根据业务需求自由定义）
# database = ["mysql", "postgres", "mongodb", "redis"]
# cloud = ["kubernetes", "docker", "pod"]
# middleware = ["kafka", "rabbitmq", "elasticsearch"]
# business = ["order", "payment", "product"]

# 状态词（可选）
# [status_words]
# english = ["processing", "queued"]
# chinese = ["处理中", "队列中"]

# 动作词（可选）
# [action_verbs]
# english = ["deploy", "rollback"]
# chinese = ["部署", "回滚"]

# 实体名词（可选）
# [entity_nouns]
# english = ["migration", "notification"]
# chinese = ["迁移任务", "通知"]
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_builtin_dict() {
        let dict = SemanticDict::builtin();

        // 测试核心词性
        assert!(dict.core_pos.contains("n"));
        assert!(dict.core_pos.contains("v"));
        assert!(dict.core_pos.contains("eng"));

        // 测试停用词
        assert!(dict.stop_words.contains("的"));
        assert!(dict.stop_words.contains("the"));

        // 测试领域词
        assert!(dict.domain_words.contains("error"));
        assert!(dict.domain_words.contains("database"));

        // 测试状态词
        assert!(dict.status_words.contains("failed"));
        assert!(dict.status_words.contains("失败"));

        // 测试动作词
        assert!(dict.action_verbs.contains("connect"));
        assert!(dict.action_verbs.contains("连接"));

        // 测试实体名词
        assert!(dict.entity_nouns.contains("connection"));
        assert!(dict.entity_nouns.contains("会话"));
    }

    #[test]
    fn test_merge_add_mode() {
        let mut dict = SemanticDict::builtin();
        let original_count = dict.status_words.len();

        let conf = SemanticDictConf {
            enabled: true,
            version: 1,
            mode: MergeMode::Add,
            status_words: Some(StatusWordsConf {
                english: vec!["custom_status".to_string()],
                chinese: vec!["自定义状态".to_string()],
            }),
            stop_words: None,
            domain_words: None,
            action_verbs: None,
            entity_nouns: None,
        };

        dict.merge(conf);

        // ADD 模式：新增词汇
        assert!(dict.status_words.contains("custom_status"));
        assert!(dict.status_words.contains("自定义状态"));
        // 原有词汇仍然存在
        assert!(dict.status_words.contains("failed"));
        assert!(dict.status_words.len() > original_count);
    }

    #[test]
    fn test_merge_replace_mode() {
        let mut dict = SemanticDict::builtin();

        let conf = SemanticDictConf {
            enabled: true,
            version: 1,
            mode: MergeMode::Replace,
            status_words: Some(StatusWordsConf {
                english: vec!["only_this".to_string()],
                chinese: vec![],
            }),
            stop_words: None,
            domain_words: None,
            action_verbs: None,
            entity_nouns: None,
        };

        dict.merge(conf);

        // REPLACE 模式：只有新词汇
        assert!(dict.status_words.contains("only_this"));
        // 原有词汇被替换
        assert!(!dict.status_words.contains("failed"));
        assert_eq!(dict.status_words.len(), 1);
    }

    #[test]
    fn test_load_external_config() {
        let config_content = r#"
version = 1
mode = "add"

[status_words]
english = ["aborted", "cancelled"]
chinese = ["中止", "取消"]

[action_verbs]
english = ["deploy"]
chinese = ["部署"]
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let conf = load_semantic_dict(temp_file.path()).unwrap();
        assert!(conf.enabled);
        assert_eq!(conf.version, 1);
        assert_eq!(conf.mode, MergeMode::Add);

        let status = conf.status_words.unwrap();
        assert_eq!(status.english, vec!["aborted", "cancelled"]);
        assert_eq!(status.chinese, vec!["中止", "取消"]);
    }

    #[test]
    fn test_global_semantic_dict() {
        // 测试全局词典可以访问
        assert!(!SEMANTIC_DICT.core_pos.is_empty());
        assert!(!SEMANTIC_DICT.stop_words.is_empty());
        assert!(!SEMANTIC_DICT.domain_words.is_empty());
    }

    #[test]
    fn test_load_external_config_can_disable() {
        let config_content = r#"
version = 1
enabled = false
mode = "add"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let conf = load_semantic_dict(temp_file.path()).unwrap();
        assert!(!conf.enabled);
    }

    #[test]
    fn test_load_external_config_accepts_legacy_enable_key() {
        let config_content = r#"
version = 1
enable = false
mode = "add"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let conf = load_semantic_dict(temp_file.path()).unwrap();
        assert!(!conf.enabled);
    }

    #[test]
    fn test_check_config_returns_none_when_disabled() {
        let config_content = r#"
version = 1
enabled = false
mode = "add"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let result = check_semantic_dict_config(Some(temp_file.path())).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_config_returns_structured_warnings() {
        let config_content = r#"
version = 1
mode = "add"

[stop_words]
english = ["the", "", "the"]
chinese = []
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let detailed = check_semantic_dict_config_detailed(Some(temp_file.path()))
            .unwrap()
            .unwrap();
        assert!(detailed.message.contains("语义词典配置有效"));
        assert!(!detailed.message.contains("问题"));
        assert!(detailed.warnings.iter().any(|w| w.contains("empty word")));
        assert!(detailed.warnings.iter().any(|w| w.contains("duplicate")));
        assert!(
            detailed
                .warnings
                .iter()
                .any(|w| w == "stop_words.chinese: empty category")
        );

        let legacy = check_semantic_dict_config(Some(temp_file.path()))
            .unwrap()
            .unwrap();
        assert_eq!(legacy, detailed.message);
    }

    #[test]
    fn test_flexible_domain_categories() {
        // 测试灵活的 domain_words 分类
        let config_content = r#"
version = 1
mode = "add"

[domain_words]
# 通用分类
system = ["cache", "queue"]
network = ["websocket", "grpc"]

# 自定义分类 - 数据库
database = ["mysql", "postgres", "mongodb", "redis"]

# 自定义分类 - 云原生
cloud = ["kubernetes", "docker", "pod"]

# 自定义分类 - 业务领域
business = ["order", "payment", "product"]
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let conf = load_semantic_dict(temp_file.path()).unwrap();
        assert_eq!(conf.version, 1);
        assert_eq!(conf.mode, MergeMode::Add);

        // 验证所有分类都被正确加载
        let domain = conf.domain_words.as_ref().unwrap();
        assert!(domain.categories.contains_key("system"));
        assert!(domain.categories.contains_key("network"));
        assert!(domain.categories.contains_key("database"));
        assert!(domain.categories.contains_key("cloud"));
        assert!(domain.categories.contains_key("business"));

        // 验证具体的词汇
        assert_eq!(
            domain.categories.get("database").unwrap(),
            &vec!["mysql", "postgres", "mongodb", "redis"]
        );
        assert_eq!(
            domain.categories.get("cloud").unwrap(),
            &vec!["kubernetes", "docker", "pod"]
        );

        // 测试合并到词典
        let mut dict = SemanticDict::builtin();
        let original_count = dict.domain_words.len();

        dict.merge(conf);

        // 验证所有自定义分类的词汇都被添加
        assert!(dict.domain_words.contains("mysql"));
        assert!(dict.domain_words.contains("postgres"));
        assert!(dict.domain_words.contains("kubernetes"));
        assert!(dict.domain_words.contains("docker"));
        assert!(dict.domain_words.contains("order"));
        assert!(dict.domain_words.contains("payment"));

        // 验证原有词汇仍然存在（ADD 模式）
        assert!(dict.domain_words.contains("error"));
        assert!(dict.domain_words.contains("database"));

        // 验证词典大小增加
        assert!(dict.domain_words.len() > original_count);
    }

    #[test]
    fn test_replace_mode_with_custom_categories() {
        // 测试 REPLACE 模式下的自定义分类
        let config_content = r#"
version = 1
mode = "replace"

[domain_words]
# Kubernetes 专用分类
k8s_resources = ["pod", "deployment", "service"]
k8s_network = ["ingress", "endpoint"]
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let conf = load_semantic_dict(temp_file.path()).unwrap();
        let mut dict = SemanticDict::builtin();

        dict.merge(conf);

        // 验证只有配置的词汇（REPLACE 模式）
        assert!(dict.domain_words.contains("pod"));
        assert!(dict.domain_words.contains("deployment"));
        assert!(dict.domain_words.contains("ingress"));

        // 验证原有词汇被替换
        assert!(!dict.domain_words.contains("error"));
        assert!(!dict.domain_words.contains("timeout"));

        // 验证词典大小等于配置的词汇数量
        assert_eq!(dict.domain_words.len(), 5); // pod, deployment, service, ingress, endpoint
    }
}
