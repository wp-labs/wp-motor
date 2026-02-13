# extract_subject_object PipeFun 使用指南

## 概述

`extract_subject_object` 是一个基于 jieba-rs 中文分词和词性标注的 NLP PipeFun，用于从日志文本中提取主客体结构（Subject-Action-Object-Status）。支持中英文混合文本，针对日志分析场景优化，可自动识别日志中的主体、动作、对象和状态。

## 功能特性

### 1. 主客体结构识别

自动将日志文本分解为四个核心要素：

- **Subject（主体）**：执行动作的主体（谁/什么）
- **Action（动作）**：执行的动作（做什么）
- **Object（对象）**：动作作用的对象（作用于谁/什么）
- **Status（状态）**：结果状态（结果如何）

### 2. 智能词角色分类

基于词性和词缀规则自动分类：

#### **英文分类规则**
- **动作词 (Action)**
  - 词典匹配：connect, login, process, send, receive 等
  - `-ing` 后缀：connecting, running, processing
  - `-ed` 后缀：failed, connected, started
  - `-tion/-sion` 后缀：authentication, connection（除非在实体白名单）

- **状态词 (Status)**
  - failed, success, timeout, exception, crashed 等
  - 表示终态/结果的词

- **实体词 (Entity)**
  - 实体名词白名单：connection（作为实体）, session, transaction
  - 领域关键词：database, server, client, request, response
  - 默认分类

#### **中文分类规则**
- **动作词**：v（动词）、vn（名动词）、vd（副动词）
- **实体词**：n（名词）、nr（人名）、ns（地名）、nt（机构名）等
- **状态词**：失败、成功、超时、异常、错误等

### 3. Debug 模式

可选的 debug 模式输出详细分析信息：
- 分词结果
- 词性标注
- 匹配规则
- 置信度评分

## 使用示例

### 示例 1：基本用法

```oml
name: basic_extract
---
log_message : chars = take() ;

# 提取主客体结构
structure = pipe read(log_message) | extract_subject_object ;

# 输入: "database connection failed"
# 输出:
#   structure = {
#     "subject": "database",
#     "action": "",
#     "object": "",
#     "status": "failed"
#   }
```

### 示例 2：提取各个字段

```oml
name: extract_fields
---
log : chars = take() ;

# 提取结构
structure = pipe read(log) | extract_subject_object ;

# 分别提取各字段
subject = structure.subject ;
action = structure.action ;
object = structure.object ;
status = structure.status ;

# 输入: "Server failed to connect database"
# 输出:
#   subject: "Server"
#   action: "connect"
#   object: "database"
#   status: "failed"
```

### 示例 3：与条件判断结合

```oml
name: conditional_analysis
---
log_message : chars = take() ;
structure = pipe read(log_message) | extract_subject_object ;

# 判断是否为失败状态
is_failed = match structure.status {
    "failed" => true,
    "失败" => true,
    "exception" => true,
    "异常" => true,
    _ => false
};

# 判断是否为连接相关操作
is_connection = match structure.action {
    "connect" => true,
    "连接" => true,
    _ => false
};
```

### 示例 4：英文日志

```oml
name: english_log
---
# 测试用例 1: 主体 + 状态
log1 : chars = chars(database connection failed) ;
result1 = pipe read(log1) | extract_subject_object ;
# subject="database", status="failed"

# 测试用例 2: 主体 + 动作 + 状态
log2 : chars = chars(User authentication failed) ;
result2 = pipe read(log2) | extract_subject_object ;
# subject="User", action="authentication", status="failed"

# 测试用例 3: 状态 + 动作 + 对象
log3 : chars = chars(Failed to connect database) ;
result3 = pipe read(log3) | extract_subject_object ;
# subject="database", action="connect", status="Failed"

# 测试用例 4: 完整结构
log4 : chars = chars(Server failed to connect database) ;
result4 = pipe read(log4) | extract_subject_object ;
# subject="Server", action="connect", object="database", status="failed"

# 测试用例 5: 主体 + 动作 + 状态
log5 : chars = chars(Request processing timeout) ;
result5 = pipe read(log5) | extract_subject_object ;
# subject="Request", action="processing", status="timeout"
```

### 示例 5：中文日志

