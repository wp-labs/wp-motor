# NLP 词典配置说明

## 概述

NLP 词典配置系统允许通过 TOML 配置文件动态定制 `extract_main_word` 和 `extract_subject_object` 函数使用的词典。

## 配置文件位置

默认配置文件路径：
```
crates/wp-oml/nlp_dict/nlp_dict.toml
```

可以通过环境变量 `NLP_DICT_CONFIG` 指定自定义配置文件：
```bash
export NLP_DICT_CONFIG=/path/to/custom_nlp_dict.toml
```

## 配置文件结构

```toml
# 版本号（必须为 1）
version = 1

# 核心词性集合
[core_pos]
enabled = true
tags = ["n", "nr", "ns", "nt", "v", "a", "eng", "m", ...]

# 停用词
[stop_words]
enabled = true
chinese = ["的", "了", "在", ...]
english = ["the", "a", "an", ...]

# 日志领域关键词
[domain_words]
enabled = true
log_level = ["error", "warn", "info", ...]
system = ["exception", "timeout", "database", ...]
network = ["http", "https", "tcp", ...]
security = ["attack", "virus", "malware", ...]

# 状态词
[status_words]
enabled = true
english = ["failed", "success", "timeout", ...]
chinese = ["失败", "成功", "超时", ...]

# 动作词
[action_verbs]
enabled = true
english = ["connect", "login", "process", ...]
chinese = ["连接", "登录", "处理", ...]

# 实体名词
[entity_nouns]
enabled = true
english = ["connection", "session", "transaction", ...]
chinese = ["连接", "会话", "事务", ...]
```

## 自定义词典

### 方法 1：修改默认配置

直接编辑 `crates/wp-oml/nlp_dict/nlp_dict.toml`：

```toml
# 添加自定义领域词
[domain_words]
enabled = true
log_level = ["error", "warn", "info", "debug", "fatal", "trace", "custom_level"]
system = [
    "exception", "timeout", "database",
    "my_custom_keyword",  # 添加自定义关键词
]
```

### 方法 2：使用环境变量

1. 创建自定义配置文件 `/etc/wp-motor/nlp_dict.toml`
2. 设置环境变量：
   ```bash
   export NLP_DICT_CONFIG=/etc/wp-motor/nlp_dict.toml
   ```
3. 重新启动应用程序

### 方法 3：禁用特定词典

如果不需要某些词典，可以禁用它们：

```toml
# 禁用停用词过滤
[stop_words]
enabled = false
chinese = []
english = []
```

## 配置示例

### 示例 1：添加自定义状态词

```toml
[status_words]
enabled = true
english = [
    "failed", "success", "timeout",
    "aborted",      # 新增：中止状态
    "cancelled",    # 新增：取消状态
]
chinese = [
    "失败", "成功", "超时",
    "中止",         # 新增
    "取消",         # 新增
]
```

### 示例 2：扩展领域词典

```toml
[domain_words]
enabled = true

# 日志级别
log_level = ["error", "warn", "info", "debug", "fatal", "trace"]

# 添加数据库相关关键词
database = [
    "mysql", "redis", "mongodb", "postgresql",
    "query", "transaction", "cursor", "index",
]

# 添加云服务关键词
cloud = [
    "aws", "azure", "gcp", "s3", "ec2", "lambda",
]
```

### 示例 3：行业特定词典（金融）

```toml
[domain_words]
enabled = true

# 金融领域关键词
finance = [
    "payment", "transaction", "account", "balance",
    "transfer", "withdraw", "deposit", "refund",
    "支付", "交易", "账户", "余额", "转账",
]

[status_words]
enabled = true
english = [
    "failed", "success", "pending",
    "authorized", "settled", "reversed",  # 金融状态
]
chinese = [
    "失败", "成功", "待处理",
    "已授权", "已结算", "已冲正",  # 金融状态
]
```

## 词典类型说明

### 1. core_pos（核心词性）

用于 `extract_main_word` 函数，定义哪些词性的词会被提取。

**常用词性标签**：
- `n` - 普通名词
- `nr` - 人名
- `ns` - 地名
- `nt` - 机构名
- `v` - 动词
- `a` - 形容词
- `eng` - 英文
- `m` - 数词

