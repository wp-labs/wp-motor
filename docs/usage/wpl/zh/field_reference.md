# WPL 字段引用使用指南

## 概述

在 WPL 中，使用 `@` 符号来引用集合中的字段。支持两种字段名格式：
- **普通标识符**：`@field_name`
- **单引号字符串**：`@'@special-field'`（用于包含特殊字符的字段名）

## 快速开始

### 基本语法

```wpl
# 引用普通字段
@field_name

# 引用带特殊字符的字段
@'special-field-name'

# 指定字段类型和别名
datatype@field_name: alias_name
```

### 简单示例

```wpl
# JSON 解析 - 提取字段
rule parse_json {
    json(
        @src_ip: source_ip,
        @dst_ip: dest_ip,
        @message: msg
    )
}

# 使用单引号处理特殊字段名
rule parse_json_special {
    json(
        @'@client-ip': client,
        @'event.type': event,
        @'log/level': level
    )
}
```

## 普通字段引用

### 支持的字符

普通字段名（不带引号）支持以下字符：
- 字母和数字（a-z, A-Z, 0-9）
- 下划线（`_`）
- 斜杠（`/`）
- 连字符（`-`）
- 点号（`.`）
- 方括号（`[`, `]`）- 用于数组索引
- 星号（`*`）- 用于通配符

### 示例

```wpl
# 简单字段名
@user_id
@username
@ip_address

# 路径式字段名
@process/name
@parent/process/pid
@network/protocol

# 数组索引
@items[0]
@data[5]/value
@process[0]/path

# 通配符
@items[*]
@logs/*/message
```

## 单引号字段引用

### 何时使用

当字段名包含以下特殊字符时，必须使用单引号：
- `@` 符号
- 空格
- 逗号（`,`）
- 等号（`=`）
- 括号（`(`, `)`）
- 尖括号（`<`, `>`）
- 井号（`#`）
- 其他非标准字符

### 基本语法

```wpl
@'field name with spaces'
@'@field-with-at-sign'
@'field,with,commas'
```

### 转义字符

**双引号字符串**支持以下转义序列：

| 转义序列 | 含义 | 示例 |
|---------|------|------|
| `\"` | 双引号 | `@"field\"name"` |
| `\\` | 反斜杠 | `@"path\\to\\file"` |
| `\n` | 换行符 | `@"multi\nline"` |
| `\t` | 制表符 | `@"tab\tseparated"` |
| `\r` | 回车符 | `@"carriage\rreturn"` |
| `\xHH` | 十六进制字节 | `@"hex\x41value"` |

**单引号字符串**是**原始字符串**（raw string）：
- **只支持** `\'` 转义单引号本身
- 其他所有反斜杠 `\` 都按字面意思处理
- `\n`、`\t`、`\\` 等不会被转义

### 示例

```wpl
# 双引号 - 支持完整转义
@"field\"name"      # 结果: field"name
@"path\\file"       # 结果: path\file
@"line\nbreak"      # 结果: line换行break

