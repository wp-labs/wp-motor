# chars_replace 函数使用指南

## 概述

`chars_replace` 是 WPL (WP Language) 中的字符串替换函数，用于在日志字段中查找并替换指定的子字符串。

## 快速开始

### 基本语法

```wpl
chars_replace(target, replacement)
```

- **target**: 要查找并替换的字符串
- **replacement**: 替换后的新字符串

### 简单示例

```wpl
# 将 "error" 替换为 "warning"
chars_replace(error, warning)

# 将 "ERROR" 替换为 "WARN"
chars_replace(ERROR, WARN)
```

## 参数格式

### 1. 不带引号（简单标识符）

适用于简单的字段名或关键词：

```wpl
chars_replace(old_value, new_value)
chars_replace(test-old, test-new)
chars_replace(错误, 警告)
```

**支持的字符**：
- 字母（a-z, A-Z）
- 数字（0-9）
- 下划线（_）
- 点（.）
- 斜杠（/）
- 连字符（-）
- Unicode 字符（中文、日文等）

### 2. 带引号（特殊字符）

适用于包含特殊字符的字符串：

```wpl
chars_replace("test,old", "test,new")         # 包含逗号
chars_replace("hello world", "goodbye world") # 包含空格
chars_replace("status=error", "status=ok")    # 包含等号
chars_replace("[ERROR]", "[WARN]")            # 包含方括号
```

**必须使用引号的场景**：
- 包含逗号（,）
- 包含空格
- 包含等号（=）
- 包含方括号（[]）
- 包含其他特殊符号

### 3. 混合使用

可以混合使用带引号和不带引号的参数：

```wpl
chars_replace("test,old", new_value)
chars_replace(old_value, "new,value")
```

### 4. 空字符串（删除文本）

使用空引号删除目标字符串：

```wpl
# 删除 "DEBUG: " 前缀
chars_replace("DEBUG: ", "")

# 删除逗号
chars_replace(",", "")
```

## 实际应用场景

### 场景 1：标准化日志级别

```wpl
# 统一大小写
chars_replace(error, ERROR)
chars_replace(warning, WARNING)

# 标准化格式
chars_replace("[ERROR]", "ERROR:")
chars_replace("[WARN]", "WARNING:")
```

### 场景 2：清理日志内容

```wpl
# 删除调试前缀
chars_replace("DEBUG: ", "")

# 删除多余的空格
chars_replace("  ", " ")

# 删除换行符
chars_replace("\n", " ")
```

### 场景 3：URL 参数替换

```wpl
chars_replace("status=error", "status=ok")
chars_replace("code=500", "code=200")
```

### 场景 4：CSV 字段处理

```wpl
# 替换带逗号的名字
chars_replace("Smith, John", "John Smith")
chars_replace("Doe, Jane", "Jane Doe")
```

### 场景 5：路径标准化

```wpl
# Windows 路径转 Unix 路径
chars_replace("\\", "/")

# 简化路径
chars_replace("/usr/local/", "/opt/")
```

### 场景 6：多语言支持

```wpl
# 中文替换
chars_replace(错误, 警告)
chars_replace("错误：", "警告：")

# 日文替换
chars_replace(エラー, 警告)
```

### 场景 7：敏感信息脱敏

```wpl
# 替换密码
chars_replace("password=12345", "password=***")

# 替换令牌
chars_replace("token=abc123xyz", "token=***")
```

## 使用限制

### 不支持的特性

1. **转义字符**：
   ```wpl
   # ❌ 不支持（会解析错误）
   chars_replace("say \"hello\"", "say 'hi'")
   ```

2. **正则表达式**：
   ```wpl
   # ❌ 不支持正则
   chars_replace("[0-9]+", "NUMBER")  # 会按字面匹配
   ```

3. **通配符**：
   ```wpl
   # ❌ 不支持通配符
   chars_replace("error*", "warning")  # 会按字面匹配
   ```

### 类型限制

`chars_replace` 只能处理**字符串类型**的字段：

```wpl
# ✅ 正确 - 字段是字符串
message: "error occurred" -> chars_replace(error, warning)

# ❌ 错误 - 字段是数字
status_code: 500 -> chars_replace(500, 200)  # 会失败

# ❌ 错误 - 字段是 IP 地址
ip_address: 192.168.1.1 -> chars_replace(192, 10)  # 会失败
```

### 替换行为

- **全局替换**：替换字段中**所有**匹配的子字符串
  ```wpl
  # 输入: "hello hello hello"
  chars_replace(hello, hi)
  # 输出: "hi hi hi"
  ```

- **大小写敏感**：区分大小写
  ```wpl
  # 输入: "Error error ERROR"
  chars_replace(error, warning)
  # 输出: "Error warning ERROR"  # 只替换小写的 error
  ```

## 完整示例

### 示例 1：日志级别标准化流水线

```wpl
rule log_normalization {
    # 标准化不同格式的 ERROR
    | chars_replace("[ERROR]", "ERROR:")
    | chars_replace("ERR:", "ERROR:")
    | chars_replace("Err:", "ERROR:")

    # 标准化 WARNING
    | chars_replace("[WARN]", "WARNING:")
    | chars_replace("Warn:", "WARNING:")

    # 删除调试信息
    | chars_replace("DEBUG: ", "")
}
```

### 示例 2：CSV 数据清理