### 2. stop_words（停用词）

在提取关键词时需要过滤的词。

**用途**：
- 过滤"的"、"了"、"the"、"a" 等无意义的词
- 提高关键词提取的准确性

### 3. domain_words（领域关键词）

特定领域的关键词，优先级最高。

**用途**：
- 日志分析中的领域词汇
- 行业特定术语
- 优先被 `extract_main_word` 识别

### 4. status_words（状态词）

表示结果或终态的词。

**用途**：
- `extract_subject_object` 识别日志状态
- 识别操作结果（成功/失败/超时等）

### 5. action_verbs（动作词）

表示动作的动词基词形式。

**用途**：
- `extract_subject_object` 识别操作动作
- 识别日志中的行为（连接/登录/处理等）

### 6. entity_nouns（实体名词）

特殊的实体名词（覆盖词缀规则）。

**用途**：
- 将 "connection"、"transaction" 等 `-tion` 结尾的词识别为实体而非动作
- 防止词缀规则误判

## 配置验证

### 运行配置加载测试

```bash
cargo test -p wp-oml test_load_default_config
```

### 运行 NLP 功能测试

```bash
# 测试关键词提取
cargo test -p wp-oml test_extract_main_word

# 测试主客体分析
cargo test -p wp-oml test_extract_subject_object

# 测试准确率
cargo test -p wp-oml test_accuracy
```

## 性能考虑

### 1. 配置加载时机

配置在应用程序启动时加载一次，使用 `lazy_static!` 实现全局单例：

```rust
pub static NLP_DICT: Lazy<NlpDict> = Lazy::new(|| {
    // 加载配置并构建词典
});
```

### 2. 内存优化

所有词典使用 `HashSet` 存储，查找时间复杂度为 O(1)。

### 3. 词典大小建议

- **停用词**：100-200 个
- **领域关键词**：200-500 个
- **状态词**：50-100 个
- **动作词**：100-200 个
- **实体名词**：50-100 个

**注意**：过大的词典会增加内存占用，但对查找性能影响不大。

## 故障排查

### 配置加载失败

如果配置加载失败，系统会输出警告并使用空词典：

```
Warning: Failed to load NLP dict config: <error message>. Using empty dict.
```

**常见原因**：
1. 配置文件不存在
2. TOML 格式错误
3. 版本号不匹配（必须为 1）

### 测试自定义配置

```bash
# 设置环境变量指向自定义配置
export NLP_DICT_CONFIG=/path/to/custom.toml

# 运行测试
cargo test -p wp-oml -- --nocapture
```

## 最佳实践

### 1. 版本控制

将自定义配置文件纳入版本控制：

```bash
git add nlp_dict/nlp_dict.toml
git commit -m "Update NLP dictionary for production"
```

### 2. 环境分离

为不同环境使用不同配置：

```bash
# 开发环境
export NLP_DICT_CONFIG=nlp_dict/dev.toml

# 生产环境
export NLP_DICT_CONFIG=nlp_dict/prod.toml
```

### 3. 定期更新

根据日志分析结果定期更新词典：

1. 收集未能正确识别的词汇
2. 分析词性和类别
3. 添加到相应词典
4. 运行测试验证

### 4. 备份配置

```bash
# 备份当前配置
cp nlp_dict/nlp_dict.toml nlp_dict/nlp_dict.toml.bak

# 恢复备份
cp nlp_dict/nlp_dict.toml.bak nlp_dict/nlp_dict.toml
```

## 相关文档

- [extract_main_word 使用指南](../../docs/usage/oml/extract_main_word.md)
- [extract_subject_object 使用指南](../../docs/usage/oml/extract_subject_object.md)
- [OML 管道函数参考](../../docs/usage/oml/pipe_functions.md)

## 技术实现

- **加载器**：`crates/wp-oml/src/core/evaluator/transform/pipe/nlp_dict_loader.rs`
- **使用代码**：`crates/wp-oml/src/core/evaluator/transform/pipe/extract_word.rs`
- **默认配置**：`crates/wp-oml/nlp_dict/nlp_dict.toml`

---

更新时间：2026-02-01
