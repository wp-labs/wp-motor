# not() - 结果反转包装函数

## 概述

`not()` 是一个包装函数，用于反转（取反）内部管道函数的成功/失败结果。当内部函数匹配成功时，`not()` 返回失败；当内部函数匹配失败时，`not()` 返回成功。

**语法**：
```wpl
| not(inner_function)
```

**参数**：
- `inner_function`: 任何字段管道函数（如 `f_chars_has`, `has`, `chars_has` 等）

**返回**：
- 内部函数成功 → `not()` 失败
- 内部函数失败 → `not()` 成功

**重要特性**：
- ✅ **保留字段值**：`not()` 只反转结果，不修改字段内容
- ✅ **支持嵌套**：可以使用 `not(not(...))` 实现双重否定
- ✅ **自动字段选择**：继承内部函数的字段选择行为
- ✅ **零性能开销**：仅在执行时克隆单个字段进行测试

## 基本用法

### 1. 字符串不等于检查

```wpl
# 检查 dev_type 不等于 "NDS"
(chars:dev_type) | not(chars_has(NDS))

# 等价于
(chars:dev_type) | chars_not_has(NDS)
```

### 2. 字段不存在检查

```wpl
# 检查字段不存在
| not(f_has(optional_field))
```

### 3. 使用目标字段函数

```wpl
# 检查指定字段不等于某值
| not(f_chars_has(status, ERROR))

# 检查指定字段不在列表中
| not(f_chars_in(level, [DEBUG, TRACE]))
```

## 高级用法

### 双重否定

双重否定等同于肯定断言：

```wpl
# not(not(...)) 等同于直接使用内部函数
| not(not(f_chars_has(status, OK)))

# 等价于
| f_chars_has(status, OK)
```

### 组合复杂条件

```wpl
# 字段存在但值不等于目标值
| f_has(dev_type)           # 确保字段存在
| not(chars_has(NDS))       # 确保值不等于 NDS
```

### 与数值函数结合

```wpl
# 检查状态码不在成功范围内
| not(f_digit_range(status, 200, 299))

# 检查端口号不在常用端口列表中
| not(f_digit_in(port, [80, 443, 8080]))
```

### 与正则表达式结合

```wpl
# 检查消息不匹配错误模式
| not(regex_match('(?i)error|fail|exception'))
```

## 与现有函数对比

### not(chars_has) vs chars_not_has

虽然功能相似，但语义略有不同：

| 函数 | 字段不存在 | 非Chars类型 | 值不等于 | 值等于 |
|------|-----------|------------|---------|--------|
| `not(chars_has(X))` | ✅ 成功 | ✅ 成功 | ✅ 成功 | ❌ 失败 |
| `chars_not_has(X)` | ✅ 成功 | ✅ 成功 | ✅ 成功 | ❌ 失败 |

**推荐使用**：
- 简单场景：使用 `chars_not_has` （更直观）
- 复杂场景：使用 `not()` 包装其他函数（更灵活）

```wpl
# ✅ 推荐：简单否定
| chars_not_has(ERROR)

# ✅ 推荐：复杂条件否定
| not(f_digit_range(code, 400, 499))
```

## 实际应用场景

### 场景 1：过滤非错误日志

```wpl
rule filter_non_errors {
    # 解析日志级别
    (symbol(ERROR), symbol(WARN), symbol(INFO), symbol(DEBUG):level)

    # 只保留非 ERROR 和 WARN 级别的日志
    | take(level)
    | not(chars_in([ERROR, WARN]))
}
```

**输入**：
```
INFO: Application started
ERROR: Connection failed
DEBUG: Processing request
```

**输出**：
```
INFO: Application started     # ✅ 通过（非错误）
                              # ❌ 过滤掉 ERROR
DEBUG: Processing request     # ✅ 通过（非错误）
```

### 场景 2：排除特定设备类型

```wpl
rule exclude_device_types {
    # 解析设备类型字段
    (chars:dev_type)

    # 排除 NDS 和 IDS 设备
    | not(f_chars_in(dev_type, [NDS, IDS]))
}
```

**输入**：
```
dev_type=FIREWALL
dev_type=NDS
dev_type=ROUTER
dev_type=IDS
```

**输出**：
```
dev_type=FIREWALL    # ✅ 通过
                     # ❌ 过滤掉 NDS
dev_type=ROUTER      # ✅ 通过
                     # ❌ 过滤掉 IDS
```

### 场景 3：非标准端口检查

```wpl
rule non_standard_ports {
    # 解析端口号
    (digit:port)

    # 排除标准端口 80 和 443
    | not(f_digit_in(port, [80, 443]))

    # 同时必须在有效范围内
    | digit_range(1, 65535)
}
```

**输入**：
```
80
8080
443
9000
```

