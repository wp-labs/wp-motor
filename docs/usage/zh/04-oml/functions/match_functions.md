# OML Match 表达式函数匹配

本文档介绍 OML `match` 表达式中的函数匹配功能。

## 概述

从版本 1.13.4 开始，OML 的 `match` 表达式支持使用函数进行模式匹配，提供比简单值比较更灵活的匹配方式。

### 基本语法

```oml
field_name = match read(source_field) {
    function_name(arguments) => result_value,
    _ => default_value,
};
```

### 与 Pipe Function 的区别

| 特性 | Match 函数 | Pipe 函数 |
|------|-----------|-----------|
| **用途** | 多分支条件判断 | 二元过滤（保留/忽略） |
| **返回** | 根据条件返回不同的值 | 匹配返回原值，不匹配返回 ignore |
| **场景** | 分类、路由、决策 | 过滤、清洗 |

**示例对比**:

```oml
# Match: 根据前缀分类到不同结果
EventType = match read(log) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    starts_with('[INFO]') => chars(info),
    _ => chars(other),
};

# Pipe: 过滤出 ERROR 日志，其他变为 ignore
ErrorLog = pipe take(log) | starts_with('[ERROR]');
```

## 支持的函数

### 字符串匹配函数

#### starts_with(prefix)

检查字段值是否以指定前缀开始。

**语法**: `starts_with('prefix')`

**参数**:
- `prefix`: 字符串，要匹配的前缀（必须使用引号）

**匹配规则**:
- 字段值以指定前缀开始 → 匹配成功
- 字段值不以指定前缀开始 → 匹配失败
- 字段不是字符串类型 → 匹配失败
- 大小写敏感

**示例**:
```oml
EventType = match read(log_line) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    _ => chars(info),
};
```

#### ends_with(suffix)

检查字段值是否以指定后缀结束。

**语法**: `ends_with('suffix')`

**参数**:
- `suffix`: 字符串，要匹配的后缀（必须使用引号）

**匹配规则**:
- 字段值以指定后缀结束 → 匹配成功
- 字段值不以指定后缀结束 → 匹配失败
- 字段不是字符串类型 → 匹配失败
- 大小写敏感

**示例**:
```oml
FileType = match read(filename) {
    ends_with('.json') => chars(json),
    ends_with('.xml') => chars(xml),
    ends_with('.log') => chars(log),
    _ => chars(unknown),
};
```

#### contains(substring)

检查字段值是否包含指定子串。

**语法**: `contains('substring')`

**参数**:
- `substring`: 字符串，要匹配的子串（必须使用引号）

**匹配规则**:
- 字段值包含指定子串 → 匹配成功
- 字段值不包含指定子串 → 匹配失败
- 字段不是字符串类型 → 匹配失败
- 大小写敏感

**示例**:
```oml
ErrorType = match read(message) {
    contains('exception') => chars(exception),
    contains('timeout') => chars(timeout),
    contains('failed') => chars(failure),
    _ => chars(normal),
};
```

#### regex_match(pattern)

使用正则表达式匹配字段值。

**语法**: `regex_match('pattern')`

**参数**:
- `pattern`: 字符串，正则表达式模式（必须使用引号）

**匹配规则**:
- 字段值匹配正则表达式 → 匹配成功
- 字段值不匹配正则表达式 → 匹配失败
- 正则表达式语法错误 → 匹配失败并记录警告
- 字段不是字符串类型 → 匹配失败

**注意**: 使用标准 Rust regex 语法

**示例**:
```oml
EventPattern = match read(log_message) {
    regex_match('^\[\d{4}-\d{2}-\d{2}') => chars(timestamped),
    regex_match('^ERROR:.*timeout') => chars(error_timeout),
    regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}') => chars(ip_address),
    _ => chars(unmatched),
};
```

#### is_empty()

检查字段值是否为空字符串。

**语法**: `is_empty()`

**参数**: 无

**匹配规则**:
- 字段值为空字符串 → 匹配成功
- 字段值非空 → 匹配失败
- 字段不是字符串类型 → 匹配失败

