use crate::core::prelude::*;
use crate::language::{ExtractMainWord, ExtractSubjectObject};
use jieba_rs::Jieba;
use lazy_static::lazy_static;
use serde_json::json;
use std::collections::HashMap;
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, Value};

// 导入 NLP 词典
use super::nlp_dict_loader::NLP_DICT;

lazy_static! {
    // Jieba 中文分词器实例（全局单例）
    static ref JIEBA: Jieba = Jieba::new();

    // 中英文字段映射表（保留用于其他功能扩展）
    #[allow(dead_code)]
    static ref FIELD_MAPPING: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("url路径", "urlPath");
        m.insert("状态码", "statusCode");
        m.insert("用户名", "username");
        m.insert("密码", "password");
        m.insert("请求体", "requestBody");
        m.insert("请求头", "requestHeaders");
        m.insert("响应头", "responseHeaders");
        m.insert("响应体", "responseBody");
        m.insert("解密账号", "decryptedAccount");
        m.insert("解密密码", "decryptedPassword");
        m.insert("病毒名", "virusName");
        m.insert("文件路径", "filePath");
        m.insert("文件大小", "fileSize");
        m.insert("文件创建时间", "fileCreateTime");
        m.insert("文件MD5", "fileMd5");
        m.insert("referer路径", "refererPath");
        m.insert("描述", "describe");
        m.insert("描述信息", "describeInfo");
        m.insert("检测的引擎", "engine");
        m
    };
}

/// 词角色分类
enum WordRole {
    /// 实体（subject/object）
    Entity,
    /// 动作词
    Action,
    /// 状态词
    Status,
}

/// Debug信息收集器
struct DebugInfo {
    tokens: Vec<String>,
    pos_tags: Vec<(String, String)>,
    subject_rule: String,
    action_rule: String,
    object_rule: String,
    status_rule: String,
    subject_confidence: f32,
    action_confidence: f32,
    object_confidence: f32,
    status_confidence: f32,
}

impl Default for DebugInfo {
    fn default() -> Self {
        Self {
            tokens: Vec::new(),
            pos_tags: Vec::new(),
            subject_rule: String::new(),
            action_rule: String::new(),
            object_rule: String::new(),
            status_rule: String::new(),
            subject_confidence: 0.0,
            action_confidence: 0.0,
            object_confidence: 0.0,
            status_confidence: 0.0,
        }
    }
}

impl DebugInfo {
    fn to_json(&self) -> String {
        json!({
            "tokenization": self.tokens,
            "pos_tags": self.pos_tags,
            "rules_matched": {
                "subject": self.subject_rule,
                "action": self.action_rule,
                "object": self.object_rule,
                "status": self.status_rule
            },
            "confidence": {
                "subject": self.subject_confidence,
                "action": self.action_confidence,
                "object": self.object_confidence,
                "status": self.status_confidence
            }
        })
        .to_string()
    }
}

/// 英文词角色判断
fn classify_eng(word: &str) -> WordRole {
    let lower = word.to_lowercase();

    // 优先级1：领域词典明确匹配
    if NLP_DICT.status_words.contains(lower.as_str()) {
        return WordRole::Status;
    }
    if NLP_DICT.action_verbs.contains(lower.as_str()) {
        return WordRole::Action;
    }

    // 优先级2：实体名词白名单（覆盖词缀规则）
    if NLP_DICT.entity_nouns.contains(lower.as_str()) {
        return WordRole::Entity;
    }

    // 优先级3：词缀规则（动态识别）
    // "-ing" 结尾 → 动作（进行时）
    if lower.ends_with("ing") && lower.len() > 4 {
        return WordRole::Action;
    }
    // "-ed" 结尾 → 动作（过去式/完成时）
    if lower.ends_with("ed") && lower.len() > 3 {
        return WordRole::Action;
    }
    // "-tion"/"-sion" 结尾 → 动作（名词化动词：authentication, connection）
    if (lower.ends_with("tion") || lower.ends_with("sion")) && lower.len() > 5 {
        return WordRole::Action;
    }

    // 默认：实体
    WordRole::Entity
}

