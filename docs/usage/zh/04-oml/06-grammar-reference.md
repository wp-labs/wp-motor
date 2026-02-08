# OML 语法参考

本文档提供 OML 的完整语法定义（EBNF 格式），用于精确理解语法规则。

> 基于源码 `crates/wp-oml` 的解析实现，词法细节复用 `wp_parser` 与 `wpl` 的既有解析能力。

---

## 📚 文档导航

| 章节 | 内容 |
|------|------|
| [EBNF 符号说明](#ebnf-符号说明) | 语法符号含义 |
| [顶层结构](#顶层结构) | OML 文件结构 |
| [求值表达式](#求值表达式) | 表达式类型、值表达式、函数调用等 |
| [高级表达式](#高级表达式) | 格式化字符串、管道、match、聚合 |
| [SQL 表达式](#sql-表达式) | SQL 查询语法 |
| [隐私段](#隐私段) | 数据脱敏语法 |
| [词法与约定](#词法与约定) | 标识符、字面量、注释 |
| [数据类型](#数据类型) | 8 种数据类型 |
| [完整示例](#完整示例) | 综合示例 |
| [管道函数速查](#管道函数速查) | 常用管道函数 |
| [语法要点](#语法要点) | 必需元素、可选元素、注意事项 |

---

## EBNF 符号说明

- `=` : 定义
- `,` : 连接（序列）
- `|` : 或（选择）
- `[ ]` : 可选（0 或 1 次）
- `{ }` : 重复（0 或多次）
- `( )` : 分组
- `"text"` : 字面量
- `(* ... *)` : 注释

---

## 顶层结构

```ebnf
oml              = header, sep_line, aggregate_items, [ sep_line, privacy_items ] ;

header           = "name", ":", name, eol,
                   [ "rule", ":", rule_path, { rule_path }, eol ] ;

sep_line         = "---" ;

name             = path ;                       (* 例如: test *)
rule_path        = wild_path ;                  (* 例如: wpx/abc, wpx/efg *)

aggregate_items  = aggregate_item, { aggregate_item } ;
aggregate_item   = target_list, "=", eval, ";" ;

target_list      = target, { ",", target } ;
target           = target_name, [ ":", data_type ] ;
target_name      = wild_key | "_" ;            (* 允许带通配符 '*'；'_' 表示匿名/丢弃 *)
data_type        = type_ident ;                (* auto|ip|chars|digit|float|time|bool|obj|array *)
```

**说明**：
- `name : <配置名称>` - 必需的配置名称声明
- `rule : <规则路径>` - 可选的规则关联
- `---` - 分隔符，区分声明区和配置区
- 每个配置条目必须以 `;` 结束

---

## 求值表达式

### 表达式类型

```ebnf
eval             = take_expr
                 | read_expr
                 | fmt_expr
                 | pipe_expr
                 | map_expr
                 | collect_expr
                 | match_expr
                 | sql_expr
                 | value_expr
                 | fun_call ;
```

### 读取表达式

```ebnf
(* 变量获取：take/read 支持统一参数形态；可跟缺省体 *)
take_expr        = "take", "(", [ arg_list ], ")", [ default_body ] ;
read_expr        = "read", "(", [ arg_list ], ")", [ default_body ] ;

arg_list         = arg, { ",", arg } ;
arg              = "option", ":", "[", key, { ",", key }, "]"
                 | ("in"|"keys"), ":", "[", key, { ",", key }, "]"
                 | "get",    ":", simple
                 | json_path ;                 (* 见 wp_parser::atom::take_json_path *)

default_body     = "{", "_", ":", gen_acq, [ ";" ], "}" ;
gen_acq          = take_expr | read_expr | value_expr | fun_call ;
```

**说明**：
- `@` 仅作为变量获取语法糖用于 fmt/pipe/collect 的 var_get 位置
- `@ref` 等价于 `read(ref)`，但不支持缺省体
- 不作为独立求值表达式

**示例**：
```oml
# 基本读取
value = read(field) ;

# 带默认值
value = read(field) { _ : chars(default) } ;

# option 参数
value = read(option:[id, uid, user_id]) ;

# keys 参数
values = collect read(keys:[field1, field2]) ;

# JSON 路径
name = read(/user/info/name) ;
```

### 值表达式

```ebnf
(* 常量值：类型名+括号包裹的字面量 *)
value_expr       = data_type, "(", literal, ")" ;
```

**示例**：
```oml
text = chars(hello) ;
count = digit(42) ;
address = ip(192.168.1.1) ;
flag = bool(true) ;
```

### 函数调用

```ebnf
(* 内置函数（零参占位）：Now::* 家族 *)
fun_call         = ("Now::time"
                   |"Now::date"
                   |"Now::hour"), "(", ")" ;
```

**示例**：
```oml
now = Now::time() ;
today = Now::date() ;
hour = Now::hour() ;
```

---

## 高级表达式

### 格式化字符串

```ebnf
(* 字符串格式化，至少 1 个参数 *)
fmt_expr         = "fmt", "(", string, ",", var_get, { ",", var_get }, ")" ;
var_get          = ("read" | "take"), "(", [ arg_list ], ")"
                 | "@", ident ;                  (* '@ref' 等价 read(ref)，不支持缺省体 *)
```

**示例**：
```oml
message = fmt("{}-{}", @user, read(city)) ;
id = fmt("{}:{}", read(host), read(port)) ;
```

### 管道表达式

```ebnf
(* 管道：可省略 pipe 关键字 *)
pipe_expr        = ["pipe"], var_get, "|", pipe_fun, { "|", pipe_fun } ;

pipe_fun         = "nth",           "(", unsigned, ")"
                 | "get",           "(", ident,   ")"
                 | "base64_decode", "(", [ encode_type ], ")"
                 | "path",          "(", ("name"|"path"), ")"
                 | "url",           "(", ("domain"|"host"|"uri"|"path"|"params"), ")"
                 | "Time::to_ts_zone", "(", [ "-" ], unsigned, ",", ("ms"|"us"|"ss"|"s"), ")"
                 | "starts_with",   "(", string, ")"
                 | "map_to",        "(", (string | number | bool), ")"
                 | "base64_encode" | "html_escape" | "html_unescape"
                 | "str_escape" | "str_unescape" | "json_escape" | "json_unescape"
                 | "Time::to_ts" | "Time::to_ts_ms" | "Time::to_ts_us"
                 | "to_json" | "to_str" | "skip_empty" | "ip4_to_int"
                 | "extract_main_word" | "extract_subject_object" ;

encode_type      = ident ;                     (* 例如: Utf8/Gbk/Imap/... *)
```

**示例**：
```oml
# 使用 pipe 关键字
result = pipe read(data) | to_json | base64_encode ;

# 省略 pipe 关键字
result = read(data) | to_json | base64_encode ;

# 时间转换
ts = read(time) | Time::to_ts_zone(0, ms) ;

# URL 解析
host = read(url) | url(host) ;

# 字符串前缀检查
is_http = read(url) | starts_with('http://') ;

# 映射到常量值
status = read(code) | map_to(200) ;

# 提取主要单词
keyword = read(message) | extract_main_word ;

# 提取主客体结构
log_struct = read(message) | extract_subject_object ;

# 字符串反转义
text = read(escaped) | str_unescape ;
```

### 对象聚合

```ebnf
(* 聚合到对象：object 内部为子赋值序列；分号可选但推荐 *)
map_expr         = "object", "{", map_item, { map_item }, "}" ;
map_item         = map_targets, "=", sub_acq, [ ";" ] ;
map_targets      = ident, { ",", ident }, [ ":", data_type ] ;
sub_acq          = take_expr | read_expr | value_expr | fun_call ;
```

**示例**：
```oml
info : obj = object {
    name : chars = read(name) ;
    age : digit = read(age) ;
    city : chars = read(city) ;
} ;
```

### 数组聚合

```ebnf
(* 聚合到数组：从 VarGet 收集（支持 keys/option 通配） *)
collect_expr     = "collect", var_get ;
```

**示例**：
```oml
# 收集多个字段
ports = collect read(keys:[sport, dport]) ;

# 使用通配符
metrics = collect read(keys:[cpu_*]) ;
```

### 模式匹配

```ebnf
(* 模式匹配：单源/双源两种形态，支持 in/!= 与缺省分支 *)
match_expr       = "match", match_source, "{", case1, { case1 }, [ default_case ], "}"
                 | "match", "(", var_get, ",", var_get, ")", "{", case2, { case2 }, [ default_case ], "}" ;

match_source     = var_get ;
case1            = cond1, "=>", calc, [ "," ], [ ";" ] ;
case2            = "(", cond1, ",", cond1, ")", "=>", calc, [ "," ], [ ";" ] ;
default_case     = "_", "=>", calc, [ "," ], [ ";" ] ;
calc             = read_expr | take_expr | value_expr | collect_expr ;

cond1            = "in", "(", value_expr, ",", value_expr, ")"
                 | "!", value_expr
                 | value_expr ;                 (* 省略运算符表示等于 *)
```

**示例**：
```oml
# 单源匹配
level = match read(status) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(400), digit(499)) => chars(error) ;
    _ => chars(other) ;
} ;

# 双源匹配
result = match (read(a), read(b)) {
    (digit(1), digit(2)) => chars(case1) ;
    _ => chars(default) ;
} ;
```

---

## SQL 表达式

```ebnf
sql_expr        = "select", sql_body, "where", sql_cond, ";" ;
sql_body        = sql_safe_body ;              (* 源码对白名单化：仅 [A-Za-z0-9_.] 与 '*' *)
sql_cond        = cond_expr ;

cond_expr       = cmp, { ("and" | "or"), cmp }
                 | "not", cond_expr
                 | "(", cond_expr, ")" ;

cmp             = ident, sql_op, cond_rhs ;
sql_op          = sql_cmp_op ;                 (* 见 wp_parser::sql_symbol::symbol_sql_cmp *)
cond_rhs        = read_expr | take_expr | fun_call | sql_literal ;
sql_literal     = number | string ;
```

### 严格模式说明

- **严格模式（默认开启）**：当主体 `<cols from table>` 不满足白名单规则时，解析报错
- **兼容模式**：设置环境变量 `OML_SQL_STRICT=0`，若主体非法则回退原文（不推荐）
- **白名单规则**：
  - 列清单：`*` 或由 `[A-Za-z0-9_.]+` 组成的列名（允许点号作限定）
  - 表名：`[A-Za-z0-9_.]+`（单表，不支持 join/子查询）
  - `from` 大小写不敏感；多余空白允许

**示例**：
```oml
# 正确示例
name, email = select name, email from users where id = read(user_id) ;

# 使用字符串常量
data = select * from table where type = 'admin' ;

# IP 范围查询
zone = select zone from ip_geo
    where ip_start_int <= ip4_int(read(src_ip))
      and ip_end_int >= ip4_int(read(src_ip)) ;
```

**错误示例（严格模式）**：
```oml
# ❌ 表名含非法字符
data = select a, b from table-1 where ... ;

# ❌ 列清单含函数
data = select sum(a) from t where ... ;

# ❌ 不支持 join
data = select a from t1 join t2 ... ;
```

---

## 隐私段

> 注：引擎默认不启用运行期隐私/脱敏处理；以下为 DSL 语法能力说明，供需要的场景参考。

```ebnf
privacy_items   = privacy_item, { privacy_item } ;
privacy_item    = ident, ":", privacy_type ;

privacy_type    = "privacy_ip"
                 | "privacy_specify_ip"
                 | "privacy_id_card"
                 | "privacy_mobile"
                 | "privacy_mail"
                 | "privacy_domain"
                 | "privacy_specify_name"
                 | "privacy_specify_domain"
                 | "privacy_specify_address"
                 | "privacy_specify_company"
                 | "privacy_keymsg" ;
```

**示例**：
```oml
name : privacy_example
---
field = read() ;
---
src_ip : privacy_ip
pos_sn : privacy_keymsg
```

---

## 词法与约定

```ebnf
path            = ident, { ("/" | "."), ident } ;
wild_path       = path | path, "*" ;          (* 允许通配 *)
wild_key        = ident, { ident | "*" } ;    (* 允许 '*' 出现在键名中 *)
type_ident      = ident ;                      (* 如 auto/ip/chars/digit/float/time/bool/obj/array *)
ident           = letter, { letter | digit | "_" } ;
key             = ident ;

string          = "\"", { any-but-quote }, "\""
                | "'", { any-but-quote }, "'" ;

literal         = string | number | ip | bool | datetime | ... ;
json_path       = "/" , ... ;                 (* 如 /a/b/[0]/1 *)
simple          = ident | number | string ;
unsigned        = digit, { digit } ;
eol             = { " " | "\t" | "\r" | "\n" } ;

letter          = "A" | ... | "Z" | "a" | ... | "z" ;
digit           = "0" | ... | "9" ;
alnum           = letter | digit ;
```

---

## 数据类型

OML 支持以下数据类型：

| 类型 | 说明 | 示例 |
|------|------|------|
| `auto` | 自动推断（默认） | `field = read() ;` |
| `chars` | 字符串 | `name : chars = read() ;` |
| `digit` | 整数 | `count : digit = read() ;` |
| `float` | 浮点数 | `ratio : float = read() ;` |
| `ip` | IP 地址 | `addr : ip = read() ;` |
| `time` | 时间 | `timestamp : time = Now::time() ;` |
| `bool` | 布尔值 | `flag : bool = read() ;` |
| `obj` | 对象 | `info : obj = object { ... } ;` |
| `array` | 数组 | `items : array = collect read(...) ;` |

---

## 完整示例

```oml
name : csv_example
rule : /csv/data
---
# 基本取值与缺省
version : chars = Now::time() ;
pos_sn = read() { _ : chars(FALLBACK) } ;

# object 聚合
values : obj = object {
    cpu_free, memory_free : digit = read() ;
} ;

# collect 数组聚合 + 管道
ports : array = collect read(keys:[sport, dport]) ;
ports_json = pipe read(ports) | to_json ;
first_port = pipe read(ports) | nth(0) ;

# 省略 pipe 关键字的管道写法
url_host = read(http_url) | url(host) ;

# match
quarter : chars = match read(month) {
    in (digit(1), digit(3))   => chars(Q1) ;
    in (digit(4), digit(6))   => chars(Q2) ;
    in (digit(7), digit(9))   => chars(Q3) ;
    in (digit(10), digit(12)) => chars(Q4) ;
    _ => chars(QX) ;
} ;

# 双源 match
X : chars = match (read(city1), read(city2)) {
    (ip(127.0.0.1), ip(127.0.0.100)) => chars(bj) ;
    _ => chars(sz) ;
} ;

# SQL（where 中可混用 read/take/Now::time/常量）
name, pinying = select name, pinying from example where pinying = read(py) ;
_, _ = select name, pinying from example where pinying = 'xiaolongnu' ;

---
# 隐私配置（按键绑定处理器枚举）
src_ip : privacy_ip
pos_sn : privacy_keymsg
```

---

## 管道函数速查

| 函数 | 语法 | 说明 |
|------|------|------|
| `base64_encode` | `base64_encode` | Base64 编码 |
| `base64_decode` | `base64_decode` / `base64_decode(编码)` | Base64 解码 |
| `html_escape` | `html_escape` | HTML 转义 |
| `html_unescape` | `html_unescape` | HTML 反转义 |
| `json_escape` | `json_escape` | JSON 转义 |
| `json_unescape` | `json_unescape` | JSON 反转义 |
| `str_escape` | `str_escape` | 字符串转义 |
| `str_unescape` | `str_unescape` | 字符串反转义 |
| `Time::to_ts` | `Time::to_ts` | 时间转时间戳（秒，UTC+8） |
| `Time::to_ts_ms` | `Time::to_ts_ms` | 时间转时间戳（毫秒，UTC+8） |
| `Time::to_ts_us` | `Time::to_ts_us` | 时间转时间戳（微秒，UTC+8） |
| `Time::to_ts_zone` | `Time::to_ts_zone(时区,单位)` | 时间转指定时区时间戳 |
| `nth` | `nth(索引)` | 获取数组元素 |
| `get` | `get(字段名)` | 获取对象字段 |
| `path` | `path(name\|path)` | 提取文件路径部分 |
| `url` | `url(domain\|host\|uri\|path\|params)` | 提取 URL 部分 |
| `starts_with` | `starts_with('前缀')` | 检查字符串是否以指定前缀开始 |
| `map_to` | `map_to(值)` | 映射到指定常量值 |
| `extract_main_word` | `extract_main_word` | 提取主要单词（第一个非空单词） |
| `extract_subject_object` | `extract_subject_object` | 提取日志主客体结构（subject/action/object/status） |
| `to_str` | `to_str` | 转换为字符串 |
| `to_json` | `to_json` | 转换为 JSON |
| `ip4_to_int` | `ip4_to_int` | IPv4 转整数 |
| `skip_empty` | `skip_empty` | 跳过空值 |

---

## 语法要点

### 必需元素

1. **配置名称**：`name : <名称>`
2. **分隔符**：`---`
3. **分号**：每个顶层条目必须以 `;` 结束

### 可选元素

1. **类型声明**：`field : <type> = ...`（默认为 `auto`）
2. **rule 字段**：`rule : <规则路径>`
3. **默认值**：`read() { _ : <默认值> }`
4. **pipe 关键字**：`pipe read() | func` 可简写为 `read() | func`

### 注释

```oml
# 单行注释（使用 # 或 //）
// 也支持 C++ 风格注释
```

### 目标通配

```oml
* = take() ;           # 取走所有字段
alert* = take() ;      # 取走所有以 alert 开头的字段
*_log = take() ;       # 取走所有以 _log 结尾的字段
```

### 读取语义

- **read**：非破坏性（可反复读取，不从 src 移除）
- **take**：破坏性（取走后从 src 移除，后续不可再取）

---

## 下一步

- [核心概念](./02-core-concepts.md) - 理解语法设计理念
- [实战指南](./03-practical-guide.md) - 查看实际应用示例
- [函数参考](./04-functions-reference.md) - 查阅所有可用函数
- [快速入门](./01-quickstart.md) - 快速上手 OML
