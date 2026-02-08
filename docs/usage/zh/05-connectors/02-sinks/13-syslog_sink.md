# Syslog Sink 

本文档与代码实现对齐，描述 Syslog Sink 的实际可用参数与示例。

## 概述

Syslog Sink 将数据以 RFC3164 文本格式发送到 Syslog 服务器，支持 UDP 与 TCP。消息格式：`<PRI>TIMESTAMP HOSTNAME APP_NAME: MESSAGE`。

## 连接器定义

使用仓库内置模板（`connectors/sink.d/10-syslog-udp.toml`、`11-syslog-tcp.toml`）：

```toml
[[connectors]]
id = "syslog_udp_sink"
type = "syslog"
allow_override = ["addr", "port", "protocol", "app_name"]
[connectors.params]
addr = "127.0.0.1"
port = 1514
protocol = "udp"

[[connectors]]
id = "syslog_tcp_sink"
type = "syslog"
allow_override = ["addr", "port", "protocol", "app_name"]
[connectors.params]
addr = "127.0.0.1"
port = 1514
protocol = "tcp"
```

## 可用参数（路由 `params`）

- `addr`：Syslog 服务器地址（IP 或主机名）。
- `port`：端口（1–65535）。
- `protocol`：`udp` 或 `tcp`（大小写不敏感）。
- `app_name`：应用名称，默认为当前进程名（如 `wp-engine`）。

## 配置示例

1) 基础 UDP 输出
```toml
version = "2.0"
[sink_group]
name = "/sink/syslog_basic"
oml  = ["application_logs"]

[[sink_group.sinks]]
name = "syslog_output"
connect = "syslog_udp_sink"
params = { addr = "syslog.example.com", port = 1514, protocol = "udp" }
```


2) 按条件分流
```toml
version = "2.0"
[sink_group]
name = "/sink/syslog_by_cond"

[[sink_group.sinks]]
name = "error"
connect = "syslog_tcp_sink"
filter = "./error_filter.wpl"
params = { addr = "syslog-errors.example.com", port = 514, protocol = "tcp" }

[[sink_group.sinks]]
name = "info"
connect = "syslog_udp_sink"
filter = "./info_filter.wpl"
params = { addr = "syslog-info.example.com", port = 1514, protocol = "udp" }
```
