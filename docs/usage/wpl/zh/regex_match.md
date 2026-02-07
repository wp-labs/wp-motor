# regex_match 函数使用指南

## 概述

`regex_match` 是 WPL (WP Language) 中的正则表达式匹配函数，用于检查日志字段的字符串内容是否匹配指定的正则表达式模式。使用 Rust 的 regex 引擎，支持完整的正则表达式语法。

## 快速开始

### 基本语法

```wpl
regex_match('pattern')
```

- **pattern**: 正则表达式模式（推荐使用**单引号**）
- 匹配成功返回 Ok，失败返回 Err

### 简单示例

```wpl
# 匹配纯数字
regex_match('^\d+$')

# 匹配邮箱格式
regex_match('^\w+@\w+\.\w+$')

# 匹配 IP 地址
regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$')

# 匹配 HTTP 方法
regex_match('^(GET|POST|PUT|DELETE)$')
```

## 重要提示：引号使用

### 推荐：使用单引号（原始字符串）

```wpl
# ✅ 推荐：单引号不处理转义，适合正则表达式
regex_match('^\d+$')           # \d 保持原样
regex_match('^\w+@\w+\.\w+$')  # \w, \. 保持原样
regex_match('^[A-Z]+\d+$')     # 完美工作
```

### 避免：双引号会导致转义问题

```wpl
# ❌ 错误：双引号会尝试转义 \d
regex_match("^\d+$")  # 解析失败！\d 不是有效的转义序列

# ❌ 错误：\w 也会失败
regex_match("^\w+$")  # 解析失败！
```

**原因**：WPL 的双引号字符串解析器只支持 `\"`, `\\`, `\n`, `\t` 这几种转义字符，而正则表达式中的 `\d`, `\w`, `\s` 等会导致解析错误。

## 正则表达式语法

`regex_match` 使用 Rust regex 引擎，支持以下特性：

### 1. 基本匹配

```wpl
# 字面字符
regex_match('hello')          # 匹配 "hello"
regex_match('error')          # 匹配 "error"
```

### 2. 字符类

```wpl
# 数字
regex_match('\d')             # 匹配任意数字 [0-9]
regex_match('\d+')            # 匹配一个或多个数字
regex_match('\d{3}')          # 匹配恰好3个数字

# 字母和数字
regex_match('\w')             # 匹配 [a-zA-Z0-9_]
regex_match('\w+')            # 匹配一个或多个单词字符

# 空白字符
regex_match('\s')             # 匹配空格、制表符、换行符
regex_match('\s+')            # 匹配一个或多个空白字符

# 自定义字符类
regex_match('[a-z]')          # 匹配小写字母
regex_match('[A-Z]')          # 匹配大写字母
regex_match('[0-9]')          # 匹配数字
regex_match('[a-zA-Z0-9]')    # 匹配字母或数字
```

### 3. 量词

```wpl
# * - 0次或多次
regex_match('a*')             # 匹配 "", "a", "aa", "aaa"...

# + - 1次或多次
regex_match('a+')             # 匹配 "a", "aa", "aaa"... (不匹配空串)

# ? - 0次或1次
regex_match('colou?r')        # 匹配 "color" 或 "colour"

# {n} - 恰好n次
regex_match('\d{4}')          # 匹配4位数字

# {n,m} - n到m次
regex_match('\d{2,4}')        # 匹配2到4位数字

# {n,} - 至少n次
regex_match('\d{3,}')         # 匹配3位或更多数字
```

### 4. 锚点

```wpl
# ^ - 字符串开始
regex_match('^\d+')           # 必须以数字开头

# $ - 字符串结束
regex_match('\d+$')           # 必须以数字结尾

# ^...$ - 完全匹配
regex_match('^\d+$')          # 整个字符串必须是数字
```

### 5. 分组和选择

