# Source Basics

This document introduces the fundamental concepts and usage of data source configuration in the Warp Parse system.

## Overview

Sources are the data input endpoints of the Warp Parse system, responsible for receiving data from various sources and feeding it into the processing pipeline.

## Core Concepts

### 1. Connectors

Connectors define how to establish connections and communicate with specific types of data sources. The system includes several built-in connector types:

- **File Connector**: Read data from files
- **Kafka Connector**: Consume data from Kafka message queues
- **Syslog Connector**: Receive Syslog protocol data
- **TCP Connector**: Receive data through TCP sockets

### 2. Connector Definitions

Connector definitions are stored in the `connectors/source.d/` directory:

```toml
# connectors/source.d/kafka_src.toml
[[connectors]]
id = "kafka_src"
type = "kafka"
allow_override = ["topic", "group_id", "config"]

[connectors.params]
brokers = "localhost:9092"
topic = ["access_log"]
group_id = "wparse_default_group"
```

### 3. Source Configuration Structure

```toml
# wpsrc.toml
[[sources]]
key = "main_source"
enable = true
connect = "kafka_src"
tags = ["env:production"]

[[sources.params]]
topic = "events"
# Or use params_override (alias)
```

## Configuration Steps

1. **Define Connector**: Create connector configuration files in the `connectors/source.d/` directory
2. **Configure Source**: Reference the connector in the source configuration file and specify parameters
3. **Validate Configuration**: Use CLI tools to verify configuration correctness
4. **Start System**: Start the Warp Parse system to begin receiving data

## Configuration Field Reference

### Common Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `key` | String | Yes | Unique identifier for the source |
| `enable` | Boolean | No | Whether to enable this source (default: true) |
| `connect` | String | Yes | Referenced connector ID |
| `tags` | Array | No | List of tags for the source |
| `params` | Table | No | Connector parameters (can override connector defaults) |

### Parameter Override Rules

- Only parameters listed in the connector's `allow_override` can be overridden
- Parameters not in the list use the default values from the connector definition

## Configuration Examples

### Basic File Source

```toml
# wpsrc.toml
[[sources]]
key = "file_access_log"
enable = true
connect = "file_src"

[sources.params]
base = "/var/log/nginx"
file = "access.log"
```

### Kafka Source Configuration

```toml
# wpsrc.toml
[[sources]]
key = "kafka_access_logs"
enable = true
connect = "kafka_src"
tags = ["env:production", "type:access_log"]

[sources.params]
topic = "nginx_access_log"
```

### Syslog Source Configuration

```toml
# wpsrc.toml
[[sources]]
key = "syslog_tcp_1"
enable = true
connect = "syslog_tcp_src"
tags = ["protocol:tcp", "env:production"]

[sources.params]
addr = "127.0.0.1"
port = 1514
protocol = "tcp"
```

## Related Documentation

- [Connector Management](../README.md)
- [File Source Configuration](./02-file_source.md)
- [Kafka Source Configuration](./03-kafka_source.md)
- [Syslog Source Configuration](./04-syslog_source.md)
- [TCP Source Configuration](./08-tcp_source.md)
