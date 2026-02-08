# WPL 管道函数

本页描述 WPL 中可在字段级管道中使用的内置函数。这些函数用于对解析后的字段进行选择、条件判断、过滤与转换。

## 函数分类

WPL 管道函数分为四类：

| 类型 | 前缀 | 说明 |
|------|------|------|
| **选择器函数** | 无前缀 | 选择特定字段作为活跃字段 |
| **字段集合操作** | `f_` | 在字段集合中查找指定字段进行操作 |
| **活跃字段操作** | 无前缀 | 直接对当前活跃字段进行检查 |
| **转换函数** | 无前缀 | 对字段数据进行转换处理 |

> **活跃字段说明**：管道中的操作默认作用于"活跃字段"。可以使用选择器函数切换活跃字段，或在 `f_` 前缀函数中用 `_` 表示当前活跃字段。


### Selector 函数

在使用无前缀别名前，需要先选定活动字段：

| 函数名 | 参数 | 说明 |
|--------|------|------|
| `take` | 1 | 选定指定字段作为活动字段 |
| `last` | 0 | 选定最后一个字段作为活动字段 |

## 函数概览


### 选择器函数

| 函数名 | 参数 | 说明 |
|--------|------|------|
| `take` | 1 | 选择指定字段作为活跃字段 |
| `last` | 0 | 选择最后一个字段作为活跃字段 |

### 字段集合操作函数（`f_` 前缀）

| 函数名 | 参数 | 说明 |
|--------|------|------|
| `f_has` | 1 | 检查指定字段是否存在 |
| `f_chars_has` | 2 | 检查指定字段值是否等于指定字符串 |
| `f_chars_not_has` | 2 | 检查指定字段值是否不等于指定字符串 |
| `f_chars_in` | 2 | 检查指定字段值是否在字符串列表中 |
| `f_digit_has` | 2 | 检查指定字段数值是否等于指定数字 |
| `f_digit_in` | 2 | 检查指定字段数值是否在数字列表中 |
| `f_ip_in` | 2 | 检查指定 IP 字段是否在 IP 列表中 |

### 活跃字段操作函数（无前缀）

| 函数名 | 参数 | 说明 |
|--------|------|------|
| `has` | 0 | 检查活跃字段是否存在 |
| `chars_has` | 1 | 检查活跃字段值是否等于指定字符串 |
| `chars_not_has` | 1 | 检查活跃字段值是否不等于指定字符串 |
| `chars_in` | 1 | 检查活跃字段值是否在字符串列表中 |
| `digit_has` | 1 | 检查活跃字段数值是否等于指定数字 |
| `digit_in` | 1 | 检查活跃字段数值是否在数字列表中 |
| `ip_in` | 1 | 检查活跃 IP 字段是否在 IP 列表中 |

### 转换函数

| 函数名 | 参数 | 说明 |
|--------|------|------|
| `json_unescape` | 0 | 对 chars 类型字段进行 JSON 反转义 |
| `base64_decode` | 0 | 对 chars 类型字段进行 Base64 解码 |

## 选择器函数详解

### `take`

选择指定名称的字段作为活跃字段，后续的无前缀操作将作用于该字段。

**语法：**
```
take(<field_name>)
```

**参数：**
- `field_name`：要选择的字段名

**示例：**
```wpl
rule select_field {
  (
    json(chars@name, digit@age)
    |take(name)
    |chars_has(admin)
  )
}
```


### `last`

选择字段集合中的最后一个字段作为活跃字段。

**语法：**
```
last()
```

**示例：**
```wpl
rule use_last {
  (
    json(chars@a, chars@b, chars@c)
    |last()
    |chars_has(value)
  )
}
```


## 字段存在检查函数详解

### `has`

检查当前活跃字段是否存在。

**语法：**
```
has()
```

**示例：**
```wpl
rule check_active {
  (
    json(chars@name)
    |take(name)
    |has()
  )
}
```

---

### `f_has`

检查指定字段是否存在于字段集合中。

**语法：**
```
f_has(<field_name>)
has()
```

**参数：**
- `field_name`：要检查的字段名（`f_has` 使用）
- 无参数（`has` 使用，需先通过 `take()` 选定活动字段）

**示例：**
```wpl
rule check_field {
  (
    json |f_has(age)
  )
}

rule check_field_with_take {
  (
    json(chars@code) |take(code) |has()
  )
}
```


