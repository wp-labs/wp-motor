use crate::core::prelude::*;
use crate::language::{ExtractMainWord, ExtractSubjectObject};
use jieba_rs::Jieba;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use wp_model_core::model::types::value::ObjectValue;
use wp_model_core::model::{DataField, Value};

lazy_static! {
    // Jieba 中文分词器实例（全局单例）
    static ref JIEBA: Jieba = Jieba::new();

    // 核心词性集合（名词、动词、形容词、英文、数字）
    static ref CORE_POS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // 名词类
        set.insert("n");   // 普通名词
        set.insert("nr");  // 人名
        set.insert("ns");  // 地名
        set.insert("nt");  // 机构名
        set.insert("nz");  // 其他专名
        set.insert("ng");  // 名词性语素
        // 动词类
        set.insert("v");   // 动词
        set.insert("vn");  // 名动词
        set.insert("vd");  // 副动词
        // 形容词类
        set.insert("a");   // 形容词
        set.insert("ad");  // 副形词
        set.insert("an");  // 名形词
        // 英文和数字
        set.insert("eng"); // 英文
        set.insert("m");   // 数词
        set.insert("x");   // 字符串（常为代码、路径等）
        // 时间和成语
        set.insert("t");   // 时间词
        set.insert("i");   // 成语/习语
        set
    };

    // 日志常见停用词（需要过滤的词）
    static ref LOG_STOP: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // 中文停用词
        set.insert("的");
        set.insert("了");
        set.insert("在");
        set.insert("是");
        set.insert("我");
        set.insert("有");
        set.insert("和");
        set.insert("就");
        set.insert("不");
        set.insert("人");
        set.insert("都");
        set.insert("一");
        set.insert("一个");
        set.insert("上");
        set.insert("也");
        set.insert("很");
        set.insert("到");
        set.insert("说");
        set.insert("要");
        set.insert("去");
        set.insert("你");
        set.insert("会");
        set.insert("着");
        set.insert("没有");
        set.insert("看");
        set.insert("好");
        set.insert("自己");
        set.insert("这");
        // 英文停用词
        set.insert("the");
        set.insert("a");
        set.insert("an");
        set.insert("is");
        set.insert("are");
        set.insert("was");
        set.insert("were");
        set.insert("be");
        set.insert("been");
        set.insert("being");
        set.insert("of");
        set.insert("at");
        set.insert("in");
        set.insert("to");
        set.insert("for");
        set.insert("and");
        set.insert("or");
        set.insert("but");
        set
    };

    // 日志领域关键词（强制保留的词）
    static ref LOG_DOMAIN: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // 日志级别
        set.insert("error");
        set.insert("warn");
        set.insert("info");
        set.insert("debug");
        set.insert("fatal");
        set.insert("trace");
        // 系统相关
        set.insert("exception");
        set.insert("failure");
        set.insert("timeout");
        set.insert("connection");
        set.insert("database");
        set.insert("server");
        set.insert("client");
        set.insert("request");
        set.insert("response");
        set.insert("login");
        set.insert("logout");
        set.insert("auth");
        set.insert("authentication");
        set.insert("permission");
        set.insert("access");
        // 网络相关
        set.insert("http");
        set.insert("https");
        set.insert("tcp");
        set.insert("udp");
        set.insert("ip");
        set.insert("port");
        set.insert("socket");
        // 安全相关
        set.insert("attack");
        set.insert("virus");
        set.insert("malware");
        set.insert("threat");
        set.insert("alert");
        set.insert("blocked");
        set.insert("denied");
        set
    };

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

    // 状态词集合（表示结果/终态的词）
    static ref STATUS_WORDS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // 英文状态词
        set.insert("failed");
        set.insert("failure");
        set.insert("success");
        set.insert("succeeded");
        set.insert("timeout");
        set.insert("timed");
        set.insert("exception");
        set.insert("crashed");
        set.insert("disconnected");
        set.insert("stopped");
        set.insert("completed");
        set.insert("pending");
        set.insert("running");
        set.insert("started");
        set.insert("connected");
        set.insert("refused");
        set.insert("dropped");
        set.insert("rejected");
        set.insert("expired");
        set.insert("closed");
        // 中文状态词
        set.insert("失败");
        set.insert("成功");
        set.insert("超时");
        set.insert("异常");
        set.insert("错误");
        set.insert("崩溃");
        set.insert("断开");
        set.insert("拒绝");
        set.insert("丢失");
        set
    };

    // 动作词集合（日志中常见的动作动词基词形式）
    static ref ACTION_VERBS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("connect");
        set.insert("login");
        set.insert("logout");
        set.insert("respond");
        set.insert("start");
        set.insert("stop");
        set.insert("fail");
        set.insert("run");
        set.insert("process");
        set.insert("send");
        set.insert("receive");
        set.insert("read");
        set.insert("write");
        set.insert("open");
        set.insert("close");
        set.insert("bind");
        set.insert("listen");
        set.insert("authenticate");
        set.insert("authorize");
        set.insert("create");
        set.insert("delete");
        set.insert("update");
        set.insert("upload");
        set.insert("download");
        set.insert("retry");
        set.insert("handle");
        set.insert("load");
        set.insert("fetch");
        set.insert("parse");
        set.insert("resolve");
        set.insert("block");
        set.insert("deny");
        // 中文动作词
        set.insert("连接");
        set.insert("登录");
        set.insert("登出");
        set.insert("请求");
        set.insert("响应");
        set.insert("启动");
        set.insert("停止");
        set.insert("处理");
        set.insert("发送");
        set.insert("接收");
        set.insert("读取");
        set.insert("写入");
        set.insert("认证");
        set.insert("访问");
        set.insert("创建");
        set.insert("删除");
        set.insert("更新");
        set.insert("下载");
        set.insert("上传");
        set.insert("重试");
        set
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

