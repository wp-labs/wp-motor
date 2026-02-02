# digit_range 函数使用指南

## 概述

`digit_range` 是 WPL (WP Language) 中的数值范围检查函数，用于判断日志字段的数值是否在指定的单个范围内。这是一个简单高效的闭区间检查函数。

## 快速开始

### 基本语法

```wpl
digit_range(begin, end)
```

- **begin**: 范围下界（标量值）
- **end**: 范围上界（标量值）
- 检查方式：`begin <= value <= end`（闭区间）

### 简单示例

```wpl
# 检查数值是否在 [0, 100] 范围内
digit_range(0, 100)

# 检查 HTTP 成功状态码（200-299）
digit_range(200, 299)

# 检查端口号是否在标准范围内（0-65535）
digit_range(0, 65535)
```

## 参数格式

### 1. 单个范围

检查单个连续范围：

```wpl
# 检查端口号是否在标准范围内（0-65535）
digit_range(0, 65535)

# 检查 HTTP 成功状态码（200-299）
digit_range(200, 299)

# 检查年龄是否成年（18-150）
digit_range(18, 150)
```

### 2. 范围限制说明

**注意**: 从 1.13.1 版本开始，`digit_range` 仅支持单个范围检查。如需检查多个不连续的范围，请使用多个规则或分支逻辑（见下文"与分支逻辑配合"部分）。

```wpl
# ✅ 单个范围检查
digit_range(200, 299)  # 2xx 状态码

# ❌ 不再支持多范围数组语法
# digit_range([200, 300], [299, 399])  # 旧版本语法，已废弃
```

### 3. 边界值处理

范围检查是**闭区间**，包含边界值：

```wpl
# [100, 200] - 包含 100 和 200
digit_range(100, 200)

# 检查值 100: ✅ 通过（等于下界）
# 检查值 200: ✅ 通过（等于上界）
# 检查值 150: ✅ 通过（在范围内）
# 检查值 99:  ❌ 失败（小于下界）
# 检查值 201: ❌ 失败（大于上界）
```

### 4. 负数范围

支持负数范围：

```wpl
# 温度范围检查（-20°C 到 40°C）
digit_range(-20, 40)

# 海拔范围（死海 -400m 到 珠峰 8848m）
digit_range(-400, 8848)
```

## 实际应用场景

### 场景 1：HTTP 状态码分类

```wpl
rule http_success_check {
    # 选择状态码字段
    | take(status_code)

    # 检查是否为成功状态码（2xx）
    | digit_range(200, 299)
}

# 示例数据：
# status_code: 200  → ✅ 通过（在 [200,299] 范围内）
# status_code: 204  → ✅ 通过（在 [200,299] 范围内）
# status_code: 301  → ❌ 失败（不在范围内）
# status_code: 404  → ❌ 失败（不在范围内）

# 如需检查多个状态码范围（如 2xx 或 3xx），使用分支逻辑：
rule http_ok_or_redirect {
    | take(status_code)
    | (digit_range(200, 299) | digit_range(300, 399))
}
```

### 场景 2：性能指标监控

```wpl
rule response_time_check {
    # 选择响应时间字段（毫秒）
    | take(response_time)

    # 检查响应时间是否在正常范围（0-500ms）
    | digit_range(0, 500)
}

# 示例数据：
# response_time: 50   → ✅ 通过（快速响应）
# response_time: 200  → ✅ 通过（正常响应）
# response_time: 1000 → ❌ 失败（超时）
```

### 场景 3：端口号验证

```wpl
rule system_port_check {
    # 选择端口字段
    | take(port)

    # 检查是否为系统保留端口（1-1023）
    | digit_range(1, 1023)
}

# 示例数据：
# port: 80    → ✅ 通过（HTTP 默认端口）
# port: 443   → ✅ 通过（HTTPS 默认端口）
# port: 8080  → ❌ 失败（用户端口）
```

### 场景 4：时间段过滤

```wpl
rule morning_hours_check {
    # 选择小时字段
    | take(hour)

    # 检查是否在上午工作时间（9-12）
    | digit_range(9, 12)
}

# 示例数据：
# hour: 10  → ✅ 通过（上午工作时间）
# hour: 11  → ✅ 通过（上午工作时间）
# hour: 15  → ❌ 失败（下午时间）

# 检查多个时间段，使用分支逻辑：
rule business_hours_check {
    | take(hour)
    | (digit_range(9, 12) | digit_range(14, 18))
}
```

### 场景 5：年龄分段

```wpl
rule adult_age_check {
    # 选择年龄字段
    | take(age)

    # 成年人年龄段（18-65）
    | digit_range(18, 65)
}

# 示例数据：
# age: 30  → ✅ 通过（成年人）
# age: 50  → ✅ 通过（成年人）
# age: 15  → ❌ 失败（未成年）
# age: 70  → ❌ 失败（老年人）
```

### 场景 6：优先级过滤

```wpl
rule high_priority_filter {
    # 选择优先级字段
    | take(priority)

    # 只处理高优先级（1-3）
    | digit_range(1, 3)
}

# 示例数据：
# priority: 1  → ✅ 通过（高优先级）
# priority: 3  → ✅ 通过（高优先级）
# priority: 5  → ❌ 失败（中优先级）
```

