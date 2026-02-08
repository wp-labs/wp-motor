# Sinks 配置指南

本指南介绍如何配置和使用 Warp Parse 系统的各种数据输出（Sink）。

## 支持的 Sink 类型

| 类型 | 说明 | 文档 |
|------|------|------|
| `blackhole` | 黑洞输出（用于测试） | - |
| `file` | 输出到本地文件 | [文件 Sink 配置](./12-file_sink.md) |
| `syslog` | 输出到 Syslog 服务器 (UDP/TCP) | [Syslog Sink 配置](./13-syslog_sink.md) |
| `tcp` | 输出到 TCP 服务端 | [TCP Sink 配置](./15-tcp_sink.md) |
| `kafka` | 输出到 Kafka | [Kafka Sink 配置](./18-kafka_sink.md) |
| `mysql` | 输出到 MySQL | [MySQL Sink 配置](./19-mysql_sink.md) |
| `doris` | 输出到 Doris | [Doris Sink 配置](./17-doris.md) |
| `prometheus` | Prometheus 指标暴露 | [Prometheus Sink 配置](./14-prometheus_sink.md) |