# 单引号 - 原始字符串，只转义 \'
@'field\'s name'    # 结果: field's name
@'path\to\file'     # 结果: path\to\file (字面反斜杠)
@'raw\nstring'      # 结果: raw\nstring (字面 \n)
@'C:\Users\test'    # 结果: C:\Users\test (Windows 路径)
```

**推荐使用场景**：
- **单引号**：Windows 路径、Unix 路径、正则表达式、包含反斜杠的字符串
- **双引号**：需要换行符、制表符等转义字符的场景

## 实际应用场景

### 场景 1：解析 Elasticsearch 日志

```wpl
# Elasticsearch 字段通常使用 @ 前缀
rule elasticsearch_log {
    json(
        @'@timestamp': timestamp,
        @'@version': version,
        @message: msg,
        @'log.level': level,
        @'event.action': action
    )
}
```

### 场景 2：解析带空格的字段名

```wpl
# CSV 或其他格式可能包含带空格的列名
rule csv_with_spaces {
    (
        @'First Name': first_name,
        @'Last Name': last_name,
        @'Email Address': email,
        @'Phone Number': phone
    )
}
```

### 场景 3：解析嵌套 JSON 字段

```wpl
# JSON 字段路径包含特殊字符
rule nested_json {
    json(
        @'user.id': uid,
        @'user.profile.name': username,
        @'event#metadata': metadata,
        @'geo.location.lat': latitude,
        @'geo.location.lon': longitude
    )
}
```

### 场景 4：处理 Prometheus 指标

```wpl
# Prometheus 指标名称包含多种特殊字符
rule prometheus_metrics {
    (
        @'http_requests_total{method="GET"}': get_requests,
        @'http_requests_total{method="POST"}': post_requests,
        @'process_cpu_seconds_total': cpu_usage
    )
}
```

### 场景 5：Windows 事件日志

```wpl
# Windows 路径包含反斜杠
rule windows_events {
    json(
        @'Event.System.Provider': provider,
        @'Event.EventData.CommandLine': cmdline,
        @'Process\\Path': process_path
    )
}
```

### 场景 6：混合使用普通和特殊字段名

```wpl
rule mixed_fields {
    json(
        # 普通字段名
        @username: user,
        @ip_address: ip,
        @timestamp: time,

        # 特殊字段名
        @'@client-ip': client,
        @'user.email': email,
        @'event#type': event_type,
        @'log level': level
    )
}
```

### 场景 7：KV 解析带特殊字段

```wpl
# 键值对中包含特殊字符的键
rule kv_special_keys {
    kv(
        @'@timestamp': time,
        @'event-type': type,
        @'user/name': username,
        @'session#id': session
    )
}
```

## take() 函数引号支持

`take()` 函数用于选择当前字段，同样支持单引号和双引号来处理包含特殊字符的字段名。

### 基本语法

```wpl
# 普通字段名
| take(field_name)

# 双引号字段名
| take("@special-field")

# 单引号字段名
| take('@special-field')
```

### 使用场景

#### 1. 选择带特殊字符的字段

```wpl
rule select_special_fields {
    # 双引号
    | take("@timestamp")
    | take("field with spaces")
    | take("field,with,commas")

    # 单引号
    | take('@client-ip')
    | take('event.type')
    | take('log/level')
}
```

#### 2. 转义字符支持

```wpl
rule escaped_fields {
    # 双引号内转义
    | take("field\"name")
    | take("path\\with\\backslash")

    # 单引号内转义
    | take('field\'s name')
    | take('path\\to\\file')
}
```

#### 3. 实际应用

```wpl
# Elasticsearch 日志处理
rule elasticsearch {
    | take("@timestamp")
    | take("@version")
    | take("log.level")
}

# CSV 数据处理
rule csv_processing {
    | take('First Name')
    | take('Last Name')
    | take('Email Address')
}

# 混合使用
rule mixed_usage {
    | take(user_id)          # 普通字段
    | take("@timestamp")     # 双引号
    | take('event.type')     # 单引号
}
```

### 支持的转义字符

| 转义序列 | 含义 | 双引号 | 单引号 |
|---------|------|--------|--------|
| `\"` | 双引号 | ✅ | ❌ (字面 `\"`) |
| `\'` | 单引号 | ❌ (字面 `\'`) | ✅ |
| `\\` | 反斜杠 | ✅ | ❌ (字面 `\\`) |
| `\n` | 换行符 | ✅ | ❌ (字面 `\n`) |
| `\t` | 制表符 | ✅ | ❌ (字面 `\t`) |

**说明**：
- **双引号**：支持完整转义，类似 C/Java/JavaScript 字符串
- **单引号**：原始字符串（raw string），只支持 `\'` 转义单引号本身，其他反斜杠都是字面字符

### 最佳实践

```wpl
# ✅ 推荐 - 优先使用不带引号
| take(field_name)

# ✅ 推荐 - 特殊字符使用引号
| take("@timestamp")
| take('@client-ip')

# ✅ 推荐 - 根据内容选择引号类型
| take("field with spaces")         # 双引号，适合简单字符串
| take('it\'s a field')              # 单引号，只需转义 \'
| take('C:\Windows\System32')       # 单引号，Windows 路径
| take("line\nbreak")                # 双引号，需要换行符转义
```