### 场景 7：数据质量检查

```wpl
rule data_quality_check {
    # 检查温度传感器数据
    | take(temperature)

    # 正常温度范围（-40°C 到 80°C）
    | digit_range(-40, 80)
}

# 示例数据：
# temperature: 25   → ✅ 通过（正常室温）
# temperature: -10  → ✅ 通过（冬季温度）
# temperature: 100  → ❌ 失败（异常数据）
# temperature: -100 → ❌ 失败（传感器故障）
```

## 使用限制

### 类型限制

`digit_range` 只能处理**数值类型**的字段：

```wpl
# ✅ 正确 - 字段是数字
status_code: 200 -> digit_range(200, 299)

# ❌ 错误 - 字段是字符串
level: "200" -> digit_range(200, 299)  # 会失败

# ❌ 错误 - 字段是 IP 地址
ip: 192.168.1.1 -> digit_range(192, 200)  # 会失败
```

### 参数要求

1. **参数必须是标量值**：
   ```wpl
   # ✅ 正确 - 使用标量值
   digit_range(1, 10)

   # ❌ 错误 - 不支持数组参数（旧版本语法）
   digit_range([1], [10])  # 已废弃
   ```

2. **范围下界应小于等于上界**：
   ```wpl
   # ✅ 正确
   digit_range(1, 10)      # 1 <= x <= 10
   digit_range(10, 10)     # x == 10（单点）

   # ⚠️ 逻辑错误（不会匹配任何值）
   digit_range(10, 1)      # 10 <= x <= 1（永远为假）
   ```

### 不支持的特性

1. **不支持浮点数精确匹配**：
   ```wpl
   # ⚠️ 注意：内部使用 i64，浮点数会被舍入
   digit_range(1, 10)  # 只能匹配整数
   ```

2. **不支持无限范围**：
   ```wpl
   # ❌ 不支持
   digit_range(0, infinity)  # 没有无限值
   ```

3. **不支持多范围数组**：
   ```wpl
   # ❌ 不支持（旧版本语法已废弃）
   digit_range([1, 100], [10, 200])  # 请使用分支逻辑替代
   ```

## 完整示例

### 示例 1：日志严重性过滤

```wpl
rule log_error_filter {
    # 选择严重性级别字段
    | take(severity)

    # 过滤 ERROR 级别（1-2）
    | digit_range(1, 2)

    # 后续处理...
}

# 日志级别定义：
# 1 = CRITICAL
# 2 = ERROR
# 3 = WARNING
# 4 = WARN
# 5 = INFO
# 6 = DEBUG
```

### 示例 2：性能监控组合

```wpl
rule performance_monitor {
    # 检查响应时间
    | take(response_ms)
    | digit_range(0, 1000)  # 0-1000ms 认为正常

    # 检查状态码
    | take(status)
    | digit_range(200, 299)  # 2xx

    # 两个条件都满足才通过
}
```

### 示例 3：时间窗口分析

```wpl
rule weekday_check {
    # 工作日检查（1=周一, 7=周日）
    | take(day_of_week)
    | digit_range(1, 5)  # 周一到周五

    # 只分析工作日数据
}
```

## 性能说明

- **时间复杂度**：O(1) - 单次比较
- **空间复杂度**：O(1) - 原地检查
- **性能特点**：
  - 纳秒级执行时间
  - 简单的整数比较操作
  - 性能极佳，适合高频调用

## 错误处理

### 常见错误

1. **字段不存在**
   ```
   错误: <pipe> | not in range
   原因: 当前没有活动字段
   解决: 使用 take() 先选择字段
   ```

2. **字段类型不匹配**
   ```
   错误: <pipe> | not in range
   原因: 字段不是数字类型（Digit）
   解决: 确保字段是数值类型
   ```

3. **数值不在任何范围内**
   ```
   错误: <pipe> | not in range
   原因: 字段值不满足任何一个范围条件
   解决: 检查范围设置是否正确
   ```

## 与其他函数配合使用

### 与字段选择器配合

```wpl
# 先选择字段，再检查范围
| take(status_code)
| digit_range(200, 299)
```

### 与条件检查配合

```wpl
# 先检查字段存在，再检查范围
| has()
| digit_range(0, 100)
```

### 与转换函数配合

```wpl
# 组合使用进行复杂验证
| take(response_time)
| digit_range(0, 1000)  # 响应时间正常
| take(status_code)
| digit_range(200, 299)  # 状态码正常
```

### 与分支逻辑配合

```wpl
# 使用 alt 实现"或"逻辑 - 检查多个不连续范围
(
    # 分支 1：检查是否为成功状态码
    | take(status)
    | digit_range(200, 299)
)
|
(
    # 分支 2：检查是否为重定向状态码
    | take(status)
    | digit_range(300, 399)
)
```

## 最佳实践

### 1. 范围设计原则

