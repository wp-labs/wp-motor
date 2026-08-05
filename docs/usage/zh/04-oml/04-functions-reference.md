# OML 函数参考

本文档只列出 `wp-motor/crates/wp-oml/src/parser` 当前解析实现真正支持的内置函数、内置表达式和管道函数。

> 说明：
> 1. `match` 条件函数单独整理在 [functions/match_functions.md](./functions/match_functions.md)
> 2. 历史文档中的 `sxf_get(...)` 不在当前解析器支持范围内
> 3. 管道表达式既可以写成 `pipe read(x) | ...`，也可以省略 `pipe` 写成 `read(x) | ...`

---

## 速查

### 内置表达式 / 内置函数

| 名称 | 说明 | 示例 |
|------|------|------|
| `calc(...)` | 显式数值表达式 | `score = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;` |
| `lookup_nocase(...)` | 基于静态 object 的忽略大小写查表 | `score = lookup_nocase(score_map, read(status), 40.0) ;` |
| `Now::time()` | 当前时间 | `event_time = Now::time() ;` |
| `Now::date()` | 当前日期，返回 `YYYYMMDD` 数字 | `today = Now::date() ;` |
| `Now::hour()` | 当前小时，返回 `YYYYMMDDHH` 数字 | `hour = Now::hour() ;` |
| `access_direct(src, dst)` | 判断访问方向（`内到内`/`内到外`/`外到内`/`外到外`） | `dir = access_direct(read(sip), read(dip)) | on_fail('unknown') ;` |

### 管道函数

| 分类 | 名称 | 示例 |
|------|------|------|
| 访问 | `nth(index)` | `first = read(items) | nth(0) ;` |
| 访问 | `get(name)` | `code = read(obj) | get(code) ;` |
| 条件 | `starts_with('prefix')` | `secure = read(url) | starts_with('https://') ;` |
| 映射 | `map_to(value)` | `flag = read(level) | starts_with('ERROR') | map_to(true) ;` |
| 编解码 | `base64_encode` | `encoded = read(msg) | base64_encode ;` |
| 编解码 | `base64_decode([EncodeType])` | `decoded = read(msg) | base64_decode(Utf8) ;` |
| 转义 | `html_escape` | `safe = read(html) | html_escape ;` |
| 转义 | `html_unescape` | `raw = read(html) | html_unescape ;` |
| 转义 | `str_escape` | `escaped = read(message) | str_escape ;` |
| 转义 | `json_escape` | `escaped = read(text) | json_escape ;` |
| 转义 | `json_unescape` | `raw = read(text) | json_unescape ;` |
| 时间 | `Time::to_ts` | `ts = read(time) | Time::to_ts ;` |
| 时间 | `Time::to_ts_ms` | `ts = read(time) | Time::to_ts_ms ;` |
| 时间 | `Time::to_ts_us` | `ts = read(time) | Time::to_ts_us ;` |
| 时间 | `Time::to_ts_zone(zone, unit)` | `ts = read(time) | Time::to_ts_zone(0, ms) ;` |
| 转换 | `to_json` | `payload = read(arr) | to_json ;` |
| 转换 | `to_str` | `ip_s = read(ip) | to_str ;` |
| 转换 | `ip4_to_int` | `ip_i = read(src_ip) | ip4_to_int ;` |
| 网络 | `intranet_ip` | `side = read(src_ip) | intranet_ip ;` |
| 控制 | `on_fail('值')` | `side = read(src_ip) | intranet_ip | on_fail('unknown') ;` |
| 提取 | `path([name|path])` | `file = read(path) | path(name) ;` |
| 提取 | `url([domain|host|uri|path|params])` | `host = read(url) | url(host) ;` |
| 控制 | `skip_empty` | `v = read(maybe_empty) | skip_empty ;` |
| 文本 | `extract_main_word` | `kw = read(msg) | extract_main_word ;` |
| 文本 | `extract_subject_object` | `info = read(msg) | extract_subject_object ;` |

