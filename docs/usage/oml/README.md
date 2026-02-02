# OML (Object Mapping Language) 使用指南

## 概述

OML (Object Mapping Language) 是 WP-Motor 的数据转换语言，用于定义日志数据的提取、转换和映射规则。OML 支持丰富的字段访问器和管道函数（PipeFun），可以进行编码转换、时间处理、NLP 分析等复杂操作。

## 快速开始

### 基本语法

```oml
name : my_model
rule : /path/to/logs/*
---
field_name : type = accessor | pipe_chain ;
```

### 简单示例

```oml
name : simple_example
---
# 直接提取字段
username : chars = take() ;

# 从指定字段读取
message : chars = read(raw_message) ;

# 使用管道函数转换
encoded : chars = pipe read(password) | base64_encode ;

# 字面量赋值
log_level : chars = chars(INFO) ;
timestamp : time = Now::time() ;
```

## 核心概念

### 1. 字段访问器 (Accessor)

- **take()** - 提取当前字段（会移除源数据）
- **take(field_name)** - 从指定字段提取
- **take(option: [f1, f2, f3])** - 从多个候选字段提取（按顺序）
- **read()** - 读取当前字段（不移除源数据）
- **read(field_name)** - 从指定字段读取

### 2. 管道函数 (PipeFun)

管道函数用于对字段进行链式转换，使用 `|` 连接：

```oml
result = pipe read(input) | function1 | function2 | function3 ;
```

### 3. 数据类型

支持的字段类型：
- `chars` - 字符串
- `digit` - 整数
- `float` - 浮点数
- `ip` - IP 地址
- `time` - 时间戳
- `bool` - 布尔值
- `array` - 数组
- `obj` - 对象

## 管道函数分类

### 编码与转义

