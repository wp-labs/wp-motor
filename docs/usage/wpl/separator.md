# WPL 分隔符使用指南

## 概述

分隔符（Separator）是 WPL 中用于分割日志字段的关键语法元素。通过灵活使用分隔符，可以解析各种格式的结构化日志数据。

## 快速开始

### 基本语法

```wpl
| take(field_name) separator
```

分隔符写在字段定义之后，用于指示该字段在何处结束。

### 简单示例

```wpl
# 使用空格分隔
| take(ip) \s
| take(method) \s
| take(path)

# 使用逗号分隔
| take(name) ,
| take(age) ,
| take(city)
```

## 内置分隔符

### 1. 空格分隔符 `\s`

匹配单个空格字符。

```wpl
# 输入: "192.168.1.1 GET /api/users"
rule parse_log {
    | take(ip) \s
    | take(method) \s
    | take(path)
}
```

**适用场景**：
- Apache/Nginx 访问日志
- 空格分隔的简单日志
- 标准格式日志文件

### 2. 制表符分隔符 `\t`

匹配单个制表符（Tab）字符。

```wpl
# 输入: "user001\t25\tBeijing"
rule parse_tsv {
    | take(user_id) \t
    | take(age) \t
    | take(city)
}
```

**适用场景**：
- TSV（Tab-Separated Values）文件
- 数据库导出文件
- 制表符对齐的日志

### 3. 通用空白分隔符 `\S`

匹配空格**或**制表符（二选一）。

```wpl
# 输入: "field1 field2\tfield3"（混合空格和制表符）
rule parse_flexible {
    | take(col1) \S
    | take(col2) \S
    | take(col3)
}
```

**适用场景**：
- 格式不统一的日志（混合空格和 Tab）
- 手工编辑过的配置文件
- 宽松的数据解析

**行为说明**：
- 遇到空格或制表符都会停止
- 灵活处理格式不一致的数据源

### 4. 行尾分隔符 `\0`

读取到行尾（换行符或字符串结束）。

```wpl
# 输入: "prefix_value some remaining text until end"
rule parse_to_end {
    | take(prefix) _
    | take(remaining) \0
}
```

**适用场景**：
- 解析最后一个字段
- 读取消息正文
- 获取剩余所有内容

**别名**：
- `\0` 和 `0` 等价

### 5. 自定义字符分隔符

使用任意单个字符作为分隔符。

```wpl
# 逗号分隔
| take(name) ,
| take(age) ,

# 竖线分隔
| take(id) |
| take(status) |

# 分号分隔
| take(key) ;
| take(value) ;
```

**支持的字符**：
- 逗号 `,`
- 竖线 `|`
- 分号 `;`
- 冒号 `:`
- 等号 `=`
- 斜杠 `/`
- 等任意单字符

### 6. 自定义字符串分隔符

使用多字符字符串作为分隔符。

```wpl
# 使用 " | " 分隔
| take(field1) " | "
| take(field2) " | "

# 使用 " :: " 分隔
| take(module) " :: "
| take(function) " :: "
```

**适用场景**：
- 格式化输出的日志
- 特定格式协议
- 需要明确边界的数据

## 实际应用场景

### 场景 1：解析 Nginx 访问日志

```wpl
# 日志格式: 192.168.1.1 - - [29/Jan/2024:10:30:45 +0800] "GET /api/users HTTP/1.1" 200 1234
rule nginx_access_log {
    | take(client_ip) \s
    | take(identity) \s
    | take(user) \s
    | take(timestamp) \s
    | take(request) \s
    | take(status_code) \s
    | take(bytes_sent) \0
}
```

### 场景 2：解析 TSV 数据

```wpl
# 输入: "2024-01-29\t10:30:45\tERROR\tDatabase connection failed"
rule tsv_log {
    | take(date) \t
    | take(time) \t
    | take(level) \t
    | take(message) \0
}
```

### 场景 3：解析 CSV 数据

```wpl
# 输入: "John Smith,30,New York,Engineer"
rule csv_parser {
    | take(name) ,
    | take(age) ,
    | take(city) ,
    | take(job) \0
}
```

### 场景 4：解析结构化日志

```wpl
# 输入: "level=error | module=database | msg=Connection timeout"
rule structured_log {
    | take(level_prefix) =
    | take(level_value) " | "
    | take(module_prefix) =
    | take(module_value) " | "
    | take(msg_prefix) =
    | take(message) \0
}
```

### 场景 5：处理混合空白的日志

```wpl
# 输入: "192.168.1.1 \tGET\t /api/data"（混合空格和制表符）
rule flexible_whitespace {
    | take(ip) \S
    | take(method) \S
    | take(path) \0
}
```

### 场景 6：解析 Syslog 格式

