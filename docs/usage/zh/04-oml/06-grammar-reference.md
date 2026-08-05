# OML 语法参考

本文档按 `wp-motor/crates/wp-oml/src/parser` 的当前实现整理 OML 语法。它是接近 EBNF 的实现摘要，不是脱离源码的“理想化 DSL 设计稿”。

> 重点边界：
> - 当前没有正式的隐私段语法
> - 当前 SQL 入口关键字是 `select`
> - 当前没有 `query ...` 主语法
> - `static` 只支持纯常量表达式

---

## EBNF 约定

- `|`：选择
- `[ ... ]`：可选
- `{ ... }`：重复 0 次或多次
- `(...)`：分组
- 字面量关键字直接写成字符串

---

## 注释

OML 支持 `//` 单行注释和 `#` 单行注释。解析前由 `CommentParser::ignore_comment` 预处理剥离（见 `wp-motor/crates/wp-oml/src/parser/code.rs`），因此注释可出现在任意位置——语句之间、语句之后或同行——不影响以下语法：

```oml
// 顶层注释
# 同样支持
name : demo_alert
rule : /demo/log
---
// 头部与 body 的分隔符是 "---"
field1 = read(f1) ;   // 行尾注释
field2 = read(f2) ;   # 井号行尾注释
```

注意：

- 注释在预处理阶段被移除，不会进入解析器
- `tree-sitter-oml` 的 `comment` 规则同时识别 `#` 和 `//` 两种形式
- 与 WPL 不同（WPL 因 `#[...]` 注解不能把 `#` 当注释），OML 没有注解语法，`#` 可作为注释标记

---

## 顶层结构

```ebnf
oml               = name_decl, { head_decl }, sep_line,
                    { static_block }, aggregate_item, { aggregate_item },
                    [ sep_line ] ;

name_decl         = "name", ":", obj_path ;
head_decl         = enable_decl | rule_decl ;
enable_decl       = "enable", ":", ("true" | "false") ;
rule_decl         = "rule", ":", rule_path, { rule_path } ;
rule_path         = path_with_wildcard ;
sep_line          = "---" ;

static_block      = "static", "{", static_item, { static_item }, "}" ;
static_item       = target, "=", static_expr, ";" ;

aggregate_item    = target_list, "=", eval_expr, ";" ;
target_list       = target, { ",", target } ;
target            = target_name, [ ":", data_type ] ;
target_name       = wild_key | "_" ;
```

## 顶层说明

- `name` 必填，且必须是第一项
- `rule` 与 `enable` 都是可选的，顺序不限
- `rule` 后面可以跟多个规则路径
- `---` 是头部和主体之间的强制分隔线
- `static` 必须位于 `---` 之后、普通聚合语句之前
- 主体至少要有一条聚合语句
- 结尾允许额外出现一个 `---`，但当前实现不会把它当作“隐私段开始”

---

## 求值表达式

```ebnf
eval_expr         = calc_expr
                  | match_expr
                  | lookup_expr
                  | object_expr
                  | pipe_expr
                  | collect_expr
                  | sql_expr
                  | fmt_expr
                  | access_direct_expr
                  | direct_expr
                  | value_expr
                  | builtin_fun
                  | static_ref ;
```

### 直接读取

```ebnf
direct_expr       = read_expr | take_expr ;
read_expr         = "read", "(", [ read_args ], ")", [ default_body ] ;
take_expr         = "take", "(", [ read_args ], ")", [ default_body ] ;

read_args         = read_arg, { ",", read_arg } ;
read_arg          = option_arg | keys_arg | get_arg | json_path ;
option_arg        = "option", ":", "[", key_list, "]" ;
keys_arg          = ("in" | "keys"), ":", "[", key_list, "]" ;
get_arg           = "get", ":", simple_key ;
default_body      = "{", "_", ":", default_expr, [ ";" ], "}" ;
default_expr      = read_expr | take_expr | value_expr | builtin_fun | static_ref ;
```

**说明**

- `read(...)` / `take(...)` 接受的参数键当前只有 `option`、`in`、`keys`、`get` 和 JSON Path
- `in:` 与 `keys:` 在当前实现中语义等价
- `@field` 是 `read(field)` 的语法糖，但它不是独立顶层表达式，只出现在 `fmt`、`pipe`、`calc`、`match` 等需要 `var_get` 的位置

**示例**

```oml
id = read(user_id) ;
uid = read(option:[uid, user_id, id]) ;
metrics = collect read(keys:[cpu_*, mem_*]) ;
name = read(/user/info/name) ;
country = read(country) { _ : chars(CN) } ;
```

### `access_direct(...)`