- [base64_encode](./pipe_functions.md#base64_encode) - Base64 编码
- [base64_decode](./pipe_functions.md#base64_decode) - Base64 解码
- [html_escape](./pipe_functions.md#html_escape) - HTML 转义
- [html_unescape](./pipe_functions.md#html_unescape) - HTML 反转义
- [json_escape](./pipe_functions.md#json_escape) - JSON 转义
- [json_unescape](./pipe_functions.md#json_unescape) - JSON 反转义
- [str_escape](./pipe_functions.md#str_escape) - 字符串转义

### 时间转换

- [Time::to_ts](./pipe_functions.md#time_to_ts) - 转换为 UNIX 时间戳（秒）
- [Time::to_ts_ms](./pipe_functions.md#time_to_ts_ms) - 转换为毫秒时间戳
- [Time::to_ts_us](./pipe_functions.md#time_to_ts_us) - 转换为微秒时间戳
- [Time::to_ts_zone](./pipe_functions.md#time_to_ts_zone) - 按时区转换时间戳

### 数据提取与转换

- [nth](./pipe_functions.md#nth) - 提取数组第 N 个元素
- [get](./pipe_functions.md#get) - 从对象获取字段
- [to_str](./pipe_functions.md#to_str) - 转换为字符串
- [to_json](./pipe_functions.md#to_json) - 转换为 JSON 字符串
- [skip_empty](./pipe_functions.md#skip_empty) - 跳过空值

### 网络与路径

- [url](./pipe_functions.md#url) - URL 解析（domain/host/path/params）
- [path](./pipe_functions.md#path) - 路径解析（name/path）
- [ip4_to_int](./pipe_functions.md#ip4_to_int) - IPv4 转整数

### NLP 文本处理

- [extract_main_word](./extract_main_word.md) - 提取文本主要词汇（中英文混合）
- [extract_subject_object](./extract_subject_object.md) - 提取日志主客体结构

## 常用示例

### 示例 1：日志编码转换

```oml
name : log_decode
---
# Base64 编码的日志消息
raw_message : chars = take() ;

# 解码为 UTF-8
decoded_message = pipe read(raw_message) | base64_decode() ;

# 解码为 GBK 编码
decoded_gbk = pipe read(raw_message) | base64_decode(Gbk) ;
```

### 示例 2：时间戳标准化

```oml
name : timestamp_normalize
---
occur_time : time = take() ;

# 转换为 UNIX 秒级时间戳
ts_sec = pipe read(occur_time) | Time::to_ts ;

# 转换为毫秒时间戳
ts_ms = pipe read(occur_time) | Time::to_ts_ms ;

# 东8区时间戳
ts_utc8 = pipe read(occur_time) | Time::to_ts_zone(8, s) ;
```

### 示例 3：URL 解析

```oml
name : url_parse
---
http_url : chars = take() ;

# 提取域名
domain = pipe read(http_url) | url(domain) ;

# 提取主机（包含端口）
host = pipe read(http_url) | url(host) ;

# 提取路径
uri_path = pipe read(http_url) | url(path) ;

# 提取查询参数
query_params = pipe read(http_url) | url(params) ;
```

### 示例 4：NLP 日志分析

```oml
name : log_nlp_analysis
---
log_message : chars = take() ;

# 提取主要关键词
keyword = pipe read(log_message) | extract_main_word ;

# 提取主客体结构
structure = pipe read(log_message) | extract_subject_object ;

# 分别提取结构字段
subject = structure.subject ;   # 主体
action = structure.action ;     # 动作
object = structure.object ;     # 对象
status = structure.status ;     # 状态
```

### 示例 5：链式转换

```oml
name : chain_transform
---
raw_data : chars = take() ;

# 多级转换：解码 → HTML 转义 → JSON 转义
processed = pipe read(raw_data)
    | base64_decode()
    | html_escape
    | json_escape ;
```

### 示例 6：数组操作

```oml
name : array_operations
---
# 收集多个字段到数组
items : array = collect take(keys: [field1, field2, field3]) ;

# 提取第一个元素
first = pipe read(items) | nth(0) ;

# 转换为 JSON 字符串
items_json = pipe read(items) | to_json ;
```

## 高级特性

### 条件匹配

```oml
name : conditional
---
log_level : chars = take() ;

severity = match log_level {
    "ERROR" => digit(3),
    "WARN" => digit(2),
    "INFO" => digit(1),
    _ => digit(0)
};
```

### 动态时间

```oml
name : dynamic_time
---
# 获取当前时间
current_time : time = Now::time() ;

# 获取当前日期（字符串格式）
current_date : chars = Now::date() ;
```

### 字段收集

```oml
name : field_collection
---
# 收集所有匹配的字段到数组
all_errors : array = collect take(keys: [error*, err*]) ;

# 收集对象字段
user_info : obj = collect take(keys: [user_id, user_name, user_email]) ;
```

## 性能优化建议

### 1. 避免重复读取

```oml
# ❌ 不推荐 - 重复读取
field1 = pipe read(source) | function1 ;
field2 = pipe read(source) | function2 ;

# ✅ 推荐 - 读取一次，分支处理
temp = read(source) ;
field1 = pipe read(temp) | function1 ;
field2 = pipe read(temp) | function2 ;
```

### 2. 合并管道操作

```oml
# ❌ 不推荐 - 多次管道调用
temp1 = pipe read(input) | base64_decode() ;
temp2 = pipe read(temp1) | html_unescape ;
result = pipe read(temp2) | json_unescape ;

# ✅ 推荐 - 链式调用
result = pipe read(input) | base64_decode() | html_unescape | json_unescape ;
```

### 3. 使用 skip_empty 过滤

```oml
# 跳过空值字段，避免后续处理
valid_data = pipe read(input) | skip_empty | process_function ;
```

## 调试技巧

### 1. 逐步验证

```oml
# 分步调试管道
step1 = pipe read(input) | base64_decode() ;
step2 = pipe read(step1) | html_unescape ;
final = pipe read(step2) | json_unescape ;
```

### 2. 保留中间结果

```oml
# 保留每个转换步骤的结果用于调试
raw : chars = take() ;
decoded : chars = pipe read(raw) | base64_decode() ;
unescaped : chars = pipe read(decoded) | html_unescape ;
final : chars = pipe read(unescaped) | json_unescape ;
```

### 3. 使用 to_str 查看结果

```oml
# 将复杂类型转换为字符串便于查看
debug_output = pipe read(complex_field) | to_json | to_str ;
```

## 错误处理

### 类型不匹配

当管道函数接收到不匹配的类型时，通常会返回原值：

```oml
# ip4_to_int 只处理 IPv4，IPv6 会返回原值
ip_int = pipe read(ip_field) | ip4_to_int ;
```

### 空值处理

使用 `skip_empty` 过滤空值：

```oml
# 跳过空值，避免后续函数处理空数据
result = pipe read(field) | skip_empty | process ;
```

## 文档索引

- [管道函数完整参考](./pipe_functions.md) - 所有 PipeFun 的详细文档
- [extract_main_word 使用指南](./extract_main_word.md) - NLP 关键词提取
- [extract_subject_object 使用指南](./extract_subject_object.md) - 日志主客体分析
- [OML PipeFun 开发指南](../../guide/oml_pipefun_development_guide.md) - 如何开发新的管道函数

## 相关资源

- **WPL 字段引用**: [field_reference.md](../wpl/field_reference.md)
- **源代码位置**:
  - Pipe 函数定义: `crates/wp-oml/src/language/syntax/functions/pipe/`
  - Pipe 函数实现: `crates/wp-oml/src/core/evaluator/transform/pipe/`
  - 测试用例: `crates/wp-oml/tests/test_case.rs`

---

更新时间：2026-02-01