```wpl
# 输入: "Jan 29 10:30:45 hostname app[1234]: Error message here"
rule syslog {
    | take(month) \s
    | take(day) \s
    | take(time) \s
    | take(hostname) \s
    | take(app_tag) ": "
    | take(message) \0
}
```

### 场景 7：解析键值对日志

```wpl
# 输入: "user=admin;action=login;ip=192.168.1.1;status=success"
rule kv_log {
    | take(user_key) =
    | take(user_value) ;
    | take(action_key) =
    | take(action_value) ;
    | take(ip_key) =
    | take(ip_value) ;
    | take(status_key) =
    | take(status_value) \0
}
```

## 分隔符优先级

WPL 支持三个级别的分隔符设置：

### 1. 字段级分隔符（优先级 3，最高）

```wpl
| take(field1) ,  # 该字段使用逗号
| take(field2) \s # 该字段使用空格
```

### 2. 组级分隔符（优先级 2）

```wpl
group {
    sep = \t  # 组内所有字段默认使用制表符
    | take(field1)
    | take(field2)
}
```

### 3. 继承分隔符（优先级 1，最低）

从上游规则继承的默认分隔符。

### 优先级规则

字段级 > 组级 > 继承级

```wpl
group {
    sep = \t         # 组级：制表符
    | take(f1)       # 使用 \t
    | take(f2) ,     # 使用 ,（字段级覆盖组级）
    | take(f3)       # 使用 \t
}
```

## 分隔符行为

### 全局替换 vs 单次匹配

分隔符只在当前字段结束位置匹配一次：

```wpl
# 输入: "hello,world,test"
| take(first) ,   # 读取 "hello"，消费第一个逗号
| take(second) ,  # 读取 "world"，消费第二个逗号
| take(third) \0  # 读取 "test"
```

### 分隔符消费行为

默认情况下，分隔符会被**消费**（从输入中移除）：

```wpl
# 输入: "a,b,c"
| take(x) ,  # 读取 "a"，消费 ","，剩余 "b,c"
| take(y) ,  # 读取 "b"，消费 ","，剩余 "c"
```

### 分隔符不存在的情况

如果到达字符串末尾仍未找到分隔符，读取到末尾：

```wpl
# 输入: "field1 field2"
| take(f1) ,  # 未找到逗号，读取全部 "field1 field2"
```

## 高级用法

### 组合使用多种分隔符

```wpl
# 输入: "192.168.1.1:8080/api/users?id=123"
rule url_parse {
    | take(ip) :
    | take(port) /
    | take(api_path) /
    | take(resource) ?
    | take(query_string) \0
}
```

### 处理可选字段

```wpl
# 输入可能是: "user,30,city" 或 "user,,city"（age 为空）
rule optional_fields {
    | take(name) ,
    | take(age) ,      # 可能为空字符串
    | take(city) \0
}
```

### 跳过不需要的字段

```wpl
# 只提取第 1 和第 3 个字段
rule skip_fields {
    | take(field1) ,
    | take(_skip) ,    # 临时变量，不保存
    | take(field3) \0
}
```

## 使用限制

### 1. 分隔符不支持正则表达式

```wpl
# ❌ 不支持正则
| take(field) [0-9]+

# ✅ 使用固定字符串
| take(field) \s
```

### 2. 分隔符区分大小写

```wpl
# "ABC" 和 "abc" 是不同的分隔符
| take(field1) ABC
| take(field2) abc
```

### 3. 空字符串不能作为分隔符

```wpl
# ❌ 不支持
| take(field) ""

# ✅ 使用 \0 读取到末尾
| take(field) \0
```

### 4. 转义字符限制

当前支持的转义字符：
- `\s` - 空格
- `\t` - 制表符
- `\S` - 空格或制表符
- `\0` - 行尾

其他转义字符（如 `\n`、`\r`）需要使用实际字符。

## 性能说明

### 单字符分隔符

性能最优，推荐优先使用：

```wpl
| take(f1) ,
| take(f2) \s
| take(f3) \t
```

- 时间复杂度：O(n)
- 扫描速度：约 500 MB/s

### 多字符分隔符

性能略低，但仍然高效：

```wpl
| take(f1) " | "
| take(f2) " :: "
```

- 时间复杂度：O(n × m)，m 为分隔符长度
- 扫描速度：约 300-400 MB/s

### 通用空白分隔符 `\S`

需要逐字符检查，性能介于两者之间：

```wpl
| take(f1) \S
```

- 时间复杂度：O(n)
- 扫描速度：约 400 MB/s

## 错误处理

### 常见错误

#### 1. 分隔符未找到

```
错误: 未找到分隔符 ','
原因: 输入字符串中不包含指定的分隔符
解决: 检查输入格式或使用 \0 读取到末尾
```

#### 2. 分隔符语法错误

```
错误: invalid separator
原因: 使用了不支持的分隔符语法
解决: 参考本文档使用正确的分隔符格式
```

#### 3. 字段顺序错误