/// 中文词角色判断（根据词性）
fn classify_cn(pos: &str, word: &str) -> Option<WordRole> {
    let lower = word.to_lowercase();
    if NLP_DICT.status_words.contains(lower.as_str()) {
        return Some(WordRole::Status);
    }
    if NLP_DICT.action_verbs.contains(lower.as_str()) {
        return Some(WordRole::Action);
    }
    match pos {
        "v" | "vn" | "vd" => Some(WordRole::Action),
        "n" | "nr" | "ns" | "nt" | "nz" | "ng" => Some(WordRole::Entity),
        _ => {
            if NLP_DICT.domain_words.contains(lower.as_str()) {
                Some(WordRole::Entity)
            } else {
                None // 停用词/虚词等，不参与分配
            }
        }
    }
}

/// 日志主客体分析（带debug信息）
///
/// 对日志文本进行分词+词性标注，将词按角色分配到：
/// - subject：主体（第一个实体词，或 action 之前的实体）
/// - action：动作词（动词）
/// - object：对象（action 之后的第一个实体词）
/// - status：状态词（终态标记）
fn analyze_subject_object_with_debug(
    text: &str,
    enable_debug: bool,
) -> (String, String, String, String, Option<DebugInfo>) {
    let tags = JIEBA.tag(text, true);

    let mut subject = String::new();
    let mut action = String::new();
    let mut object = String::new();
    let mut status = String::new();
    let mut action_seen = false;

    let mut debug = if enable_debug {
        Some(DebugInfo::default())
    } else {
        None
    };

    // 收集debug信息：分词和词性
    if let Some(ref mut d) = debug {
        for tag in &tags {
            d.tokens.push(tag.word.to_string());
            d.pos_tags
                .push((tag.word.to_string(), tag.tag.to_string()));
        }
    }

    for tag in &tags {
        let word = tag.word.trim();
        if word.is_empty() {
            continue;
        }
        let word_lower = word.to_lowercase();
        if NLP_DICT.stop_words.contains(word_lower.as_str()) {
            continue;
        }

        let pos = tag.tag;
        let role = if pos == "eng" {
            Some(classify_eng(word))
        } else {
            classify_cn(pos, word)
        };

        if let Some(role) = role {
            match role {
                WordRole::Status => {
                    if status.is_empty() {
                        status = word.to_string();
                        if let Some(ref mut d) = debug {
                            d.status_rule = if NLP_DICT.status_words.contains(word_lower.as_str()) {
                                "rule1: status_word_match".to_string()
                            } else {
                                "rule2: cn_pos_match".to_string()
                            };
                            d.status_confidence = if NLP_DICT.status_words.contains(word_lower.as_str()) {
                                1.0
                            } else {
                                0.7
                            };
                        }
                    }
                }
                WordRole::Action => {
                    if action.is_empty() {
                        action = word.to_string();
                        action_seen = true;
                        if let Some(ref mut d) = debug {
                            d.action_rule = if NLP_DICT.action_verbs.contains(word_lower.as_str()) {
                                "rule1: action_verb_match".to_string()
                            } else if pos == "eng" && word_lower.ends_with("ing") {
                                "rule2: eng_ing_suffix".to_string()
                            } else if pos == "eng" && word_lower.ends_with("ed") {
                                "rule2: eng_ed_suffix".to_string()
                            } else {
                                format!("rule3: cn_pos({})", pos)
                            };
                            d.action_confidence = if NLP_DICT.action_verbs.contains(word_lower.as_str()) {
                                1.0
                            } else {
                                0.7
                            };
                        }
                    }
                }
                WordRole::Entity => {
                    if subject.is_empty() {
                        subject = word.to_string();
                        if let Some(ref mut d) = debug {
                            d.subject_rule = if NLP_DICT.domain_words.contains(word_lower.as_str()) {
                                "rule1: domain_entity_match".to_string()
                            } else {
                                format!("rule2: core_pos({}) + non_stopword", pos)
                            };
                            d.subject_confidence = if NLP_DICT.domain_words.contains(word_lower.as_str()) {
                                1.0
                            } else {
                                0.8
                            };
                        }
                    } else if action_seen && object.is_empty() {
                        object = word.to_string();
                        if let Some(ref mut d) = debug {
                            d.object_rule = if NLP_DICT.domain_words.contains(word_lower.as_str()) {
                                "rule1: domain_entity_match (after_action)".to_string()
                            } else {
                                format!("rule2: core_pos({}) + after_action", pos)
                            };
                            d.object_confidence = if NLP_DICT.domain_words.contains(word_lower.as_str()) {
                                1.0
                            } else {
                                0.8
                            };
                        }
                    }
                }
            }
        }
    }

    (subject, action, object, status, debug)
}

