# Syslog Sink

This document is aligned with the code implementation and describes the actual available parameters and examples for Syslog Sink.

## Overview

Syslog Sink sends data to Syslog servers in RFC3164 text format, supporting UDP and TCP. Message format: `<PRI>TIMESTAMP HOSTNAME APP_NAME: MESSAGE`.

## Connector Definition

Use the repository's built-in templates (`connectors/sink.d/10-syslog-udp.toml`, `11-syslog-tcp.toml`):

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

## Available Parameters (Route `params`)

- `addr`: Syslog server address (IP or hostname).
- `port`: Port number (1–65535).
- `protocol`: `udp` or `tcp` (case-insensitive).
- `app_name`: Application name, defaults to current process name (e.g., `wp-engine`).

## Configuration Examples

1) Basic UDP Output
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


2) Conditional Routing
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