组合 src/dst 两个 IP 的内外归属得到访问方向，返回 `L2L`/`L2W`/`W2L`/`W2W`。

```ebnf
access_direct_expr = "access_direct", "(", var_get, ",", var_get, ")", [ "|", pipe_item, { "|", pipe_item } ] ;
```

**说明**
- intranet/internet判定使用内网网段知识（`knowdb.toml` 的 `[intranet_nets]` 节）
- src/dst 缺失或非法输出 `Ignore`，可用管道 `| on_fail('unknown')` 提供兜底
- 支持 IPv4 + IPv6；IPv4-mapped IPv6 按 IPv4 判定

**示例**
```oml
direction = access_direct(read(sip), read(dip)) | on_fail('unknown') ;
# 内网→内网: "L2L"；内网→外网: "L2W"；外网→内网: "W2L"；外网→外网: "W2W"
# src/dst 缺失: "unknown"（on_fail 兜底）
```

---

## 值字面量与内置函数

```ebnf
value_expr        = data_type, "(", literal_body, ")" ;
builtin_fun       = "Now::time", "(", ")"
                  | "Now::date", "(", ")"
                  | "Now::hour", "(", ")" ;
static_ref        = ident ;
```

`data_type` 集合（对齐 `tree-sitter-oml`）：

```text
auto, ip, chars, digit, float, time, bool, obj, array,
time_iso, time_3339, time_2822, time_timestamp, time/clf,
url, domain, ip_net, kv, json, base64,
array/<子类型>, time/<子类型>
```

**说明**

- 值字面量的实际类型集合复用 WPL 的 datatype 解析能力
- `obj` 是 OML 特有类型，不在 WPL 类型表中
- `array/<子类型>` 与 `time/<子类型>` 支持斜杠子类型（如 `array/digit`、`time/iso`）
- 当前能直接解析的内置函数只有 `Now::time()`、`Now::date()`、`Now::hour()`
- 裸标识符只在“静态符号引用”场景下作为表达式使用

---

## `fmt(...)`

```ebnf
fmt_expr          = "fmt", "(", string, ",", var_get, { ",", var_get }, ")" ;
var_get           = read_expr | take_expr | "@", ident ;
```

**说明**

- `fmt` 至少要有一个参数
- 格式字符串当前按双引号字符串解析

**示例**

```oml
message = fmt("{host}:{code}", @host, read(status_code)) ;
```

---

## 管道表达式

```ebnf
pipe_expr         = [ "pipe" ], var_get, pipe_item, { pipe_item } ;
pipe_item         = "|", pipe_fun ;
pipe_fun          = pipe_fun_with_args | pipe_fun_simple ;

pipe_fun_with_args = "nth", "(", unsigned, ")"
                   | "get", "(", key_path, ")"
                   | "starts_with", "(", string, ")"
                   | "map_to", "(", map_value, ")"
                   | "base64_decode", "(", [ encode_type ], ")"
                   | "path", "(", [ "name" | "path" ], ")"
                   | "url", "(", [ "domain" | "host" | "uri" | "path" | "params" ], ")"
                   | "Time::to_ts_zone", "(", signed_int, ",", ("s" | "ss" | "ms" | "us"), ")" ;

pipe_fun_simple   = "base64_encode"
                  | "html_escape"
                  | "html_unescape"
                  | "str_escape"
                  | "json_escape"
                  | "json_unescape"
                  | "Time::to_ts"
                  | "Time::to_ts_ms"
                  | "Time::to_ts_us"
                  | "to_json"
                  | "to_str"
                  | "skip_empty"
                  | "ip4_to_int"
                  | "extract_main_word"
                  | "extract_subject_object" ;
```

**说明**

- `pipe` 关键字可以省略
- 无前缀管道的起点仍然必须是 `read(...)`、`take(...)` 或 `@field`
- 当前实现中没有 `sxf_get(...)`、`str_unescape`

**示例**

```oml
host = read(url) | url(host) ;
ts = pipe read(time) | Time::to_ts_zone(0, ms) ;
flag = @message | starts_with('ERROR') | map_to(true) ;
```

---

## 对象与数组

```ebnf
object_expr       = "object", "{", object_item, { object_item }, "}" ;
object_item       = object_targets, "=", object_value, [ ";" ] ;
object_targets    = ident, { ",", ident }, [ ":", data_type ] ;
object_value      = read_expr | take_expr | value_expr | builtin_fun | static_ref ;

collect_expr      = "collect", var_get ;
```

**说明**

- `object { ... }` 内部当前不再递归接受所有顶层表达式
- 子项值当前只支持 `read/take`、值字面量、`Now::*`、静态符号
- `collect` 的输入来源是一个 `var_get`