/// 提取日志主客体结构 - extract_subject_object
///
/// 输入一段日志文本，输出一个包含四个字段的对象：
/// - subject：主体（谁/什么）
/// - action：动作（做什么）
/// - object：对象（作用于谁/什么）
/// - status：状态（结果如何）
/// - debug：调试信息（仅在debug模式下）
impl ValueProcessor for ExtractSubjectObject {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                let cleaned = x.trim();
                if cleaned.is_empty() {
                    return DataField::from_obj(
                        in_val.get_name().to_string(),
                        ObjectValue::default(),
                    );
                }

                let (subject, action, object, status, debug) =
                    analyze_subject_object_with_debug(cleaned, self.debug);

                let mut obj = ObjectValue::default();
                obj.insert(
                    "subject".to_string(),
                    DataField::from_chars("subject", subject),
                );
                obj.insert(
                    "action".to_string(),
                    DataField::from_chars("action", action),
                );
                obj.insert(
                    "object".to_string(),
                    DataField::from_chars("object", object),
                );
                obj.insert(
                    "status".to_string(),
                    DataField::from_chars("status", status),
                );

                // 如果启用debug，添加debug字段
                if let Some(d) = debug {
                    obj.insert(
                        "debug".to_string(),
                        DataField::from_chars("debug", d.to_json()),
                    );
                }

                DataField::from_obj(in_val.get_name().to_string(), obj)
            }
            _ => in_val,
        }
    }
}

/// 提取主要词（核心词）- extract_main_word
///
/// 使用 jieba-rs 进行中文分词 + 词性标注，智能提取文本中的第一个核心词。
///
/// 提取规则（按优先级）：
/// 1. 日志领域关键词（error, timeout, database 等）
/// 2. 核心词性（名词、动词、形容词等）+ 非停用词
/// 3. 回退：第一个非空分词
impl ValueProcessor for ExtractMainWord {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                // 步骤1：清洗文本（去除首尾空格）
                let cleaned_log = x.trim();

                if cleaned_log.is_empty() {
                    return DataField::from_chars(in_val.get_name().to_string(), String::new());
                }

                // 步骤2：jieba-rs 核心工作：分词+词性标注（使用HMM模式获得更细粒度的分词）
                let tags = JIEBA.tag(cleaned_log, true);