```oml
name: chinese_log
---
# 测试用例 1: 主体 + 动作 + 状态
log1 : chars = chars(数据库连接失败) ;
result1 = pipe read(log1) | extract_subject_object ;
# subject="数据库", action="连接", status="失败"

# 测试用例 2: 主体 + 动作 + 状态
log2 : chars = chars(用户登录失败) ;
result2 = pipe read(log2) | extract_subject_object ;
# subject="用户", action="登录", status="失败"

# 测试用例 3: 完整结构
log3 : chars = chars(服务器连接数据库超时) ;
result3 = pipe read(log3) | extract_subject_object ;
# subject="服务器", action="连接", object="数据库", status="超时"
```

### 示例 6：混合语言日志

```oml
name: mixed_language
---
log : chars = chars(HTTP请求超时) ;
structure = pipe read(log) | extract_subject_object ;

# 输出:
#   subject: "HTTP"
#   action: "请求"
#   object: ""
#   status: "超时"
```

### 示例 7：复杂日志场景

```oml
name: complex_scenario
---
log : chars = chars(The server is running) ;
structure = pipe read(log) | extract_subject_object ;

# 输出:
#   subject: "server"
#   action: "running"  # -ing 后缀识别为动作
#   object: ""
#   status: ""
```

## 工作原理

### 处理流程

```
输入日志文本
    ↓
jieba 分词 + 词性标注
    ↓
遍历每个词：
    ↓
跳过停用词（的、了、the、a 等）
    ↓
词角色分类：
  - 英文词 → classify_eng()
  - 中文词 → classify_cn()
    ↓
角色分配规则：
  1. Status → 第一个状态词
  2. Action → 第一个动作词（标记 action_seen）
  3. Entity →
     - 第一个实体词 → Subject
     - action_seen 后的第一个实体词 → Object
    ↓
输出结构化结果
```

### 词角色分类详解

#### **classify_eng (英文词分类)**

优先级从高到低：

1. **领域词典匹配**
   - `STATUS_WORDS`: failed, success, timeout → Status
   - `ACTION_VERBS`: connect, login, process → Action

2. **实体名词白名单**
   - `ENTITY_NOUNS`: connection, session, transaction → Entity
   - 覆盖词缀规则（避免 connection 被识别为动作）

3. **词缀规则**
   - `-ing` 结尾 (len > 4) → Action (connecting, running)
   - `-ed` 结尾 (len > 3) → Action (failed, connected)
   - `-tion/-sion` 结尾 (len > 5) → Action (authentication)

4. **默认**
   - 其他词 → Entity

#### **classify_cn (中文词分类)**

根据词性标注（POS）：

1. **词典匹配**
   - `STATUS_WORDS`: 失败、成功、超时 → Status
   - `ACTION_VERBS`: 连接、登录、处理 → Action

2. **词性规则**
   - `v/vn/vd` → Action（动词类）
   - `n/nr/ns/nt/nz/ng` → Entity（名词类）

3. **领域词回退**
   - 其他词性但在 `LOG_DOMAIN` → Entity

### 分配规则

```rust
// 伪代码
for word in tokens:
    if is_stopword(word):
        continue

    role = classify(word)

    match role:
        Status =>
            if status.is_empty():
                status = word

        Action =>
            if action.is_empty():
                action = word
                action_seen = true

        Entity =>
            if subject.is_empty():
                subject = word  // 第一个实体 → 主体
            elif action_seen and object.is_empty():
                object = word   // action 后的第一个实体 → 对象
```

## Debug 模式

### 启用方式

Debug 模式在代码中通过 `ExtractSubjectObject` 结构体的 `debug` 字段控制：

```rust
// 在 Rust 代码中
pub struct ExtractSubjectObject {
    pub debug: bool,  // 设置为 true 启用 debug
}
```

### Debug 输出结构

启用 debug 后，输出对象会包含 `debug` 字段，内容为 JSON 格式：

```json
{
  "tokenization": ["Server", "failed", "to", "connect", "database"],
  "pos_tags": [
    ["Server", "eng"],
    ["failed", "eng"],
    ["to", "eng"],
    ["connect", "eng"],
    ["database", "eng"]
  ],
  "rules_matched": {
    "subject": "rule2: core_pos(eng) + non_stopword",
    "action": "rule1: action_verb_match",
    "object": "rule1: domain_entity_match (after_action)",
    "status": "rule1: status_word_match"
  },
  "confidence": {
    "subject": 0.8,
    "action": 1.0,
    "object": 1.0,
    "status": 1.0
  }
}
```