```wpl
# ✅ 推荐：语义清晰的范围
digit_range(200, 299)  # HTTP 2xx 状态码

# ⚠️ 避免：使用旧的数组语法
# digit_range([200], [299])  # 已废弃
```

### 2. 简洁明了的单范围

```wpl
# ✅ 推荐：简单直接的单范围
digit_range(0, 100)

# ✅ 推荐：多个范围使用分支逻辑
(digit_range(0, 50) | digit_range(100, 150))
```

### 3. 范围的可读性

```wpl
# ✅ 推荐：添加注释说明范围含义
| digit_range(200, 299)  # HTTP 成功状态码

# ✅ 推荐：使用有意义的范围
| digit_range(18, 65)  # 工作年龄段
```

### 4. 边界值测试

```wpl
# 测试边界值是否符合预期
digit_range(100, 200)
# 测试：100 ✅, 200 ✅, 99 ❌, 201 ❌
```

### 5. 使用分支处理不连续范围

```wpl
# ✅ 推荐：清晰的分支逻辑
(
    digit_range(1, 10)   # 第一个范围
    |
    digit_range(50, 100) # 第二个范围
)

# ❌ 避免：使用已废弃的数组语法
# digit_range([1, 50], [10, 100])
```

## 调试技巧

### 1. 测试单个范围

```wpl
# 从简单的单个范围开始
| digit_range(0, 100)

# 如需多个范围，使用分支逻辑
| (digit_range(0, 100) | digit_range(200, 300))
```

### 2. 验证字段类型

```wpl
# 使用 digit_has() 确认字段是数字类型
| take(my_field)
| digit_has(0)  # 如果失败，说明不是数字字段
```

### 3. 检查边界值

```bash
# 准备测试数据
echo "value: 99" | wp-motor test.wpl    # 测试下界-1
echo "value: 100" | wp-motor test.wpl   # 测试下界
echo "value: 200" | wp-motor test.wpl   # 测试上界
echo "value: 201" | wp-motor test.wpl   # 测试上界+1
```

## 常见问题 (FAQ)

### Q1: 如何检查单个特定值？

```wpl
# 将下界和上界设为相同值
digit_range(200, 200)  # 只匹配 200
```

### Q2: 范围可以倒序吗？

技术上可以，但逻辑上没有意义：

```wpl
digit_range(100, 50)  # begin > end，永远不匹配
```

### Q3: 如何实现"不在范围内"的逻辑？

WPL 中管道失败会中断，可以使用分支逻辑：

```wpl
# 使用否定逻辑（复杂）
# 建议：使用其他字段函数或在应用层处理
```

### Q4: 支持浮点数吗？

内部使用 `i64`，浮点数会被转换：

```wpl
# 字段值 3.14 会被视为 3
digit_range(3, 4)  # 可能匹配 3.14（取决于解析方式）
```

### Q5: 如何检查多个不连续的范围？

使用分支逻辑（alt 操作符）：

```wpl
# 检查 [1,10] 或 [100,200] 范围
(digit_range(1, 10) | digit_range(100, 200))
# 匹配：[1,10] 或 [100,200] 内的任何值
```

### Q6: 性能够用吗？

非常快！范围检查是简单的数值比较：
- O(1) 时间复杂度
- 纳秒级执行时间
- 适合高频调用场景

### Q7: 旧的数组语法还能用吗？

不推荐使用，已废弃：

```wpl
# ❌ 旧语法（已废弃）
digit_range([200], [299])

# ✅ 新语法（推荐）
digit_range(200, 299)
```

## 与 digit_in 的对比

| 特性 | digit_range | digit_in |
|------|------------|----------|
| 用途 | 单个范围检查 | 集合成员检查 |
| 参数 | 两个标量（begin, end） | 一个数组（值列表） |
| 适用场景 | 连续范围 | 离散值 |
| 示例 | `digit_range(0, 100)` | `digit_in([200, 404, 500])` |
| 复杂度 | O(1) | O(n) |

```wpl
# digit_range - 适合连续范围
digit_range(200, 299)  # 200, 201, ..., 299

# digit_in - 适合离散值
digit_in([200, 404, 500])  # 只匹配这三个值

# 多个不连续范围 - 使用分支逻辑
(digit_range(200, 299) | digit_range(300, 399))
```

## 更多资源

- **开发指南**: `docs/guide/wpl_field_func_development_guide.md`
- **源代码**: `crates/wp-lang/src/ast/processor/function.rs`
- **测试用例**: `crates/wp-lang/src/eval/builtins/pipe_fun.rs`

## 版本历史

- **1.13.1** (2026-02-02)
  - 重构为双参数形式：`digit_range(begin, end)`
  - 废弃旧的数组语法：`digit_range([begins], [ends])`
  - 简化为单范围检查，性能优化至 O(1)
  - 支持负数范围
  - 添加完整的测试覆盖

---

**提示**: `digit_range` 现在是一个简单高效的单范围检查函数，适合处理连续的数值范围验证场景，如状态码、性能指标、时间段等。对于多个不连续范围，请使用分支逻辑（alt 操作符）。对于离散值检查，请使用 `digit_in` 函数。