/// 英文词角色判断
fn classify_eng(word: &str) -> WordRole {
    let lower = word.to_lowercase();
    if STATUS_WORDS.contains(lower.as_str()) {
        return WordRole::Status;
    }
    if ACTION_VERBS.contains(lower.as_str()) {
        return WordRole::Action;
    }
    // "-ing" 结尾 → 动作
    if lower.ends_with("ing") && lower.len() > 4 {
        return WordRole::Action;
    }
    // "-ed" 结尾且不在 STATUS_WORDS → 动作（过去式动词）
    if lower.ends_with("ed") && lower.len() > 3 {
        return WordRole::Action;
    }
    WordRole::Entity
}

/// 中文词角色判断（根据词性）
fn classify_cn(pos: &str, word: &str) -> Option<WordRole> {
    let lower = word.to_lowercase();
    if STATUS_WORDS.contains(lower.as_str()) {
        return Some(WordRole::Status);
    }
    if ACTION_VERBS.contains(lower.as_str()) {
        return Some(WordRole::Action);
    }
    match pos {
        "v" | "vn" | "vd" => Some(WordRole::Action),
        "n" | "nr" | "ns" | "nt" | "nz" | "ng" => Some(WordRole::Entity),
        _ => {
            if LOG_DOMAIN.contains(lower.as_str()) {
                Some(WordRole::Entity)
            } else {
                None // 停用词/虚词等，不参与分配
            }
        }
    }
}

/// 日志主客体分析
///
/// 对日志文本进行分词+词性标注，将词按角色分配到：
/// - subject：主体（第一个实体词，或 action 之前的实体）
/// - action：动作词（动词）
/// - object：对象（action 之后的第一个实体词）
/// - status：状态词（终态标记）
fn analyze_subject_object(text: &str) -> (String, String, String, String) {
    let tags = JIEBA.tag(text, true);

    let mut subject = String::new();
    let mut action = String::new();
    let mut object = String::new();
    let mut status = String::new();
    let mut action_seen = false;

    for tag in &tags {
        let word = tag.word.trim();
        if word.is_empty() {
            continue;
        }
        let word_lower = word.to_lowercase();
        if LOG_STOP.contains(word_lower.as_str()) {
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
                    }
                }
                WordRole::Action => {
                    if action.is_empty() {
                        action = word.to_string();
                        action_seen = true;
                    }
                }
                WordRole::Entity => {
                    if subject.is_empty() {
                        subject = word.to_string();
                    } else if action_seen && object.is_empty() {
                        object = word.to_string();
                    }
                }
            }
        }
    }

    (subject, action, object, status)
}

/// 提取日志主客体结构 - extract_subject_object
///
/// 输入一段日志文本，输出一个包含四个字段的对象：
/// - subject：主体（谁/什么）
/// - action：动作（做什么）
/// - object：对象（作用于谁/什么）
/// - status：状态（结果如何）
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

                let (subject, action, object, status) = analyze_subject_object(cleaned);

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
                    if LOG_DOMAIN.contains(word_lower.as_str()) {
                        return DataField::from_chars(
                            in_val.get_name().to_string(),
                            word.to_string(),
                        );
                    }

                    // 规则2：核心词性 + 非停用词
                    if CORE_POS.contains(pos) && !LOG_STOP.contains(word_lower.as_str()) {
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

        let src = DataRecord { items: data };

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

        let src = DataRecord { items: data };

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

        let src = DataRecord { items: data };

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
}
