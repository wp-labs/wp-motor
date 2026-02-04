# OML 使用指南

本目录包含 WP-Motor OML (Object Mapping Language) 语言的使用文档。

## 📚 文档索引

### 核心概念

- **[函数索引](./function_index.md)** - 所有可用 pipe function 的完整列表
- **[Match 表达式函数](./match_functions.md)** - Match 表达式中的函数匹配 ⭐ 新增

### 函数详细文档

#### Match 表达式

- **[match_functions](./match_functions.md)** - Match 表达式函数匹配完整指南 ⭐ 新增
  - 字符串匹配：`starts_with`, `ends_with`, `contains`, `regex_match`, `is_empty`, `iequals`
  - 数值比较：`gt`, `lt`, `eq`, `in_range`
  - 适合日志分类、路由决策、条件判断等场景

#### 字符串匹配

- **[starts_with](./starts_with.md)** - 字符串前缀匹配函数 ⭐ 新增
  - 基本用法：`starts_with('prefix')`
  - 前缀匹配，失败时转换为 ignore 类型
  - 大小写敏感
  - 适合 URL 协议过滤、路径前缀检查等场景

#### 值映射

- **[map_to](./map_to.md)** - 类型感知的条件值赋值函数 ⭐ 新增
  - 基本用法：`map_to(value)`
  - 支持多种类型：字符串、整数、浮点数、布尔值
  - 自动类型推断
  - 保留 ignore 字段
  - 适合条件标记、分类映射、优先级赋值等场景

## 🚀 快速开始

### 基本管道结构

**注意**：`pipe` 关键字是可选的，可以省略直接写管道函数链。

```oml
name : my_rule
---
# 完整写法（带 pipe 关键字）
result = pipe take(source_field)
    | starts_with('prefix')
    | map_to('mapped_value');

# 简化写法（省略 pipe 关键字） - 推荐
result = take(source_field)
    | starts_with('prefix')
    | map_to('mapped_value');
```

### 常用模式

#### 模式 1: URL 协议过滤

```oml
name : filter_https
---
# 简化写法（推荐）
secure_url = take(url) | starts_with('https://');

# 完整写法
# secure_url = pipe take(url) | starts_with('https://');
```

#### 模式 2: 条件标记

```oml
name : mark_secure
---
# 简化写法（推荐）
is_secure = take(url)
    | starts_with('https://')
    | map_to(true);
```

#### 模式 3: 多条件分类

```oml
name : classify_protocols
---
# HTTP 分类
http_level = take(url) | starts_with('http://') | map_to(1);

# HTTPS 分类
https_level = take(url) | starts_with('https://') | map_to(3);

# FTP 分类
ftp_level = take(url) | starts_with('ftp://') | map_to(2);
```

#### 模式 4: 路径规范化

```oml
name : normalize_paths
---
# 只接受 API v1 路径
api_v1 = take(path) | starts_with('/api/v1/');

# 只接受 API v2 路径
api_v2 = take(path) | starts_with('/api/v2/');
```

## 📖 函数选择指南

### 字符串处理

| 需求 | 推荐函数 | 示例 |
|------|----------|------|
| 前缀匹配 | `starts_with` | `starts_with('https://')` |
| 类型转换 | `to_str` | `to_str` |
| JSON 转换 | `to_json` | `to_json` |

### 值映射

| 需求 | 推荐函数 | 示例 |
|------|----------|------|
| 映射到字符串 | `map_to` | `map_to('value')` |
| 映射到整数 | `map_to` | `map_to(123)` |
| 映射到浮点数 | `map_to` | `map_to(3.14)` |
| 映射到布尔值 | `map_to` | `map_to(true)` |

### 性能优先级

1. **最快**：`take`, `get`, `nth` (< 100ns)
2. **快**：`starts_with`, `map_to`, `skip_empty` (< 1μs)
3. **中等**：`base64_encode`, `base64_decode`, `to_json` (1-10μs)
4. **较慢**：`Time::to_ts*`, `url`, `path` (1-10μs)

**建议**：合理使用管道链，避免不必要的转换操作。

## ⚠️ 常见陷阱

### 1. 字符串未加引号

```oml
# ❌ 错误：字符串未加引号
starts_with(https://)  # 语法错误

# ✅ 正确：使用引号
starts_with('https://')
```

### 2. map_to 类型混淆

```oml
# ❌ 错误：布尔值加引号
map_to('true')  # 这是字符串，不是布尔值

# ✅ 正确：布尔值不加引号
map_to(true)
```

### 3. ignore 字段传播

```oml
# 理解 ignore 的传播机制
result = pipe take(url)
    | starts_with('https://')  # 失败时返回 ignore
    | map_to('secure');       # ignore 会跳过此步骤

# 如果 url 不是 https://，result 最终为 ignore
```

### 4. 整数与浮点数

```oml
# map_to(100) 是整数 (Digit)
priority = pipe take(field) | map_to(100);

# map_to(100.0) 是浮点数 (Float)
threshold = pipe take(field) | map_to(100.0);
```