```
错误: 字段解析失败
原因: 字段定义顺序与实际数据不匹配
解决: 调整字段顺序以匹配输入格式
```

## 最佳实践

### 1. 优先使用内置分隔符

```wpl
# ✅ 推荐
| take(f1) \s
| take(f2) \t

# ⚠️ 避免（除非必要）
| take(f1) " "
| take(f2) "\t"
```

### 2. 明确指定最后字段的分隔符

```wpl
# ✅ 推荐（明确到行尾）
| take(message) \0

# ⚠️ 不清晰
| take(message)  # 依赖默认行为
```

### 3. 使用 `\S` 处理不规范数据

```wpl
# ✅ 推荐（兼容性好）
| take(field1) \S
| take(field2) \S

# ⚠️ 可能失败（如果混合了空格和制表符）
| take(field1) \s
| take(field2) \s
```

### 4. 复杂格式使用多字符分隔符

```wpl
# ✅ 清晰准确
| take(level) " | "
| take(message) " | "

# ⚠️ 可能误匹配
| take(level) |
| take(message) |
```

### 5. 组合使用字段级和组级分隔符

```wpl
# ✅ 推荐（减少重复）
group {
    sep = ,
    | take(f1)
    | take(f2)
    | take(f3) \0  # 最后字段使用不同分隔符
}
```

## 调试技巧

### 1. 逐字段验证

```wpl
# 先解析第一个字段
| take(field1) ,

# 确认成功后添加第二个
| take(field1) ,
| take(field2) ,

# 依次添加...
```

### 2. 使用临时字段查看中间结果

```wpl
| take(field1) ,
| take(_debug) ,    # 临时字段，查看剩余内容
| take(field2) \0
```

### 3. 打印分隔符位置

在测试环境中，使用调试模式查看分隔符匹配情况：

```bash
wp-motor --debug rule.wpl < test.log
```

### 4. 验证分隔符字符

对于不可见字符（如制表符），使用十六进制查看器确认：

```bash
# 查看文件中的制表符
cat -A test.log
# 或
hexdump -C test.log | head
```

## 常见问题 (FAQ)

### Q1: `\s` 和 `\S` 有什么区别？

- `\s`：只匹配空格 (space)
- `\S`：匹配空格或制表符 (space OR tab)

```wpl
# 输入: "a b"
| take(x) \s  # ✅ 匹配成功

# 输入: "a\tb"
| take(x) \s  # ❌ 匹配失败（这是制表符，不是空格）
| take(x) \S  # ✅ 匹配成功
```

### Q2: 如何处理连续的分隔符？

WPL 会将连续分隔符视为多个空字段：

```wpl
# 输入: "a,,c"
| take(f1) ,  # 读取 "a"
| take(f2) ,  # 读取 ""（空字符串）
| take(f3) \0 # 读取 "c"
```

### Q3: 分隔符会影响性能吗？

单字符分隔符性能最优，多字符分隔符略慢，但对于大多数场景影响可忽略。

### Q4: 如何解析嵌套结构？

使用多级分隔符：

```wpl
# 输入: "k1=v1;k2=v2|k3=v3;k4=v4"
rule nested {
    | take(group1) |
    | take(group2) \0
}
# 然后在每个 group 内再用 ; 和 = 解析
```

### Q5: 分隔符可以是中文吗？

可以，支持 Unicode 字符：

```wpl
# 使用中文逗号分隔
| take(field1) ，
| take(field2) ，
```

### Q6: `\0` 和省略分隔符有区别吗？

建议显式使用 `\0`，语义更清晰：

```wpl
# ✅ 推荐（明确）
| take(message) \0

# ⚠️ 可以但不明确
| take(message)
```

### Q7: 如何处理引号内的分隔符？

对于包含引号的复杂格式，建议使用专门的解析器（如 JSON、KV 解析器）：

```wpl
# 复杂 CSV（带引号）
# 输入: "field1","field with , comma","field3"
# 建议使用 CSV 解析器而非手动分隔符
```

## 更多资源

- **WPL 语法参考**: `docs/guide/wpl_syntax.md`
- **解析器开发指南**: `docs/guide/wpl_field_func_development_guide.md`
- **chars_replace 使用指南**: `docs/usage/wpl/chars_replace.md`
- **源代码**: `crates/wp-lang/src/ast/syntax/wpl_sep.rs`

## 版本历史

- **1.11.0** (2026-01-29)
  - 新增 `\t` 制表符分隔符支持
  - 新增 `\S` 通用空白分隔符（空格或制表符）
  - 优化 Whitespace 分隔符性能
  - 添加完整的测试覆盖

- **1.10.x** 及更早版本
  - 支持 `\s`（空格）和 `\0`（行尾）
  - 支持自定义字符和字符串分隔符

---

**提示**: 分隔符是 WPL 解析的核心，选择合适的分隔符可以大大简化日志解析规则。
