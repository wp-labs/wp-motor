# WPL 函数参考

本文档提供所有 WPL 函数的标准化参考。

---

## 📋 WPL 所有函数速查

### 预处理管道函数

| 函数 | 说明 | 示例 |
|------|------|------|
| [`decode/base64`](#decodebase64) | 对整行 Base64 解码 | `\|decode/base64\|` |
| [`decode/hex`](#decodehex) | 对整行十六进制解码 | `\|decode/hex\|` |
| [`unquote/unescape`](#unquoteunescape) | 移除引号并还原转义 | `\|unquote/unescape\|` |
| [`plg_pipe/<name>`](#plg_pipename) | 自定义预处理扩展 | `\|plg_pipe/dayu\|` |

### 选择器函数

| 函数 | 说明 | 示例 |
|------|------|------|
| [`take(name)`](#take) | 选择指定字段为活跃字段 | `\|take(name)\|` |
| [`last()`](#last) | 选择最后一个字段为活跃字段 | `\|last()\|` |

### 字段集操作函数（f_ 前缀）

| 函数 | 说明 | 示例 |
|------|------|------|
| [`f_has(name)`](#f_has) | 检查字段是否存在 | `\|f_has(status)\|` |
| [`f_chars_has(name, val)`](#f_chars_has) | 检查字段值等于字符串 | `\|f_chars_has(status, success)\|` |
| [`f_chars_not_has(name, val)`](#f_chars_not_has) | 检查字段值不等于字符串 | `\|f_chars_not_has(level, error)\|` |
| [`f_chars_in(name, [...])`](#f_chars_in) | 检查字段值在字符串列表 | `\|f_chars_in(method, [GET, POST])\|` |
| [`f_digit_has(name, num)`](#f_digit_has) | 检查字段值等于数字 | `\|f_digit_has(code, 200)\|` |
| [`f_digit_in(name, [...])`](#f_digit_in) | 检查字段值在数字列表 | `\|f_digit_in(status, [200, 201])\|` |
| [`f_ip_in(name, [...])`](#f_ip_in) | 检查 IP 在列表 | `\|f_ip_in(client_ip, [127.0.0.1])\|` |

### 活跃字段操作函数

| 函数 | 说明 | 示例 |
|------|------|------|
| [`has()`](#has) | 检查活跃字段存在 | `\|take(name)\| has()\|` |
| [`chars_has(val)`](#chars_has) | 检查活跃字段等于字符串 | `\|take(status)\| chars_has(success)\|` |
| [`chars_not_has(val)`](#chars_not_has) | 检查活跃字段不等于字符串 | `\|take(level)\| chars_not_has(error)\|` |
| [`chars_in([...])`](#chars_in) | 检查活跃字段在字符串列表 | `\|take(method)\| chars_in([GET, POST])\|` |
| [`digit_has(num)`](#digit_has) | 检查活跃字段等于数字 | `\|take(code)\| digit_has(200)\|` |
| [`digit_in([...])`](#digit_in) | 检查活跃字段在数字列表 | `\|take(status)\| digit_in([200, 201])\|` |
| [`ip_in([...])`](#ip_in) | 检查活跃 IP 在列表 | `\|take(client_ip)\| ip_in([127.0.0.1])\|` |

### 转换函数

| 函数 | 说明 | 示例 |
|------|------|------|
| [`json_unescape()`](#json_unescape) | JSON 反转义 | `\|take(message)\| json_unescape()\|` |
| [`base64_decode()`](#base64_decode) | Base64 解码 | `\|take(payload)\| base64_decode()\|` |

### 常用场景速查

| 我想做什么 | 使用方法 |
|-----------|---------|
| **对整行 Base64 解码** | `\|decode/base64\|` |
| **对整行十六进制解码** | `\|decode/hex\|` |
| **移除引号和转义** | `\|unquote/unescape\|` |
| **检查字段是否存在** | `\|f_has(field_name)\|` |
| **检查字段值等于某字符串** | `\|f_chars_has(status, success)\|` |
| **检查字段值在列表中** | `\|f_chars_in(method, [GET, POST])\|` |
| **检查状态码是否为 200** | `\|f_digit_has(code, 200)\|` |
| **检查 IP 是否在白名单** | `\|f_ip_in(client_ip, [127.0.0.1, 192.168.1.1])\|` |
| **选择特定字段验证** | `\|take(status)\| chars_has(ok)\|` |
| **对字段 JSON 反转义** | `\|take(message)\| json_unescape()\|` |
| **对字段 Base64 解码** | `\|take(payload)\| base64_decode()\|` |
| **链式验证多个条件** | `\|f_has(method)\| f_chars_in(method, [GET, POST])\|` |

---

## 预处理管道函数

### decode/base64

**说明：** 对整行输入进行 Base64 解码

**语法：**
```wpl
|decode/base64|
```

**参数：** 无

**示例：**
```wpl
rule base64_log {
  |decode/base64|
  (json(chars@user, digit@code))
}
```

**输入（Base64）：**
```
eyJ1c2VyIjoiYWRtaW4iLCJjb2RlIjoyMDB9
```

**解码后：**
```json
{"user":"admin","code":200}
```

**注意：**
- 作用于整行原始输入
- 必须在字段解析前执行
- 解码失败会报错

---

### decode/hex

**说明：** 对整行输入进行十六进制解码

**语法：**
```wpl
|decode/hex|
```

**参数：** 无

**示例：**
```wpl
rule hex_log {
  |decode/hex|
  (chars:data)
}
```

**输入：**
```
48656c6c6f20576f726c64
```

**输出：**
```
data: Hello World
```

---

### unquote/unescape

**说明：** 移除外层引号并还原反斜杠转义

**语法：**
```wpl
|unquote/unescape|
```

**参数：** 无

**示例：**
```wpl
rule unescape_log {
  |unquote/unescape|
  (chars:message)
}
```

**转换效果：**
| 输入 | 输出 |
|------|------|
| `\"hello\"` | `hello` |
| `path\\to\\file` | `path\to\file` |

---

### plg_pipe/<name>

**说明：** 自定义预处理扩展

**语法：**
```wpl
|plg_pipe/<name>|
```

**参数：** `<name>` - 注册的扩展名称

**示例：**
```wpl
rule custom_preproc {
  |plg_pipe/dayu|
  (json(_@_origin, _@payload/packet_data))
}
```

**注意：**
- 需要通过代码注册扩展
- 注册接口：`wpl::register_wpl_pipe!`

---

## 选择器函数

### take

**说明：** 选择指定字段为活跃字段

**语法：**
```wpl
|take(<field_name>)|
```

**参数：**
- `field_name` - 要选择的字段名

**返回：** 无（改变活跃字段状态）

**示例：**
```wpl
rule take_example {
  (
    json(chars@name, digit@age)
    |take(name)              # 选择 name 为活跃字段
    |chars_has(admin)        # 验证 name 的值
  )
}
```

**使用场景：**
- 需要对特定字段进行验证
- 链式验证多个字段

---

### last

**说明：** 选择最后一个字段为活跃字段

**语法：**
```wpl
|last()|
```

**参数：** 无

**返回：** 无（改变活跃字段状态）

**示例：**
```wpl
rule last_example {
  (
    json(chars@a, chars@b, chars@c)
    |last()                  # 选择 c 为活跃字段
    |chars_has(value)        # 验证 c 的值
  )
}
```

---

## 字段集操作函数

### f_has

**说明：** 检查指定字段是否存在

**语法：**
```wpl
|f_has(<field_name>)|
```

**参数：**
- `field_name` - 要检查的字段名

**返回：** 布尔值（字段存在返回 true，否则失败）

**示例：**
```wpl
rule check_field {
  (json |f_has(status) |f_has(message))
}
```

---

### f_chars_has

**说明：** 检查指定字段值是否等于字符串

**语法：**
```wpl
|f_chars_has(<field_name>, <value>)|
```

**参数：**
- `field_name` - 字段名（或 `_` 表示活跃字段）
- `value` - 要匹配的字符串值

**返回：** 布尔值

**示例：**
```wpl
rule check_string {
  (json |f_chars_has(status, success))
}
```

---

### f_chars_not_has

**说明：** 检查指定字段值是否不等于字符串

**语法：**
```wpl
|f_chars_not_has(<field_name>, <value>)|
```

**参数：**
- `field_name` - 字段名
- `value` - 不应匹配的字符串值

**返回：** 布尔值

**示例：**
```wpl
rule exclude_error {
  (json |f_chars_not_has(level, error))
}
```

---

### f_chars_in

**说明：** 检查指定字段值是否在字符串列表中

**语法：**
```wpl
|f_chars_in(<field_name>, [<value1>, <value2>, ...])|
```

**参数：**
- `field_name` - 字段名
- `[...]` - 允许的字符串值列表

**返回：** 布尔值

**示例：**
```wpl
rule check_method {
  (json |f_chars_in(method, [GET, POST, PUT]))
}
```

---

### f_digit_has

**说明：** 检查指定字段数值是否等于指定数字

**语法：**
```wpl
|f_digit_has(<field_name>, <number>)|
```

**参数：**
- `field_name` - 字段名
- `number` - 要匹配的数字

**返回：** 布尔值

**示例：**
```wpl
rule check_status {
  (json |f_digit_has(code, 200))
}
```

---

### f_digit_in

**说明：** 检查指定字段数值是否在数字列表中

**语法：**
```wpl
|f_digit_in(<field_name>, [<num1>, <num2>, ...])|
```

**参数：**
- `field_name` - 字段名
- `[...]` - 允许的数字值列表

**返回：** 布尔值

**示例：**
```wpl
rule check_success {
  (json |f_digit_in(status, [200, 201, 204]))
}
```

---

### f_ip_in

**说明：** 检查指定 IP 字段是否在 IP 列表中

**语法：**
```wpl
|f_ip_in(<field_name>, [<ip1>, <ip2>, ...])|
```

**参数：**
- `field_name` - 字段名
- `[...]` - 允许的 IP 地址列表（支持 IPv4/IPv6）

**返回：** 布尔值

**示例：**
```wpl
rule check_trusted {
  (json |f_ip_in(client_ip, [127.0.0.1, 192.168.1.1]))
}
```

---

## 活跃字段操作函数

### has

**说明：** 检查活跃字段是否存在

**语法：**
```wpl
|has()|
```

**参数：** 无

**返回：** 布尔值

**示例：**
```wpl
rule check_active {
  (
    json(chars@name)
    |take(name)
    |has()                   # 检查 name 是否存在
  )
}
```

---

### chars_has

**说明：** 检查活跃字段值是否等于指定字符串

**语法：**
```wpl
|chars_has(<value>)|
```

**参数：**
- `value` - 要匹配的字符串值

**返回：** 布尔值

**示例：**
```wpl
rule check_value {
  (
    json(chars@status)
    |take(status)
    |chars_has(success)
  )
}
```

---

### chars_not_has

**说明：** 检查活跃字段值是否不等于指定字符串

**语法：**
```wpl
|chars_not_has(<value>)|
```

**参数：**
- `value` - 不应匹配的字符串值

**返回：** 布尔值

**示例：**
```wpl
rule exclude_value {
  (
    json(chars@level)
    |take(level)
    |chars_not_has(error)
  )
}
```

---

### chars_in

**说明：** 检查活跃字段值是否在字符串列表中

**语法：**
```wpl
|chars_in([<value1>, <value2>, ...])|
```

**参数：**
- `[...]` - 允许的字符串值列表

**返回：** 布尔值

**示例：**
```wpl
rule check_method {
  (
    json(chars@method)
    |take(method)
    |chars_in([GET, POST, PUT])
  )
}
```

---

### digit_has

**说明：** 检查活跃字段数值是否等于指定数字

**语法：**
```wpl
|digit_has(<number>)|
```

**参数：**
- `number` - 要匹配的数字

**返回：** 布尔值

**示例：**
```wpl
rule check_code {
  (
    json(digit@code)
    |take(code)
    |digit_has(200)
  )
}
```

---

### digit_in

**说明：** 检查活跃字段数值是否在数字列表中

**语法：**
```wpl
|digit_in([<num1>, <num2>, ...])|
```

**参数：**
- `[...]` - 允许的数字值列表

**返回：** 布尔值

**示例：**
```wpl
rule check_success {
  (
    json(digit@status)
    |take(status)
    |digit_in([200, 201, 204])
  )
}
```

---

### ip_in

**说明：** 检查活跃 IP 字段是否在 IP 列表中

**语法：**
```wpl
|ip_in([<ip1>, <ip2>, ...])|
```

**参数：**
- `[...]` - 允许的 IP 地址列表（支持 IPv4/IPv6）

**返回：** 布尔值

**示例：**
```wpl
rule check_client {
  (
    json(ip@client_ip)
    |take(client_ip)
    |ip_in([127.0.0.1, 192.168.1.1])
  )
}
```

---

## 转换函数

### json_unescape

**说明：** 对活跃字段进行 JSON 反转义

**语法：**
```wpl
|json_unescape()|
```

**参数：** 无

**转换效果：**
| 输入 | 输出 |
|------|------|
| `hello\\nworld` | `hello` + 换行 + `world` |
| `path\\\\to` | `path\to` |
| `say\\\"hi\\\"` | `say"hi"` |

**示例：**
```wpl
rule parse_json_log {
  (
    json(chars@message)
    |take(message)
    |json_unescape()
  )
}
```

---

### base64_decode

**说明：** 对活跃字段进行 Base64 解码

**语法：**
```wpl
|base64_decode()|
```

**参数：** 无

**转换效果：**
| 输入 | 输出 |
|------|------|
| `aGVsbG8=` | `hello` |
| `Zm9vYmFy` | `foobar` |

**示例：**
```wpl
rule decode_payload {
  (
    json(chars@payload)
    |take(payload)
    |base64_decode()
  )
}
```

---

## 📊 函数对照表

### 字段集 vs 活跃字段

| 功能 | 字段集操作（f_ 前缀） | 活跃字段操作（无前缀） |
|------|---------------------|---------------------|
| 检查存在 | `f_has(name)` | `take(name) \| has()` |
| 字符串相等 | `f_chars_has(name, val)` | `take(name) \| chars_has(val)` |
| 字符串不等 | `f_chars_not_has(name, val)` | `take(name) \| chars_not_has(val)` |
| 字符串在列表 | `f_chars_in(name, [a, b])` | `take(name) \| chars_in([a, b])` |
| 数字相等 | `f_digit_has(name, 200)` | `take(name) \| digit_has(200)` |
| 数字在列表 | `f_digit_in(name, [200, 201])` | `take(name) \| digit_in([200, 201])` |
| IP 在列表 | `f_ip_in(name, [1.1.1.1])` | `take(name) \| ip_in([1.1.1.1])` |

---

## 💡 使用模式

### 链式调用

```wpl
rule chain {
  (json
    |f_has(method)
    |f_chars_in(method, [GET, POST])
    |f_digit_in(status, [200, 201])
    |f_ip_in(client_ip, [10.0.0.1])
  )
}
```

### 选择器 + 验证

```wpl
rule selector {
  (json(chars@name, digit@age)
    |take(name)
    |chars_has(admin)
    |take(age)
    |digit_in([18, 19, 20])
  )
}
```

### 预处理 + 字段级管道

```wpl
rule full {
  |decode/base64|                      # 预处理：整行解码
  (json |f_has(name) |f_digit_in(age, [18, 25]))  # 字段级：验证
}
```

---

## 相关文档

- 快速入门：[01-quickstart.md](./01-quickstart.md)
- 核心概念：[02-core-concepts.md](./02-core-concepts.md)
- 实战指南：[03-practical-guide.md](./03-practical-guide.md)
- 语言参考：[04-language-reference.md](./04-language-reference.md)
- 语法规范：[06-grammar-reference.md](./06-grammar-reference.md)
