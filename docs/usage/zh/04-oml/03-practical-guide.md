# OML 实战指南

按任务导向组织的实用指南，帮助你快速找到解决方案。

---

## 📚 任务导航

| 任务类型 | 跳转 |
|---------|------|
| [WPL 与 OML 关联](#wpl-与-oml-关联) | 理解关联机制、一对一/一对多关联 |
| [数据提取](#数据提取) | 字段提取的各种方式 |
| [数据转换](#数据转换) | 类型转换、时间、URL、Base64 等 |
| [数据聚合](#数据聚合) | 创建对象、数组 |
| [条件处理](#条件处理) | 状态码分类、端口识别、IP 范围等 |
| [数据富化](#数据富化-sql-查询) | SQL 查询、多表关联 |
| [复杂场景](#复杂场景) | Web 日志、系统监控完整处理 |

---

## WPL 与 OML 关联

### 任务：理解关联机制

**核心概念**：OML 通过 `rule` 字段匹配 WPL 的 `package/rule` 路径来建立关联。

**WPL 规则**：
```wpl
package nginx {
  rule access_log {
    (ip:client_ip, time:timestamp, chars:request_uri, digit:status)
  }
}
```

**完整路径**：`/nginx/access_log`（格式：`/package/rule`）

**OML 配置**：
```oml
name : nginx_processor
rule : /nginx/access_log    # 匹配 WPL 的 package/rule
---
client : ip = read(client_ip) ;
time : time = read(timestamp) ;
uri = read(request_uri) ;
status : digit = read(status) ;
```

**说明**：只有 WPL rule 为 `/nginx/access_log` 的数据会被这个 OML 处理。

---

### 任务：一对多关联（通配符匹配）

**场景**：一个 WPL 规则可以被多个 OML 配置处理

**WPL 规则**：
```
package : nginx
rule : access_log
# 完整路径：/nginx/access_log
```

**OML 配置 1**（基础处理）：
```oml
name : nginx_basic
rule : /nginx/*    # 匹配所有 nginx 相关规则
---
timestamp : time = Now::time() ;
source = chars(nginx) ;
```

**OML 配置 2**（访问日志专用）：
```oml
name : nginx_access_detail
rule : /nginx/access_log    # 精确匹配访问日志
---
user_id = read(user_id) ;
uri = read(request_uri) ;
status : digit = read(status) ;
```

**说明**：同一条数据可以被多个 OML 配置处理（如果在不同的 Sink Group 中）。

---

### 任务：通配符模式匹配

**场景**：使用通配符处理多种类型的数据

**支持的通配符模式**：

| OML rule | 匹配的 WPL rule | 说明 |
|----------|----------------|------|
| `/nginx/*` | `/nginx/access_log`<br/>`/nginx/error_log` | 前缀匹配 |
| `*/access_log` | `/nginx/access_log`<br/>`/apache/access_log` | 后缀匹配 |
| `/nginx/access*` | `/nginx/access_log`<br/>`/nginx/access_v2` | 部分匹配 |
| `*` | 任意规则 | 全匹配 |

**示例**：处理所有访问日志
```oml
name : all_access_logs
rule : */access_log    # 匹配所有 access_log
---
timestamp : time = Now::time() ;
uri = read(request_uri) ;
status : digit = read(status) ;
```

---

### 任务：多个 WPL 规则共享一个 OML

**场景**：不同来源的数据使用相同的转换逻辑

**WPL 规则 1**：
```
package : nginx
rule : access_log
# 路径：/nginx/access_log
```

**WPL 规则 2**：
```
package : apache
rule : access_log
# 路径：/apache/access_log
```

**共享的 OML 配置**：
```oml
name : web_access_handler
rule : */access_log    # 匹配所有 access_log
---
# 统一的字段映射
timestamp : time = read(time) ;
client_ip : ip = read(option:[remote_addr, client_ip]) ;
uri = read(option:[request_uri, request]) ;
status : digit = read(option:[status, status_code]) ;

# 统一的输出格式
access : obj = object {
    time : time = read(timestamp) ;
    ip : ip = read(client_ip) ;
    uri : chars = read(uri) ;
    status : digit = read(status) ;
} ;
```

**说明**：使用 `option` 参数处理不同来源的字段名差异。

---

## 数据提取

### 综合示例：字段提取的各种方式

```oml
name : data_extraction
rule : /app/data
---
# 1. 简单提取
user_id = read(user_id) ;

# 2. 提供默认值
country = read(country) { _ : chars(CN) } ;

# 3. 按优先级尝试多个字段
user_id = read(option:[id, user_id, uid]) ;

# 4. 提取嵌套数据
username = read(/user/info/name) ;

# 5. 批量提取匹配模式
cpu_metrics = collect read(keys:[cpu_*]) ;
```

---

## 数据转换

### 综合示例：常用类型转换

```oml
name : type_conversion
rule : /app/data
---
# 字符串转各种类型
port : digit = read(port) ;                    # 转整数
ip : ip = read(ip_addr) ;                      # 转 IP
cpu : float = read(cpu_usage) ;                # 转浮点数
active : bool = read(is_active) ;              # 转布尔值

# 时间转时间戳
ts_sec = read(event_time) | Time::to_ts_zone(0, s) ;    # 秒
ts_ms = read(event_time) | Time::to_ts_zone(8, ms) ;    # 毫秒（UTC+8）

# URL 解析
domain = read(url) | url(domain) ;
path = read(url) | url(path) ;
params = read(url) | url(params) ;

# Base64 编解码
decoded = read(encoded) | base64_decode(Utf8) ;
encoded = read(message) | base64_encode ;

# IP 转整数
ip_int = read(src_ip) | ip4_to_int ;
```

---

## 数据聚合

### 任务：创建对象

```oml
name : create_object
rule : /system/metrics
---
system_info : obj = object {
    host : chars = read(hostname) ;
    cpu : float = read(cpu_usage) ;
    memory : float = read(mem_usage) ;
} ;
```

---

### 任务：创建嵌套对象

```oml
name : nested_object
rule : /app/deployment
---
deployment : obj = object {
    application : obj = object {
        name : chars = read(app_name) ;
        version : chars = read(version) ;
    } ;
    infrastructure : obj = object {
        region : chars = read(region) ;
        instance_id : chars = read(instance_id) ;
    } ;
} ;
```

---

### 任务：创建数组

```oml
name : create_array
rule : /network/ports
---
# 收集多个端口
ports : array = collect read(keys:[sport, dport]) ;

# 转换为 JSON 字符串
ports_json = read(ports) | to_json ;

# 获取数组元素
first_port = read(ports) | nth(0) ;
```

---

## 条件处理

### 任务：状态码分类

```oml
name : status_classification
rule : /http/response
---
status_level = match read(status_code) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(400), digit(499)) => chars(client_error) ;
    in (digit(500), digit(599)) => chars(server_error) ;
    _ => chars(unknown) ;
} ;
```

---

### 任务：端口服务识别

```oml
name : port_service
rule : /network/traffic
---
service = match read(port) {
    digit(22) => chars(SSH) ;
    digit(80) => chars(HTTP) ;
    digit(443) => chars(HTTPS) ;
    digit(3306) => chars(MySQL) ;
    _ => chars(Unknown) ;
} ;
```

---

### 任务：IP 地址范围匹配

```oml
name : ip_zone_match
rule : /network/connection
---
zone = match read(src_ip) {
    in (ip(10.0.0.0), ip(10.255.255.255)) => chars(Private) ;
    in (ip(172.16.0.0), ip(172.31.255.255)) => chars(Private) ;
    in (ip(192.168.0.0), ip(192.168.255.255)) => chars(Private) ;
    _ => chars(Public) ;
} ;
```

---

### 任务：多条件组合判断

```oml
name : multi_condition
rule : /firewall/rule
---
traffic_type = match (read(protocol), read(port)) {
    (chars(tcp), digit(22)) => chars(SSH) ;
    (chars(tcp), digit(443)) => chars(HTTPS) ;
    (chars(udp), digit(53)) => chars(DNS) ;
    _ => chars(Other) ;
} ;
```

---

## 数据富化（SQL 查询）

### 任务：用户信息查询

**场景**：根据 user_id 查询用户详细信息

**输入**：
```
user_id = "1001"
```

**数据库表 (users)**：
| id | name | email | department |
|----|------|-------|------------|
| 1001 | 张三 | zhangsan@example.com | 研发部 |

**OML**：
```oml
name : user_lookup
---
user_name, user_email, user_dept =
    select name, email, department
    from users
    where id = read(user_id) ;
```

**输出**：
```
user_name = "张三"
user_email = "zhangsan@example.com"
user_dept = "研发部"
```

---

### 任务：IP 地理位置查询

**场景**：查询 IP 地址的地理位置信息

**输入**：
```
src_ip = "203.0.113.1"
```

**数据库表 (ip_geo)**：
| ip_start_int | ip_end_int | country | city |
|--------------|------------|---------|------|
| 3405803776 | 3405804031 | US | Los Angeles |

**OML**：
```oml
name : ip_geolocation
---
# 先将 IP 转为整数
ip_int = pipe read(src_ip) | ip4_to_int ;

# 查询地理位置
country, city =
    select country, city
    from ip_geo
    where ip_start_int <= read(ip_int)
      and ip_end_int >= read(ip_int) ;
```

**输出**：
```
ip_int = 3405803777
country = "US"
city = "Los Angeles"
```

---

### 任务：多表关联查询

**场景**：通过多次查询关联多个表的数据

**输入**：
```
order_id = "ORD-2024-001"
```

**OML**：
```oml
name : multi_table_lookup
---
# 第一步：查询订单信息
user_id, amount =
    select user_id, amount
    from orders
    where id = read(order_id) ;

# 第二步：查询用户信息
user_name, level =
    select name, level
    from users
    where id = read(user_id) ;

# 第三步：查询折扣信息
discount =
    select discount
    from user_levels
    where level = read(level) ;
```

**输出**：
```
user_id = "U1001"
amount = "199.99"
user_name = "王五"
level = "VIP"
discount = "0.9"
```

---

## 复杂场景

### 场景：Web 访问日志完整处理

**任务**：处理 Web 访问日志，包含字段提取、类型转换、条件判断、数据聚合

**输入**：
```
timestamp = "15/Jan/2024:14:30:00 +0800"
src_ip = "203.0.113.1"
method = "GET"
url = "/api/users?page=1"
status = "200"
size = "1234"
```

**OML**：
```oml
name : web_log_processing
---
# 时间处理
event_ts = pipe read(timestamp) | Time::to_ts_zone(0, s) ;

# 字段提取
source_ip : ip = read(src_ip) ;
http_method = read(method) ;
status_code : digit = read(status) ;
response_size : digit = read(size) ;

# URL 解析
request_path = pipe read(url) | url(path) ;
query_params = pipe read(url) | url(params) ;

# 状态码分类
status_category = match read(status_code) {
    in (digit(200), digit(299)) => chars(Success) ;
    in (digit(400), digit(499)) => chars(Client_Error) ;
    in (digit(500), digit(599)) => chars(Server_Error) ;
    _ => chars(Unknown) ;
} ;

# 数据聚合
access_log : obj = object {
    timestamp : digit = read(event_ts) ;
    client : obj = object {
        ip : ip = read(source_ip) ;
    } ;
    request : obj = object {
        method : chars = read(http_method) ;
        path : chars = read(request_path) ;
        query : chars = read(query_params) ;
    } ;
    response : obj = object {
        status : digit = read(status_code) ;
        category : chars = read(status_category) ;
        size : digit = read(response_size) ;
    } ;
} ;
```

**输出**：
```json
{
    "access_log": {
        "timestamp": 1705318200,
        "client": {
            "ip": "203.0.113.1"
        },
        "request": {
            "method": "GET",
            "path": "/api/users",
            "query": "page=1"
        },
        "response": {
            "status": 200,
            "category": "Success",
            "size": 1234
        }
    }
}
```

---

### 场景：系统监控数据处理

**任务**：处理系统监控数据，包含数据提取、告警判断、嵌套对象创建

**输入**：
```
hostname = "prod-web-01"
cpu_user = "65.5"
cpu_system = "15.2"
mem_used = "6144"
mem_total = "8192"
```

**OML**：
```oml
name : system_monitoring
---
# 时间戳
event_time = Now::time() ;

# 告警判断
cpu_alert = match read(cpu_user) {
    in (digit(0), digit(60)) => chars(Normal) ;
    in (digit(60), digit(80)) => chars(Warning) ;
    _ => chars(Critical) ;
} ;

mem_alert = match read(mem_used) {
    in (digit(0), digit(6000)) => chars(Normal) ;
    in (digit(6000), digit(7000)) => chars(Warning) ;
    _ => chars(Critical) ;
} ;

# 数据聚合
metrics : obj = object {
    host : obj = object {
        name : chars = read(hostname) ;
        timestamp : time = read(event_time) ;
    } ;
    cpu : obj = object {
        user : float = read(cpu_user) ;
        system : float = read(cpu_system) ;
        alert : chars = read(cpu_alert) ;
    } ;
    memory : obj = object {
        used : digit = read(mem_used) ;
        total : digit = read(mem_total) ;
        alert : chars = read(mem_alert) ;
    } ;
} ;
```

**输出**：
```json
{
    "metrics": {
        "host": {
            "name": "prod-web-01",
            "timestamp": "2024-01-15 14:30:00"
        },
        "cpu": {
            "user": 65.5,
            "system": 15.2,
            "alert": "Warning"
        },
        "memory": {
            "used": 6144,
            "total": 8192,
            "alert": "Warning"
        }
    }
}
```

---

---

## 下一步

- **[🌟 完整功能示例](./07-complete-example.md)** - 查看所有 OML 功能的完整演示
- [函数参考](./04-functions-reference.md) - 查阅所有可用函数
- [核心概念](./02-core-concepts.md) - 深入理解 OML 设计
- [集成指南](./05-integration.md) - 将 OML 集成到数据流
- [语法参考](./06-grammar-reference.md) - 查看完整语法定义
