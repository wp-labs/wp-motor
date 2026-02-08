# WPL 实战指南

本文档采用任务导向的方式，帮助你快速找到解决方案。

---

## 📚 任务导航

| 任务类型 | 跳转 |
|---------|------|
| [解析 Web 服务器日志](#1-解析-web-服务器日志) | Nginx/Apache 访问日志、错误日志 |
| [解析 JSON 数据](#2-解析-json-数据) | 提取 JSON 字段、嵌套 JSON |
| [解析 KV 键值对](#3-解析-kv-键值对) | 基础 KV、嵌套 KV、混合格式 |
| [处理编码数据](#4-处理编码数据) | Base64、Hex 解码 |
| [字段验证与过滤](#5-字段验证与过滤) | 检查字段、IP 范围、端口范围 |
| [复杂场景](#6-复杂场景) | 可变字段、多格式、嵌套结构 |
| [常见问题](#7-常见问题) | 调试技巧、性能优化 |

---

## 📋 快速参考

### 常用模式速查

| 模式 | 语法 | 说明 |
|------|------|------|
| **可选字段** | `opt(type:name)` | 字段可能不存在 |
| **重复字段** | `some_of(type:name)` | 匹配 1 到多个 |
| **跳过字段** | `_` 或 `n*_` | 跳过 1 个或 n 个字段 |
| **JSON 提取** | `json(type@path:name)` | 提取 JSON 字段 |
| **KV 提取** | `kvarr` | 解析键值对 |
| **Base64 解码** | `\|decode/base64\|` | 预处理管道 |
| **字段验证** | `type/check` | 验证字段值 |
| **择一匹配** | `one_of(...)` | 多个模式选一个 |

### 常用类型速查

| 类型 | 说明 | 示例 |
|------|------|------|
| `ip` | IP 地址 | `192.168.1.1` |
| `digit` | 整数 | `8080` |
| `chars` | 字符串 | `hello` |
| `time/clf` | Apache 时间格式 | `[06/Aug/2019:12:12:19 +0800]` |
| `http/request` | HTTP 请求 | `GET /index.html HTTP/1.1` |
| `json` | JSON 数据 | `{"key":"value"}` |
| `kvarr` | 键值对数组 | `key1=val1;key2=val2` |

---

## 📖 如何使用本指南

根据你的任务，找到对应章节，复制规则并根据实际情况调整。

---

## 1. 解析 Web 服务器日志

### 任务 1.1：解析 Nginx/Apache 访问日志

**场景：** 标准 Nginx/Apache 访问日志

**输入：**
```
192.168.1.2 - - [06/Aug/2019:12:12:19 +0800] "GET /index.html HTTP/1.1" 200 1024 "http://example.com/" "Mozilla/5.0"
```

**WPL 规则：**
```wpl
package nginx {
  rule access_log {
    (
      ip:client_ip,
      2*_,
      time/clf<[,]>:time,
      http/request":request,
      http/status:status,
      digit:bytes,
      chars":referer,
      http/agent":user_agent
    )
  }
}
```

**输出：**
```
client_ip: 192.168.1.2
time: 2019-08-06 12:12:19
request: GET /index.html HTTP/1.1
status: 200
bytes: 1024
referer: http://example.com/
user_agent: Mozilla/5.0
```

**要点：**
- `2*_` 忽略两个 `-` 字段
- `time/clf<[,]>` 解析方括号包裹的 CLF 时间
- `http/request"` 自动解析引号包裹的 HTTP 请求并提取方法、路径、协议
- `chars"` 提取引号包裹的字符串

---

### 任务 1.2：解析带变量的 Nginx 日志

**场景：** 自定义 Nginx log_format

**输入：**
```
2023-01-01T12:00:00+08:00|INFO|192.168.1.1|GET|/api/users|200|0.123
```

**WPL 规则：**
```wpl
package nginx {
  rule custom_log {
    (
      time_3339:timestamp,
      chars:level,
      ip:client_ip,
      http/method:method,
      chars:path,
      http/status:status,
      float:response_time
    )\|
  }
}
```

**输出：**
```
timestamp: 2023-01-01 12:00:00
level: INFO
client_ip: 192.168.1.1
method: GET
path: /api/users
status: 200
response_time: 0.123
```

**要点：**
- `)\|` 指定组级分隔符为管道符 `|`
- `time_3339` 解析 RFC 3339 时间格式
- `http/method` 专门解析 HTTP 方法
- `float` 解析浮点数（响应时间）

---

### 任务 1.3：解析带 referer 为空的日志

**场景：** referer 可能为 `-` 或空

**输入：**
```
192.168.1.1 [06/Aug/2019:12:12:19 +0800] "GET /index.html" 200 1024 "-"
```

**WPL 规则：**
```wpl
package nginx {
  rule access_log_optional {
    (
      ip:client_ip,
      time/clf<[,]>:time,
      http/request":request,
      http/status:status,
      digit:bytes,
      opt(chars"):referer
    )
  }
}
```

**输出：**
```
client_ip: 192.168.1.1
time: 2019-08-06 12:12:19
request: GET /index.html
status: 200
bytes: 1024
referer: -
```

**要点：**
- `opt(chars")` 将 referer 标记为可选字段
- 即使 referer 解析失败，其他字段仍能正常提取

---

## 2. 解析 JSON 数据

### 任务 2.1：提取 JSON 字段

**场景：** API 响应日志

**输入：**
```json
{"user":"admin","code":200,"message":"success","timestamp":"2023-01-01T12:00:00"}
```

**WPL 规则：**
```wpl
package api {
  rule response {
    (json(
      chars@user,
      digit@code,
      chars@message,
      time_3339@timestamp
    ))
  }
}
```

**输出：**
```
user: admin
code: 200
message: success
timestamp: 2023-01-01 12:00:00
```

**要点：**
- `json(type@key)` 语法提取指定键的值
- 类型自动验证和转换（`time_3339` 转换时间格式）

---

### 任务 2.2：处理嵌套 JSON

**场景：** 嵌套的 JSON 结构

**输入：**
```json
{"user":{"name":"Alice","age":25,"profile":{"city":"Beijing"}},"status":"active"}
```

**WPL 规则：**
```wpl
package api {
  rule nested_json {
    (json(
      chars@user/name,
      digit@user/age,
      chars@user/profile/city,
      chars@status
    ))
  }
}
```

**输出：**
```
user/name: Alice
user/age: 25
user/profile/city: Beijing
status: active
```

**要点：**
- 使用 `/` 分隔嵌套路径：`@user/name`, `@user/profile/city`
- 路径层级无限制

---

### 任务 2.3：JSON 反转义

**场景：** JSON 字符串包含转义字符

**输入：**
```json
{"path":"c:\\users\\admin\\file.txt","message":"line1\nline2"}
```

**WPL 规则：**
```wpl
package api {
  rule json_unescape {
    (json(chars@path, chars@message) |json_unescape())
  }
}
```

**输出：**
```
path: c:\users\admin\file.txt
message: line1
line2
```

**要点：**
- `|json_unescape()` 将 `\\n` 转换为实际换行符
- `\\\\` 转换为 `\`
- `\\\"` 转换为 `"`

---

### 任务 2.4：可选 JSON 字段

**场景：** 某些字段可能不存在

**输入：**
```json
{"user":"admin","code":200}
```

**WPL 规则：**
```wpl
package api {
  rule optional_fields {
    (json(
      chars@user,
      digit@code,
      opt(chars)@message,
      opt(chars)@data
    ))
  }
}
```

**输出：**
```
user: admin
code: 200
```

**要点：**
- `opt(type)@key` 标记字段为可选
- 不存在的字段不会导致解析失败

---

## 3. 解析 KV 键值对

### 任务 3.1：基础 KV 解析（分号分隔）

**场景：** 简单的 KV 格式日志

**输入：**
```
host=server1;port=8080;user=admin;status=online
```

**WPL 规则：**
```wpl
package config {
  rule kv_semicolon {
    (kvarr)
  }
}
```

**输出：**
```
host: server1
port: 8080
user: admin
status: online
```

**要点：**
- `kvarr` 自动解析所有KV对
- 自动识别分隔符

---

### 任务 3.2：固定数量 KV（逗号分隔）

**场景：** 华为防火墙日志（12 个固定 KV）

**输入：**
```
k1=v1,k2=v2,k3=v3,k4=v4,k5=v5,k6=v6,k7=v7,k8=v8,k9=v9,k10=v10,k11=v11,k12=v12
```

**WPL 规则：**
```wpl
package firewall {
  rule fixed_kv {
    (kvarr)
  }
}
```

**输出：**
```
k1: v1
k2: v2
...
k12: v12
```

**要点：**
- `kvarr` 自动解析所有KV对
- 不需要指定数量

---

### 任务 3.3：提取指定 KV 字段

**场景：** 只提取需要的字段

**输入：**
```
hostname=server1 port=3306 user=root db=test timeout=30
```

**WPL 规则：**
```wpl
package database {
  rule extract_kv {
    (kvarr(
      chars@hostname,
      digit@port,
      chars@user,
      opt(chars)@db
    ))
  }
}
```

**输出：**
```
hostname: server1
port: 3306
user: root
db: test
```

**要点：**
- `kvarr(type@key)` 提取指定键的值
- 未列出的键（如 `timeout`）被忽略
- `opt(type)@key` 标记可选字段

---

### 任务 3.4：混合 KV 格式

**场景：** 可选 KV + 多个 KV

**输入：**
```
1234,2023-01-01T12:00:00,ABC123,LOGIN:host=server;user=admin,port=8080,action=success
```

**WPL 规则：**
```wpl
package firewall {
  rule mixed_kv {
    (
      digit:id,
      time:timestamp,
      sn:serial,
      chars:type\:,
      opt(kvarr),
      kvarr
    )
  }
}
```

**输出：**
```
id: 1234
timestamp: 2023-01-01 12:00:00
serial: ABC123
type: LOGIN
host: server
user: admin
port: 8080
action: success
```

**要点：**
- `chars:type\:` 冒号作为分隔符
- `opt(kvarr)` 可选的KV
- `kvarr` 自动解析KV

---

## 4. 处理编码数据

### 任务 4.1：Base64 解码

**场景：** 华为防火墙日志（整行 Base64 编码）

**输入（Base64）：**
```
MTIzNCwyMDIzLTAxLTAxVDEyOjAwOjAwLEFCQzEyMyxMT0dJTjpob3N0PXNlcnZlcjt1c2VyPWFkbWluLHBvcnQ9ODA4MCxhY3Rpb249c3VjY2Vzcw==
```

**解码后：**
```
1234,2023-01-01T12:00:00,ABC123,LOGIN:host=server;user=admin,port=8080,action=success
```

**WPL 规则：**
```wpl
package firewall {
  rule huawei_log {
    |decode/base64|
    (
      digit:id,
      time:timestamp,
      sn:serial,
      chars:type\:,
      opt(kvarr),
      kvarr
    )
  }
}
```

**输出：**
```
id: 1234
timestamp: 2023-01-01 12:00:00
serial: ABC123
type: LOGIN
host: server
user: admin
port: 8080
action: success
```

**要点：**
- `|decode/base64|` 预处理管道，对整行进行 Base64 解码
- 解码后再进行字段解析

---

### 任务 4.2：十六进制解码

**场景：** 二进制数据的十六进制表示

**输入：**
```
48656c6c6f20576f726c64
```

**WPL 规则：**
```wpl
package binary {
  rule hex_decode {
    |decode/hex|
    (chars:data)
  }
}
```

**输出：**
```
data: Hello World
```

**要点：**
- `|decode/hex|` 将十六进制字符串解码为原始文本

---

### 任务 4.3：组合多步预处理

**场景：** Base64 + JSON 反转义

**输入（Base64）：**
```
eyJwYXRoIjoiY1xcXFx1c2Vyc1xcXFxmaWxlIiwidGV4dCI6ImxpbmUxXG5saW5lMiJ9
```

**解码后：**
```json
{"path":"c:\\users\\file","text":"line1\nline2"}
```

**WPL 规则：**
```wpl
package security {
  rule multi_step {
    |decode/base64|unquote/unescape|
    (json(chars@path, chars@text))
  }
}
```

**输出：**
```
path: c:\users\file
text: line1
line2
```

**要点：**
- 可以链接多个预处理步骤
- 执行顺序：从左到右

---

## 5. 字段验证与过滤

### 任务 5.1：检查字段存在

**场景：** 确保必需字段存在

**输入：**
```json
{"status":"ok","message":"success","data":null}
```

**WPL 规则：**
```wpl
package api {
  rule check_required {
    (json |f_has(status) |f_has(message))
  }
}
```

**输出：**
```
status: ok
message: success
data: null
```

**要点：**
- `|f_has(field)` 检查字段是否存在
- 字段不存在时解析失败

---

### 任务 5.2：验证状态码

**场景：** 只处理成功的响应（200/201/204）

**输入：**
```json
{"code":200,"status":"success","data":"result"}
```

**WPL 规则：**
```wpl
package api {
  rule validate_success {
    (json |f_digit_in(code, [200, 201, 204]))
  }
}
```

**输出：**
```
code: 200
status: success
data: result
```

**要点：**
- `|f_digit_in(field, [list])` 验证数字字段值在列表中
- 不在列表中时解析失败

---

### 任务 5.3：过滤特定方法

**场景：** 只处理 GET/POST 请求

**输入：**
```json
{"method":"GET","path":"/api/users","status":200}
```

**WPL 规则：**
```wpl
package api {
  rule filter_methods {
    (json |f_chars_in(method, [GET, POST]))
  }
}
```

**输出：**
```
method: GET
path: /api/users
status: 200
```

**要点：**
- `|f_chars_in(field, [list])` 验证字符串字段值在列表中

---

### 任务 5.4：链式验证

**场景：** 多个条件组合

**输入：**
```json
{"user":"admin","age":25,"status":"active"}
```

**WPL 规则：**
```wpl
package api {
  rule chain_validation {
    (json(chars@user, digit@age, chars@status)
      |take(user)
      |chars_has(admin)
      |take(age)
      |digit_in([18, 25, 30])
      |take(status)
      |chars_has(active)
    )
  }
}
```

**输出：**
```
user: admin
age: 25
status: active
```

**要点：**
- `take(field)` 选择字段为活跃字段
- 然后对活跃字段进行验证
- 可以链式调用多个验证

---

## 6. 复杂场景

### 任务 6.1：可变数量字段（some_of）

**场景：** 字段数量和顺序不固定

**输入：**
```
192.168.1.1 k1=v1 200 k2=v2 300 k3=v3
```

**WPL 规则：**
```wpl
package mixed {
  rule variable_fields {
    some_of(ip, kv, digit)
  }
}
```

**输出：**
```
ip: 192.168.1.1
k1: v1
digit: 200
k2: v2
digit: 300
k3: v3
```

**要点：**
- `some_of(...)` 循环匹配所有可能的类型
- 尽可能多地消费字段

---

### 任务 6.2：择一匹配（alt）

**场景：** 某个位置可能是不同类型

**输入：**
```
user_id:12345 action:login
user_id:admin action:logout
```

**WPL 规则：**
```wpl
package auth {
  rule flexible_user_id {
    (
      chars:key1,
      alt(digit, chars):user_id,
      chars:key2,
      chars:action
    )
  }
}
```

**输出（输入 1）：**
```
key1: user_id
user_id: 12345
key2: action
action: login
```

**输出（输入 2）：**
```
key1: user_id
user_id: admin
key2: action
action: logout
```

**要点：**
- `alt(type1, type2)` 尝试多种类型
- 匹配第一个成功的类型

---

### 任务 6.3：读到行尾

**场景：** 最后一个字段包含所有剩余内容

**输入：**
```
2023-01-01 ERROR This is a very long error message with many details
```

**WPL 规则：**
```wpl
package log {
  rule read_to_end {
    (
      time:timestamp,
      chars:level,
      chars\0:message
    )
  }
}
```

**输出：**
```
timestamp: 2023-01-01
level: ERROR
message: This is a very long error message with many details
```

**要点：**
- `\0` 表示读到行尾
- 常用于日志的 message 字段

---

## 7. 常见问题

### Q1: 如何处理可变数量的字段？

**答案：** 使用 `N*type` 或 `some_of`

```wpl
# KV 自动解析
kvarr

# 混合类型
some_of(ip, digit, kvarr)
```

---

### Q2: 如何忽略某些字段？

**答案：** 使用 `_` 或 `N*_`

```wpl
# 忽略 1 个字段
(ip, _, time)

# 忽略 3 个字段
(ip, 3*_, time)
```

---

### Q3: 分隔符不一致怎么办？

**答案：** 使用字段级分隔符优先级

```wpl
# 不同字段不同分隔符
(digit\;, ip\,, chars\s)

# 或使用组级分隔符
(digit, ip, chars)\,
```

---

### Q4: JSON 字段可能不存在怎么办？

**答案：** 使用 `opt(type)@key`

```wpl
json(
  chars@user,
  opt(chars)@email,
  opt(digit)@age
)
```

---

### Q5: 如何提取嵌套 JSON 字段？

**答案：** 使用路径语法 `@path/to/field`

```wpl
json(
  chars@user/name,
  digit@user/age,
  chars@data/result
)
```

---

### Q6: 预处理管道失败怎么办？

**答案：** 检查以下几点
1. 管道名称是否正确
2. 是否以 `|` 结尾
3. 输入数据格式是否正确

```wpl
# 正确
|decode/base64|

# 错误：缺少结尾 |
|decode/base64
```

---

### Q7: 如何调试解析失败的规则？

**步骤：**
1. 简化规则，从最简单的字段开始
2. 使用 `opt()` 标记可疑字段
3. 检查分隔符是否正确
4. 检查格式控制符（引号、括号等）

```wpl
# 原规则（失败）
(digit, time, chars, json)

# 调试规则
(digit, opt(time), opt(chars), opt(json))
# 逐步确定哪个字段导致失败
```

---

## 下一步

### 查阅参考
→ [04-language-reference.md](./04-language-reference.md) - 完整类型和语法
→ [05-functions-reference.md](./05-functions-reference.md) - 所有函数详解

### 深入理解
→ [02-core-concepts.md](./02-core-concepts.md) - 理解 WPL 设计理念

---

## 相关资源

- 快速入门：[01-quickstart.md](./01-quickstart.md)
- 核心概念：[02-core-concepts.md](./02-core-concepts.md)
- 语言参考：[04-language-reference.md](./04-language-reference.md)
- 函数参考：[05-functions-reference.md](./05-functions-reference.md)