```wpl
# (...) - 分组
regex_match('(ab)+')          # 匹配 "ab", "abab", "ababab"...

# | - 选择（或）
regex_match('cat|dog')        # 匹配 "cat" 或 "dog"
regex_match('^(GET|POST)$')   # 匹配 "GET" 或 "POST"

# (?:...) - 非捕获分组
regex_match('(?:ab)+')        # 与 (ab)+ 功能相同，但不捕获
```

### 6. 特殊字符转义

```wpl
# 转义元字符
regex_match('\.')             # 匹配点号 .
regex_match('\[')             # 匹配左方括号 [
regex_match('\(')             # 匹配左括号 (
regex_match('\$')             # 匹配美元符号 $
regex_match('\*')             # 匹配星号 *
```

### 7. 标志

```wpl
# (?i) - 大小写不敏感
regex_match('(?i)error')      # 匹配 "error", "ERROR", "Error"

# (?m) - 多行模式
regex_match('(?m)^line')      # ^ 匹配每行开始

# (?s) - 单行模式（. 匹配换行符）
regex_match('(?s).*')         # . 可以匹配换行符
```

## 实际应用场景

### 场景 1：日志级别匹配

```wpl
rule log_level_filter {
    # 选择日志消息字段
    | take(message)

    # 匹配包含 ERROR 或 FATAL 的消息（大小写不敏感）
    | regex_match('(?i)(error|fatal)')
}

# 示例数据：
# message: "Error occurred"     → ✅ 匹配
# message: "FATAL exception"    → ✅ 匹配
# message: "Warning message"    → ❌ 不匹配
```

### 场景 2：邮箱地址验证

```wpl
rule email_validation {
    # 选择邮箱字段
    | take(email)

    # 验证邮箱格式
    | regex_match('^\w+(\.\w+)*@\w+(\.\w+)+$')
}

# 示例数据：
# email: "user@example.com"           → ✅ 匹配
# email: "john.doe@company.co.uk"     → ✅ 匹配
# email: "invalid-email"              → ❌ 不匹配
# email: "@example.com"               → ❌ 不匹配
```

### 场景 3：IP 地址匹配

```wpl
rule ip_address_filter {
    # 选择 IP 地址字段
    | take(client_ip)

    # 匹配内网 IP（192.168.x.x）
    | regex_match('^192\.168\.\d{1,3}\.\d{1,3}$')
}

# 示例数据：
# client_ip: "192.168.1.1"    → ✅ 匹配
# client_ip: "192.168.0.100"  → ✅ 匹配
# client_ip: "10.0.0.1"       → ❌ 不匹配
# client_ip: "8.8.8.8"        → ❌ 不匹配
```

### 场景 4：URL 路径过滤

```wpl
rule api_endpoint_filter {
    # 选择请求路径
    | take(path)

    # 匹配 API 端点（/api/v1/...）
    | regex_match('^/api/v\d+/')
}

# 示例数据：
# path: "/api/v1/users"         → ✅ 匹配
# path: "/api/v2/products"      → ✅ 匹配
# path: "/static/image.png"     → ❌ 不匹配
```

### 场景 5：时间戳格式验证

```wpl
rule timestamp_validation {
    # 选择时间戳字段
    | take(timestamp)

    # 匹配 ISO 8601 格式（YYYY-MM-DD HH:MM:SS）
    | regex_match('^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$')
}

# 示例数据：
# timestamp: "2024-01-29 15:30:45"  → ✅ 匹配
# timestamp: "2024-1-9 5:3:5"       → ❌ 不匹配（缺少前导零）
# timestamp: "29/01/2024 15:30"     → ❌ 不匹配（格式不同）
```

### 场景 6：HTTP 方法验证

```wpl
rule http_method_check {
    # 选择 HTTP 方法字段
    | take(method)

    # 只接受安全的 HTTP 方法
    | regex_match('^(GET|HEAD|OPTIONS)$')
}

# 示例数据：
# method: "GET"     → ✅ 匹配
# method: "HEAD"    → ✅ 匹配
# method: "POST"    → ❌ 不匹配
# method: "DELETE"  → ❌ 不匹配
```

### 场景 7：版本号匹配

