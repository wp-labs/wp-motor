# starts_with 函数使用指南

## 简介

`starts_with` 是一个 WPL pipe 函数，用于检查字符串字段是否以指定的前缀开始。

## 语法

```wpl
field_name | starts_with('prefix')
```

## 参数

- `prefix`: 字符串类型，要检查的前缀

## 行为

- 如果字段值以指定前缀开始，字段保持不变并继续传递
- 如果字段值不以指定前缀开始，字段转换为 **ignore 类型**，后续规则将忽略该字段
- 如果字段不是字符串类型，字段转换为 **ignore 类型**
- 前缀匹配是大小写敏感的
- 空前缀 `''` 匹配任何字符串

## 使用示例

### 示例 1: 过滤 HTTPS URL

```wpl
rule https_filter {
    (chars:url) | starts_with('https://')
}
```

**输入:** `https://example.com`
**输出:** 匹配成功，提取 `url = "https://example.com"`

**输入:** `http://example.com`
**输出:** `url` 字段转换为 ignore，规则匹配失败

### 示例 2: 过滤 API 路径

```wpl
rule api_filter {
    (chars:path) | starts_with('/api/')
}
```

**输入:** `/api/users`
**输出:** 匹配成功，提取 `path = "/api/users"`

**输入:** `/home/users`
**输出:** `path` 字段转换为 ignore，规则匹配失败

### 示例 3: 检查日志级别

```wpl
rule error_log {
    (chars:log_level) | starts_with('ERROR'),
    (chars:message)
}
```

**输入:** `ERROR: Database connection failed`
**输出:** 匹配成功
- `log_level = "ERROR:"`
- `message = "Database connection failed"`

### 示例 4: 结合其他函数使用

```wpl
rule secure_url {
    (chars:url) | starts_with('https://') | chars_has('secure')
}
```

只匹配以 `https://` 开头且包含 `secure` 的 URL。

### 示例 5: 多分支条件

```wpl
rule protocol_filter {
    (
        (chars:url) | starts_with('https://')
    ) | (
        (chars:url) | starts_with('wss://')
    )
}
```

匹配以 `https://` 或 `wss://` 开头的 URL。

## 与 regex_match 的对比

| 特性 | starts_with | regex_match |
|------|------------|-------------|
| 性能 | 更快（字符串前缀检查） | 较慢（正则表达式编译和匹配） |
| 功能 | 只能检查前缀 | 支持复杂模式匹配 |
| 使用难度 | 简单直观 | 需要了解正则表达式 |
| 失败行为 | 转换为 ignore | 解析失败 |

**何时使用 starts_with:**
- 只需要检查字符串前缀
- 性能要求高
- 简单场景
- 需要在管道中继续处理（通过 ignore 字段）

**何时使用 regex_match:**
- 需要复杂的模式匹配
- 需要检查中间或结尾内容
- 需要使用正则表达式特性（捕获组等）

## 注意事项

1. **大小写敏感**: `starts_with('HTTP')` 不会匹配 `http://example.com`
2. **完全匹配前缀**: 前缀必须完全匹配，不支持通配符
3. **仅支持字符串**: 如果字段不是字符串类型，将转换为 ignore
4. **ignore 类型传播**: 转换为 ignore 的字段在后续管道函数中会被跳过

## 实现细节

- 定义位置: `crates/wp-lang/src/ast/processor/function.rs`
- 实现位置: `crates/wp-lang/src/eval/builtins/pipe_fun.rs`
- 解析器: `crates/wp-lang/src/parser/wpl_fun.rs`
- 测试: `crates/wp-lang/src/eval/builtins/pipe_fun.rs` (tests 模块)

## 相关函数

- `regex_match(pattern)`: 正则表达式匹配
- `chars_has(value)`: 检查字段值是否等于指定字符串
- `chars_in([values...])`: 检查字段值是否在列表中
- `chars_replace(target, replacement)`: 替换字符串中的子串