```wpl
rule csv_cleanup {
    # 标准化姓名格式（从 "Last, First" 到 "First Last"）
    | chars_replace("Smith, John", "John Smith")
    | chars_replace("Doe, Jane", "Jane Doe")

    # 删除多余的引号
    | chars_replace("\"", "")

    # 标准化分隔符
    | chars_replace(";", ",")
}
```

### 示例 3：多步骤替换

```wpl
rule multi_step_replace {
    # 第一步：替换日志级别
    | chars_replace(error, ERROR)

    # 第二步：添加时间戳前缀（通过替换空字符串）
    | chars_replace("", "[2024-01-29] ")

    # 第三步：替换服务名
    | chars_replace(old-service, new-service)
}
```

## 性能说明

- **时间复杂度**：O(n) - n 为字段长度
- **空间复杂度**：O(n) - 需要创建新字符串
- **性能建议**：
  - 短字符串（< 1KB）：性能优秀，延迟 < 1μs
  - 长字符串（1-10KB）：仍然快速，延迟 < 10μs
  - 超长字符串（> 10KB）：考虑性能影响

## 错误处理

### 常见错误

1. **字段不存在**
   ```
   错误: chars_replace | no active field
   原因: 当前没有活动字段
   解决: 使用 take() 或其他选择器先选择字段
   ```

2. **字段类型不匹配**
   ```
   错误: chars_replace
   原因: 字段不是字符串类型
   解决: 确保字段是 Chars 类型
   ```

3. **语法错误**
   ```
   错误: invalid symbol, expected need ','
   原因: 包含逗号的参数未使用引号
   解决: 使用引号包裹参数
   ```

## 与其他函数配合使用

### 与字段选择器配合

```wpl
# 先选择字段，再替换
| take(message)
| chars_replace(error, warning)
```

### 与条件检查配合

```wpl
# 只在特定条件下替换
| chars_has(error)
| chars_replace(error, warning)
```

### 与转换函数配合

```wpl
# 先解码 Base64，再替换
| base64_decode()
| chars_replace(old_value, new_value)
```

## 最佳实践

### 1. 优先使用不带引号的格式

```wpl
# ✅ 推荐（简洁）
chars_replace(error, warning)

# ⚠️ 可以但不必要
chars_replace("error", "warning")
```

### 2. 复杂字符串使用引号

```wpl
# ✅ 正确
chars_replace("status=error", "status=ok")

# ❌ 错误（语法错误）
chars_replace(status=error, status=ok)
```

### 3. 空字符串删除文本

```wpl
# ✅ 推荐（明确意图）
chars_replace("DEBUG: ", "")

# ⚠️ 不清晰
chars_replace("DEBUG: ", nothing)  # 不存在 nothing 关键字
```

### 4. 按顺序执行多次替换

```wpl
# ✅ 正确（逐步替换）
| chars_replace(error, ERROR)
| chars_replace(ERROR, WARNING)
# 结果: error -> ERROR -> WARNING

# ⚠️ 注意顺序
| chars_replace(ERROR, WARNING)
| chars_replace(error, ERROR)
# 结果: error -> ERROR（第二步不会再变成 WARNING）
```

### 5. 测试边界情况

```wpl
# 测试空字符串
chars_replace("", "prefix")  # 在每个字符间插入

# 测试单字符
chars_replace(",", ";")      # 简单替换

# 测试长字符串
chars_replace("very long string to find", "replacement")
```

## 调试技巧

### 1. 逐步测试

```wpl
# 第一步：只做替换
| chars_replace(error, warning)

# 第二步：添加更多替换
| chars_replace(error, warning)
| chars_replace(warning, info)
```

### 2. 检查字段类型

```wpl
# 使用 has() 确认字段存在
| has()

# 使用 chars_has() 确认是字符串类型
| chars_has(some_value)
```

### 3. 查看替换结果

在测试环境中打印替换前后的值：
```bash
# 使用 WP-Motor 的调试模式
wp-motor --debug rule.wpl < test.log
```

## 常见问题 (FAQ)

### Q1: 如何替换换行符？

```wpl
# 方法 1：使用实际的换行符（如果解析器支持）
chars_replace("\n", " ")

# 方法 2：根据实际编码处理
chars_replace("
", " ")  # 实际换行
```

### Q2: 如何同时替换多个不同的字符串？

```wpl
# 使用多个 chars_replace 调用
| chars_replace(error, ERROR)
| chars_replace(warning, WARNING)
| chars_replace(info, INFO)
```

### Q3: 如何实现大小写不敏感的替换？

chars_replace 是大小写敏感的，需要多次调用：

```wpl
| chars_replace(error, ERROR)
| chars_replace(Error, ERROR)
| chars_replace(ERROR, ERROR)
```

### Q4: 替换会修改原始字段吗？

是的，chars_replace 会直接修改活动字段的值。

### Q5: 性能够用吗？

对于大多数日志处理场景，性能完全足够：
- 单条日志 < 10KB：几乎无感知
- 高吞吐量场景：可处理 100K+ 日志/秒

## 更多资源

- **开发指南**: `docs/guide/wpl_field_func_development_guide.md`
- **解析器实现**: `docs/guide/chars_replace_parser_tests.md`
- **性能分析**: `docs/guide/take_quoted_string_performance.md`
- **源代码**: `crates/wp-lang/src/ast/processor/function.rs`

## 版本历史

- **1.11.0** (2026-01-29)
  - 初始实现
  - 支持基本字符串替换
  - 支持带引号字符串（包含逗号、空格等）
  - 添加完整的测试覆盖

---

**提示**: 如果您在使用过程中遇到问题，请参考错误处理章节或查看开发指南。