## 字符串操作函数详解

### `chars_has`

检查当前活跃字段的值是否等于指定字符串。

**语法：**
```
chars_has(<value>)
```

**参数：**
- `value`：要匹配的字符串值

**示例：**
```wpl
rule check_value {
  (
    json(chars@status)
    |take(status)
    |chars_has(success)
  )
}

rule check_error_with_take {
  (
    json(chars@msg) |take(msg) |chars_has(error)
  )
}
```

---


### `f_chars_has`

检查字段集合中指定字段的值是否等于指定字符串。

**语法：**
```
f_chars_has(<field_name>, <value>)
```

**参数：**
- `field_name`：要检查的字段名（使用 `_` 表示当前活跃字段）
- `value`：要匹配的字符串值

**示例：**
```wpl
rule check_message {
  (
    json |f_chars_has(message, error)
  )
}

# 使用 _ 表示活跃字段
rule check_active_field {
  (
    json(chars@name)
    |take(name)
    |f_chars_has(_, admin)
  )
}
```

---

### `chars_not_has`

检查当前活跃字段的值是否不等于指定字符串。

**语法：**
```
chars_not_has(<value>)
```

**参数：**
- `value`：不应匹配的字符串值

**示例：**
```wpl
rule exclude_value {
  (
    json(chars@status)
    |take(status)
    |chars_not_has(failed)
  )
}
```

---

### `f_chars_not_has`

检查字段集合中指定字段的值是否不等于指定字符串。

**语法：**
```
f_chars_not_has(<field_name>, <value>)
```

**参数：**
- `field_name`：要检查的字段名
- `value`：不应存在的字符串值

**示例：**
```wpl
rule filter_success {
  (
    json |f_chars_not_has(status, failed)
  )
}

rule filter_success_with_take {
  (
    json(chars@status) |take(status) |chars_not_has(failed)
  )
}
```


### `chars_in`
>>>>>>> 805600da457bb0fcfd70e2d5ce70e7774105068b

检查当前活跃字段的值是否在给定的字符串列表中。

**语法：**
```
chars_in([<value1>, <value2>, ...])
```

**参数：**
- `[...]`：允许的字符串值列表

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

### `f_chars_in`

检查字段集合中指定字段的值是否在给定的字符串列表中。

**语法：**
```
f_chars_in(<field_name>, [<value1>, <value2>, ...])
chars_in([<value1>, <value2>, ...])
```

**参数：**
- `field_name`：要检查的字段名（`f_chars_in` 使用）
- `[...]`：允许的字符串值列表
- 无前缀形式需先通过 `take()` 选定活动字段

**示例：**
```wpl
rule check_method {
  (
    json |f_chars_in(method, [GET, POST, PUT])
  )
}

rule check_method_with_take {
  (
    json(chars@method) |take(method) |chars_in([GET, POST, PUT])
  )
}
```


## 数字操作函数详解

### `digit_has`

检查当前活跃字段的数值是否等于指定数字。

**语法：**
```
digit_has(<number>)
```

**参数：**
- `number`：要匹配的数字

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

### `f_digit_has`

检查字段集合中指定字段的数值是否等于指定数字。

**语法：**
```
f_digit_has(<field_name>, <number>)
digit_has(<number>)
```

**参数：**
- `field_name`：要检查的字段名（`f_digit_has` 使用）
- `number`：要匹配的数字
- 无前缀形式需先通过 `take()` 选定活动字段

**示例：**
```wpl
rule check_status {
  (
    json |f_digit_has(code, 200)
  )
}

rule check_status_with_take {
  (
    json(digit@code) |take(code) |digit_has(200)
  )
}
```

---

### `digit_in`

检查当前活跃字段的数值是否在给定的数字列表中。

**语法：**
```
digit_in([<num1>, <num2>, ...])
```

**参数：**
- `[...]`：允许的数字值列表

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

### `f_digit_in`

检查字段集合中指定字段的数值是否在给定的数字列表中。

**语法：**
```
f_digit_in(<field_name>, [<num1>, <num2>, ...])
digit_in([<num1>, <num2>, ...])
```

**参数：**
- `field_name`：要检查的字段名（`f_digit_in` 使用）
- `[...]`：允许的数字值列表
- 无前缀形式需先通过 `take()` 选定活动字段