### Debug 字段说明

- **tokenization**: 分词结果（所有词）
- **pos_tags**: 词性标注（词 + 词性）
- **rules_matched**: 每个字段匹配的规则
  - `rule1`: 优先级高（词典匹配）
  - `rule2`: 优先级中（词缀规则、词性规则）
  - `rule3`: 优先级低（回退规则）
- **confidence**: 置信度（0.0-1.0）
  - 1.0：词典直接匹配
  - 0.7-0.8：规则推断

## 准确率测试

代码中包含完整的准确率测试框架（见 `extract_word.rs:1045-1337`）：

### 测试数据集

12 个标注测试用例，覆盖：
- 英文：主体+状态、主体+动作+状态、状态+动作+对象、完整结构
- 中文：主体+动作+状态、完整结构
- 混合：中英文混合场景

### 准确率指标

```
Subject Accuracy: >= 70%
Action Accuracy:  >= 70%
Object Accuracy:  >= 70%
Status Accuracy:  >= 80%
```

### 运行测试

```bash
cd crates/wp-oml
cargo test test_accuracy -- --nocapture
```

输出示例：
```
======= Accuracy Test Report =======
Test Case                          Subject    Action     Object     Status     Full
------------------------------------------------------------------------------------------------
EN: entity + status                ✓          ✓          ✓          ✓          ✓
EN: entity + action + status       ✓          ✓          ✓          ✓          ✓
EN: status + action + object       ✓          ✓          ✓          ✓          ✓
...

===== Accuracy Statistics =====
Subject Accuracy: 12/12 = 100.0%
Action Accuracy:  11/12 = 91.7%
Object Accuracy:  12/12 = 100.0%
Status Accuracy:  12/12 = 100.0%
Full Match Rate:  11/12 = 91.7%
```

## 领域词典

### STATUS_WORDS (状态词)

```rust
// 英文
"failed", "failure", "success", "succeeded", "timeout", "exception",
"crashed", "disconnected", "stopped", "completed", "pending",
"refused", "dropped", "rejected", "expired", "closed"

// 中文
"失败", "成功", "超时", "异常", "错误", "崩溃",
"断开", "拒绝", "丢失"
```

### ACTION_VERBS (动作词)

```rust
// 英文
"connect", "login", "logout", "respond", "start", "stop", "fail",
"run", "process", "send", "receive", "read", "write", "open",
"close", "bind", "listen", "authenticate", "authorize", "create",
"delete", "update", "upload", "download", "retry", "handle",
"load", "fetch", "parse", "resolve", "block", "deny"

// 中文
"连接", "登录", "登出", "请求", "响应", "启动", "停止",
"处理", "发送", "接收", "读取", "写入", "认证", "访问",
"创建", "删除", "更新", "下载", "上传", "重试"
```

### ENTITY_NOUNS (实体名词)

```rust
// 英文（-tion 结尾但作为实体）
"connection", "transaction", "session", "application",
"configuration", "permission", "operation", "exception"

// 中文
"连接", "会话", "事务", "应用", "配置", "权限"
```

### LOG_DOMAIN (日志领域词)

```rust
// 日志级别
"error", "warn", "info", "debug", "fatal", "trace"

// 系统相关
"exception", "failure", "timeout", "connection", "database",
"server", "client", "request", "response", "login", "logout",
"auth", "authentication", "permission", "access"

// 网络相关
"http", "https", "tcp", "udp", "ip", "port", "socket"

// 安全相关
"attack", "virus", "malware", "threat", "alert", "blocked", "denied"
```

### LOG_STOP (停用词)

```rust
// 中文停用词
"的", "了", "在", "是", "我", "有", "和", "就", "不", "人",
"都", "一", "一个", "上", "也", "很", "到", "说", "要", "去",
"你", "会", "着", "没有", "看", "好", "自己", "这"

// 英文停用词
"the", "a", "an", "is", "are", "was", "were", "be", "been",
"being", "of", "at", "in", "to", "for", "and", "or", "but"
```

## 边界情况处理