## 字段类型指定

可以为字段指定数据类型：

```wpl
# 不带引号的字段
ip@source_ip: src
digit@port: port_num
time@timestamp: time

# 带引号的字段
ip@'@client-ip': client
digit@'user.age': age
chars@'event message': msg
```

支持的类型包括：
- `ip` - IP 地址
- `digit` - 整数
- `float` - 浮点数
- `time` - 时间戳
- `chars` - 字符串
- `json` - JSON 对象
- `kv` - 键值对
- 等等

## 字段别名

使用 `:` 为字段指定别名：

```wpl
# 普通字段别名
@source_ip: src
@destination_ip: dst
@user_id: uid

# 特殊字段别名
@'@timestamp': time
@'event.type': event
@'log/level': level

# 带类型和别名
ip@'@client-ip': client_ip
digit@'user.age': age
chars@'user name': username
```

## 使用限制

### 1. 不支持双引号

只支持单引号，不支持双引号：

```wpl
# ✅ 正确
@'@field-name'

# ❌ 错误
@"@field-name"
```

### 2. 转义字符限制

转义字符只在单引号字符串内有效：

```wpl
# ✅ 正确 - 单引号内转义
@'user\'s name'

# ❌ 错误 - 普通字段名不支持转义
@user\'s_name
```

### 3. 空字段名

字段名不能为空：

```wpl
# ❌ 错误
@''

# ✅ 正确
@'_'  # 使用下划线作为字段名
```

### 4. 嵌套引用

单引号不支持嵌套：

```wpl
# ❌ 错误
@'field\'nested\''

# ✅ 正确 - 使用转义
@'field\'nested'
```

## 性能说明

### 解析性能

- **普通字段名**：零拷贝，性能最优
  ```wpl
  @field_name  # 直接引用，无需分配
  ```

- **单引号字段名**：需要解码转义字符
  ```wpl
  @'@field'    # 无转义字符，性能接近普通字段
  @'field\'s'  # 有转义字符，需要额外处理
  ```

### 性能对比

| 字段名类型 | 解析时间 | 内存分配 | 推荐场景 |
|-----------|---------|---------|---------|
| 普通字段名 | ~10ns | 零拷贝 | 优先使用 |
| 单引号（无转义） | ~15ns | 一次分配 | 特殊字符 |
| 单引号（有转义） | ~30ns | 一次分配 | 必要时使用 |

### 优化建议

1. **优先使用普通字段名**
   ```wpl
   # ✅ 推荐
   @user_id
   @timestamp

   # ⚠️ 仅在必要时使用
   @'@timestamp'
   ```

2. **避免不必要的转义**
   ```wpl
   # ✅ 推荐
   @'simple-field'

   # ⚠️ 避免
   @'field\twith\tescape'  # 仅在确实需要制表符时使用
   ```

3. **批量操作时考虑性能**
   ```wpl
   # 大量字段解析时，优先使用普通字段名
   json(
       @user_id,      # 快
       @username,     # 快
       @'@metadata'   # 稍慢
   )
   ```

## 错误处理

### 常见错误

#### 1. 字段名包含特殊字符但未使用引号

```
错误: 解析失败，意外的字符 '@'
原因: 字段名包含 @ 但未使用单引号
解决: @'@field-name'
```

#### 2. 单引号未闭合

```
错误: 字符串未闭合
原因: 缺少结束的单引号
解决: 确保引号成对出现 @'field-name'
```

#### 3. 转义字符错误

```
错误: 无效的转义序列
原因: 使用了不支持的转义字符
解决: 只使用支持的转义序列 \', \\, \n, \t, \r, \xHH
```

#### 4. 空字段名

```
错误: 字段名不能为空
原因: @'' 或 @ 后无内容
解决: 提供有效的字段名
```

## 最佳实践

### 1. 命名规范

```wpl
# ✅ 推荐 - 使用下划线分隔
@user_id
@client_ip
@event_timestamp

# ⚠️ 避免 - 除非必要
@'user id'
@'client-ip'
```

### 2. 保持一致性

