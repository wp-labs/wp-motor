# extract_main_word PipeFun 使用指南

## 概述

`extract_main_word` 是一个基于 jieba-rs 中文分词的 PipeFun，用于从文本中提取第一个主要词（核心词）。它支持中英文混合文本，并针对日志分析场景进行了优化。

## 功能特性

### 1. 智能分词

使用 jieba-rs 对中英文文本统一分词，无需区分语言：

```oml
name: extract_word_example
---
main_word = pipe read(message) | extract_main_word ;
```

### 2. 词性标注筛选

基于词性（POS）标注筛选核心词，优先返回：
- **名词类**：n（普通名词）、nr（人名）、ns（地名）、nt（机构名）、nz（专名）、ng（名词性语素）
- **动词类**：v（动词）、vn（名动词）、vd（副动词）
- **形容词类**：a（形容词）、ad（副形词）、an（名形词）
- **其他**：eng（英文）、m（数词）、x（字符串/代码）

### 3. 停用词过滤

自动过滤常见停用词：
- **中文停用词**：的、了、在、是、我、有、和、就、不、都、一、上、也、很、到、说、要、去、你、会、着、没有、看、好、自己、这
- **英文停用词**：the、a、an、is、are、was、were、be、been、being、of、at、in、to、for、and、or、but

### 4. 日志领域词优先

优先识别并返回日志领域关键词（最高优先级）：
- **日志级别**：error、warn、info、debug、fatal、trace
- **系统相关**：exception、failure、timeout、connection、database、server、client、request、response、login、logout、auth、authentication、permission、access
- **网络相关**：http、https、tcp、udp、ip、port、socket
- **安全相关**：attack、virus、malware、threat、alert、blocked、denied

## 使用示例

### 示例 1：英文文本

```oml
name: english_example
---
# 输入: "hello world test"
main_word = pipe read(message) | extract_main_word ;
# 输出: "hello"
```

### 示例 2：中文文本

```oml
name: chinese_example
---
# 输入: "中文分词测试"
main_word = pipe read(message) | extract_main_word ;
# 输出: "中文" (名词)

# 输入: "我们中出了一个叛徒"
main_word = pipe read(message) | extract_main_word ;
# 输出: "出" (动词)
```

### 示例 3：日志分析（领域词优先）

```oml
name: log_analysis
---
# 输入: "error: connection timeout"
keyword = pipe read(log_message) | extract_main_word ;
# 输出: "error" (领域关键词，优先级最高)

# 输入: "database connection failed"
keyword = pipe read(log_message) | extract_main_word ;
# 输出: "database" (领域关键词)

# 输入: "用户登录失败异常"
keyword = pipe read(log_message) | extract_main_word ;
# 输出: "用户" (名词)
```

### 示例 4：混合文本

```oml
name: mixed_text
---
# 输入: "HTTP请求超时"
keyword = pipe read(message) | extract_main_word ;
# 输出: "HTTP" (英文 + 中文混合)
```

### 示例 5：链式处理

```oml
name: pipe_chain
---
# 从日志中提取主要词，转为小写
keyword = pipe read(log) | extract_main_word | to_str ;

# 提取主要词后进行条件判断
is_error = match keyword {
    "error" => true,
    "exception" => true,
    _ => false
};
```

## 工作原理

### 处理流程

```
输入文本
    ↓
清洗文本（trim）
    ↓
jieba 分词 + 词性标注
    ↓
规则筛选：
  1. 领域关键词？→ 直接返回
  2. 核心词性 + 非停用词？→ 返回
  3. 回退：返回第一个非空词
    ↓
输出结果
```

### 规则优先级

1. **最高优先级**：日志领域关键词（LOG_DOMAIN）
2. **中等优先级**：核心词性（CORE_POS）+ 非停用词（!LOG_STOP）
3. **回退策略**：第一个非空分词结果

## 性能优化

### 全局单例

jieba 分词器使用 `lazy_static!` 实现全局单例，只初始化一次，提高性能：

```rust
lazy_static! {
    static ref JIEBA: Jieba = Jieba::new();
}
```

### 词典缓存

jieba-rs 内部会缓存词典，多次调用分词不会重复加载。

## 边界情况处理

| 输入 | 输出 | 说明 |
|------|------|------|
| 空字符串 `""` | `""` | 返回空字符串 |
| 全空格 `"   "` | `""` | trim 后为空 |
| 全停用词 `"的是在了不"` | `"的"` | 回退到第一个词 |
| 单个词 `"测试"` | `"测试"` | 直接返回 |
| 英文大小写 `"Error Message"` | `"error"` | 识别领域词（不区分大小写） |

## 扩展与定制

### 添加自定义领域词

修改 `LOG_DOMAIN` 集合添加新的领域关键词：

```rust
static ref LOG_DOMAIN: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("error");
    set.insert("custom_keyword");  // 添加自定义词
    // ...
    set
};
```

### 调整停用词

修改 `LOG_STOP` 集合添加或删除停用词：

```rust
static ref LOG_STOP: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("的");
    // set.insert("是");  // 注释掉不需要过滤的词
    // ...
    set
};
```

### 调整核心词性

修改 `CORE_POS` 集合调整关注的词性：

```rust
static ref CORE_POS: HashSet<&'static str> = {
    let mut set = HashSet::new();
    set.insert("n");   // 名词
    set.insert("v");   // 动词
    // 添加或删除词性标签
    set
};
```

## 与其他 PipeFun 组合

### 与 to_str 组合

```oml
result = pipe read(text) | extract_main_word | to_str ;
```

### 与 skip_empty 组合

```oml
result = pipe read(text) | extract_main_word | skip_empty ;
```

### 与 base64_encode 组合

```oml
encoded = pipe read(text) | extract_main_word | base64_encode ;
```

## 注意事项

1. **词性依赖**：分词结果依赖 jieba-rs 的词性标注准确性
2. **领域词更新**：根据实际日志场景定期更新领域关键词列表
3. **性能考虑**：首次调用会初始化 jieba 词典，后续调用性能稳定
4. **编码要求**：输入必须是有效的 UTF-8 字符串

## 相关资源

- [OML PipeFun 开发指南](./oml_pipefun_development_guide.md)
- [jieba-rs GitHub](https://github.com/messense/jieba-rs)
- [jieba-rs 文档](https://docs.rs/jieba-rs)

---

更新时间：2026-01-31