```wpl
rule version_check {
    # 选择版本字段
    | take(version)

    # 匹配语义化版本号（如 1.2.3）
    | regex_match('^\d+\.\d+\.\d+$')
}

# 示例数据：
# version: "1.0.0"     → ✅ 匹配
# version: "2.10.5"    → ✅ 匹配
# version: "1.0"       → ❌ 不匹配（缺少补丁版本）
# version: "v1.2.3"    → ❌ 不匹配（有前缀）
```

### 场景 8：SQL 注入检测

```wpl
rule sql_injection_detection {
    # 选择用户输入字段
    | take(user_input)

    # 检测常见的 SQL 注入模式
    | regex_match('(?i)(union|select|insert|update|delete|drop|;|--|\|)')
}

# 示例数据：
# user_input: "SELECT * FROM users"  → ✅ 匹配（检测到）
# user_input: "'; DROP TABLE --"     → ✅ 匹配（检测到）
# user_input: "normal search query"  → ❌ 不匹配（安全）
```

### 场景 9：文件扩展名过滤

```wpl
rule image_file_filter {
    # 选择文件名字段
    | take(filename)

    # 只匹配图片文件
    | regex_match('\.(?i)(jpg|jpeg|png|gif|bmp|svg)$')
}

# 示例数据：
# filename: "photo.jpg"      → ✅ 匹配
# filename: "image.PNG"      → ✅ 匹配（大小写不敏感）
# filename: "document.pdf"   → ❌ 不匹配
```

### 场景 10：MAC 地址验证

```wpl
rule mac_address_validation {
    # 选择 MAC 地址字段
    | take(mac)

    # 匹配 MAC 地址格式（XX:XX:XX:XX:XX:XX）
    | regex_match('^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$')
}

# 示例数据：
# mac: "00:1B:44:11:3A:B7"  → ✅ 匹配
# mac: "AA:BB:CC:DD:EE:FF"  → ✅ 匹配
# mac: "invalid-mac"        → ❌ 不匹配
```

## 使用限制

### 类型限制

`regex_match` 只能处理**字符串类型**的字段：

```wpl
# ✅ 正确 - 字段是字符串
message: "error occurred" -> regex_match('error')

# ❌ 错误 - 字段是数字
status_code: 404 -> regex_match('\d+')  # 会失败

# ❌ 错误 - 字段是 IP 地址（非字符串类型）
ip: 192.168.1.1 -> regex_match('\d+')  # 会失败
```

### 性能考虑

1. **正则表达式编译开销**：
   - 每次调用都会重新编译正则表达式
   - 复杂的正则表达式编译可能需要几微秒

2. **匹配性能**：
   - 简单模式：微秒级
   - 复杂模式（大量回溯）：可能较慢
   - 建议：避免过度复杂的正则表达式

3. **优化建议**：
   ```wpl
   # ✅ 推荐：简单直接的模式
   regex_match('^\d{4}$')

   # ⚠️ 慎用：复杂的回溯模式
   regex_match('^(a+)+b$')  # 可能导致性能问题
   ```

### 不支持的特性

1. **不支持命名捕获组**：
   ```wpl
   # ❌ 不支持（无法提取捕获的内容）
   regex_match('(?P<year>\d{4})')
   ```

2. **不支持替换**：
   ```wpl
   # ❌ regex_match 只做匹配，不做替换
   # 需要替换请使用 chars_replace
   ```

3. **不支持多个模式**：
   ```wpl
   # ❌ 不能传递多个模式
   regex_match('pattern1', 'pattern2')

   # ✅ 使用选择符 |
   regex_match('pattern1|pattern2')
   ```

## 完整示例

### 示例 1：日志分类流水线

```wpl
rule log_classification {
    # 选择日志消息
    | take(message)

    # 分类为错误日志
    (
        | regex_match('(?i)(error|exception|failed|fatal)')
        | tag(level, ERROR)
    )
    |
    # 或分类为警告日志
    (
        | regex_match('(?i)(warn|warning|deprecated)')
        | tag(level, WARNING)
    )
    |
    # 或分类为普通日志
    (
        | tag(level, INFO)
    )
}
```