**输出**：
```
                # ❌ 过滤掉 80（标准端口）
8080            # ✅ 通过
                # ❌ 过滤掉 443（标准端口）
9000            # ✅ 通过
```

### 场景 4：排除测试环境日志

```wpl
rule exclude_test_env {
    # 解析环境标识
    (chars:env)

    # 排除测试和开发环境
    | not(f_chars_in(env, [test, dev, staging]))
}
```

## 性能考虑

### 性能特征

| 操作 | 性能影响 |
|------|---------|
| 单层 `not()` | < 200ns（克隆单个字段） |
| 双层 `not(not())` | < 400ns（两次克隆） |
| 字段选择继承 | 0ns（无额外开销） |

### 性能优化建议

```wpl
# ✅ 推荐：使用专用函数（更快）
| chars_not_has(ERROR)

# ⚠️ 可接受：使用 not() 包装（稍慢）
| not(chars_has(ERROR))

# ❌ 不推荐：过度嵌套
| not(not(not(chars_has(ERROR))))  # 无意义的多重否定
```

## 常见陷阱

### 陷阱 1：混淆管道级 not() 和组级 not()

```wpl
# ❌ 错误：这是组级 not()，不是管道级
not(symbol(ERROR):test)

# ✅ 正确：管道级 not() 用于管道函数
(chars:status) | not(chars_has(ERROR))
```

### 陷阱 2：期望 not() 修改字段值

```wpl
# ❌ 误解：以为 not() 会修改字段
(chars:status) | not(chars_has(ERROR))
# 字段值仍然是原始值，not() 只反转匹配结果

# ✅ 正确：如需修改值，使用转换函数
(chars:status) | chars_replace(ERROR, OK)
```

### 陷阱 3：not() 包装非字段函数

```wpl
# ❌ 错误：take 不是字段管道函数
| not(take(field_name))
# 会报错：not() can only wrap field pipe functions

# ✅ 正确：包装字段管道函数
| not(f_has(field_name))
```

## 与组级 not() 的区别

WPL 中有两种 `not()`：

| 特性 | 管道级 `not()` | 组级 `not()` |
|------|---------------|-------------|
| 用途 | 反转管道函数结果 | 反转字段组匹配 |
| 语法位置 | 管道中 `\| not(...)` | 字段组定义 `not(...)` |
| 参数类型 | 管道函数 | 字段定义 |
| 返回结果 | 成功/失败 | ignore 字段 |

**示例对比**：

```wpl
# 管道级 not()：反转函数结果
(chars:status) | not(chars_has(ERROR))

# 组级 not()：字段存在时失败
not(symbol(ERROR):error_marker)
```

## 最佳实践

### 1. 优先使用专用函数

```wpl
# ✅ 推荐：使用 chars_not_has
| chars_not_has(ERROR)

# ⚠️ 次选：使用 not() 包装
| not(chars_has(ERROR))
```

### 2. not() 用于无专用函数的场景

```wpl
# ✅ 推荐：没有 digit_not_in，使用 not()
| not(f_digit_in(port, [80, 443]))

# ✅ 推荐：没有 digit_not_range，使用 not()
| not(digit_range(200, 299))
```

### 3. 组合多个条件

```wpl
# ✅ 推荐：清晰的逻辑组合
| f_has(status)              # 字段必须存在
| not(chars_in([ERROR, FATAL]))  # 且不是错误状态
```

### 4. 避免过度嵌套

```wpl
# ❌ 不推荐：双重否定令人困惑
| not(not(chars_has(OK)))

# ✅ 推荐：直接使用肯定形式
| chars_has(OK)
```

## 故障排查

### 问题：not() 没有反转结果

**可能原因**：混淆了管道级和组级 `not()`

```wpl
# 检查是否在正确位置使用
(chars:status) | not(chars_has(ERROR))  # ✅ 正确
not(chars:status)                        # ❌ 错误（这是组级）
```

### 问题：报错 "can only wrap field pipe functions"

**解决方案**：确保包装的是字段管道函数

```wpl
| not(take(field))        # ❌ take 是选择器
| not(f_has(field))       # ✅ f_has 是管道函数
```

## 版本历史

- **1.15.1** (2026-02-07)
  - 新增 `not()` 管道级包装函数
  - 支持反转任何字段管道函数结果
  - 支持嵌套和自动字段选择

## 相关文档

- [字段存在性检查函数](./function_index.md#字段存在性检查-field-existence)
- [字符串匹配函数](./function_index.md#字符串匹配函数-string-matching)
- [组级 not() 文档](./groups.md)
- [函数索引](./function_index.md)

---

**提示**: `not()` 是一个强大的工具，但不要过度使用。大多数情况下，使用专用的否定函数（如 `chars_not_has`）更直观、性能更好。
