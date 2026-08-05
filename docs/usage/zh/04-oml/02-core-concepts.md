# OML 核心概念

本文档介绍 OML 的核心设计理念和基础概念，帮助你深入理解 OML 的工作原理。

---

## 📚 文档导航

### 快速导航

| 主题 | 内容 |
|------|------|
| [**设计理念**](#oml-设计理念) | WPL 协作关系、声明式语法、不可变数据流 |
| [**类型系统**](#类型系统) | 8 种基本类型、自动推断、类型转换 |
| [**读取语义**](#读取语义read-vs-take) | read vs take、破坏性与非破坏性、读取优先级 |
| [**表达式类型**](#表达式类型) | 值表达式、函数调用、管道、条件、聚合 |
| [**数值表达式**](#数值表达式calc) | 算术语法、类型规则、失败行为 |
| [**默认值机制**](#默认值机制) | 默认值语法、函数默认值、限制说明 |
| [**通配符**](#通配符与批量处理) | 通配符语法、批量目标、使用限制 |
| [**参数化读取**](#参数化读取) | option 优先级、keys 收集、JSON 路径 |
| [**表达式组合**](#表达式组合) | 嵌套对象、管道链、复杂 match |
| [**作用域规则**](#作用域规则) | 目标字段作用域、全局字段 |
| [**最佳实践**](#最佳实践) | 读取模式选择、类型声明、默认值、通配符使用 |

---

## OML 设计理念

### WPL 与 OML 的协作关系

OML 不是独立工作的，它与 WPL 紧密配合：

```
1. WPL 解析原始数据
   ↓
2. 生成结构化数据 + rule 标识（如 /nginx/access_log）
   ↓
3. 系统查找匹配的 OML 配置（通过 rule 字段）
   ↓
4. 执行 OML 转换
   ↓
5. 输出到 Sink
```

**关键点**：
- 每个 OML 配置通过 `rule` 字段声明它处理哪些 WPL 规则的数据
- 一个 WPL 规则可以对应多个 OML 配置
- 支持通配符匹配，如 `rule : /nginx/*` 匹配所有 nginx 相关规则

**示例**：
```oml
name : nginx_access_handler
rule : /nginx/access_log    # 只处理这个 WPL 规则的数据
---
# 转换逻辑...
```

### 声明式而非命令式

OML 采用声明式语法，你只需要描述**想要什么结果**，而不是**如何实现**：

```oml
# 声明式：描述结果
user_info : obj = object {
    id : chars = read(user_id) ;
    name : chars = read(username) ;
} ;
```

对比命令式伪代码：
```
user_info = new Object()
user_info.id = get_field("user_id")
user_info.name = get_field("username")
convert_to_chars(user_info.id)
convert_to_chars(user_info.name)
```

### 不可变数据流

OML 中的数据转换是单向流动的：

```
输入数据 → OML 转换 → 输出数据
```

- 输入数据不会被修改（除非使用 `take`）
- 每个转换步骤都产生新的值
- 便于理解和调试

## 类型系统

### 基本类型

OML 提供 8 种基本数据类型：

```oml
name : types_example
---
# 字符串
text : chars = chars(hello) ;

# 整数
count : digit = digit(42) ;

# 浮点数
ratio : float = float(3.14) ;

# IP 地址
address : ip = ip(192.168.1.1) ;

# 时间
timestamp : time = Now::time() ;

# 布尔值
flag : bool = bool(true) ;

# 对象
info : obj = object { ... } ;

# 数组
items : array = collect read(keys:[...]) ;
```

### 自动类型推断

当不指定类型时，OML 会自动推断：

```oml
name : auto_type
---
# 自动推断为 chars
name = read(name) ;

# 自动推断为 digit
count = digit(100) ;

# 显式指定类型（推荐）
port : digit = read(port) ;
```

**最佳实践**：对于重要字段，建议显式声明类型以避免意外。

### 类型转换

OML 会在必要时自动进行类型转换：

```oml
name : type_cast
---
# 从字符串 "8080" 转换为整数 8080
port : digit = read(port) ;

# 从字符串 "192.168.1.1" 转换为 IP
ip_addr : ip = read(src_ip) ;

# 从字符串转换为时间
event_time : time = read(timestamp) ;
```

## 读取语义：read vs take

这是 OML 中最重要的概念之一。

### read：非破坏性读取

**特性**：
- 从源数据**克隆**值到目标
- 源数据保持不变
- 可以多次读取同一字段

**使用场景**：
- 字段需要在多处使用
- 需要保留原始数据

**示例**：
```oml
name : read_example
---
# 假设输入：data = "hello"

field1 = read(data) ;  # field1 = "hello"，src.data 仍存在
field2 = read(data) ;  # field2 = "hello"，可以再次读取
field3 = read(data) ;  # field3 = "hello"，仍然可以读取
```

### take：破坏性读取

**特性**：
- 从源数据**移走**值到目标
- 源数据中该字段被删除
- 只能读取一次

**使用场景**：
- 字段只需要使用一次
- 需要确保字段不被重复使用
- 优化性能（避免数据复制）

**示例**：
```oml
name : take_example
---
# 假设输入：data = "hello"

field1 = take(data) ;  # field1 = "hello"，src.data 被移除
field2 = take(data) ;  # 失败！data 已经不存在
```

### 读取优先级

`read` 和 `take` 都遵循以下查找顺序：

1. 先查找**目标记录**（dst）
2. 如果未找到，再查找**源记录**（src）

```oml
name : lookup_priority
---
# 假设：src.value = "A"

field1 = read(value) ;     # "A" (从 src 读取)
field2 = read(field1) ;    # "A" (从 dst 读取，field1 已在目标中)
```

## 表达式类型

### 值表达式

直接构造常量值：

```oml
name : value_expr
---
# 字符串
text = chars(hello) ;

# 整数
count = digit(100) ;

# IP
ip_addr = ip(192.168.1.1) ;
```

### 函数调用

调用内置函数：

```oml
name : function_call
---
# 时间函数
now = Now::time() ;
today = Now::date() ;
hour = Now::hour() ;
```

### 管道表达式

链式处理数据：

```oml
name : pipe_expr
---
# 读取 → 转 JSON → Base64 编码
encoded = pipe read(data) | to_json | base64_encode ;

# 也可以省略 pipe 关键字
encoded2 = read(data) | to_json | base64_encode ;
```

### 数值表达式

使用 `calc(...)` 显式执行算术表达式：

```oml
name : calc_expr
---
risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;
bucket     : digit = calc(read(uid) % 16) ;
distance   : float = calc(abs(read(actual) - read(expect))) ;
```

### 数值表达式：`calc(...)`

**支持范围**：
- 运算符：`+ - * / %`
- 函数：`abs(...)`、`round(...)`、`floor(...)`、`ceil(...)`
- 操作数：数值字面量、`read(...)`、`take(...)`、`@field`

**类型规则**：
- `digit op digit` 在 `+ - *` 下返回 `digit`
- 只要任一操作数是浮点，`+ - *` 返回 `float`
- `/` 始终返回 `float`
- `%` 仅支持 `digit % digit`

**失败行为**：
- 除零、缺失字段、非数值输入返回 `ignore`
- 整数溢出返回 `ignore`
- `NaN` / `inf` 输入或结果返回 `ignore`

这意味着 `calc(...)` 不会隐式兜底为 `0`，也不会把非法结果继续传给后续步骤。

### 条件表达式

基于条件选择值：

```oml
name : match_expr
---
level = match read(status) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(400), digit(499)) => chars(error) ;
    _ => chars(other) ;
} ;
```

`match` 分支值仍然使用既有子表达式集合，不直接写 `calc(...)`。如果需要先算再匹配，建议先绑定到临时字段：

```oml
__risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;
level = match read(__risk_score) {
    gt(80) => chars(high) ;
    _ => chars(normal) ;
} ;
```

支持 OR 语法（`|` 分隔多个备选条件）：

```oml
name : match_or_expr
---
tier = match read(city) {
    chars(bj) | chars(sh) | chars(gz) => chars(tier1) ;
    chars(cd) | chars(wh) => chars(tier2) ;
    _ => chars(other) ;
} ;
```

### 聚合表达式

创建复合数据结构：

```oml
name : aggregate_expr
---
# 对象聚合
info : obj = object {
    name = read(name) ;
    age = read(age) ;
} ;

# 数组聚合：按 keys 收集已有字段
items : array = collect read(keys:[a, b, c]) ;

# 对象数组聚合：构造对象字面量列表
signatures = array {
    object { signer = read(process_sign) ; } ;
    object { signer = read(process_parent_sign) ; } ;
} ;
```

- `object { ... }` 构造单层或嵌套对象
- `collect` 把输入字段按 `keys` 收集成数组
- `array { ... }` 直接构造对象/值字面量数组（元素缺失自动跳过）

## 默认值机制

### 默认值语法

当字段不存在或读取失败时，使用默认值：

```oml
name : default_value
---
# 语法：read(...) { _ : <默认值> }
country = read(country) { _ : chars(CN) } ;
version = read(version) { _ : chars(1.0.0) } ;
port = read(port) { _ : digit(8080) } ;
```

### 默认值可以是函数调用

```oml
name : default_with_function
---
# 如果 timestamp 不存在，使用当前时间
event_time = read(timestamp) { _ : Now::time() } ;
```

### 默认值可以是读取

```oml
name : default_with_read
---
# 如果 id 不存在，尝试读取 user_id
user_id = read(id) { _ : read(user_id) } ;
```

### 限制

- `@ref` 语法糖不支持默认值
- 默认值表达式不能是 `match`、`object`、`collect` 等复杂表达式

## 通配符与批量处理

### 通配符语法

使用 `*` 匹配多个字段：

```oml
name : wildcard
---
# 收集所有以 cpu_ 开头的字段
cpu_metrics = collect read(keys:[cpu_*]) ;

# 收集所有以 /path 结尾的字段
paths = collect read(keys:[*/path]) ;
```

### 批量目标

目标字段名包含 `*` 时进入批量模式：

```oml
name : batch_target
---
# 取走所有字段
* = take() ;

# 取走所有以 alert_ 开头的字段
alert* = take() ;

# 取走所有以 _log 结尾的字段
*_log = take() ;
```

**限制**：批量模式只支持 `read` 和 `take`，不支持其他表达式。

## 参数化读取

### option：按优先级尝试

```oml
name : option_param
---
# 按顺序尝试读取 id、uid、user_id
user_id = read(option:[id, uid, user_id]) ;
```

**行为**：
- 从左到右尝试每个字段
- 返回第一个存在的字段值
- 如果都不存在，返回失败（可配合默认值）

### keys：收集多个字段

```oml
name : keys_param
---
# 收集多个字段为数组
ports = collect read(keys:[sport, dport]) ;
```

**行为**：
- 读取所有指定的字段
- 支持通配符 `*`
- 返回数组

### JSON 路径

读取嵌套数据：

```oml
name : json_path
---
# 读取 /user/info/name
username = read(/user/info/name) ;

# 读取数组元素 /items[0]/id
first_id = read(/items[0]/id) ;
```

## 表达式组合

### 嵌套对象

```oml
name : nested_objects
---
deployment : obj = object {
    app : obj = object {
        name = read(app_name) ;
        version = read(app_version) ;
    } ;
    env : obj = object {
        region = read(region) ;
        zone = read(zone) ;
    } ;
} ;
```

嵌套对象可以任意深度；对象、数组可以互相嵌套（如 `object` 内含 `array`，`array` 内含 `object`）。

### 对象数组

用 `array { ... }` 构造对象字面量数组：

```oml
name : object_array
---
roles_obj = object {
    source = object {
        entity_type = chars(host) ;
        host = object {
            id = read(asset_id) ;
            name = read(computer_name) ;
            ip = read(ip) ;
        } ;
    } ;
} ;

signatures = array {
    object { signer = read(process_sign) ; } ;
    object { signer = read(process_parent_sign) ; } ;
} ;
```

输出 JSON：

```json
{
  "roles_obj": {
    "source": {
      "entity_type": "host",
      "host": { "id": "2868257359929541780", "name": "DESKTOP-NGMF7JI", "ip": "10.95.209.76" }
    }
  },
  "signatures": [
    { "signer": "Microsoft Windows" },
    { "signer": "Microsoft Windows Publisher" }
  ]
}
```

- 数组元素可以是 `object { ... }`、嵌套 `array { ... }`、`read/take`、值、函数等表达式
- 元素缺失（`read` 不到）或为 null 时自动跳过，不进入数组

### 管道链

```oml
name : pipe_chain
---
# 多级转换
result = read(data) | to_json | base64_encode | html_escape ;

# 数组操作
first_user = read(users) | nth(0) | get(name) ;
```

### match 中的复杂表达式

```oml
name : complex_match
---
# 单源 match + collect
status = match read(code) {
    in (digit(200), digit(299)) => collect read(keys:[a, b]) ;
    _ => read(default_value) ;
} ;

# 多源 match（支持任意数量源字段）
zone = match (read(city), read(region), read(country)) {
    (chars(bj), chars(north), chars(cn)) => chars(zone1) ;
    _ => chars(unknown) ;
} ;

# OR + 多源 match
priority = match (read(city), read(level)) {
    (chars(bj) | chars(sh), chars(high)) => chars(priority) ;
    (chars(gz), chars(low) | chars(mid)) => chars(normal) ;
    _ => chars(default) ;
} ;
```

## 作用域规则

### 目标字段作用域

在 `object` 内部创建的字段，只在对象内可见：

```oml
name : scope_example
---
info : obj = object {
    name = read(username) ;  # name 只在 info 对象内
} ;

# 这里无法访问 name
other = read(name) ;  # 失败！name 不在外部作用域
```

### 全局目标字段

顶层定义的字段可以被后续读取：

```oml
name : global_scope
---
# 定义全局字段
temp = read(data) ;

# 后续可以读取
result = read(temp) ;
```

## 易错提醒

1. **分号必需**：每个顶层条目必须以 `;` 结束
2. **类型不匹配**：显式类型声明与实际值不符会导致转换错误
3. **take 后再读**：使用 `take` 后该字段被移除，无法再次读取
4. **@ref 限制**：`@ref` 只能在特定位置使用，不支持默认值
5. **批量模式限制**：目标名含 `*` 时，右值只能是 `read` 或 `take`

## 最佳实践

### 1. 选择合适的读取模式

```oml
# 推荐：字段复用时用 read
temp = read(data) ;
result1 = pipe read(temp) | to_json ;
result2 = pipe read(temp) | base64_encode ;

# 推荐：一次性使用时用 take
final = take(data) | to_json ;
```

### 2. 显式类型声明

```oml
# 推荐：明确类型
port : digit = read(port) ;
ip_addr : ip = read(src_ip) ;

# 可接受：简单场景自动推断
name = read(name) ;
```

### 3. 提供默认值

```oml
# 推荐：关键字段提供默认值
version = read(version) { _ : chars(1.0.0) } ;
timeout = read(timeout) { _ : digit(30) } ;
```

### 4. 合理使用通配符

```oml
# 推荐：明确的通配符模式
cpu_metrics = collect read(keys:[cpu_*]) ;

# 避免：过于宽泛的通配符
all = collect read(keys:[*]) ;  # 可能包含不需要的字段
```

## 下一步

- [实战指南](./03-practical-guide.md) - 按任务查找解决方案
- [函数参考](./04-functions-reference.md) - 查阅所有可用函数
- [快速入门](./01-quickstart.md) - 回顾基础用法
- [语法参考](./06-grammar-reference.md) - 查看完整语法定义