                // 步骤3：定制规则筛选，返回第一个核心词
                for tag in &tags {
                    let word = tag.word;
                    let pos = tag.tag;

                    // 跳过空白字符
                    if word.trim().is_empty() {
                        continue;
                    }

                    let word_lower = word.to_lowercase();

                    // 规则1：日志领域词（优先级最高，直接返回）
                    if NLP_DICT.domain_words.contains(word_lower.as_str()) {
                        return DataField::from_chars(
                            in_val.get_name().to_string(),
                            word.to_string(),
                        );
                    }

                    // 规则2：核心词性 + 非停用词
                    if NLP_DICT.core_pos.contains(pos) && !NLP_DICT.stop_words.contains(word_lower.as_str()) {
                        return DataField::from_chars(
                            in_val.get_name().to_string(),
                            word.to_string(),
                        );
                    }
                }
                DataField::from_chars(in_val.get_name().to_string(), String::new())
            }
            _ => in_val,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, Value};

    #[test]
    fn test_extract_main_word() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            // 英文测试
            DataField::from_chars("A1", "hello world test"),
            DataField::from_chars("A2", "  single  "),
            DataField::from_chars("A3", ""),
            // 中文测试
            DataField::from_chars("B1", "我们中出了一个叛徒"),
            DataField::from_chars("B2", "中文分词测试"),
            DataField::from_chars("B3", "今天天气很好"),
            // 日志测试
            DataField::from_chars("C1", "error: connection timeout"),
            DataField::from_chars("C2", "database connection failed"),
            DataField::from_chars("C3", "用户登录失败异常"),
            // 混合测试
            DataField::from_chars("D1", "HTTP请求超时"),
            DataField::from_chars("D2", "的是在了不"), // 全停用词
        ];

        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X1  =  pipe read(A1) | extract_main_word ;
        X2  =  pipe read(A2) | extract_main_word ;
        X3  =  pipe read(A3) | extract_main_word ;
        Y1  =  pipe read(B1) | extract_main_word ;
        Y2  =  pipe read(B2) | extract_main_word ;
        Y3  =  pipe read(B3) | extract_main_word ;
        Z1  =  pipe read(C1) | extract_main_word ;
        Z2  =  pipe read(C2) | extract_main_word ;
        Z3  =  pipe read(C3) | extract_main_word ;
        W1  =  pipe read(D1) | extract_main_word ;
        W2  =  pipe read(D2) | extract_main_word ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        // 英文：提取第一个非停用词
        let x1 = target.field("X1").unwrap();
        if let Value::Chars(s) = x1.get_value() {
            assert_eq!(s.as_str(), "hello");
        } else {
            panic!("Expected Chars value");
        }

        let x2 = target.field("X2").unwrap();
        if let Value::Chars(s) = x2.get_value() {
            assert_eq!(s.as_str(), "single");
        } else {
            panic!("Expected Chars value");
        }

        let x3 = target.field("X3").unwrap();
        if let Value::Chars(s) = x3.get_value() {
            assert_eq!(s.as_str(), "");
        } else {
            panic!("Expected Chars value");
        }

        // 中文：提取第一个核心词（名词、动词等）
        let y1 = target.field("Y1").unwrap();
        if let Value::Chars(s) = y1.get_value() {
            println!("Y1: {}", s);
            // "我们中出了一个叛徒" 应该提取核心词
            assert!(!s.is_empty());
        }

        let y2 = target.field("Y2").unwrap();
        if let Value::Chars(s) = y2.get_value() {
            println!("Y2: {}", s);
            // "中文分词测试" 应该提取核心词
            assert!(!s.is_empty());
        }

        let y3 = target.field("Y3").unwrap();
        if let Value::Chars(s) = y3.get_value() {
            println!("Y3: {}", s);
            // "今天天气很好" 应该提取核心词
            assert!(!s.is_empty());
        }

        // 日志：优先提取领域关键词
        let z1 = target.field("Z1").unwrap();
        if let Value::Chars(s) = z1.get_value() {
            println!("Z1: {}", s);
            // "error: connection timeout" 应该提取领域关键词
            assert!(s.as_str() == "error" || s.as_str() == "connection" || s.as_str() == "timeout");
        }

        let z2 = target.field("Z2").unwrap();
        if let Value::Chars(s) = z2.get_value() {
            println!("Z2: {}", s);
            // "database connection failed" 应该提取领域关键词
            assert!(
                s.as_str() == "database" || s.as_str() == "connection" || s.as_str() == "failed"
            );
        }

        let z3 = target.field("Z3").unwrap();
        if let Value::Chars(s) = z3.get_value() {
            println!("Z3: {}", s);
            // "用户登录失败异常" 应该提取核心词
            assert!(!s.is_empty());
        }

        // 混合
        let w1 = target.field("W1").unwrap();
        if let Value::Chars(s) = w1.get_value() {
            println!("W1: {}", s);
            // "HTTP请求超时" 应该提取核心词
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn test_extract_main_word_english() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            // 英文句子测试
            DataField::from_chars("E1", "User authentication failed"),
            DataField::from_chars("E2", "The server is running"),
            DataField::from_chars("E3", "Failed to connect database"),
            DataField::from_chars("E4", "Request processing timeout occurred"),
            // 数字和特殊字符
            DataField::from_chars("E5", "Port 8080 is already in use"),
            DataField::from_chars("E6", "API call returned 404"),
            // 技术术语
            DataField::from_chars("E7", "NullPointerException thrown"),
            DataField::from_chars("E8", "Redis cache miss"),
            // 只有停用词
            DataField::from_chars("E9", "the a an is"),
            // 动词开头
            DataField::from_chars("E10", "Starting application server"),
            DataField::from_chars("E11", "Connecting to remote host"),
        ];

        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test_english
        ---
        R1  =  pipe read(E1) | extract_main_word ;
        R2  =  pipe read(E2) | extract_main_word ;
        R3  =  pipe read(E3) | extract_main_word ;
        R4  =  pipe read(E4) | extract_main_word ;
        R5  =  pipe read(E5) | extract_main_word ;
        R6  =  pipe read(E6) | extract_main_word ;
        R7  =  pipe read(E7) | extract_main_word ;
        R8  =  pipe read(E8) | extract_main_word ;
        R9  =  pipe read(E9) | extract_main_word ;
        R10 =  pipe read(E10) | extract_main_word ;
        R11 =  pipe read(E11) | extract_main_word ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        // 验证提取结果
        let r1 = target.field("R1").unwrap();
        if let Value::Chars(s) = r1.get_value() {
            println!("R1: {}", s);
            // "User authentication failed" -> 应提取非停用词
            assert!(!s.is_empty());
        }

        let r2 = target.field("R2").unwrap();
        if let Value::Chars(s) = r2.get_value() {
            println!("R2: {}", s);
            // "The server is running" -> 应提取 "server" (过滤停用词 the/is)
            assert_eq!(s.as_str(), "server");
        }

        let r3 = target.field("R3").unwrap();
        if let Value::Chars(s) = r3.get_value() {
            println!("R3: {}", s);
            // "Failed to connect database" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r4 = target.field("R4").unwrap();
        if let Value::Chars(s) = r4.get_value() {
            println!("R4: {}", s);
            // "Request processing timeout occurred" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r5 = target.field("R5").unwrap();
        if let Value::Chars(s) = r5.get_value() {
            println!("R5: {}", s);
            // "Port 8080 is already in use" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r6 = target.field("R6").unwrap();
        if let Value::Chars(s) = r6.get_value() {
            println!("R6: {}", s);
            // "API call returned 404" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r7 = target.field("R7").unwrap();
        if let Value::Chars(s) = r7.get_value() {
            println!("R7: {}", s);
            // "NullPointerException thrown" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r8 = target.field("R8").unwrap();
        if let Value::Chars(s) = r8.get_value() {
            println!("R8: {}", s);
            // "Redis cache miss" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r9 = target.field("R9").unwrap();
        if let Value::Chars(s) = r9.get_value() {
            println!("R9: {}", s);
            // "the a an is" -> 全停用词，返回第一个词
            assert!(s.is_empty());
        }

        let r10 = target.field("R10").unwrap();
        if let Value::Chars(s) = r10.get_value() {
            println!("R10: {}", s);
            // "Starting application server" -> 应提取核心词
            assert!(!s.is_empty());
        }

        let r11 = target.field("R11").unwrap();
        if let Value::Chars(s) = r11.get_value() {
            println!("R11: {}", s);
            // "Connecting to remote host" -> 应提取核心词
            assert!(!s.is_empty());
        }
    }

    fn print_saso(target: &DataRecord, name: &str) {
        if let Some(field) = target.field(name) {
            if let Value::Obj(obj) = field.get_value() {
                let subject = obj
                    .get("subject")
                    .map(|f| f.get_value().to_string())
                    .unwrap_or_default();
                let action = obj
                    .get("action")
                    .map(|f| f.get_value().to_string())
                    .unwrap_or_default();
                let object = obj
                    .get("object")
                    .map(|f| f.get_value().to_string())
                    .unwrap_or_default();
                let status = obj
                    .get("status")
                    .map(|f| f.get_value().to_string())
                    .unwrap_or_default();
                println!(
                    "{}: subject={}, action={}, object={}, status={}",
                    name, subject, action, object, status
                );
            }
        }
    }

    fn get_saso(target: &DataRecord, name: &str) -> (String, String, String, String) {
        if let Some(field) = target.field(name) {
            if let Value::Obj(obj) = field.get_value() {
                let get = |key: &str| -> String {
                    obj.get(key)
                        .and_then(|f| {
                            if let Value::Chars(s) = f.get_value() {
                                Some(s.to_string())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                };
                return (get("subject"), get("action"), get("object"), get("status"));
            }
        }
        Default::default()
    }

    #[test]
    fn test_extract_subject_object() {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            // 英文：主体 + 状态
            DataField::from_chars("M1", "database connection failed"),
            // 英文：主体 + 动作 + 状态
            DataField::from_chars("M2", "User authentication failed"),
            // 英文：动作 + 对象（无显式 subject）
            DataField::from_chars("M3", "Failed to connect database"),
            // 英文：主体 + 动作 + 对象 + 状态
            DataField::from_chars("M4", "Server failed to connect database"),
            // 英文：领域词 + 动作词 + 状态
            DataField::from_chars("M5", "Request processing timeout"),
            // 中文：主体 + 状态
            DataField::from_chars("M6", "数据库连接失败"),
            // 中文：主体 + 动作 + 状态
            DataField::from_chars("M7", "用户登录失败"),
            // 中文：主体 + 动作 + 对象
            DataField::from_chars("M8", "服务器连接数据库超时"),
            // 混合
            DataField::from_chars("M9", "HTTP请求超时"),
        ];

        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test_saso
        ---
        S1  =  pipe read(M1) | extract_subject_object ;
        S2  =  pipe read(M2) | extract_subject_object ;
        S3  =  pipe read(M3) | extract_subject_object ;
        S4  =  pipe read(M4) | extract_subject_object ;
        S5  =  pipe read(M5) | extract_subject_object ;
        S6  =  pipe read(M6) | extract_subject_object ;
        S7  =  pipe read(M7) | extract_subject_object ;
        S8  =  pipe read(M8) | extract_subject_object ;
        S9  =  pipe read(M9) | extract_subject_object ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        // 输出所有结果
        for name in ["S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9"] {
            print_saso(&target, name);
        }

        // S1: "database connection failed" → subject=database, status=failed
        let (subject, _, _, status) = get_saso(&target, "S1");
        assert_eq!(subject, "database");
        assert_eq!(status, "failed");

        // S2: "User authentication failed" → subject=User, status=failed
        let (subject, _, _, status) = get_saso(&target, "S2");
        assert_eq!(subject, "User");
        assert_eq!(status, "failed");

        // S3: "Failed to connect database"
        //   → 无显式 subject，第一个实体 database 作为 subject
        //   → action=connect, status=Failed
        let (subject, action, _, status) = get_saso(&target, "S3");
        assert_eq!(subject, "database");
        assert_eq!(action, "connect");
        assert_eq!(status, "Failed");

        // S4: "Server failed to connect database"
        //   → subject=Server, action=connect, object=database, status=failed
        let (subject, action, object, status) = get_saso(&target, "S4");
        assert_eq!(subject, "Server");
        assert_eq!(action, "connect");
        assert_eq!(object, "database");
        assert_eq!(status, "failed");

        // S5: "Request processing timeout"
        //   → subject=Request（领域实体词），action=processing，status=timeout
        let (subject, action, _, status) = get_saso(&target, "S5");
        assert_eq!(subject, "Request");
        assert_eq!(action, "processing");
        assert_eq!(status, "timeout");

        // S6-S9 中文，主要验证非空
        let (subject, _, _, status) = get_saso(&target, "S6");
        println!("S6 check: subject={}, status={}", subject, status);
        assert!(!subject.is_empty());
        assert!(!status.is_empty());

        let (subject, _, _, status) = get_saso(&target, "S7");
        println!("S7 check: subject={}, status={}", subject, status);
        assert!(!subject.is_empty());
        assert!(!status.is_empty());

        let (subject, _, _, status) = get_saso(&target, "S8");
        println!("S8 check: subject={}, status={}", subject, status);
        assert!(!subject.is_empty());
        assert!(!status.is_empty());

        let (subject, _, _, status) = get_saso(&target, "S9");
        println!("S9 check: subject={}, status={}", subject, status);
        assert!(!subject.is_empty());
        assert!(!status.is_empty());
    }

    // -----------------------------------------------------------------------
    // 准确率测试框架
    // -----------------------------------------------------------------------

    /// 标注的期望结果
    #[derive(Debug, Clone, PartialEq)]
    struct Expected {
        subject: &'static str,
        action: &'static str,
        object: &'static str,
        status: &'static str,
    }

    /// 测试用例
    struct TestCase {
        text: &'static str,
        expected: Expected,
        description: &'static str,
    }

    /// 准确率测试数据集
    const ACCURACY_TEST_CASES: &[TestCase] = &[
        // 英文：主体 + 状态
        TestCase {
            text: "database connection failed",
            expected: Expected {
                subject: "database",
                action: "",
                object: "",
                status: "failed",
            },
            description: "EN: entity + status",
        },
        // 英文：主体 + 动作 + 状态
        TestCase {
            text: "User authentication failed",
            expected: Expected {
                subject: "User",
                action: "authentication",
                object: "",
                status: "failed",
            },
            description: "EN: entity + action + status",
        },
        // 英文：状态 + 动作 + 对象
        TestCase {
            text: "Failed to connect database",
            expected: Expected {
                subject: "database",
                action: "connect",
                object: "",
                status: "Failed",
            },
            description: "EN: status + action + object",
        },
        // 英文：主体 + 状态 + 动作 + 对象 + 状态
        TestCase {
            text: "Server failed to connect database",
            expected: Expected {
                subject: "Server",
                action: "connect",
                object: "database",
                status: "failed",
            },
            description: "EN: full structure",
        },
        // 英文：主体 + 动作 + 状态
        TestCase {
            text: "Request processing timeout",
            expected: Expected {
                subject: "Request",
                action: "processing",
                object: "",
                status: "timeout",
            },
            description: "EN: entity + action + status",
        },
        // 中文：主体 + 动作 + 状态
        TestCase {
            text: "数据库连接失败",
            expected: Expected {
                subject: "数据库",
                action: "连接",
                object: "",
                status: "失败",
            },
            description: "CN: entity + action + status",
        },
        // 中文：主体 + 动作 + 状态
        TestCase {
            text: "用户登录失败",
            expected: Expected {
                subject: "用户",
                action: "登录",
                object: "",
                status: "失败",
            },
            description: "CN: entity + action + status",
        },
        // 中文：主体 + 动作 + 对象 + 状态
        TestCase {
            text: "服务器连接数据库超时",
            expected: Expected {
                subject: "服务器",
                action: "连接",
                object: "数据库",
                status: "超时",
            },
            description: "CN: full structure",
        },
        // 混合
        TestCase {
            text: "HTTP请求超时",
            expected: Expected {
                subject: "HTTP",
                action: "请求",
                object: "",
                status: "超时",
            },
            description: "Mixed: entity + action + status",
        },
        // 复杂英文
        TestCase {
            text: "The server is running",
            expected: Expected {
                subject: "server",
                action: "running",
                object: "",
                status: "",
            },
            description: "EN: entity + progressive verb",
        },
        // 领域词优先
        TestCase {
            text: "error: connection timeout",
            expected: Expected {
                subject: "error",
                action: "",
                object: "",
                status: "timeout",
            },
            description: "EN: domain word priority",
        },
        // 中文复杂场景
        TestCase {
            text: "应用程序启动失败",
            expected: Expected {
                subject: "应用程序",
                action: "启动",
                object: "",
                status: "失败",
            },
            description: "CN: compound entity + action + status",
        },
    ];

    #[test]
    fn test_accuracy() {
        let cache = &mut FieldQueryCache::default();

        let mut total = 0;
        let mut correct_subject = 0;
        let mut correct_action = 0;
        let mut correct_object = 0;
        let mut correct_status = 0;
        let mut fully_correct = 0;

        println!("\n======= Accuracy Test Report =======");
        println!(
            "{:<50} {:<10} {:<10} {:<10} {:<10} {:<10}",
            "Test Case", "Subject", "Action", "Object", "Status", "Full"
        );
        println!("{}", "-".repeat(100));

        for test_case in ACCURACY_TEST_CASES {
            total += 1;

            let data = vec![DataField::from_chars("msg", test_case.text)];
            let src = DataRecord::from(data);

            let mut conf = r#"
                name : accuracy_test
                ---
                result = pipe read(msg) | extract_subject_object ;
                "#;

            let model = oml_parse_raw(&mut conf).assert();
            let target = model.transform(src, cache);

            let (subject, action, object, status) = get_saso(&target, "result");

            let subj_ok = subject == test_case.expected.subject;
            let act_ok = action == test_case.expected.action;
            let obj_ok = object == test_case.expected.object;
            let stat_ok = status == test_case.expected.status;
            let full_ok = subj_ok && act_ok && obj_ok && stat_ok;

            if subj_ok {
                correct_subject += 1;
            }
            if act_ok {
                correct_action += 1;
            }
            if obj_ok {
                correct_object += 1;
            }
            if stat_ok {
                correct_status += 1;
            }
            if full_ok {
                fully_correct += 1;
            }

            println!(
                "{:<50} {:<10} {:<10} {:<10} {:<10} {:<10}",
                test_case.description,
                if subj_ok { "✓" } else { "✗" },
                if act_ok { "✓" } else { "✗" },
                if obj_ok { "✓" } else { "✗" },
                if stat_ok { "✓" } else { "✗" },
                if full_ok { "✓" } else { "✗" }
            );

            if !full_ok {
                println!(
                    "  Expected: sub={}, act={}, obj={}, stat={}",
                    test_case.expected.subject,
                    test_case.expected.action,
                    test_case.expected.object,
                    test_case.expected.status
                );
                println!(
                    "  Got:      sub={}, act={}, obj={}, stat={}",
                    subject, action, object, status
                );
            }
        }

        println!("{}", "=".repeat(100));
        println!("\n===== Accuracy Statistics =====");
        println!(
            "Subject Accuracy: {}/{} = {:.1}%",
            correct_subject,
            total,
            (correct_subject as f32 / total as f32) * 100.0
        );
        println!(
            "Action Accuracy:  {}/{} = {:.1}%",
            correct_action,
            total,
            (correct_action as f32 / total as f32) * 100.0
        );
        println!(
            "Object Accuracy:  {}/{} = {:.1}%",
            correct_object,
            total,
            (correct_object as f32 / total as f32) * 100.0
        );
        println!(
            "Status Accuracy:  {}/{} = {:.1}%",
            correct_status,
            total,
            (correct_status as f32 / total as f32) * 100.0
        );
        println!(
            "Full Match Rate:  {}/{} = {:.1}%",
            fully_correct,
            total,
            (fully_correct as f32 / total as f32) * 100.0
        );
        println!("================================\n");

        // 设定准确率阈值
        let subject_acc = (correct_subject as f32 / total as f32) * 100.0;
        let action_acc = (correct_action as f32 / total as f32) * 100.0;
        let status_acc = (correct_status as f32 / total as f32) * 100.0;

        assert!(
            subject_acc >= 70.0,
            "Subject accuracy too low: {:.1}%",
            subject_acc
        );
        assert!(
            action_acc >= 70.0,
            "Action accuracy too low: {:.1}%",
            action_acc
        );
        assert!(
            status_acc >= 80.0,
            "Status accuracy too low: {:.1}%",
            status_acc
        );
    }

    #[test]
    fn test_debug_mode() {
        use super::analyze_subject_object_with_debug;

        // 测试debug信息输出
        println!("\n===== Debug Mode Test =====\n");

        let test_cases = [
            "User authentication failed",
            "The server is running",
            "database connection failed",
        ];

        for text in &test_cases {
            println!("Text: {}", text);
            let (subject, action, object, status, debug) =
                analyze_subject_object_with_debug(text, true);

            if let Some(ref d) = debug {
                println!("Results:");
                println!("  subject: {} (confidence: {:.2}, rule: {})", subject, d.subject_confidence, d.subject_rule);
                println!("  action:  {} (confidence: {:.2}, rule: {})", action, d.action_confidence, d.action_rule);
                println!("  object:  {} (confidence: {:.2}, rule: {})", object, d.object_confidence, d.object_rule);
                println!("  status:  {} (confidence: {:.2}, rule: {})", status, d.status_confidence, d.status_rule);

                println!("Debug Info:");
                println!("  Tokens: {:?}", d.tokens);
                println!("  POS Tags:");
                for (word, pos) in &d.pos_tags {
                    println!("    {} / {}", word, pos);
                }
            }
            println!();
        }
    }
}

