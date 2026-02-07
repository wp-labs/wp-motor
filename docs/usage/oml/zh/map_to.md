# map_to 函数使用指南

## 简介

`map_to` 是一个 OML pipe 函数，用于在字段存在（非 ignore）时将字段值映射到指定的值。支持多种类型：字符串、整数、浮点数、布尔值。

## 语法

```oml
field_name = pipe take(source_field) | map_to(value);
```

## 参数

`map_to` 支持多种类型的参数，解析器会自动推断类型:

- **字符串**: 使用引号包围，如 `'text'` 或 `"text"`
- **整数**: 直接写数字，如 `123` 或 `-456`
- **浮点数**: 带小数点的数字，如 `3.14` 或 `-2.5`
- **布尔值**: `true` 或 `false`

## 行为

- 如果字段为**非 ignore** 类型，使用参数值替换字段值（并转换为相应类型）
- 如果字段为 **ignore** 类型，保持不变
- 自动进行类型转换

## 类型推断规则

| 输入 | 推断类型 | 结果字段类型 |
|------|----------|------------|
| `'text'` 或 `"text"` | 字符串 | Chars |
| `123` | 整数 | Digit |
| `3.14` | 浮点数 | Float |
| `true` / `false` | 布尔值 | Bool |

## 使用场景

### 场景 1: 映射到字符串标记

```oml
name : classify_status
---
status_label = pipe take(http_code) | map_to('success');
```

**输入:** `http_code = 200`
**输出:** `status_label = "success"` (Chars类型)

### 场景 2: 映射到整数优先级

```oml
name : set_priority
---
priority = pipe take(log_level) | map_to(1);
```

**输入:** `log_level = "ERROR"`
**输出:** `priority = 1` (Digit类型)

### 场景 3: 映射到浮点数阈值

```oml
name : set_threshold
---
threshold = pipe take(category) | map_to(0.95);
```

**输入:** `category = "high"`
**输出:** `threshold = 0.95` (Float类型)

### 场景 4: 映射到布尔标记

```oml
name : mark_secure
---
is_secure = pipe take(protocol) | map_to(true);
```

**输入:** `protocol = "https"`
**输出:** `is_secure = true` (Bool类型)

### 场景 5: 与过滤函数组合

```oml
name : classify_secure_requests
---
security_level = pipe take(url) | starts_with('https://') | map_to(3);
```

**输入:** `url = "https://api.example.com"`
**输出:** `security_level = 3` (Digit类型)

**输入:** `url = "http://api.example.com"`
**输出:** `security_level = (ignore)` (因为 starts_with 失败)

### 场景 6: 多级分类

```oml
name : multi_level_classification
---
# 检查 URL 是否为 HTTPS，如果是则设置安全级别为高优先级
priority = pipe take(url)
    | starts_with('https://')
    | map_to(10);

# 标记为已验证
verified = pipe take(url)
    | starts_with('https://')
    | map_to(true);
```

### 场景 7: 协议分类

```oml
name : protocol_classification
---
# HTTP 标记为 1
http_level = pipe take(protocol) | starts_with('http://') | map_to(1);

# HTTPS 标记为 3
https_level = pipe take(protocol) | starts_with('https://') | map_to(3);

# FTP 标记为 2
ftp_level = pipe take(protocol) | starts_with('ftp://') | map_to(2);
```

### 场景 8: 状态码分类

```oml
name : status_code_classification
---
# 2xx 成功
success_flag = pipe take(status_code) | digit_range(200, 299) | map_to('success');

# 4xx 客户端错误
client_error_flag = pipe take(status_code) | digit_range(400, 499) | map_to('client_error');

# 5xx 服务器错误
server_error_flag = pipe take(status_code) | digit_range(500, 599) | map_to('server_error');
```

## 与其他函数的对比

| 函数 | 支持类型 | 用途 | 条件保留 |
|------|----------|------|---------|
| `map_to(value)` | 字符串、整数、浮点数、布尔值 | 通用映射，自动类型推断 | 保留 ignore |
| `to_str` | - | 类型转换为字符串 | 不保留 |
| `to_json` | - | 转换为 JSON 字符串 | 不保留 |

## 典型使用模式

### 1. 条件标记模式

```oml
name : conditional_marking
---
# 如果某个条件满足，标记为 true
is_valid = pipe take(field) | some_condition() | map_to(true);
```

### 2. 分类映射模式

```oml
name : classification_mapping
---
# 根据不同条件映射到不同的分类
category_a = pipe take(value) | condition_a() | map_to('A');
category_b = pipe take(value) | condition_b() | map_to('B');
```

### 3. 优先级赋值模式

```oml
name : priority_assignment
---
# 根据条件赋予不同优先级
high_priority = pipe take(level) | check_high() | map_to(10);
medium_priority = pipe take(level) | check_medium() | map_to(5);
low_priority = pipe take(level) | check_low() | map_to(1);
```

## 实现细节

- 定义位置: `crates/wp-oml/src/language/syntax/functions/pipe/other.rs`
- 实现位置: `crates/wp-oml/src/core/evaluator/transform/pipe/other.rs`
- 解析器: `crates/wp-oml/src/parser/pipe_prm.rs`
- 测试: `crates/wp-oml/src/core/evaluator/transform/pipe/other.rs` (tests 模块)

## 类型推断实现

解析器按以下顺序尝试解析参数:

1. **布尔值**: `true` 或 `false`
2. **数字**:
   - 如果是整数形式（无小数部分），推断为 `Digit`
   - 如果有小数部分，推断为 `Float`
3. **字符串**: 单引号或双引号包围的文本

## 注意事项

1. **字符串必须加引号**: `map_to(text)` 会报错，应使用 `map_to('text')`
2. **整数自动识别**: `map_to(100)` 自动识别为整数，`map_to(100.0)` 识别为浮点数
3. **布尔值不加引号**: `map_to(true)` 而非 `map_to('true')`
4. **ignore 字段保持不变**: 如果输入字段是 ignore，输出也是 ignore
5. **类型转换**: 无论输入字段的原始类型，输出字段类型由参数决定

## 性能特性

- **O(1) 时间复杂度**: 简单的值替换操作
- **类型安全**: 编译时类型检查确保类型正确
- **零开销**: 直接值替换，无额外分配

## 调试技巧

### 1. 验证类型推断

```oml
name : debug_type_inference
---
# 测试不同类型
str_field = pipe take(input) | map_to('string');    # Chars
int_field = pipe take(input) | map_to(123);         # Digit
float_field = pipe take(input) | map_to(3.14);      # Float
bool_field = pipe take(input) | map_to(true);       # Bool
```

### 2. 检查 ignore 传播

```oml
name : debug_ignore_propagation
---
# 如果 starts_with 失败，result 应该是 ignore
result = pipe take(url) | starts_with('https://') | map_to('secure');
```

## 相关函数

- `starts_with(prefix)`: 检查字符串前缀，失败时返回 ignore
- `skip_empty`: 跳过空值
- `to_str`: 转换为字符串
- `to_json`: 转换为 JSON 字符串
- `get(name)`: 从嵌套结构获取字段
- `nth(index)`: 从数组获取指定索引的元素
