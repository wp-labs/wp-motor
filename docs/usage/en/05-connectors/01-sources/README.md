# Sources Configuration Guide

This guide describes how to configure and use the various data sources in the Warp Parse system.

## Content Overview

- [Source Configuration Basics](./01-sources_basics.md)
- [File Source Configuration](./02-file_source.md)
- [Kafka Source Configuration](./03-kafka_source.md)
- [Syslog Source Configuration](./04-syslog_source.md)
- [TCP Source Configuration](./08-tcp_source.md)
- [DataRecord Mechanism Data Fields](./09-metadata.md)

## Quick Start

1. Learn the [Source Configuration Basics](./01-sources_basics.md)
2. Select the appropriate configuration guide based on your data source type
3. Refer to connector management documentation for connector definitions

## Supported Data Source Types

| Type | Description | Documentation |
|------|-------------|---------------|
| `file` | Read data from local files | [File Source Configuration](./02-file_source.md) |
| `kafka` | Consume messages from Kafka | [Kafka Source Configuration](./03-kafka_source.md) |
| `syslog` | Receive Syslog protocol data (UDP/TCP) | [Syslog Source Configuration](./04-syslog_source.md) |
| `tcp` | Receive data via TCP | [TCP Source Configuration](./08-tcp_source.md) |

## Related Documentation

- [Connector Management](../README.md)
- [Sinks Configuration Guide](../02-sinks/README.md)