**示例**:
```oml
Status = match read(field_value) {
    is_empty() => chars(missing),
    _ => chars(present),
};
```

#### iequals(value)

忽略大小写比较字段值。

**语法**: `iequals('value')`

**参数**:
- `value`: 字符串，要比较的值（必须使用引号）

**匹配规则**:
- 字段值与参数值在忽略大小写的情况下相等 → 匹配成功
- 字段值与参数值不相等 → 匹配失败
- 字段不是字符串类型 → 匹配失败

**示例**:
```oml
NormalizedStatus = match read(status) {
    iequals('success') => chars(ok),
    iequals('error') => chars(fail),
    iequals('warning') => chars(warn),
    _ => chars(unknown),
};
```

### 数值比较函数

#### gt(value)

检查字段值是否大于指定值。

**语法**: `gt(100)` （数值参数不需要引号）

**参数**:
- `value`: 数值，要比较的阈值

**匹配规则**:
- 字段值 > 参数值 → 匹配成功
- 字段值 ≤ 参数值 → 匹配失败
- 字段不是数值类型 → 匹配失败
- 支持整数 (digit) 和浮点数 (float)

**示例**:
```oml
Level = match read(count) {
    gt(1000) => chars(critical),
    gt(500) => chars(high),
    gt(100) => chars(medium),
    _ => chars(low),
};
```

#### lt(value)

检查字段值是否小于指定值。

**语法**: `lt(60)` （数值参数不需要引号）

**参数**:
- `value`: 数值，要比较的阈值

**匹配规则**:
- 字段值 < 参数值 → 匹配成功
- 字段值 ≥ 参数值 → 匹配失败
- 字段不是数值类型 → 匹配失败
- 支持整数和浮点数

**示例**:
```oml
Grade = match read(score) {
    lt(60) => chars(fail),
    lt(70) => chars(pass),
    lt(85) => chars(good),
    _ => chars(excellent),
};
```

#### eq(value)

检查字段值是否等于指定数值。

**语法**: `eq(5)` （数值参数不需要引号）

**参数**:
- `value`: 数值，要比较的目标值

**匹配规则**:
- 字段值等于参数值 → 匹配成功（浮点数比较容差 1e-10）
- 字段值不等于参数值 → 匹配失败
- 字段不是数值类型 → 匹配失败
- 支持整数和浮点数

**示例**:
```oml
Status = match read(level) {
    eq(0) => chars(disabled),
    eq(5) => chars(max_level),
    eq(1) => chars(minimum),
    _ => chars(normal),
};
```

#### in_range(min, max)

检查字段值是否在指定范围内。

**语法**: `in_range(20, 30)` （数值参数不需要引号）

**参数**:
- `min`: 数值，范围最小值
- `max`: 数值，范围最大值

**匹配规则**:
- min ≤ 字段值 ≤ max → 匹配成功
- 字段值 < min 或 字段值 > max → 匹配失败
- 字段不是数值类型 → 匹配失败
- 支持整数和浮点数
- 区间为闭区间 [min, max]

**示例**:
```oml
TempZone = match read(temperature) {
    lt(0) => chars(freezing),
    in_range(0, 10) => chars(cold),
    in_range(10, 20) => chars(cool),
    in_range(20, 30) => chars(comfortable),
    gt(30) => chars(warm),
    _ => chars(unknown),
};
```

## 使用示例

### 示例 1: 日志级别分类

```oml
name : classify_log_event
---
EventType = match read(Content) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    starts_with('[INFO]') => chars(info),
    _ => chars(debug),
};
```

### 示例 2: 文件类型识别

```oml
name : file_type_detection
---
FileType = match read(filename) {
    ends_with('.json') => chars(json),
    ends_with('.xml') => chars(xml),
    ends_with('.log') => chars(log),
    ends_with('.txt') => chars(text),
    _ => chars(unknown),
};
```

### 示例 3: 错误类型检测

```oml
name : error_type_detection
---
ErrorType = match read(message) {
    contains('exception') => chars(exception),
    contains('timeout') => chars(timeout),
    contains('failed') => chars(failure),
    _ => chars(normal),
};
```