## 💡 临时字段

OML 支持使用临时字段进行中间计算，这些字段在最终输出时会被自动过滤。

### 临时字段规则

- **命名规则**：字段名以 `__` （双下划线）开头的字段被视为临时字段
- **正常使用**：临时字段可以在规则中正常使用和引用
- **自动过滤**：转换完成后，临时字段会自动标记为 `ignore` 类型
- **零成本**：无临时字段时几乎无性能开销（~1ns）

### 性能特性

OML 采用**解析时检测 + 运行时条件过滤**的优化策略：

| 场景 | 性能开销 | 说明 |
|------|---------|------|
| 无临时字段 | **~1ns** | 仅条件检查，99%+ 成本节省 |
| 有临时字段 | ~500ns | 执行过滤逻辑 |
| 解析时检测 | ~50-500ns | 一次性成本，可忽略 |

**优化效果**：
- 在解析阶段检测是否使用了临时字段
- 运行时仅在必要时执行过滤
- 大多数场景（无临时字段）几乎零开销

### 使用示例

```oml
name : example
---
# 定义临时字段用于中间计算
__protocol = take(url) | starts_with('https://') | map_to('https');
__is_secure = match read(__protocol) {
    chars(https) => chars(true),
    _ => chars(false),
};

# 最终输出字段
security_level = match read(__is_secure) {
    chars(true) => chars(high),
    _ => chars(low),
};
```

**输出结果**：
- `__protocol` - ignore 类型（自动过滤）
- `__is_secure` - ignore 类型（自动过滤）
- `security_level` - 正常输出

### 使用场景

1. **复杂计算分解**：将复杂逻辑分解为多个步骤
2. **中间状态保存**：保存中间计算结果供后续使用
3. **避免重复计算**：将公共计算结果存储在临时字段中
4. **提高可读性**：通过命名临时字段使规则更易理解

### 最佳实践

```oml
name : best_practice
---
# ✅ 推荐：使用临时字段分解复杂逻辑
__url_type = match read(url) {
    starts_with('https://') => chars(secure),
    starts_with('http://') => chars(insecure),
    _ => chars(unknown),
};

__port = take(port) | map_to(443);

final_endpoint = fmt("{}://{}", @__url_type, @__port);

# ❌ 不推荐：复杂的嵌套表达式
# final_endpoint = fmt("{}://{}",
#     match read(url) { starts_with('https://') => chars(secure), ... },
#     take(port) | map_to(443)
# );
```

## 🔧 调试技巧

### 1. 分步测试

```oml
name : debug_step_by_step
---
# 先测试提取
step1 = pipe take(url);

# 再测试过滤
step2 = pipe take(url) | starts_with('https://');

# 最后测试映射
step3 = pipe take(url) | starts_with('https://') | map_to(true);
```

### 2. 验证类型推断

```oml
name : verify_types
---
# 验证字符串
str_test = pipe take(field) | map_to('test');

# 验证整数
int_test = pipe take(field) | map_to(123);

# 验证浮点数
float_test = pipe take(field) | map_to(3.14);

# 验证布尔值
bool_test = pipe take(field) | map_to(true);
```

### 3. 检查 ignore 传播

```oml
name : check_ignore
---
# 如果 starts_with 失败，result 应该是 ignore
result = pipe take(url)
    | starts_with('https://')
    | map_to('secure');

# 可以通过日志查看字段是否为 ignore
```

## 📝 开发指南

如果你想开发新的 pipe function，请参考：

- **[OML Pipe Function 开发指南](../../../guide/zh/oml_pipefun_development_guide.md)**
  - 完整的开发流程
  - 代码示例
  - 测试方法
  - 最佳实践

## 🆕 最新更新

### v1.13.4 (2026-02-04)

- ⭐ **新增** Match 表达式函数匹配支持
  - 字符串匹配：`starts_with`, `ends_with`, `contains`, `regex_match`, `is_empty`, `iequals`
  - 数值比较：`gt`, `lt`, `eq`, `in_range`
- ⭐ **新增** `starts_with` pipe 函数 - 字符串前缀匹配
- ⭐ **新增** `map_to` pipe 函数 - 类型感知的条件值赋值
- ⭐ **新增** 引号字符串支持 - `chars('hello world')` 支持包含空格的字符串
- ⭐ **新增** 临时字段自动过滤 - 以 `__` 开头的字段自动标记为 ignore
- 🔧 **改进** `pipe` 关键字变为可选 - 可简写为 `take(field) | func`
- 📖 完善使用文档和示例

## 📞 获取帮助

- **Issues**: https://github.com/wp-labs/wp-motor/issues
- **Documentation**: `/docs`
- **Examples**: `/examples`

## 相关链接

- [主文档](../../README.md)
- [开发指南](../../../guide/)
- [WPL 使用文档](../../wpl/)

---

**提示**: 从 [函数索引](./function_index.md) 开始，快速了解所有可用函数。
