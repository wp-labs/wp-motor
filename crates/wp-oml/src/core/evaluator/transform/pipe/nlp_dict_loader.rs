use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// NLP 词典配置文件版本
const SUPPORTED_VERSION: u32 = 1;

/// NLP 词典配置
#[derive(Debug, Deserialize)]
pub struct NlpDictConf {
    pub version: u32,
    pub core_pos: CorePosConf,
    pub stop_words: StopWordsConf,
    pub domain_words: DomainWordsConf,
    pub status_words: StatusWordsConf,
    pub action_verbs: ActionVerbsConf,
    pub entity_nouns: EntityNounsConf,
    #[serde(default)]
    #[allow(dead_code)]  // 保留用于其他功能扩展
    pub field_mapping: FieldMappingConf,
}

#[derive(Debug, Deserialize)]
pub struct CorePosConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StopWordsConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub chinese: Vec<String>,
    pub english: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DomainWordsConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub log_level: Vec<String>,
    pub system: Vec<String>,
    pub network: Vec<String>,
    pub security: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatusWordsConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub english: Vec<String>,
    pub chinese: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActionVerbsConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub english: Vec<String>,
    pub chinese: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntityNounsConf {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub english: Vec<String>,
    pub chinese: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FieldMappingConf {
    #[serde(default)]
    #[allow(dead_code)]  // 保留用于其他功能扩展
    pub enabled: bool,
    #[serde(default)]
    #[allow(dead_code)]  // 保留用于其他功能扩展
    pub mapping: Vec<(String, String)>,
}

const fn default_true() -> bool {
    true
}

/// 加载 NLP 词典配置
pub fn load_nlp_dict(config_path: &Path) -> Result<NlpDictConf, String> {
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read nlp_dict.toml: {}", e))?;

    let conf: NlpDictConf = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse nlp_dict.toml: {}", e))?;

    if conf.version != SUPPORTED_VERSION {
        return Err(format!(
            "Unsupported nlp_dict version: {}. Expected: {}",
            conf.version, SUPPORTED_VERSION
        ));
    }

    Ok(conf)
}

/// 从配置构建 HashSet
pub fn build_hashset_from_vec(words: &[String]) -> HashSet<&'static str> {
    words
        .iter()
        .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
        .collect()
}

/// 全局 NLP 词典（使用 Lazy 延迟加载）
pub static NLP_DICT: Lazy<NlpDict> = Lazy::new(|| {
    // 默认配置文件路径
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("nlp_dict")
        .join("nlp_dict.toml");

    // 优先从环境变量读取配置路径
    let config_path = std::env::var("NLP_DICT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or(default_path);

    match load_nlp_dict(&config_path) {
        Ok(conf) => NlpDict::from_conf(conf),
        Err(e) => {
            eprintln!("Warning: Failed to load NLP dict config: {}. Using empty dict.", e);
            NlpDict::empty()
        }
    }
});

/// NLP 词典运行时结构
#[derive(Debug)]
pub struct NlpDict {
    pub core_pos: HashSet<&'static str>,
    pub stop_words: HashSet<&'static str>,
    pub domain_words: HashSet<&'static str>,
    pub status_words: HashSet<&'static str>,
    pub action_verbs: HashSet<&'static str>,
    pub entity_nouns: HashSet<&'static str>,
}

impl NlpDict {
    /// 从配置构建词典
    pub fn from_conf(conf: NlpDictConf) -> Self {
        let mut dict = Self::empty();

        // 加载 core_pos
        if conf.core_pos.enabled {
            dict.core_pos = build_hashset_from_vec(&conf.core_pos.tags);
        }

        // 加载 stop_words
        if conf.stop_words.enabled {
            let mut combined = conf.stop_words.chinese.clone();
            combined.extend(conf.stop_words.english);
            dict.stop_words = build_hashset_from_vec(&combined);
        }

        // 加载 domain_words
        if conf.domain_words.enabled {
            let mut combined = conf.domain_words.log_level.clone();
            combined.extend(conf.domain_words.system);
            combined.extend(conf.domain_words.network);
            combined.extend(conf.domain_words.security);
            dict.domain_words = build_hashset_from_vec(&combined);
        }

        // 加载 status_words
        if conf.status_words.enabled {
            let mut combined = conf.status_words.english.clone();
            combined.extend(conf.status_words.chinese);
            dict.status_words = build_hashset_from_vec(&combined);
        }

        // 加载 action_verbs
        if conf.action_verbs.enabled {
            let mut combined = conf.action_verbs.english.clone();
            combined.extend(conf.action_verbs.chinese);
            dict.action_verbs = build_hashset_from_vec(&combined);
        }

        // 加载 entity_nouns
        if conf.entity_nouns.enabled {
            let mut combined = conf.entity_nouns.english.clone();
            combined.extend(conf.entity_nouns.chinese);
            dict.entity_nouns = build_hashset_from_vec(&combined);
        }

        dict
    }

    /// 创建空词典
    pub fn empty() -> Self {
        Self {
            core_pos: HashSet::new(),
            stop_words: HashSet::new(),
            domain_words: HashSet::new(),
            status_words: HashSet::new(),
            action_verbs: HashSet::new(),
            entity_nouns: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_config() {
        let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("nlp_dict")
            .join("nlp_dict.toml");

        let conf = load_nlp_dict(&default_path).expect("Failed to load default config");
        assert_eq!(conf.version, 1);
        assert!(conf.core_pos.enabled);
        assert!(conf.stop_words.enabled);
        assert!(conf.domain_words.enabled);
    }

    #[test]
    fn test_build_nlp_dict() {
        let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("nlp_dict")
            .join("nlp_dict.toml");

        let conf = load_nlp_dict(&default_path).expect("Failed to load config");
        let dict = NlpDict::from_conf(conf);

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
    fn test_global_nlp_dict() {
        // 测试全局词典可以访问
        assert!(!NLP_DICT.core_pos.is_empty());
        assert!(!NLP_DICT.stop_words.is_empty());
        assert!(!NLP_DICT.domain_words.is_empty());
    }
}
