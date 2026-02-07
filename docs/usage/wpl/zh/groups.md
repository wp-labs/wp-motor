# WPL Group 逻辑

WPL 提供了多种 group 包装器，用于控制字段解析的逻辑行为。Group 可以包含一个或多个字段，并根据不同的逻辑语义决定解析成功或失败。

## Group 类型

### seq - 序列（默认）

默认的 group 类型，要求所有字段按顺序成功解析。

**语法：**
```wpl
(field1, field2, field3)
seq(field1, field2, field3)
```

**行为：**
- 按顺序解析所有字段
- 所有字段都必须成功
- 任一字段失败则整个 group 失败

**示例：**
```wpl
(digit:id, chars:name, ip:addr)
```

### opt - 可选

将 group 标记为可选，失败时不影响整体解析。

**语法：**
```wpl
opt(field1, field2)
```

**行为：**
- 尝试解析所有字段
- 解析失败时不返回错误
- 成功时返回解析结果，失败时跳过

**示例：**
```wpl
opt(symbol([DEBUG]):level), chars:msg
```

### alt - 选择

尝试多个解析选项，任一成功即可。

**语法：**
```wpl
alt(field1, field2, field3)
```

**行为：**
- 依次尝试每个字段
- 第一个成功的字段被采用
- 所有字段都失败时，group 失败

**示例：**
```wpl
alt(ip:addr, chars:addr)  # 尝试解析 IP，失败则解析为字符串
```

### some_of - 部分匹配

要求至少一个字段成功即可。

**语法：**
```wpl
some_of(field1, field2, field3)
```

**行为：**
- 尝试解析所有字段
- 至少一个字段成功即可
- 所有字段都失败时，group 失败

**示例：**
```wpl
some_of(digit:port, chars:service)
```

### not - 负向断言

反向逻辑，当内部字段解析失败时才成功。

**语法：**
```wpl
not(field)
```

**行为：**
- 尝试解析内部字段
- 内部字段失败时，not() 成功
- 内部字段成功时，not() 失败
- 成功时返回 `ignore` 类型字段

**输入消费：**
- `not(symbol(...))` - 会消费输入（symbol 在失败时可能消费空白字符）
- `not(peek_symbol(...))` - 不消费输入（peek_symbol 永不消费）

**示例：**
```wpl
# 确保不存在 ERROR 关键字
not(symbol(ERROR):check)

# 与 peek_symbol 配合，不消费输入
not(peek_symbol(ERROR):check), (chars:msg)
```

## 使用场景

### 1. 条件解析

```wpl
# 解析可选的调试信息
opt(symbol([DEBUG]):level), chars:msg
```

### 2. 格式兼容

```wpl
# 支持多种 IP 地址格式
alt(ip:addr, chars:addr)
```

### 3. 负向过滤

```wpl
# 只处理非错误日志
not(symbol(ERROR)), (chars:msg)
```

### 4. 宽松匹配

```wpl
# 至少匹配端口或服务名之一
some_of(digit:port, chars:service)
```

## 组合使用

Group 可以嵌套组合，实现复杂的解析逻辑：

```wpl
# 可选的 IP 或域名
opt(alt(ip:addr, chars:domain))

# 确保不是 ERROR，然后解析消息
not(peek_symbol(ERROR)), (alt(json, kv, chars):msg)
```

## 注意事项

1. **Group 不能嵌套在 Group 内部**
   ```wpl
   # ❌ 错误：不支持嵌套
   (chars, (digit, chars))

   # ✓ 正确：使用多个并列 group
   (chars), (digit, chars)
   ```

2. **not() 只能包含单个字段**
   ```wpl
   # ✓ 正确
   not(symbol(ERROR):check)

   # ❌ 错误
   not(symbol(ERROR), symbol(FATAL))
   ```

3. **输入消费行为取决于内部 parser**
   - 使用 `peek_symbol` 等非消费 parser 可以实现前瞻断言
   - 使用 `symbol`、`digit` 等消费 parser 会改变输入位置