### 示例 4: 分数等级映射

```oml
name : score_grade_mapping
---
Grade = match read(score) {
    gt(90) => chars(A),
    in_range(80, 90) => chars(B),
    in_range(70, 80) => chars(C),
    in_range(60, 70) => chars(D),
    _ => chars(F),
};
```

### 示例 5: 温度区间分类

```oml
name : temperature_classification
---
TempZone = match read(temperature) {
    lt(0) => chars(freezing),
    in_range(0, 10) => chars(cold),
    in_range(10, 20) => chars(cool),
    in_range(20, 30) => chars(comfortable),
    in_range(30, 40) => chars(warm),
    gt(40) => chars(hot),
    _ => chars(unknown),
};
```

### 示例 6: 混合使用多种函数

```oml
name : log_classification
---
EventType = match read(log_line) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    contains('exception') => chars(exception),
    ends_with('failed') => chars(failure),
    is_empty() => chars(empty),
    _ => chars(other),
};
```

### 示例 7: 正则表达式匹配

```oml
name : regex_pattern_match
---
EventPattern = match read(log_message) {
    regex_match('^\[\d{4}-\d{2}-\d{2}\s\d{2}:\d{2}:\d{2}\]') => chars(timestamped),
    regex_match('^ERROR:.*timeout') => chars(error_timeout),
    regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}') => chars(ip_address),
    _ => chars(unmatched),
};
```

### 示例 8: 大小写不敏感状态匹配

```oml
name : case_insensitive_status
---
NormalizedStatus = match read(status) {
    iequals('success') => chars(ok),
    iequals('error') => chars(fail),
    iequals('warning') => chars(warn),
    iequals('pending') => chars(wait),
    _ => chars(unknown),
};
```

## 注意事项

### 1. 参数引号规则

```oml
# ✅ 字符串参数必须加引号
starts_with('prefix')
iequals('value')

# ✅ 数值参数不需要引号
gt(100)
eq(5)
in_range(20, 30)

# ❌ 错误示例
starts_with(prefix)   # 缺少引号
gt('100')             # 不应该加引号
```

### 2. 大小写敏感性

```oml
# 大多数字符串函数都是大小写敏感的
starts_with('ERROR')  # 不会匹配 'error:'

# 使用 iequals 进行大小写不敏感匹配
iequals('success')    # 匹配 'SUCCESS', 'Success', 'success'
```

### 3. 匹配顺序

```oml
# match 按从上到下的顺序匹配，第一个匹配成功的分支会被执行
Grade = match read(score) {
    gt(90) => chars(A),        # 先检查 > 90
    gt(80) => chars(B),        # 再检查 > 80
    gt(70) => chars(C),        # 然后检查 > 70
    _ => chars(F),
};

# 如果 score = 95，只会匹配到第一个分支 (A)
```

### 4. 数值类型支持

```oml
# 支持多种数值类型
- digit(100)     # 整数
- float(3.14)    # 浮点数
- chars("123")   # 可解析的字符串

# 所有这些都可以用于数值比较函数
count = digit(150);
Level = match read(count) {
    gt(100) => chars(high),  # 匹配成功
    _ => chars(low),
};
```

### 5. 范围区间

```oml
# in_range 使用闭区间 [min, max]
in_range(10, 20)   # 包含 10 和 20

# 示例：score = 20 会匹配成功
Grade = match read(score) {
    in_range(10, 20) => chars(pass),  # 匹配！
    _ => chars(fail),
};
```

## 性能参考

| 函数类型 | 典型性能 | 说明 |
|----------|----------|------|
| 前缀/后缀匹配 | < 1μs | 简单字符串比较 |
| 子串匹配 | 1-5μs | 取决于字符串长度 |
| 正则表达式 | 5-50μs | 取决于模式复杂度 |
| 数值比较 | < 100ns | 直接数值比较 |
| 大小写转换 | 1-2μs | 需要字符串复制 |

## 函数对比

### Match 函数 vs Pipe 函数