---

## 内置表达式 / 内置函数

### `calc(...)`

显式执行数值计算。

**语法**

```oml
calc(<expr>)
```

**当前实现支持**

- 运算符：`+ - * / %`
- 一元负号：`-x`
- 括号分组
- 数学函数：`abs(...)`、`round(...)`、`floor(...)`、`ceil(...)`
- 操作数：数值字面量、`read(...)`、`take(...)`、`@field`

**类型规则**

- `+ - *`：任一操作数为浮点时结果为 `float`，否则为 `digit`
- `/`：结果总是 `float`
- `%`：只接受整数操作数，结果为 `digit`

**失败行为**

以下情况都会得到 `ignore`：

- 除零
- 字段缺失
- 非数值输入
- 整数溢出
- `NaN` / `inf`
- 对浮点数使用 `%`

**示例**

```oml
risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;
delta : digit = calc(read(cur) - read(prev)) ;
bucket : digit = calc(read(uid) % 16) ;
error_pct : digit = calc(round((read(err_cnt) * 100) / read(total_cnt))) ;
```

更多说明见 [functions/calc.md](./functions/calc.md)。

---

### `lookup_nocase(...)`

基于 `static` 中定义的 object 做忽略大小写查表。

**语法**

```oml
lookup_nocase(<dict_symbol>, <key_expr>, <default_expr>)
```

**参数**

- `dict_symbol`：静态 object 的符号名
- `key_expr`：待查 key，常见写法是 `read(status)`
- `default_expr`：未命中时返回的默认值

**当前实现特点**

- 第 1 个参数必须是静态符号名
- 第 2、3 个参数可继续嵌套 `lookup_nocase`、`read/take`、无前缀 pipe、SQL 字面量、值字面量、`Now::*`、静态符号
- 查找前会对 key 做 `trim + lowercase`

**示例**

```oml
static {
    score_map = object {
        error = float(90.0);
        warning = float(70.0);
        success = float(20.0);
    };
}

risk_score : float = lookup_nocase(score_map, read(status), 40.0) ;
```

更多说明见 [functions/lookup_nocase.md](./functions/lookup_nocase.md)。

---

### `Now::time()`

返回当前时间。

```oml
event_time : time = Now::time() ;
```

---

### `Now::date()`

返回当前日期，格式为 `YYYYMMDD`。

```oml
today : digit = Now::date() ;
```

---

### `Now::hour()`

返回当前小时，格式为 `YYYYMMDDHH`。

```oml
current_hour : digit = Now::hour() ;
```

---

### `access_direct(src, dst)`

组合 src/dst 两个 IP 的内外归属得到访问方向，返回 `内到内`/`内到外`/`外到内`/`外到外`。

**语法**
```oml
direction = access_direct(read(sip), read(dip)) ;
direction = access_direct(read(sip), read(dip)) | on_fail('unknown') ;
```

**说明**
- 内/外判定使用内网网段知识（`knowdb.toml` 的 `[intranet_nets]` 节）
- src/dst 缺失或非法输出 `Ignore`，可用管道 `| on_fail('unknown')` 兜底
- 支持 IPv4 + IPv6

**示例**
```oml
name : example_access_direct
---
sip : ip = take() ;
dip : ip = take() ;
direction = access_direct(read(sip), read(dip)) | on_fail('unknown') ;

# 内网→内网: "内到内"；内网→外网: "内到外"；外网→内网: "外到内"；外网→外网: "外到外"
# src/dst 缺失: "unknown"
```

---

## 管道函数

### 基本写法

```oml
result = pipe read(field) | function1 | function2(param) ;
result = read(field) | function1 | function2(param) ;
```

管道起点可以是：

- `read(...)`
- `take(...)`
- `@field`
- `access_direct(src, dst)`（其结果可作为管道源，如 `access_direct(a, b) | on_fail('x')`）

