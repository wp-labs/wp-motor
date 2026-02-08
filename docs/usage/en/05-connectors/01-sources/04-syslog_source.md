# Syslog Source Configuration

This document describes in detail how to configure and use the Syslog data source in the Warp Parse system.

## Overview

Syslog source is used to receive and parse standard Syslog protocol messages, supporting both UDP and TCP transport protocols, as well as multiple Syslog formats.

## Connector Definition

### UDP Syslog Connector

```toml
# connectors/source.d/10-syslog-udp.toml
[[connectors]]
id = "syslog_udp_src"
type = "syslog"
allow_override = ["addr", "port", "protocol", "tcp_recv_bytes", "header_mode", "fast_strip"]

[connectors.params]
addr = "0.0.0.0"
port = 1514
protocol = "udp"
header_mode = "strip"
tcp_recv_bytes = 256000
```

### TCP Syslog Connector

```toml
# connectors/source.d/11-syslog-tcp.toml
[[connectors]]
id = "syslog_tcp_src"
type = "syslog"
allow_override = ["addr", "port", "protocol", "tcp_recv_bytes", "header_mode", "fast_strip"]

[connectors.params]
addr = "127.0.0.1"
port = 1514
protocol = "tcp"
header_mode = "strip"
tcp_recv_bytes = 256000
```

## Supported Parameters

### Basic Network Parameters

#### addr (required)
Listen address

```toml
[[sources.params]]
addr = "0.0.0.0"    # Listen on all interfaces
addr = "127.0.0.1"   # Local interface only
addr = "10.0.0.100"  # Specific interface
```

#### port (required)
Listen port

```toml
[[sources.params]]
port = 514           # Standard syslog port (requires root privileges)
```

#### protocol (required)
Transport protocol

```toml
[[sources.params]]
protocol = "tcp"     # TCP protocol (reliable transport)
```

### Message Processing Parameters

#### header_mode
Header processing mode

```toml
[[sources.params]]
header_mode = "strip"   # Strip header only, don't inject tags
header_mode = "parse"   # Parse + inject tags + strip header (default)
header_mode = "keep"    # Keep header, pass through as-is
```

#### fast_strip
Fast strip mode (performance optimization)

```toml
[[sources.params]]
fast_strip = true   # Enable fast stripping (better performance)
```

### TCP-Specific Parameters

#### tcp_recv_bytes
TCP receive buffer size

```toml
[[sources.params]]
tcp_recv_bytes = 256000      # 256KB (default)
tcp_recv_bytes = 10485760    # 10MB
tcp_recv_bytes = 104857600   # 100MB (high performance)
```

## Configuration Examples

### Basic UDP Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "syslog_udp_1"
connect = "syslog_udp_src"
tags = ["protocol:udp", "env:production"]

[[sources.params]]
addr = "0.0.0.0"
port = 1514
protocol = "udp"
```

### Basic TCP Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "syslog_tcp_1"
connect = "syslog_tcp_src"
tags = ["protocol:tcp", "env:production"]

[[sources.params]]
addr = "127.0.0.1"
port = 1514
protocol = "tcp"
```

### Dual Protocol Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "syslog_udp_collector"
connect = "syslog_udp_src"

[[sources.params]]
addr = "0.0.0.0"
port = 1514
protocol = "udp"
header_mode = "strip"

[[sources]]
enable = true
key = "syslog_tcp_aggregator"
connect = "syslog_tcp_src"

[[sources.params]]
addr = "127.0.0.1"
port = 1515
protocol = "tcp"
header_mode = "parse"
tcp_recv_bytes = 1048576
```

## Data Processing Features

### 1. Syslog Format Support

#### RFC3164 Format (Traditional BSD Syslog)
```
<34>Oct 11 22:14:15 mymachine su: 'su root' failed for lonvick on /dev/pts/8
```

#### RFC5424 Format (Modern Syslog)
```
<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 iut="3" eventSource="Application" eventID="1011"] BOMAn application event log entry
```

### 2. Parsed Fields

When `header_mode = "parse"`, the system parses and adds the following tags:

```json
{
  "data": "Original message content",
  "tags": {
    "source_type": "syslog",
    "syslog_priority": 34,        // Numeric priority
    "syslog_facility": 4,         // Facility code
    "syslog_severity": 2,         // Severity level
    "syslog_hostname": "mymachine",
    "syslog_app_name": "su",
    "syslog_proc_id": "1234",     // Process ID (RFC5424)
    "syslog_msg_id": "ID47",      // Message ID (RFC5424)
    "syslog_timestamp": "Oct 11 22:14:15"
  }
}
```

### 3. Framing/Header Processing Optimization
```toml
# High performance scenarios:
header_mode = "strip"         # Strip header only, reduce parsing and tag injection
fast_strip = true             # Enable fast stripping

# Analysis scenarios:
header_mode = "parse"         # Parse and inject protocol-related metadata
```

## Related Documentation

- [Source Configuration Basics](./01-sources_basics.md)
- [Connector Management](../README.md)