```wpl
# ✅ 推荐 - 统一风格
rule consistent_naming {
    json(
        @user_id,
        @user_name,
        @user_email
    )
}

# ⚠️ 避免 - 混合风格
rule inconsistent_naming {
    json(
        @user_id,
        @'user name',
        @userEmail
    )
}
```

### 3. 文档化特殊字段

```wpl
# ✅ 推荐 - 添加注释说明
rule documented {
    json(
        # Elasticsearch 的 @timestamp 字段
        @'@timestamp': time,

        # 日志级别（包含空格）
        @'log level': level
    )
}
```

### 4. 使用类型前缀

```wpl
# ✅ 推荐 - 明确指定类型
time@'@timestamp': time
ip@'@client-ip': client
chars@'event message': msg
```

### 5. 别名使用规范

```wpl
# ✅ 推荐 - 使用简短的别名
@'very.long.nested.field.name': short_name
@'@timestamp': time
@'event.action': action

# ⚠️ 避免 - 别名过长
@'@timestamp': timestamp_value_from_elasticsearch
```

## 调试技巧

### 1. 逐步验证字段引用

```wpl
# 第一步：验证单个字段
rule test_single {
    json(@'@timestamp')
}

# 第二步：添加更多字段
rule test_multiple {
    json(
        @'@timestamp',
        @'event.type'
    )
}

# 第三步：添加类型和别名
rule test_complete {
    time@'@timestamp': time,
    chars@'event.type': event
}
```

### 2. 检查字段名拼写

```bash
# 使用 JSON 工具查看原始字段名
echo '{"@timestamp": "2024-01-01"}' | jq 'keys'

# 输出: ["@timestamp"]
# WPL 中使用: @'@timestamp'
```

### 3. 测试转义字符

```wpl
# 逐个测试转义字符
@'test\'quote'      # 单引号
@'test\\backslash'  # 反斜杠
@'test\nnewline'    # 换行符
```

### 4. 使用调试模式

```bash
# 使用 WP-Motor 调试模式查看解析结果
wp-motor --debug rule.wpl < test.log
```

## 常见问题 (FAQ)

### Q1: 何时必须使用单引号？

当字段名包含以下字符时必须使用单引号：
- `@`、空格、逗号、等号、括号、尖括号、井号等特殊字符

### Q2: 单引号和双引号有什么区别？

WPL 只支持单引号 `'` 用于字段名引用。双引号 `"` 用于其他语法元素（如作用域标记）。

### Q3: 如何在字段名中包含单引号？

使用反斜杠转义：`@'user\'s name'`

### Q4: 性能影响有多大？

对于大多数应用场景，性能影响可忽略不计（纳秒级差异）。只在极高性能要求时才需要考虑。

### Q5: 可以使用变量作为字段名吗？

不可以，字段名必须是静态的字面量。

### Q6: 如何处理动态字段名？

使用通配符或字段组合：
```wpl
@items[*]/name     # 匹配所有数组元素的 name 字段
@'prefix*'         # 匹配以 prefix 开头的字段（如果支持）
```

### Q7: 支持 Unicode 字符吗？

支持，字段名可以包含任意 Unicode 字符：
```wpl
@'用户名称'
@'événement'
@'フィールド'
```

## 更多资源

- **分隔符使用指南**: `docs/usage/wpl/separator.md`
- **chars_replace 使用指南**: `docs/usage/wpl/chars_replace.md`
- **WPL Field Function 开发指南**: `docs/guide/wpl_field_func_development_guide.md`
- **源代码**:
  - `crates/wp-lang/src/parser/utils.rs` (take_ref_path_or_quoted)
  - `crates/wp-lang/src/parser/wpl_field.rs` (wpl_id_field)

## 版本历史

- **1.11.0** (2026-01-29)
  - 新增单引号字段名支持（`@'@special-field'`）
  - 新增 `take()` 函数单引号和双引号支持
  - 支持 `take("@field")` 和 `take('@field')` 语法
  - 添加转义字符支持（`\"`, `\'`, `\\`, `\n`, `\t`）
  - 添加完整的测试覆盖

---

**提示**: 优先使用普通字段名以获得最佳性能，仅在字段名包含特殊字符时使用引号。