### 示例 2：安全审计过滤

```wpl
rule security_audit {
    # 检查用户输入中的危险模式
    | take(user_input)

    # 检测脚本注入
    | regex_match('(?i)(<script|javascript:|onerror=)')

    # 或检测 SQL 注入
    | regex_match('(?i)(union|select.*from|insert.*into)')

    # 或检测路径遍历
    | regex_match('(\.\./|\.\.\\)')

    # 匹配到任何一个就记录为安全事件
    | tag(security_event, true)
}
```

### 示例 3：结构化日志解析

```wpl
rule structured_log_parsing {
    # 验证 JSON 日志格式
    | take(raw_message)
    | regex_match('^\{.*\}$')

    # 验证包含必需字段
    | regex_match('"timestamp":\s*"\d{4}-\d{2}-\d{2}')
    | regex_match('"level":\s*"(INFO|WARN|ERROR)"')
    | regex_match('"message":\s*"[^"]+"')

    # 所有验证通过后继续处理
}
```

## 性能说明

- **正则编译**：每次调用都会编译，建议使用简单模式
- **匹配速度**：
  - 简单模式（如 `^\d+$`）：< 1μs
  - 中等复杂度（如邮箱验证）：1-10μs
  - 复杂模式（大量回溯）：可能 > 100μs
- **内存开销**：每个正则表达式约 1-10KB

## 错误处理

### 常见错误

1. **无效的正则表达式**
   ```
   错误: regex_match | invalid regex pattern
   原因: 正则表达式语法错误
   解决: 检查正则表达式语法
   ```

2. **字段不存在**
   ```
   错误: regex_match | no active field
   原因: 当前没有活动字段
   解决: 使用 take() 先选择字段
   ```

3. **字段类型不匹配**
   ```
   错误: regex_match | field is not a string
   原因: 字段不是字符串类型
   解决: 确保字段是 Chars 类型
   ```

4. **模式不匹配**
   ```
   错误: regex_match | not matched
   原因: 字段内容不匹配正则表达式
   解决: 这是正常的过滤逻辑
   ```

## 与其他函数配合使用

### 与字段选择器配合

```wpl
# 先选择字段，再匹配
| take(message)
| regex_match('error')
```

### 与条件检查配合

```wpl
# 组合多个条件
| take(status)
| regex_match('^[45]\d{2}$')  # 4xx 或 5xx
```

### 与 chars_replace 配合

```wpl
# 先匹配，再替换
| regex_match('error')
| chars_replace(error, ERROR)
```

### 与分支逻辑配合

```wpl
# 不同模式走不同分支
(
    | regex_match('^2\d{2}$')  # 2xx
    | tag(status_class, success)
)
|
(
    | regex_match('^4\d{2}$')  # 4xx
    | tag(status_class, client_error)
)
```

## 最佳实践

### 1. 优先使用单引号

```wpl
# ✅ 推荐
regex_match('^\d+$')

# ❌ 避免（会导致解析错误）
regex_match("^\d+$")
```

### 2. 使用锚点明确匹配范围

```wpl
# ✅ 推荐：完全匹配
regex_match('^\d{4}$')  # 恰好4位数字

# ⚠️ 可能不符合预期：部分匹配
regex_match('\d{4}')    # 包含4位数字即可
```

### 3. 简化正则表达式

```wpl
# ✅ 推荐：简单清晰
regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$')

# ⚠️ 过度复杂
regex_match('^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$')
```

### 4. 使用注释文档化复杂模式

```wpl
# 匹配邮箱：用户名@域名.后缀
regex_match('^\w+@\w+\.\w+$')
```

### 5. 测试边界情况

```wpl
# 测试模式的边界
regex_match('^\d{2,4}$')
# 测试：1 ❌, 12 ✅, 123 ✅, 1234 ✅, 12345 ❌
```

## 正则表达式测试

### 在线测试工具

