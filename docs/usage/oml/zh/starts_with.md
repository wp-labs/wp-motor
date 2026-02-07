# starts_with 函数使用指南

## 简介

`starts_with` 是一个 OML pipe 函数，用于检查字符串字段是否以指定的前缀开始。

## 语法

```oml
field_name = pipe take(source_field) | starts_with('prefix');
```

## 参数

- `prefix`: 字符串类型，要检查的前缀（需使用引号）

## 行为

- 如果字段值以指定前缀开始，字段保持不变并继续传递
- 如果字段值不以指定前缀开始，字段转换为 **ignore 类型**
- 如果字段不是字符串类型，字段转换为 **ignore 类型**
- 前缀匹配是大小写敏感的

## 使用场景

### 场景 1: 过滤 HTTPS URL

```oml
name : filter_secure_urls
---
secure_url = pipe take(url) | starts_with('https://');
```

**输入:** `url = "https://example.com"`
**输出:** `secure_url = "https://example.com"` (Chars类型)

**输入:** `url = "http://example.com"`
**输出:** `secure_url = (ignore)`

### 场景 2: 提取 API 路径

```oml
name : extract_api_path
---
api_path = pipe take(request_path) | starts_with('/api/v1/');
```

**输入:** `request_path = "/api/v1/users"`
**输出:** `api_path = "/api/v1/users"` (Chars类型)

**输入:** `request_path = "/admin/users"`
**输出:** `api_path = (ignore)`

### 场景 3: 分类日志级别

```oml
name : classify_error_logs
---
error_message = pipe take(log_message) | starts_with('ERROR');
warning_message = pipe take(log_message) | starts_with('WARN');
```

**输入:** `log_message = "ERROR: Connection failed"`
**输出:**
- `error_message = "ERROR: Connection failed"` (Chars类型)
- `warning_message = (ignore)`

### 场景 4: 与 map_to 组合使用

```oml
name : classify_secure_requests
---
# 如果是 HTTPS，标记为安全
is_secure = pipe take(url) | starts_with('https://') | map_to(true);

# 如果是 HTTPS，设置安全级别
security_level = pipe take(url) | starts_with('https://') | map_to(3);
```

**输入:** `url = "https://api.example.com"`
**输出:**
- `is_secure = true` (Bool类型)
- `security_level = 3` (Digit类型)

**输入:** `url = "http://api.example.com"`
**输出:**
- `is_secure = (ignore)`
- `security_level = (ignore)`

### 场景 5: 多条件过滤

```oml
name : extract_specific_prefix
---
# 提取特定前缀的字段
https_url = pipe take(url) | starts_with('https://');
ftp_url = pipe take(url) | starts_with('ftp://');
websocket_url = pipe take(url) | starts_with('wss://');
```

根据不同的协议前缀，将 URL 分类到不同的字段中。

### 场景 6: 路径规范化

```oml
name : normalize_paths
---
# 只接受绝对路径
absolute_path = pipe take(file_path) | starts_with('/');

# 只接受相对路径
relative_path = pipe take(file_path) | starts_with('./');
```

## 与其他函数的对比

| 函数 | 检查位置 | 性能 | 用途 |
|------|----------|------|------|
| `starts_with(prefix)` | 字符串开头 | 极快 | 前缀匹配 |
| `regex_match(pattern)` | 任意位置 | 较慢 | 复杂模式匹配 |
| `to_str` | - | 快 | 类型转换 |

## 常见用例

### 1. URL 协议过滤

```oml
name : url_protocol_filter
---
https_only = pipe take(url) | starts_with('https://');
```

### 2. 路径前缀提取

```oml
name : api_path_extract
---
v1_api = pipe take(path) | starts_with('/api/v1/');
v2_api = pipe take(path) | starts_with('/api/v2/');
```

### 3. 日志级别分类

```oml
name : log_level_classify
---
errors = pipe take(message) | starts_with('[ERROR]');
warnings = pipe take(message) | starts_with('[WARN]');
info = pipe take(message) | starts_with('[INFO]');
```

### 4. 文件扩展名检查（配合其他字段）

```oml
name : file_type_check
---
# 注意：这个示例假设 filename 已经被规范化
json_file = pipe take(filename) | starts_with('data_') | map_to('json');
```

## 实现细节

- 定义位置: `crates/wp-oml/src/language/syntax/functions/pipe/other.rs`
- 实现位置: `crates/wp-oml/src/core/evaluator/transform/pipe/other.rs`
- 解析器: `crates/wp-oml/src/parser/pipe_prm.rs`
- 测试: `crates/wp-oml/src/core/evaluator/transform/pipe/other.rs` (tests 模块)

## 注意事项

1. **字符串必须加引号**: `starts_with('https://')` 而非 `starts_with(https://)`
2. **大小写敏感**: `starts_with('HTTP')` 不会匹配 `http://example.com`
3. **ignore 字段传播**: 转换为 ignore 的字段在后续管道函数中会保持 ignore 状态
4. **与 map_to 配合**: 常见模式是先用 `starts_with` 过滤，再用 `map_to` 赋值

## 性能特性

- **O(n) 时间复杂度**: n 为前缀长度
- **零拷贝**: 不修改原始字符串
- **短路优化**: 发现不匹配立即返回

## 相关函数

- `map_to(value)`: 条件赋值，支持多种类型
- `skip_empty`: 跳过空值
- `to_str`: 转换为字符串
- `get(name)`: 从嵌套结构获取字段
