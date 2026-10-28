# Sinks Configuration Guide

This guide introduces how to configure and use various data outputs (Sinks) in the Warp Parse system.

## Supported Sink Types

| Type | Description | Documentation |
|------|-------------|---------------|
| `blackhole` | Blackhole output (for testing) | - |
| `file` | Output to local file (size-based rolling, gzip/zip archives) | [File Sink Configuration](./12-file_sink.md) |
| `syslog` | Output to Syslog server (UDP/TCP) | [Syslog Sink Configuration](./13-syslog_sink.md) |
| `tcp` | Output to TCP server | [TCP Sink Configuration](./15-tcp_sink.md) |
| `arrow-ipc` | Output as Arrow IPC frames over TCP | [Arrow IPC Sink Configuration](./20-arrow_ipc_sink.md) |
| `arrow-file` | Output Arrow IPC frames to a local file | [Arrow File Sink](./21-arrow_file_sink.md) |
| `kafka` | Output to Kafka | - |
| `prometheus` | Prometheus metrics exposure | [Prometheus Sink Configuration](./14-prometheus_sink.md) |
| `victorialogs` | Output to VictoriaLogs | [VictoriaLogs Sink Configuration](./16-victorialogs.md) |