| 特性 | Match starts_with | Pipe starts_with |
|------|-------------------|------------------|
| **返回值** | 根据条件返回不同值 | 匹配返回原值，不匹配返回 ignore |
| **分支数** | 支持多个分支 | 仅二元（匹配/不匹配） |
| **用途** | 分类、决策树 | 过滤、数据清洗 |
| **代码长度** | 多条件时更简洁 | 简单过滤时更简洁 |

**示例对比**:

```oml
# Match: 多分支分类
EventType = match read(log) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    starts_with('[INFO]') => chars(info),
    _ => chars(debug),
};

# Pipe: 简单过滤
ErrorLog = pipe take(log) | starts_with('[ERROR]');
# 不匹配的变成 ignore，匹配的保留原值
```

### 数值函数对比

| 场景 | 推荐函数 | 示例 |
|------|---------|------|
| 阈值判断 | `gt` / `lt` | `gt(100)` |
| 精确匹配 | `eq` | `eq(5)` |
| 区间判断 | `in_range` | `in_range(10, 20)` |
| 分段分类 | 组合使用 | 见温度分类示例 |

## 最佳实践

### 1. 优先使用简单函数

```oml
# ✅ 推荐：使用简单的 starts_with
match read(url) {
    starts_with('https://') => chars(secure),
    _ => chars(insecure),
}

# ⚠️ 避免：不必要的正则表达式
match read(url) {
    regex_match('^https://') => chars(secure),  # 性能更差
    _ => chars(insecure),
}
```

### 2. 合理组织匹配顺序

```oml
# ✅ 推荐：从具体到一般
match read(log) {
    starts_with('[ERROR]') => chars(error),     # 最具体
    starts_with('[WARN]') => chars(warning),
    contains('exception') => chars(exception),   # 较宽泛
    _ => chars(other),                          # 默认
}
```

### 3. 利用数值范围

```oml
# ✅ 推荐：使用 in_range 简化多个条件
Grade = match read(score) {
    gt(90) => chars(A),
    in_range(80, 90) => chars(B),
    in_range(70, 80) => chars(C),
    _ => chars(D),
};

# ⚠️ 避免：重复的比较
Grade = match read(score) {
    gt(90) => chars(A),
    gt(80) => chars(B),  # 实际上是 80-90
    gt(70) => chars(C),  # 实际上是 70-80
    _ => chars(D),
};
```

### 4. 使用 iequals 处理用户输入

```oml
# ✅ 推荐：使用 iequals 处理大小写不确定的输入
Status = match read(user_input) {
    iequals('yes') => chars(confirmed),
    iequals('no') => chars(rejected),
    _ => chars(invalid),
};
```

## 与传统匹配的对比

### 传统值匹配

```oml
Status = match read(status_code) {
    digit(200) => chars(success),
    digit(404) => chars(not_found),
    digit(500) => chars(error),
    _ => chars(unknown),
};
```

**特点**: 精确匹配固定值

### 函数匹配

```oml
EventType = match read(log_line) {
    starts_with('ERROR:') => chars(error),
    starts_with('WARN:') => chars(warning),
    starts_with('INFO:') => chars(info),
    _ => chars(debug),
};
```

**特点**: 基于模式或条件匹配

## 相关文档

- [OML Pipe Functions 索引](./function_index.md) - Pipe 函数完整列表
- [starts_with Pipe 函数](./starts_with.md) - Pipe 版本的 starts_with 详细说明
- [map_to 函数](./map_to.md) - 值映射函数
- [OML 语法参考](../README.md) - OML 基础语法

## 版本历史

- **1.13.4** (2026-02-04)
  - 新增 match 表达式函数匹配支持
  - 字符串匹配：`starts_with`, `ends_with`, `contains`, `regex_match`, `is_empty`, `iequals`
  - 数值比较：`gt`, `lt`, `eq`, `in_range`
  - 完善函数文档

---

**提示**: Match 函数用于多分支条件判断，Pipe 函数用于二元过滤。根据实际场景选择合适的函数类型可以让代码更简洁清晰。