不能直接把裸标识符当作无前缀管道起点。

---

## 访问类

### `nth(index)`

读取数组中的第 `index` 个元素。

```oml
first_item = read(items) | nth(0) ;
```

### `get(name)`

从对象或嵌套结构中按键读取值；参数由实现里的 `take_key` 解析，因此常见的路径样式如 `a/b/c` 也可直接写入。

```oml
status = read(obj) | get(status) ;
nested = read(obj) | get(one/two) ;
```

### `path([name|path])`

提取路径中的指定部分。

```oml
file_name = read(file_path) | path(name) ;
dir_path = read(file_path) | path(path) ;
```

### `url([domain|host|uri|path|params])`

提取 URL 中的指定部分。

```oml
host = read(url) | url(host) ;
path_only = read(url) | url(path) ;
query = read(url) | url(params) ;
```

---

## 条件 / 映射类

### `starts_with('prefix')`

仅当输入字符串以前缀开头时保留原值，否则返回 `ignore`。

```oml
https_only = read(url) | starts_with('https://') ;
```

### `map_to(value)`

将非 `ignore` 输入映射成一个固定值。

参数类型当前支持：

- `bool`
- 整数
- 浮点数
- 单引号字符串

```oml
is_error = read(level) | starts_with('ERROR') | map_to(true) ;
bucket = read(status) | starts_with('5') | map_to(500) ;
```

---

## 编解码 / 转义类

### `base64_encode`

```oml
encoded = read(message) | base64_encode ;
```

### `base64_decode([EncodeType])`

括号参数可省略；省略时默认按 `Utf8` 处理。

```oml
decoded = read(payload) | base64_decode() ;
decoded_gbk = read(payload) | base64_decode(Gbk) ;
```

`EncodeType` 以实现中的枚举为准，常见值包括 `Utf8`、`Gbk`、`Ascii`、`Imap` 等。

### `html_escape`

```oml
safe_html = read(raw_html) | html_escape ;
```

### `html_unescape`

```oml
raw_html = read(encoded_html) | html_unescape ;
```

### `str_escape`

```oml
escaped = read(message) | str_escape ;
```

### `json_escape`

```oml
escaped = read(message) | json_escape ;
```

### `json_unescape`

```oml
raw = read(message) | json_unescape ;
```

---

## 时间转换类

### `Time::to_ts`

转为秒级时间戳。

```oml
ts = read(event_time) | Time::to_ts ;
```

### `Time::to_ts_ms`

转为毫秒级时间戳。

```oml
ts = read(event_time) | Time::to_ts_ms ;
```

### `Time::to_ts_us`

转为微秒级时间戳。

```oml
ts = read(event_time) | Time::to_ts_us ;
```

### `Time::to_ts_zone(zone, unit)`

按指定时区和单位转换时间戳。

`unit` 当前支持：

- `s`
- `ss`
- `ms`
- `us`

```oml
utc_ms = read(event_time) | Time::to_ts_zone(0, ms) ;
local_s = read(event_time) | Time::to_ts_zone(8, s) ;
```

---

## 转换 / 控制类

### `to_json`

```oml
payload = read(arr) | to_json ;
```

### `to_str`

```oml
ip_text = read(src_ip) | to_str ;
```

### `ip4_to_int`

```oml
ip_num = read(src_ip) | ip4_to_int ;
```

### `skip_empty`

空字符串时返回 `ignore`，否则透传原值。

```oml
city = read(city) | skip_empty ;
```

---

## 文本抽取类

### `extract_main_word`

从文本中提取主词。

```oml
keyword = read(message) | extract_main_word ;
```

### `extract_subject_object`

从文本中提取主客体结构。

```oml
summary = read(message) | extract_subject_object ;
```

---

## 相关文档

- [06-grammar-reference.md](./06-grammar-reference.md)
- [functions/function_index.md](./functions/function_index.md)
- [functions/match_functions.md](./functions/match_functions.md)