| 输入 | 输出 | 说明 |
|------|------|------|
| 空字符串 `""` | 全空对象 | `{subject:"", action:"", object:"", status:""}` |
| 全空格 `"   "` | 全空对象 | trim 后为空 |
| 全停用词 `"的是在了不"` | 全空对象 | 所有词都被过滤 |
| 只有一个词 `"error"` | 部分填充 | 根据词角色填充对应字段 |
| 无动作词 `"database failed"` | action 为空 | subject="database", status="failed" |

## 性能优化

### 全局单例

jieba 分词器使用 `lazy_static!` 实现全局单例：

```rust
lazy_static! {
    static ref JIEBA: Jieba = Jieba::new();
}
```

**优点**：
- 只初始化一次
- 词典加载一次，常驻内存
- 多次调用复用实例

### 词典查找

使用 `HashSet` 存储词典：

```rust
static ref STATUS_WORDS: HashSet<&'static str> = { ... };
static ref ACTION_VERBS: HashSet<&'static str> = { ... };
```

**性能**：O(1) 查找时间

## 扩展与定制

### 添加自定义状态词

修改 `STATUS_WORDS` 集合：

```rust
static ref STATUS_WORDS: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("failed");
    set.insert("custom_status");  // 添加自定义状态词
    // ...
    set
};
```

### 添加自定义动作词

修改 `ACTION_VERBS` 集合：

```rust
static ref ACTION_VERBS: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("connect");
    set.insert("custom_action");  // 添加自定义动作词
    // ...
    set
};
```

### 添加自定义实体词

修改 `ENTITY_NOUNS` 集合（覆盖词缀规则）：

```rust
static ref ENTITY_NOUNS: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("connection");
    set.insert("custom_entity");  // 添加自定义实体
    // ...
    set
};
```

### 调整停用词

修改 `LOG_STOP` 集合：

```rust
static ref LOG_STOP: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("的");
    // set.insert("是");  // 注释掉不需要过滤的词
    // ...
    set
};
```

## 与其他 PipeFun 组合

### 与 extract_main_word 组合

```oml
name: combined_nlp
---
log : chars = take() ;

# 提取主要词（快速）
keyword = pipe read(log) | extract_main_word ;

# 提取完整结构（详细）
structure = pipe read(log) | extract_subject_object ;

# 使用两者结果
is_error_by_keyword = match keyword {
    "error" => true,
    _ => false
};

is_error_by_status = match structure.status {
    "failed" => true,
    "exception" => true,
    _ => false
};
```

### 与 to_json 组合

```oml
name: json_output
---
log : chars = take() ;
structure = pipe read(log) | extract_subject_object ;

# 转换为 JSON 字符串用于存储/传输
json_output = pipe read(structure) | to_json ;

# 输出: {"subject":"Server","action":"connect","object":"database","status":"failed"}
```

### 与条件判断组合

```oml
name: conditional_routing
---
log : chars = take() ;
structure = pipe read(log) | extract_subject_object ;

# 根据状态路由
severity = match structure.status {
    "failed" => digit(3),
    "timeout" => digit(3),
    "exception" => digit(3),
    "成功" => digit(1),
    "success" => digit(1),
    _ => digit(2)
};
```

## 注意事项

1. **词性依赖**：分词和词性标注依赖 jieba-rs，中文准确率较高，英文通过词缀规则补充
2. **领域适配**：词典针对日志场景优化，其他领域可能需要调整
3. **性能考虑**：首次调用会初始化 jieba 词典（~50ms），后续调用快速
4. **编码要求**：输入必须是有效的 UTF-8 字符串
5. **调试模式**：debug 模式会输出额外字段，生产环境建议关闭

## 相关资源

- [extract_main_word 使用指南](./extract_main_word.md) - 关键词提取
- [OML 管道函数参考](./pipe_functions.md) - 所有 PipeFun 文档
- [OML PipeFun 开发指南](../../guide/oml_pipefun_development_guide.md) - 开发新函数
- [jieba-rs GitHub](https://github.com/messense/jieba-rs)
- [jieba-rs 文档](https://docs.rs/jieba-rs)

## 源代码位置

- **定义**: `crates/wp-oml/src/language/syntax/functions/pipe/extract_word.rs`
- **实现**: `crates/wp-oml/src/core/evaluator/transform/pipe/extract_word.rs`
- **测试**: `crates/wp-oml/src/core/evaluator/transform/pipe/extract_word.rs:649-1374`

---

更新时间：2026-02-01
