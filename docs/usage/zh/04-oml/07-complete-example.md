# OML 完整功能示例

> 一个完整的示例，展示 OML 的所有核心功能

本文档提供一个全面的 OML 示例，涵盖所有核心功能，包括基础操作、内置函数、管道函数、高级匹配等。这是学习和参考 OML 功能的最佳起点。

---

## 📚 快速导航

| 章节 | 内容 |
|------|------|
| [原始数据](#原始数据) | 测试数据 |
| [WPL 解析规则](#wpl-解析规则) | 数据解析规则 |
| [OML 配置](#oml-配置) | 完整的 OML 转换配置 |
| [功能详解](#功能详解) | 每个功能的详细说明 |
| [关键要点](#关键要点) | WPL 与 OML 关联、功能清单 |


---

## 原始数据

```
222.133.52.20 simple_chars 80 192.168.1.10 select_one left 2025-12-29 12:00:00 {"msg":"hello"} "" aGVsbG8gd29ybGQ= ["val1","val2","val3"] /home/user/file.txt http://example.com/path/to/resource?foo=1&bar=2 [{"one":{"two":"nested"}}] foo bar baz qux 500 ext_value_1 ext_value_2 http://localhost:8080/bua/sync/health?a=test 525tab beijing shanghai 10.0.0.1 10.0.0.100 success enabled true sport:8080 dport:9090 details[0]/process_name:proc1 details[1]/process_name:proc2 optional_field:exists source_field:data another_field:value
```

---

## WPL 解析规则

```wpl
package T4 {
    rule case {
        (
            ip:sip,
            chars:simple_chars,
            digit:simple_port,
            ip:simple_ip,
            chars:select_one,
            chars:match_chars,
            time:timestamp_zone,
            json(chars@msg: json_msg),
            chars:empty_chars,
            base64 | (chars:base64),
            array/chars:array_str,
            chars:path,
            chars:url,
            array:obj,
            chars:one,
            chars:two,
            chars:three,
            chars:four,
            digit:num_range,
            chars:extend1,
            chars:extend2,
            chars:html,
            chars:str,
            chars:city1,
            chars:city2,
            ip:src_ip,
            ip:dst_ip,
            chars:status,
            chars:enabled,
            bool:enabled
        )
    }
}
```

**说明**：WPL 规则将原始数据解析为结构化字段，并附加 `rule = T4/case` 标识。

---

## OML 配置

```oml
name : T4
rule : T4/case
---

// ==================== 1. 基础操作 ====================

// 1.1 直接赋值字面量
direct_chars = chars(13);
direct_digit = digit(13);

// 1.2 简单取值
simple_chars = read(simple_chars);
simple_port : digit = read(simple_port);
simple_ip : ip = read(simple_ip);

// 1.3 选择取值（按顺序尝试多个字段）
select_chars = read(option:[select_one, select_two]);

// 1.4 默认值处理（字段不存在时使用默认值）
field_with_default = read(optional_field) { _ : chars(DEFAULT_VALUE) };
version_fallback : chars = read(version) { _ : chars(v1.0.0) };

// 1.5 多目标同时赋值
target1, target2 : chars = read();
name_alias, name_copy = read(name);

// 1.6 匿名目标（丢弃不需要的返回值）
_, useful_field = read(option:[field1, field2]);

// 1.7 take vs read 区别（破坏性 vs 非破坏性）
field_taken = take(source_field);                                    // take 会移除源字段
field_taken_again = take(source_field) { _ : chars(already_taken) }; // 再次 take 失败
field_read1 = read(another_field);                                   // read 不移除
field_read2 = read(another_field);                                   // 可重复读取

// 1.8 通配符批量操作
all_fields = take();                      // 取所有字段
path_fields = take(keys:[*/path]);        // 批量匹配：所有以 /path 结尾
a_name_fields = read(keys:[A*/name]);     // 前缀匹配：A 开头、/name 结尾

// ==================== 2. 内置函数 ====================

// 2.1 时间函数
current_time = Now::time();  // 获取当前完整时间
current_date = Now::date();  // 获取当前日期（YYYYMMDD）
current_hour = Now::hour();  // 获取当前小时（YYYYMMDDHH）

// ==================== 3. 模式匹配 ====================

// 3.1 单源 match（简单匹配）
match_chars = match read(option:[match_chars]) {
    chars(left) => chars(1);
    chars(middle) => chars(2);
    chars(right) => chars(3);
};

// 3.2 范围判断（in 操作符）
num_range = match read(option:[num_range]) {
    in (digit(0), digit(1000)) => read(num_range);
    _ => digit(0);
};

// 3.3 双源 match（匹配两个字段的组合）
location : chars = match (read(city1), read(city2)) {
    (chars(beijing), chars(shanghai)) => chars(east_region);
    (chars(chengdu), chars(chongqing)) => chars(west_region);
    _ => chars(unknown_region);
};

region_by_ip : chars = match (read(src_ip), read(dst_ip)) {
    (ip(10.0.0.1), ip(10.0.0.100)) => chars(internal);
    _ => chars(external);
};

// 3.4 match 否定条件（! 操作符）
valid_status = match read(status) {
    !chars(error) => chars(ok);
    !chars(failed) => chars(success);
    _ => chars(unknown);
};

// 3.5 布尔类型 match
is_enabled : digit = match read(enabled) {
    bool(true) => digit(1);
    bool(false) => digit(0);
    _ => digit(-1);
};

// ==================== 4. 管道函数 ====================

// 4.1 时间转换
timestamp_zone = pipe read(timestamp_zone) | Time::to_ts_zone(0, ms);  // 修改时区
timestamp_s = pipe read(timestamp_zone) | Time::to_ts;                 // 转秒级时间戳
timestamp_ms = pipe @current_time | Time::to_ts_ms;                    // 转毫秒级时间戳
timestamp_us = pipe @current_time | Time::to_ts_us;                    // 转微秒级时间戳
timestamp_zone_8 = pipe @current_time | Time::to_ts_zone(8, s);        // UTC+8 时区

// 4.2 编码/解码
base64_decoded = pipe read(base64) | base64_decode(Utf8);  // Base64 解码
base64_encoded = pipe read(base64) | base64_encode;        // Base64 编码

// 4.3 转义/反转义
html_escaped = pipe read(html) | html_escape;              // HTML 转义
html_unescaped = pipe read(html) | html_unescape;          // HTML 反转义
json_escaped = pipe read(json_escape) | json_escape;       // JSON 转义
json_unescaped = pipe @json_escaped | json_unescape;       // JSON 反转义
str_escaped = pipe read(str) | str_escape;                 // 字符串转义

// 4.4 数据转换
to_str_result = pipe read(str) | to_str;                   // 转为字符串
array_json = pipe read(array_str) | to_json;               // 数组转 JSON
ip_to_int = pipe read(simple_ip) | ip4_to_int;             // IPv4 转整数

// 4.5 集合操作
array_first = pipe read(array_str) | nth(0);               // 获取数组第 0 个元素
obj_nested = pipe read(obj) | nth(0) | get(one/two);       // 对象嵌套取值

// 4.6 数据提取
file_name = pipe read(path) | path(name);                  // 提取文件名
file_path = pipe read(path) | path(path);                  // 提取文件路径
url_domain = pipe read(url) | url(domain);                 // 提取 URL domain
url_host = pipe read(url) | url(host);                     // 提取 URL host
url_uri = pipe read(url) | url(uri);                       // 提取 URL uri
url_path = pipe read(url) | url(path);                     // 提取 URL path
url_params = pipe read(url) | url(params);                 // 提取 URL params

// 4.7 其他管道函数
skip_empty_result = pipe read(empty_chars) | skip_empty;   // 跳过空值

// 4.8 省略 pipe 关键字（新语法）
simple_transform = read(data) | to_json;                   // 直接省略 pipe
chained_ops = read(array_data) | nth(0) | to_str;          // 链式调用
url_extract = read(url_field) | url(domain);               // 简化写法

// 4.9 链式管道操作
nested_extract = pipe read(complex_obj) | nth(0) | get(level1/level2/level3);
multi_transform = pipe read(raw_data) | base64_decode(Utf8) | to_json;

// ==================== 5. 字符串操作 ====================

// 5.1 字符串格式化（fmt 函数）
splice = fmt("{one}:{two}|{three}:{four}", read(one), read(two), read(three), read(four));

// ==================== 6. 对象与数组 ====================

// 6.1 对象创建（聚合多个字段）
extends = object {
    extend1, extend2 = read();
};

// 6.2 数组收集（collect）
collected_ports : array = collect read(keys:[sport, dport, extra_port]);
wildcard_items : array = collect take(keys:[details[*]/process_name]);  // 支持通配符收集
```

---

## 功能详解

### 1. 基础操作

#### 1.1 字面量赋值
直接创建常量值：
```oml
direct_chars = chars(13);
direct_digit = digit(13);
```

#### 1.2 简单取值
从输入数据读取字段：
```oml
simple_chars = read(simple_chars);
simple_port : digit = read(simple_port);  // 显式类型转换
simple_ip : ip = read(simple_ip);
```

#### 1.3 选择取值
按优先级尝试多个字段：
```oml
select_chars = read(option:[select_one, select_two]);
// 先尝试 select_one，不存在则尝试 select_two
```

#### 1.4 默认值处理
字段不存在时使用默认值：
```oml
field_with_default = read(optional_field) { _ : chars(DEFAULT_VALUE) };
version_fallback : chars = read(version) { _ : chars(v1.0.0) };
```

#### 1.5 多目标赋值
一次赋值给多个目标：
```oml
target1, target2 : chars = read();
name_alias, name_copy = read(name);
```

#### 1.6 匿名目标
丢弃不需要的返回值：
```oml
_, useful_field = read(option:[field1, field2]);
// 第一个返回值被丢弃
```

#### 1.7 take vs read
- `take`：破坏性读取，移除源字段
- `read`：非破坏性读取，保留源字段

```oml
field_taken = take(source_field);      // 源字段被移除
field_taken_again = take(source_field) { _ : chars(already_taken) }; // 失败

field_read1 = read(another_field);     // 源字段保留
field_read2 = read(another_field);     // 可以再次读取
```

#### 1.8 通配符批量操作
使用通配符匹配多个字段：
```oml
all_fields = take();                   // 取所有字段
path_fields = take(keys:[*/path]);     // 所有以 /path 结尾
a_name_fields = read(keys:[A*/name]);  // A 开头、/name 结尾
```

---

### 2. 内置函数

时间相关函数：
```oml
current_time = Now::time();  // 2025-12-29 12:00:00
current_date = Now::date();  // 20251229
current_hour = Now::hour();  // 2025122912
```

---

### 3. 模式匹配

#### 3.1 单源 match
基于单个字段的值进行匹配：
```oml
match_chars = match read(option:[match_chars]) {
    chars(left) => chars(1);
    chars(middle) => chars(2);
    chars(right) => chars(3);
};
```

#### 3.2 范围判断
使用 `in` 操作符判断范围：
```oml
num_range = match read(option:[num_range]) {
    in (digit(0), digit(1000)) => read(num_range);
    _ => digit(0);
};
```

#### 3.3 双源 match
匹配两个字段的组合：
```oml
location : chars = match (read(city1), read(city2)) {
    (chars(beijing), chars(shanghai)) => chars(east_region);
    (chars(chengdu), chars(chongqing)) => chars(west_region);
    _ => chars(unknown_region);
};
```

#### 3.4 否定条件
使用 `!` 操作符进行否定匹配：
```oml
valid_status = match read(status) {
    !chars(error) => chars(ok);
    !chars(failed) => chars(success);
    _ => chars(unknown);
};
```

#### 3.5 布尔类型 match
匹配布尔值：
```oml
is_enabled : digit = match read(enabled) {
    bool(true) => digit(1);
    bool(false) => digit(0);
    _ => digit(-1);
};
```

---

### 4. 管道函数

#### 4.1 时间转换
```oml
timestamp_zone = pipe read(timestamp_zone) | Time::to_ts_zone(0, ms);  // UTC 毫秒
timestamp_s = pipe read(timestamp_zone) | Time::to_ts;                 // 秒级
timestamp_ms = pipe @current_time | Time::to_ts_ms;                    // 毫秒级
timestamp_us = pipe @current_time | Time::to_ts_us;                    // 微秒级
timestamp_zone_8 = pipe @current_time | Time::to_ts_zone(8, s);        // UTC+8
```

#### 4.2 编码/解码
```oml
base64_decoded = pipe read(base64) | base64_decode(Utf8);
base64_encoded = pipe read(base64) | base64_encode;
```

#### 4.3 转义/反转义
```oml
html_escaped = pipe read(html) | html_escape;
html_unescaped = pipe read(html) | html_unescape;
json_escaped = pipe read(json_escape) | json_escape;
json_unescaped = pipe @json_escaped | json_unescape;
str_escaped = pipe read(str) | str_escape;
```

#### 4.4 数据转换
```oml
to_str_result = pipe read(str) | to_str;
array_json = pipe read(array_str) | to_json;
ip_to_int = pipe read(simple_ip) | ip4_to_int;
```

#### 4.5 集合操作
```oml
array_first = pipe read(array_str) | nth(0);           // 获取第 0 个元素
obj_nested = pipe read(obj) | nth(0) | get(one/two);   // 嵌套取值
```

#### 4.6 数据提取
```oml
file_name = pipe read(path) | path(name);      // file.txt
file_path = pipe read(path) | path(path);      // /home/user
url_domain = pipe read(url) | url(domain);     // example.com
url_host = pipe read(url) | url(host);         // example.com
url_uri = pipe read(url) | url(uri);           // /path/to/resource?foo=1&bar=2
url_path = pipe read(url) | url(path);         // /path/to/resource
url_params = pipe read(url) | url(params);     // foo=1&bar=2
```

#### 4.7 控制函数
```oml
skip_empty_result = pipe read(empty_chars) | skip_empty;  // 跳过空值
```

#### 4.8 简化语法
省略 `pipe` 关键字：
```oml
simple_transform = read(data) | to_json;
chained_ops = read(array_data) | nth(0) | to_str;
url_extract = read(url_field) | url(domain);
```

#### 4.9 链式操作
```oml
nested_extract = pipe read(complex_obj) | nth(0) | get(level1/level2/level3);
multi_transform = pipe read(raw_data) | base64_decode(Utf8) | to_json;
```

---

### 5. 字符串操作

格式化字符串：
```oml
splice = fmt("{one}:{two}|{three}:{four}", read(one), read(two), read(three), read(four));
// 输出：foo:bar|baz:qux
```

---

### 6. 对象与数组

#### 6.1 对象创建
聚合多个字段为对象：
```oml
extends = object {
    extend1, extend2 = read();
};
```

#### 6.2 数组收集
收集多个字段为数组：
```oml
collected_ports : array = collect read(keys:[sport, dport, extra_port]);
// 输出：[8080, 9090, ...]

wildcard_items : array = collect take(keys:[details[*]/process_name]);
// 输出：["proc1", "proc2"]
```

---

## 关键要点

### WPL 与 OML 关联

```
原始数据
    ↓
[WPL 解析] → 生成结构化数据 + rule 标识
    ↓
数据携带: rule = "T4/case"
    ↓
[查找匹配的 OML] → 匹配 rule 字段
    ↓
[执行 OML 转换] → 应用本示例的转换逻辑
    ↓
输出到 Sink
```

**关键**：OML 的 `rule : T4/case` 与 WPL 的 `package T4 { rule case { ... } }` 对应。

### 功能覆盖清单

- ✅ 基础操作：字面量、取值、默认值、通配符
- ✅ 内置函数：时间函数
- ✅ 模式匹配：单源、双源、范围、否定、布尔
- ✅ 管道函数：时间、编解码、转义、转换、集合、提取
- ✅ 字符串操作：格式化
- ✅ 对象与数组：聚合、收集

---

## 下一步

- [快速入门](./01-quickstart.md) - 学习基础语法
- [核心概念](./02-core-concepts.md) - 理解设计理念
- [实战指南](./03-practical-guide.md) - 查找具体任务的解决方案
- [函数参考](./04-functions-reference.md) - 查阅所有可用函数
- [集成指南](./05-integration.md) - 了解如何集成到数据流

---

**提示**：这个示例是学习 OML 的最佳参考，建议收藏并在实际使用时对照查阅。