1. **regex101.com**
   - 选择 Rust flavor
   - 测试你的正则表达式
   - 查看匹配详情和性能

2. **regexr.com**
   - 可视化匹配过程
   - 提供备忘清单

### 命令行测试

```bash
# 使用 WP-Motor 测试
echo "test_value: 12345" | wp-motor test.wpl

# 查看匹配结果
wp-motor --debug test.wpl < test_data.log
```

## 常见问题 (FAQ)

### Q1: 为什么必须使用单引号？

因为 WPL 的双引号字符串解析器只支持有限的转义序列（`\"`, `\\`, `\n`, `\t`），而正则表达式需要 `\d`, `\w`, `\s` 等，这些会导致解析失败。

### Q2: 如何匹配点号（.）？

```wpl
# 使用反斜杠转义
regex_match('\.')  # 匹配字面的点号
```

### Q3: 如何实现大小写不敏感匹配？

```wpl
# 使用 (?i) 标志
regex_match('(?i)error')  # 匹配 error, ERROR, Error
```

### Q4: 正则表达式是完全匹配还是部分匹配？

默认是**部分匹配**。使用 `^` 和 `$` 实现完全匹配：

```wpl
# 部分匹配
regex_match('\d+')     # "abc123def" → ✅ 匹配

# 完全匹配
regex_match('^\d+$')   # "abc123def" → ❌ 不匹配
```

### Q5: 如何匹配多行文本？

```wpl
# 使用 (?m) 多行模式
regex_match('(?m)^ERROR')  # 匹配任意行开头的 ERROR

# 使用 (?s) 单行模式让 . 匹配换行符
regex_match('(?s)start.*end')  # 跨行匹配
```

### Q6: 性能如何？

- 简单模式：非常快（微秒级）
- 复杂模式：可能较慢
- 建议：避免过度复杂的回溯模式

### Q7: 能否提取匹配的内容？

不支持。`regex_match` 只做匹配判断，不提取内容。

## 正则表达式速查表

### 常用模式

| 模式 | 说明 | 示例 |
|------|------|------|
| `\d` | 数字 | `\d+` 匹配 "123" |
| `\w` | 单词字符 | `\w+` 匹配 "hello" |
| `\s` | 空白字符 | `\s+` 匹配 "   " |
| `.` | 任意字符 | `.*` 匹配任何内容 |
| `^` | 行首 | `^start` 必须开头匹配 |
| `$` | 行尾 | `end$` 必须结尾匹配 |
| `*` | 0或多次 | `a*` 匹配 "", "a", "aa" |
| `+` | 1或多次 | `a+` 匹配 "a", "aa" |
| `?` | 0或1次 | `a?` 匹配 "", "a" |
| `{n}` | 恰好n次 | `\d{4}` 匹配 "2024" |
| `{n,m}` | n到m次 | `\d{2,4}` 匹配 "12", "123" |
| `[abc]` | 字符集 | `[aeiou]` 匹配元音 |
| `[^abc]` | 非字符集 | `[^0-9]` 匹配非数字 |
| `\|` | 选择 | `cat\|dog` 匹配 "cat" 或 "dog" |
| `()` | 分组 | `(ab)+` 匹配 "ab", "abab" |

## 更多资源

- **Rust Regex 文档**: https://docs.rs/regex/
- **开发指南**: `docs/guide/wpl_field_func_development_guide.md`
- **源代码**: `crates/wp-lang/src/ast/processor/function.rs`
- **测试用例**: `crates/wp-lang/src/eval/builtins/pipe_fun.rs`

## 版本历史

- **1.13.1** (2026-02-02)
  - 初始实现
  - 支持完整的 Rust regex 语法
  - 支持所有标准正则表达式特性
  - 添加完整的测试覆盖

---

**提示**: `regex_match` 功能强大但也可能影响性能。对于简单的字符串匹配，优先考虑使用 `chars_has` 或 `chars_in`；对于数值范围检查，使用 `digit_range` 或 `digit_in`。正则表达式适合复杂的模式匹配场景。