**示例：**
```wpl
rule check_success_codes {
  (
    json |f_digit_in(status, [200, 201, 204])
  )
}

rule check_success_codes_with_take {
  (
    json(digit@status) |take(status) |digit_in([200, 201, 204])
  )
}
```


## IP 地址操作函数详解

### `ip_in`

检查当前活跃字段的 IP 地址是否在给定的 IP 列表中。支持 IPv4 和 IPv6。

**语法：**
```
ip_in([<ip1>, <ip2>, ...])
```

**参数：**
- `[...]`：允许的 IP 地址列表

**示例：**
```wpl
rule check_client {
  (
    json(ip@client_ip)
    |take(client_ip)
    |ip_in([127.0.0.1, 192.168.1.1])
  )
}

# 支持 IPv6
rule check_ipv6 {
  (
    json(ip@src)
    |take(src)
    |ip_in([::1, 2001:db8::1])
  )
}
```


检查字段集合中指定 IP 地址是否在给定的 IP 列表中。支持 IPv4 和 IPv6。

**语法：**
```
f_ip_in(<field_name>, [<ip1>, <ip2>, ...])
ip_in([<ip1>, <ip2>, ...])
```

**参数：**
- `field_name`：要检查的字段名（`f_ip_in` 使用）
- `[...]`：允许的 IP 地址列表
- 无前缀形式需先通过 `take()` 选定活动字段

**示例：**
```wpl
rule check_trusted_ips {
  (
    json(ip@client_ip) |f_ip_in(client_ip, [127.0.0.1, 192.168.1.1])
  )
}

rule check_ipv6 {
  (
    json(ip@src) |f_ip_in(src, [::1, 2001:db8::1])
  )
}

rule check_ip_with_take {
  (
    json(ip@client_ip) |take(client_ip) |ip_in([127.0.0.1, 192.168.1.1])
  )
}
```

---

## 转换函数详解

### `json_unescape`

对当前活跃字段进行 JSON 反转义处理。将 JSON 转义序列转换为实际字符。

**语法：**
```
json_unescape()
```


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

### `base64_decode`

对当前活跃字段进行 Base64 解码。将 Base64 编码的字符串解码为原始文本。

**语法：**
```
base64_decode()
```

**转换效果：**
```
"aGVsbG8gd29ybGQ="  →  "hello world"
```

**示例：**
```wpl
rule decode_payload {
  (
<<<<<<< HEAD
    json(chars@payload) |last() |base64_decode()
  )
}

rule decode_with_take {
  (
    json(chars@encoded_data) |take(encoded_data) |base64_decode()
=======
    json(chars@payload)
    |take(payload)
    |base64_decode()
>>>>>>> 805600da457bb0fcfd70e2d5ce70e7774105068b
  )
}
```

---

## 组合使用示例

多个管道函数可以链式调用：

### 使用 `f_` 前缀函数

```wpl
rule complex_filter {
  (
    json(
      chars@method,
      digit@status,
      ip@client_ip
    )
  )
  |f_has(method)
  |f_chars_in(method, [GET, POST])
  |f_digit_in(status, [200, 201, 204])
  |f_ip_in(client_ip, [10.0.0.1, 10.0.0.2])
}
```

<<<<<<< HEAD
### 使用 `take()` + 无前缀别名

```wpl
rule filter_with_take {
  (
    json(
      chars@method,
      digit@status,
      ip@client_ip
    )
  )
  |take(method)
  |has()
  |chars_in([GET, POST])
  |take(status)
  |digit_in([200, 201, 204])
  |take(client_ip)
  |ip_in([10.0.0.1, 10.0.0.2])
}
```

### 使用 `last()` 进行转换

```wpl
rule decode_last_field {
  (
    json(chars@payload)
  )
  |last()
  |base64_decode()
  |json_unescape()
}
```
=======
使用选择器和无前缀函数的组合：

```wpl
rule mixed_style {
  (
    json(chars@name, digit@age, chars@status)
    |take(name)
    |chars_has(admin)
    |take(age)
    |digit_in([18, 19, 20])
    |take(status)
    |chars_not_has(disabled)
  )
}
```

---
>>>>>>> 805600da457bb0fcfd70e2d5ce70e7774105068b

## 相关文档

- 语法定义：[WPL 语法（EBNF）](./03-wpl_grammar.md)
- 实现代码：`crates/wp-lang/src/parser/wpl_fun.rs`