**示例**

```oml
info : obj = object {
    host : chars = read(hostname) ;
    city, province : chars = read() ;
} ;

ports : array = collect read(keys:[sport, dport]) ;
```

---

## `calc(...)`

```ebnf
calc_expr         = "calc", "(", add_expr, ")" ;
add_expr          = mul_expr, { ("+" | "-"), mul_expr } ;
mul_expr          = unary_expr, { ("*" | "/" | "%"), unary_expr } ;
unary_expr        = [ "-" ], primary_expr ;
primary_expr      = number | var_get | calc_fun | "(", add_expr, ")" ;
calc_fun          = ("abs" | "round" | "floor" | "ceil"), "(", add_expr, ")" ;
```

**说明**

- `calc` 里的字段访问支持 `read(...)`、`take(...)`、`@field`
- `%` 仅接受整数

---

## `match ... { ... }`

```ebnf
match_expr        = "match", match_source, "{", match_case, { match_case }, "}" ;
match_source      = var_get | "(", var_get, { ",", var_get }, ")" ;
match_case        = match_cond, "=>", match_value, [ "," ], [ ";" ] ;
match_value       = read_expr | take_expr | value_expr | collect_expr | static_ref ;
```

条件项的实际实现支持以下几类：

- 等值：`chars(ok)`
- 否定：`!chars(error)`
- 区间：`in (digit(200), digit(299))`
- 静态符号区间 / 等值
- 条件函数：
  - `starts_with`
  - `ends_with`
  - `contains`
  - `regex_match`
  - `is_empty`
  - `iequals`
  - `iequals_any`
  - `gt`
  - `lt`
  - `eq`
  - `in_range`
- OR 条件：`cond1 | cond2`

**重要限制**

- `match` 分支结果当前不直接接受 `calc(...)`
- 如需在分支里复用计算结果，先在前面绑定成字段，再在 `match` 中 `read(...)`

**示例**

```oml
level = match read(status_code) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(500), digit(599)) => chars(server_error) ;
    _ => chars(other) ;
} ;

priority = match (read(city), read(level)) {
    (chars(bj) | chars(sh), chars(high)) => chars(p1) ;
    _ => chars(p9) ;
} ;
```

---

## `lookup_nocase(...)`

```ebnf
lookup_expr       = "lookup_nocase", "(", ident, ",", lookup_arg, ",", lookup_arg, ")" ;
lookup_arg        = lookup_expr
                  | pipe_expr_noprefix
                  | direct_expr
                  | sql_literal
                  | value_expr
                  | builtin_fun
                  | static_ref ;
pipe_expr_noprefix = var_get, pipe_item, { pipe_item } ;
```

**说明**

- 第一个参数是静态 object 符号名
- 第二、第三个参数可以是嵌套表达式，但受当前解析器 `lookup_arg` 集合约束

---

## SQL

```ebnf
sql_expr          = "select", sql_body, "where", sql_cond, ";" ;
```

**当前实现特征**

- SQL 必须从 `select` 开始
- `where` 和结尾分号 `;` 都是必需的
- `select` 与 `where` 之间的 body 在严格模式下会校验为 `<cols> from <table>`
- `where` 条件由内部条件解析器处理，可引用 `read(...)`
- 针对 `fn(...) = literal` 做了兼容重写

**示例**

```oml
user_name, level =
    select name, level
    from users
    where id = read(user_id) ;
```

---

## `static { ... }`

```ebnf
static_expr       = static_value | static_object | static_calc ;
static_value      = value_expr ;
static_object     = object_expr ;
static_calc       = "calc", "(", static_calc_expr, ")" ;
```

**限制**

- 不支持 `read(...)`、`take(...)`
- 不支持 `pipe`、`collect`
- 不支持 `fmt(...)`
- 不支持 `match`
- 不支持 `lookup_nocase(...)`
- 不支持 SQL
- 不支持 `Now::*`
- 不支持在 `static` 中引用其他静态符号

其中 `static_object` 的外层写法仍然是 `object { ... }`，但对象内部每个子项也必须继续满足上述静态限制。

---

## 批量目标

当目标名中包含 `*` 时，当前实现进入批量模式：

```oml
* = take() ;
alert* = take() ;
*_log = read() ;
```

批量模式只支持 `read(...)` / `take(...)` 这两类来源。

---

## 实现差异说明

如果你在旧文档或旧示例里看到下面这些内容，请以当前实现为准：

- 隐私段
- `query ...`
- `sxf_get(...)`
- `static` 中执行运行期表达式
- `match` 分支里直接写 `calc(...)`
