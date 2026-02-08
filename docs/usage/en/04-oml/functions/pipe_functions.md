# OML 管道函数完整参考

本文档列出了 OML 支持的所有管道函数（PipeFun），包括详细说明、参数、示例和注意事项。

## 目录

- [编码与转义](#编码与转义)
- [时间转换](#时间转换)
- [数据提取与转换](#数据提取与转换)
- [网络与路径](#网络与路径)
- [NLP 文本处理](#nlp-文本处理)

---

## 编码与转义

### base64_encode

将字符串编码为 Base64 格式。

**语法**
```oml
result = pipe read(field) | base64_encode ;
```

**输入类型**: `chars` (字符串)
**输出类型**: `chars` (Base64 编码字符串)

**示例**
```oml
name : example_base64_encode
---
password : chars = take() ;
encoded_password = pipe read(password) | base64_encode ;

# 输入: "hello123"
# 输出: "aGVsbG8xMjM="
```

**实现位置**
- 定义: `crates/wp-oml/src/language/syntax/functions/pipe/base64.rs`
- 实现: `crates/wp-oml/src/core/evaluator/transform/pipe/base64.rs`

---

### base64_decode

将 Base64 编码的字符串解码为指定编码的文本。

**语法**
```oml
# 默认使用 UTF-8 编码
result = pipe read(field) | base64_decode() ;

# 指定编码格式
result = pipe read(field) | base64_decode(Gbk) ;
```

**输入类型**: `chars` (Base64 编码字符串)
**输出类型**: `chars` (解码后的字符串)

**参数**
- `encoding` (可选) - 目标编码格式，支持：
  - `Utf8` (默认)
  - `Utf16le` - UTF-16 小端
  - `Utf16be` - UTF-16 大端
  - `Gbk` - GBK 编码
  - `Gb18030` - GB18030 编码
  - `Big5` - Big5 繁体中文
  - `ShiftJIS` - 日文编码
  - `EucJp` - 日文 EUC-JP
  - `EucKr` - 韩文 EUC-KR
  - `Latin1` - ISO-8859-1

**示例**
```oml
name : example_base64_decode
---
encoded : chars = take() ;

# UTF-8 解码
decoded_utf8 = pipe read(encoded) | base64_decode() ;

# GBK 解码
decoded_gbk = pipe read(encoded) | base64_decode(Gbk) ;

# UTF-16LE 解码
decoded_utf16 = pipe read(encoded) | base64_decode(Utf16le) ;

# 输入: "aGVsbG8xMjM="
# 输出 (UTF-8): "hello123"
```

**注意事项**
- 如果输入不是有效的 Base64 字符串，返回原值
- 编码转换失败时返回原值

---

### html_escape

将字符串中的 HTML 特殊字符转义为 HTML 实体。

**语法**
```oml
result = pipe read(field) | html_escape ;
```

**输入类型**: `chars` (字符串)
**输出类型**: `chars` (HTML 转义后的字符串)

**转义规则**
- `&` → `&amp;`
- `<` → `&lt;`
- `>` → `&gt;`
- `"` → `&quot;`
- `'` → `&#x27;`

**示例**
```oml
name : example_html_escape
---
raw_html : chars = take() ;
escaped = pipe read(raw_html) | html_escape ;

# 输入: "<div>Hello & World</div>"
# 输出: "&lt;div&gt;Hello &amp; World&lt;/div&gt;"
```

---

### html_unescape

将 HTML 实体转换回原始字符。

**语法**
```oml
result = pipe read(field) | html_unescape ;
```

**输入类型**: `chars` (HTML 转义字符串)
**输出类型**: `chars` (还原后的字符串)

**示例**
```oml
name : example_html_unescape
---
escaped : chars = take() ;
unescaped = pipe read(escaped) | html_unescape ;

# 输入: "&lt;div&gt;Hello &amp; World&lt;/div&gt;"
# 输出: "<div>Hello & World</div>"

# 往返转换
round_trip = pipe read(raw) | html_escape | html_unescape ;
```

---

### json_escape

将字符串中的 JSON 特殊字符转义。

**语法**
```oml
result = pipe read(field) | json_escape ;
```

**输入类型**: `chars` (字符串)
**输出类型**: `chars` (JSON 转义后的字符串)

**转义规则**
- `"` → `\"`
- `\` → `\\`
- `/` → `\/`
- 换行 → `\n`
- 制表符 → `\t`
- 回车 → `\r`
- 换页 → `\f`
- 退格 → `\b`

**示例**
```oml
name : example_json_escape
---
text : chars = take() ;
json_escaped = pipe read(text) | json_escape ;

# 输入: "Hello\nWorld"
# 输出: "Hello\\nWorld"
```

---

### json_unescape

将 JSON 转义字符还原为原始字符。

**语法**
```oml
result = pipe read(field) | json_unescape ;
```

**输入类型**: `chars` (JSON 转义字符串)
**输出类型**: `chars` (还原后的字符串)

**示例**
```oml
name : example_json_unescape
---
escaped : chars = take() ;
unescaped = pipe read(escaped) | json_unescape ;

# 输入: "Hello\\nWorld"
# 输出: "Hello\nWorld"
```

---

### str_escape

字符串转义函数，处理特殊字符。

**语法**
```oml
result = pipe read(field) | str_escape ;
```

**输入类型**: `chars` (字符串)
**输出类型**: `chars` (转义后的字符串)

**示例**
```oml
name : example_str_escape
---
raw : chars = take() ;
escaped = pipe read(raw) | str_escape ;
```

---

## 时间转换

### Time::to_ts

将时间字段转换为 UNIX 时间戳（秒级）。

**语法**
```oml
result = pipe read(time_field) | Time::to_ts ;
```

**输入类型**: `time` (时间类型)
**输出类型**: `digit` (整数时间戳，单位：秒)

**示例**
```oml
name : example_time_to_ts
---
occur_time : time = take() ;
timestamp = pipe read(occur_time) | Time::to_ts ;

# 输入: 2000-10-10 00:00:00
# 输出: 971107200 (秒级时间戳)
```

**用途**
- 时间标准化
- 数据库存储
- 时间计算和比较

---

### Time::to_ts_ms

将时间字段转换为毫秒级时间戳。

**语法**
```oml
result = pipe read(time_field) | Time::to_ts_ms ;
```

**输入类型**: `time` (时间类型)
**输出类型**: `digit` (整数时间戳，单位：毫秒)

**示例**
```oml
name : example_time_to_ts_ms
---
occur_time : time = take() ;
timestamp_ms = pipe read(occur_time) | Time::to_ts_ms ;

# 输入: 2000-10-10 00:00:00
# 输出: 971107200000 (毫秒级时间戳)
```

---

### Time::to_ts_us

将时间字段转换为微秒级时间戳。

**语法**
```oml
result = pipe read(time_field) | Time::to_ts_us ;
```

**输入类型**: `time` (时间类型)
**输出类型**: `digit` (整数时间戳，单位：微秒)

**示例**
```oml
name : example_time_to_ts_us
---
occur_time : time = take() ;
timestamp_us = pipe read(occur_time) | Time::to_ts_us ;

# 输入: 2000-10-10 00:00:00
# 输出: 971107200000000 (微秒级时间戳)
```

---

### Time::to_ts_zone

按指定时区转换时间为时间戳。

**语法**
```oml
# 东8区，秒级时间戳
result = pipe read(time_field) | Time::to_ts_zone(8, s) ;

# 西5区，毫秒级时间戳
result = pipe read(time_field) | Time::to_ts_zone(-5, ms) ;

# 零时区，微秒级时间戳
result = pipe read(time_field) | Time::to_ts_zone(0, us) ;
```

**输入类型**: `time` (时间类型)
**输出类型**: `digit` (整数时间戳)

**参数**
- `zone` - 时区偏移量（整数）
  - 正数：东时区（如东8区 = 8）
  - 负数：西时区（如西5区 = -5）
  - 零：UTC 时区
- `unit` - 时间戳单位
  - `s` - 秒
  - `ms` - 毫秒
  - `us` - 微秒

**示例**
```oml
name : example_time_to_ts_zone
---
occur_time : time = take() ;

# 东8区（北京时间）秒级时间戳
ts_utc8_s = pipe read(occur_time) | Time::to_ts_zone(8, s) ;

# 东9区（日本时间）毫秒级时间戳
ts_utc9_ms = pipe read(occur_time) | Time::to_ts_zone(9, ms) ;

# 西5区（美东时间）微秒级时间戳
ts_utc_minus5_us = pipe read(occur_time) | Time::to_ts_zone(-5, us) ;

# 零时区（UTC）
ts_utc = pipe read(occur_time) | Time::to_ts_zone(0, s) ;
```

**注意事项**
- 时区范围：-12 到 +14
- 时间戳计算会自动调整时区偏移

---

## 数据提取与转换

### nth

从数组中提取第 N 个元素（从 0 开始索引）。

**语法**
```oml
result = pipe read(array_field) | nth(0) ;  # 第一个元素
result = pipe read(array_field) | nth(2) ;  # 第三个元素
```

**输入类型**: `array` (数组)
**输出类型**: 数组元素的类型

**参数**
- `index` - 数组索引（从 0 开始）

**示例**
```oml
name : example_nth
---
# 收集字段到数组
items : array = collect take(keys: [field1, field2, field3, field4]) ;

# 提取第一个元素
first = pipe read(items) | nth(0) ;

# 提取第三个元素
third = pipe read(items) | nth(2) ;

# 输入: ["hello1", "hello2", "hello3", "hello4"]
# nth(0) → "hello1"
# nth(2) → "hello3"
```

**注意事项**
- 索引超出范围时返回原值
- 负数索引不支持

---

### get

从对象中获取指定字段的值。

**语法**
```oml
result = pipe read(object_field) | get(field_name) ;
```

**输入类型**: `obj` (对象)
**输出类型**: 字段值的类型

**参数**
- `field_name` - 要获取的字段名

**示例**
```oml
name : example_get
---
user : obj = take() ;

# 从对象中获取字段
username = pipe read(user) | get(name) ;
user_id = pipe read(user) | get(id) ;

# 假设 user = { "name": "Alice", "id": 123 }
# get(name) → "Alice"
# get(id) → 123
```

---

### to_str

将字段值转换为字符串表示。

**语法**
```oml
result = pipe read(field) | to_str ;
```

**输入类型**: 任意类型
**输出类型**: `chars` (字符串)

**示例**
```oml
name : example_to_str
---
num : digit = take() ;
num_str = pipe read(num) | to_str ;

# 输入: 12345 (整数)
# 输出: "12345" (字符串)

ip : ip = take() ;
ip_str = pipe read(ip) | to_str ;

# 输入: 192.168.1.1
# 输出: "192.168.1.1"
```

**用途**
- 类型转换
- 字符串拼接准备
- 调试输出

---

### to_json

将字段值转换为 JSON 字符串。

**语法**
```oml
result = pipe read(field) | to_json ;
```

**输入类型**: 任意类型（特别是 `array`, `obj`）
**输出类型**: `chars` (JSON 字符串)

**示例**
```oml
name : example_to_json
---
items : array = collect take(keys: [f1, f2, f3]) ;
items_json = pipe read(items) | to_json ;

# 输入: ["hello1", "hello2", "hello3"]
# 输出: "[\"hello1\",\"hello2\",\"hello3\"]"

user : obj = take() ;
user_json = pipe read(user) | to_json ;

# 输入: { "name": "Alice", "age": 30 }
# 输出: "{\"name\":\"Alice\",\"age\":30}"
```

**用途**
- 序列化数据
- API 数据传输
- 日志记录

---

### skip_empty

跳过空值字段，只传递非空值。

**语法**
```oml
result = pipe read(field) | skip_empty ;
```

**输入类型**: 任意类型
**输出类型**: 原类型（或空）

**判断规则**
- 空字符串 `""` → 跳过
- 空数组 `[]` → 跳过
- 空对象 `{}` → 跳过
- `null` → 跳过

**示例**
```oml
name : example_skip_empty
---
message : chars = take() ;

# 只处理非空消息
valid_message = pipe read(message) | skip_empty | process_function ;

# 如果 message 为空，valid_message 将不会被创建
```

**用途**
- 数据清洗
- 避免处理无效数据
- 条件处理

---

## 网络与路径

### url

解析 URL 并提取指定部分。

**语法**
```oml
result = pipe read(url_field) | url(domain) ;
result = pipe read(url_field) | url(host) ;
result = pipe read(url_field) | url(path) ;
result = pipe read(url_field) | url(params) ;
```

**输入类型**: `chars` (URL 字符串)
**输出类型**: `chars` (提取的部分)

**参数**
- `type` - 提取类型：
  - `domain` - 域名（不含端口）
  - `host` - 主机（包含端口）
  - `uri` - 完整 URI
  - `path` - 路径部分
  - `params` - 查询参数

**示例**
```oml
name : example_url
---
http_url : chars = take() ;

# 提取域名
domain = pipe read(http_url) | url(domain) ;

# 提取主机（包含端口）
host = pipe read(http_url) | url(host) ;

# 提取路径
path = pipe read(http_url) | url(path) ;

# 提取查询参数
params = pipe read(http_url) | url(params) ;

# 输入: "https://example.com:8080/path/to/page?id=123&name=test"
# url(domain) → "example.com"
# url(host) → "example.com:8080"
# url(path) → "/path/to/page"
# url(params) → "id=123&name=test"
```

**注意事项**
- 无效 URL 返回原值或空字符串
- 不同部分缺失时返回空字符串

---

### path

解析文件路径并提取部分。

**语法**
```oml
result = pipe read(path_field) | path(name) ;
result = pipe read(path_field) | path(path) ;
```

**输入类型**: `chars` (路径字符串)
**输出类型**: `chars` (提取的部分)

**参数**
- `type` - 提取类型：
  - `name` - 文件名
  - `path` - 目录路径

**示例**
```oml
name : example_path
---
file_path : chars = take() ;

# 提取文件名
filename = pipe read(file_path) | path(name) ;

# 提取目录路径
directory = pipe read(file_path) | path(path) ;

# 输入: "/var/log/system.log"
# path(name) → "system.log"
# path(path) → "/var/log"

# Windows 路径
# 输入: "C:\\Users\\Admin\\file.txt"
# path(name) → "file.txt"
# path(path) → "C:\\Users\\Admin"
```

---

### ip4_to_int

将 IPv4 地址转换为 32 位整数表示。

**语法**
```oml
result = pipe read(ip_field) | ip4_to_int ;
```

**输入类型**: `ip` (IPv4 地址)
**输出类型**: `digit` (整数)

**示例**
```oml
name : example_ip4_to_int
---
src_ip : ip = take() ;
ip_int = pipe read(src_ip) | ip4_to_int ;

# 输入: 127.0.0.1
# 输出: 2130706433

# 输入: 192.168.1.1
# 输出: 3232235777
```

**计算公式**
```
IP = a.b.c.d
整数 = (a × 256³) + (b × 256²) + (c × 256) + d
```

**注意事项**
- 只支持 IPv4 地址
- IPv6 地址会返回原值
- 用于 IP 范围比较、地理位置查询等

---

## NLP 文本处理

### extract_main_word

从文本中提取第一个主要关键词，支持中英文混合。

详细文档请参考：[extract_main_word 使用指南](./extract_main_word.md)

**语法**
```oml
result = pipe read(text_field) | extract_main_word ;
```

**输入类型**: `chars` (文本字符串)
**输出类型**: `chars` (提取的关键词)

**快速示例**
```oml
name : example_extract_main_word
---
log_message : chars = take() ;
keyword = pipe read(log_message) | extract_main_word ;

# 输入: "error: connection timeout"
# 输出: "error" (领域关键词优先)

# 输入: "用户登录失败"
# 输出: "用户" (中文核心词)
```

**特性**
- 智能中英文分词
- 词性过滤（名词、动词、形容词优先）
- 停用词过滤
- 日志领域词优先识别

---

### extract_subject_object

提取日志文本的主客体结构（Subject-Action-Object-Status）。

详细文档请参考：[extract_subject_object 使用指南](./extract_subject_object.md)

**语法**
```oml
result = pipe read(log_field) | extract_subject_object ;
```

**输入类型**: `chars` (日志文本)
**输出类型**: `obj` (对象，包含 subject/action/object/status 字段)

**快速示例**
```oml
name : example_extract_subject_object
---
log : chars = take() ;
structure = pipe read(log) | extract_subject_object ;

# 提取各个字段
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

**特性**
- 自动识别主体、动作、对象、状态
- 支持中英文混合日志
- 基于词性标注和词缀规则
- 可选 debug 模式输出详细分析信息

---

## 函数组合示例

### 示例 1：完整的编码转换链

```oml
name : encode_chain
---
raw_data : chars = take() ;

# 多级编码：原始 → Base64 → HTML 转义
encoded = pipe read(raw_data)
    | base64_encode
    | html_escape ;

# 多级解码：HTML 反转义 → Base64 解码
decoded = pipe read(encoded)
    | html_unescape
    | base64_decode() ;
```

### 示例 2：时间标准化处理

```oml
name : time_normalize
---
timestamp : time = take() ;

# 转换为不同格式
ts_sec = pipe read(timestamp) | Time::to_ts ;
ts_ms = pipe read(timestamp) | Time::to_ts_ms ;
ts_utc8 = pipe read(timestamp) | Time::to_ts_zone(8, s) ;

# 转换为字符串用于显示
ts_str = pipe read(ts_sec) | to_str ;
```

### 示例 3：URL 完整解析

```oml
name : url_full_parse
---
request_url : chars = take() ;

# 提取所有部分
url_domain = pipe read(request_url) | url(domain) ;
url_host = pipe read(request_url) | url(host) ;
url_path = pipe read(request_url) | url(path) ;
url_params = pipe read(request_url) | url(params) ;

# 组合判断
is_https = match url_host {
    "*.443" => true,
    _ => false
};
```

### 示例 4：NLP 日志分析流水线

```oml
name : log_nlp_pipeline
---
log_message : chars = take() ;

# 提取关键词
keyword = pipe read(log_message) | extract_main_word ;

# 提取主客体结构
structure = pipe read(log_message) | extract_subject_object ;

# 判断是否为错误日志
is_error = match keyword {
    "error" => true,
    "exception" => true,
    "failed" => true,
    _ => false
};

# 提取动作和状态
action = structure.action ;
status = structure.status ;
```

---

## 性能参考

| 函数类型 | 性能等级 | 说明 |
|---------|---------|------|
| 编码转换 | 中等 | Base64、HTML 转义等涉及字符串分配 |
| 时间转换 | 高 | 数值计算，性能优秀 |
| 数据提取 | 高 | 直接访问，几乎无开销 |
| URL/路径解析 | 中等 | 需要解析和提取 |
| NLP 处理 | 低 | 涉及分词和词性标注，相对耗时 |

**优化建议**
- 避免重复调用 NLP 函数
- 合并多个编码转换为链式调用
- 对于高频路径，考虑缓存结果

---

## 开发新的管道函数

如果需要开发新的管道函数，请参考：
- [OML PipeFun 开发指南](../../guide/oml_pipefun_development_guide.md)

**快速清单**
1. 在 `language/syntax/functions/pipe/` 定义结构体
2. 在 `core/evaluator/transform/pipe/` 实现 `ValueProcessor`
3. 在 `parser/pipe_prm.rs` 添加解析器
4. 编写测试用例
5. 更新文档

---

更新时间：2026-02-01
