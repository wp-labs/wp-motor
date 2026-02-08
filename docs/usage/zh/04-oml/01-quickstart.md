# OML 快速入门

> 5 分钟快速上手 OML（Object Modeling Language）

**💡 提示**：想要查看所有功能的完整演示？请访问 **[完整功能示例](./07-complete-example.md)**

---

## 📚 快速导航

| 章节 | 内容 |
|------|------|
| [什么是 OML](#什么是-oml) | OML 简介 |
| [最小示例](#最小示例) | 5 行代码上手 |
| [基本语法](#基本语法) | 配置结构、WPL 关联、语法规则 |
| [三个最常用操作](#三个最常用操作) | 读取字段、类型转换、数据聚合 |
| [常用数据类型](#常用数据类型) | 8 种数据类型速查 |
| [常用内置函数](#常用内置函数) | 时间函数 |
| [read vs take](#read-vs-take两种读取模式) | 两种读取模式对比 |
| [完整示例](#完整示例日志处理) | 日志处理完整示例 |

---

## 什么是 OML

OML 是一种声明式的数据转换语言，用于将解析后的结构化数据转换为目标格式。它提供了简洁的语法来完成字段提取、类型转换、数据聚合等常见操作。

## 最小示例

```oml
name : my_first_oml
rule : /nginx/access_log
---
user_id = read(user_id) ;
timestamp : time = Now::time() ;
```

**说明**：
- `name : my_first_oml` - OML 配置名称声明
- `rule : /nginx/access_log` - 匹配 WPL 的 package/rule 值（关键！）
- `---` - 分隔符，区分声明区和配置区
- `user_id = read(user_id)` - 从输入读取 user_id 字段
- `timestamp : time = Now::time()` - 调用内置函数获取当前时间

**重要**：`rule` 字段用于关联 WPL 解析规则，只有当数据的 WPL rule 匹配时，这个 OML 配置才会被应用。

## 基本语法

### 1. 配置结构

```oml
name : <配置名称>
rule : <WPL 规则匹配模式>
---
<目标字段>[:<类型>] = <表达式> ;
```

### 2. WPL 与 OML 的关联

**关键概念**：一个 WPL 规则可以对应一个或多个 OML 配置。

```
WPL 解析 → 数据携带 rule 标识 → 匹配 OML 的 rule 字段 → 执行转换
```

**示例**：
```oml
# OML 配置
name : nginx_access
rule : /nginx/access_log    # 匹配 WPL 的 package/rule 值
---
# 转换逻辑...
```

当 WPL 解析后的数据携带 `rule = /nginx/access_log` 时，这个 OML 配置会被自动应用。

**支持通配符**：
- `rule : /nginx/*` - 匹配所有 /nginx/ 开头的规则
- `rule : */access_log` - 匹配所有以 /access_log 结尾的规则
- `rule : *` - 匹配所有规则

### 3. 必须记住的规则

- 每个配置条目必须以分号 `;` 结束
- 使用 `---` 分隔配置的不同部分
- `rule` 字段用于匹配 WPL 的 package/rule 值
- 类型声明可选，默认为 `auto` 自动推断

## 三个最常用操作

### 操作 1：读取字段

**场景**：从输入数据中提取字段

```oml
name : read_example
---
# 读取单个字段
user_id = read(user_id) ;

# 读取并指定类型
port : digit = read(port) ;

# 读取时提供默认值
country = read(country) { _ : chars(CN) } ;
```

**输入示例**：
```
user_id = "user123"
port = "8080"
```

**输出**：
```
user_id = "user123"
port = 8080
country = "CN"  # 使用默认值
```

### 操作 2：类型转换

**场景**：转换字段类型

```oml
name : type_conversion
---
# 字符串转 IP
src_ip : ip = read(src_ip) ;

# 字符串转整数
port : digit = read(port) ;

# 字符串转浮点数
cpu_usage : float = read(cpu) ;

# 字符串转时间
event_time : time = read(time) ;
```

**输入示例**：
```
src_ip = "192.168.1.100"
port = "8080"
cpu = "85.5"
time = "2024-01-15 14:30:00"
```

**输出**：
```
src_ip = 192.168.1.100  # IP 类型
port = 8080              # 整数
cpu_usage = 85.5         # 浮点数
event_time = 2024-01-15 14:30:00  # 时间类型
```

### 操作 3：数据聚合

**场景**：将多个字段组合成对象或数组

#### 创建对象

```oml
name : create_object
---
system_info : obj = object {
    hostname : chars = read(hostname) ;
    cpu : float = read(cpu_usage) ;
    memory : float = read(mem_usage) ;
} ;
```

**输入**：
```
hostname = "web-server-01"
cpu_usage = "75.5"
mem_usage = "60.2"
```

**输出**：
```json
{
    "system_info": {
        "hostname": "web-server-01",
        "cpu": 75.5,
        "memory": 60.2
    }
}
```

#### 创建数组

```oml
name : create_array
---
# 收集多个字段到数组
ports : array = collect read(keys:[sport, dport]) ;
```

**输入**：
```
sport = "8080"
dport = "443"
```

**输出**：
```
ports = [8080, 443]
```

## 常用数据类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `auto` | 自动推断（默认） | `value = read() ;` |
| `chars` | 字符串 | `name : chars = read() ;` |
| `digit` | 整数 | `count : digit = read() ;` |
| `float` | 浮点数 | `ratio : float = read() ;` |
| `ip` | IP 地址 | `addr : ip = read() ;` |
| `time` | 时间 | `timestamp : time = Now::time() ;` |
| `obj` | 对象 | `info : obj = object { ... } ;` |
| `array` | 数组 | `items : array = collect read(...) ;` |

## 常用内置函数

```oml
name : builtin_functions
---
# 获取当前时间
now : time = Now::time() ;

# 获取当前日期（YYYYMMDD 格式）
today : digit = Now::date() ;

# 获取当前小时（YYYYMMDDHH 格式）
current_hour : digit = Now::hour() ;
```

## read vs take：两种读取模式

### read（非破坏性）
- 可以多次读取同一字段
- 不会从输入中移除字段

```oml
name : read_mode
---
field1 = read(data) ;
field2 = read(data) ;  # 仍可读取到 data
```

### take（破坏性）
- 读取后会从输入中移除
- 后续无法再读取同一字段

```oml
name : take_mode
---
field1 = take(data) ;
field2 = take(data) ;  # 读取失败，data 已被移除
```

**使用建议**：
- 需要复用字段时使用 `read`
- 确保字段只使用一次时使用 `take`

## 完整示例：日志处理

这个示例展示了 OML 的主要功能：字段提取、类型转换、条件判断、数据聚合。

**输入数据（WPL 解析后）：**
```
user_id = "user123"
uri = "/api/users"
status = "200"
timestamp = "2024-01-15 14:30:00"
```

**OML 配置：**
```oml
name : access_log_processor
rule : /nginx/access_log
---
# 基础字段提取
user_id = read(user_id) ;
request_uri = read(uri) ;
status_code : digit = read(status) ;

# 时间处理
event_time : time = read(timestamp) ;
event_date : digit = Now::date() ;

# 条件转换（状态码分类）
status_level = match read(status_code) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(400), digit(499)) => chars(client_error) ;
    in (digit(500), digit(599)) => chars(server_error) ;
    _ => chars(other) ;
} ;

# 数据聚合
log_entry : obj = object {
    user : chars = read(user_id) ;
    uri : chars = read(request_uri) ;
    status : digit = read(status_code) ;
    level : chars = read(status_level) ;
    time : time = read(event_time) ;
} ;
```

**输出结果：**
```json
{
    "log_entry": {
        "user": "user123",
        "uri": "/api/users",
        "status": 200,
        "level": "success",
        "time": "2024-01-15 14:30:00"
    }
}
```

**关键点：**
- `rule : /nginx/access_log` 匹配 WPL 的 package/rule 值
- `match` 表达式实现条件分类
- `object` 聚合多个字段为结构化输出
- 类型自动转换（`status` 从字符串转为整数）

---

## 📚 完整类型系统与功能

**OML 支持 8 种数据类型和丰富的函数库**，涵盖数据提取、转换、聚合、条件处理等。

👉 **查看完整示例：** [07-complete-example.md](./07-complete-example.md)

该文档包含：
- ✅ 所有核心功能的完整示例代码
- ✅ 可运行的原始数据、WPL 规则和 OML 配置
- ✅ 每个功能的详细说明和使用建议
- ✅ 基础操作、内置函数、管道函数、模式匹配等

**快速预览主要功能：**
- **基础操作**：字面量赋值、取值、默认值、通配符批量操作
- **内置函数**：`Now::time()`、`Now::date()`、`Now::hour()`
- **管道函数**：Base64 编解码、HTML 转义、时间转换、URL 解析
- **模式匹配**：单源/双源 match、范围判断、否定条件
- **数据聚合**：对象创建、数组收集
- **SQL 集成**：数据库查询和富化

---

## 下一步学习

### 理解概念
- [核心概念](./02-core-concepts.md) - 深入理解 OML 的设计理念
  - WPL 与 OML 协作关系
  - read vs take 读取语义
  - 类型系统和表达式

### 实战应用
- [实战指南](./03-practical-guide.md) - 按任务查找解决方案
  - 数据提取、转换、聚合
  - 条件处理、SQL 查询
  - 复杂场景示例

### 查阅参考
- [函数参考](./04-functions-reference.md) - 查阅所有可用函数
- [集成指南](./05-integration.md) - 将 OML 集成到数据流
- [语法参考](./06-grammar-reference.md) - 完整的语法规则

---

## 快速提示

1. **从简单开始**：先使用基本的 read 操作，熟悉后再使用高级特性
2. **显式类型**：对于重要字段，建议显式声明类型避免意外转换
3. **提供默认值**：对于可能缺失的字段，使用 `{ _ : <默认值> }` 语法
4. **使用对象组织数据**：复杂数据用 `object` 聚合，便于理解和维护
5. **分号不能省**：每个配置条目必须以分号结束

---

## 相关资源

- 完整功能示例：[07-complete-example.md](./07-complete-example.md)
- WPL 规则语言：[../03-wpl/README.md](../03-wpl/README.md)
- 配置指南：[../02-config/README.md](../02-config/README.md)
