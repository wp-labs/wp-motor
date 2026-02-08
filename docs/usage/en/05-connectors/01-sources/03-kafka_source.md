# Kafka Source Configuration

> **⚠️ Note**: Kafka source is currently **not implemented**. This document is a design document for planned functionality; please refer to the actual code implementation when using.

This document describes in detail how to configure and use the Kafka data source in the Warp Parse system.

## Overview

Kafka source is used to consume data from Apache Kafka message queues, supporting consumption from a single topic with flexible configuration options.

> **Note**: The system will automatically create configured topics (if they don't exist). The consumer group ID can be configured via the `group_id` parameter, defaulting to `wparse_default_group`.

## Connector Definition

### Basic Kafka Connector

```toml
# connectors/source.d/30-kafka.toml
[[connectors]]
id = "kafka_src"
type = "kafka"
allow_override = ["topic", "group_id", "config"]

[connectors.params]
brokers = "localhost:9092"
topic = ["access_log"]
group_id = "wparse_default_group"
```

## Supported Parameters

### Basic Connection Parameters

#### brokers (required)
Kafka cluster address, supports string format

```toml
[[sources.params]]
brokers = "localhost:9092"
```

#### topic (required)
Topic name to consume (array format)

```toml
[[sources.params]]
topic = ["access_log"]
```

#### group_id (optional)
Consumer group ID

```toml
[[sources.params]]
group_id = "my_consumer_group"
```

### Security Configuration

All security-related parameters must be configured via the `config` array in `key=value` string format.

#### SSL/TLS Configuration
```toml
[[sources.params]]
config = [
    "security_protocol=SSL",
    "ssl_ca_location=/path/to/ca.pem",
    "ssl_certificate_location=/path/to/client.pem",
    "ssl_key_location=/path/to/client.key",
    "ssl_key_password=key_password"
]
```

#### SASL Authentication
```toml
[[sources.params]]
config = [
    "security_protocol=SASL_PLAINTEXT",
    "sasl_mechanisms=PLAIN",
    "sasl_username=consumer_user",
    "sasl_password=consumer_pass"
]
```

#### SASL/SCRAM Authentication
```toml
[[sources.params]]
config = [
    "security_protocol=SASL_SSL",
    "sasl_mechanisms=SCRAM-SHA-256",
    "sasl_username=consumer_user",
    "sasl_password=consumer_pass"
]
```

### Advanced Configuration

#### Consumption Strategy
```toml
[[sources.params]]
config = [
    "auto_offset_reset=earliest",
    "enable_auto_commit=false",
    "auto_commit_interval_ms=5000"
]
```

#### Session and Heartbeat Configuration
```toml
[[sources.params]]
config = [
    "session_timeout_ms=30000",
    "heartbeat_interval_ms=3000",
    "max_poll_interval_ms=300000"
]
```

#### Batch Consumption Configuration
```toml
[[sources.params]]
config = [
    "max_poll_records=500",
    "fetch_min_bytes=1",
    "fetch_max_wait_ms=500"
]
```

## Configuration Examples

### Basic Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "kafka_access_logs"
connect = "kafka_src"

[[sources.params]]
topic = ["nginx_access_log"]
```

### Advanced Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "kafka_advanced"
connect = "kafka_src"

[[sources.params]]
topic = ["access_log"]
config = [
    "auto_offset_reset=earliest",
    "enable_auto_commit=false"
]
```

### Secure Cluster Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "kafka_secure_logs"
connect = "kafka_src"
tags = ["env:production", "security:tls"]

[[sources.params]]
topic = ["secure_events"]
config = [
    "auto_offset_reset=latest",
    "enable_auto_commit=true",
    "auto_commit_interval_ms=1000"
]
```


## Data Processing Features

### 1. Message Structure
Each Kafka message is converted to a data packet containing:
- **Message body**: The actual content of the message (payload)


Example:
```json
{
  "data": "Original message content",
  "tags": {
    "access_source": "access_log",
    "env": "production",
    "type": "access_log"
  }
}
```

### 3. Consumption Semantics
- **Consumer group ID**: Configured via `group_id` parameter
- **Topic auto-creation**: Topics are automatically created when they don't exist (1 partition, replication factor 1)
- **Offset commit**: Handled by underlying rdkafka library, configurable via config parameters

## Performance Optimization

### 1. Batch Consumption
```toml
[[sources.params]]
config = [
    "max_poll_records=1000",
    "fetch_min_bytes=1024",
    "fetch_max_wait_ms=100"
]
```

### 2. Connection Optimization
```toml
[[sources.params]]
config = [
    "session_timeout_ms=60000",
    "heartbeat_interval_ms=5000",
    "max_poll_interval_ms=600000"
]
```

## Related Documentation

- [Source Configuration Basics](./01-sources_basics.md)
- [Connector Management](../README.md)
