# Sources Configuration

## Overview

Source is the component responsible for data input in the warp-parse system, supporting multiple data sources and protocols. It adopts a unified connector architecture to provide flexible data access capabilities.

### Location and Directories
- **Configuration file**: `$WORK_ROOT/topology/sources/wpsrc.toml`
- **Connector definitions**: Lookup starts from `$WORK_ROOT/models/sources` and searches upward for the nearest `connectors/source.d/*.toml`

### Core Concepts
- **Connector**: Reusable input connection definition, containing `id/type/params/allow_override`
- **Parameter Override**: Safe parameter overriding through whitelist mechanism
- **Tag System**: Supports adding tags to data sources for routing and filtering

## Supported Source Types

### Built-in Sources
- **file**: File input, supports monitoring and polling
- **syslog**: Syslog protocol input (UDP/TCP)
- **tcp**: Protocol input

### Extension Sources
- **kafka**: Apache Kafka message queue input

## Configuration Rules

### Basic Rules
- Only supports `[[sources]] + connect/params` format
- Override keys must be in connector `allow_override` whitelist; exceeding causes error
- `enable` field controls whether to enable (default true)
- `tags` field supports adding data source tags

### Configuration Structure
```toml
[[sources]]
key = "source_identifier"           # Unique identifier for the source
connect = "connector_id"            # Referenced connector ID
enable = true                       # Whether to enable (optional, default true)
tags = ["source:tag1", "type:log"]  # Tags (optional)
params = {                 # Parameter override (optional)
    # Override connector parameters
}
```

## Configuration Examples

### Minimal Example
```toml
[[sources]]
key = "file_1"
connect = "file_src"
params = { base = "data/in_dat", file = "gen.dat" }
```

### File Input Example
```toml
# models/sources/wpsrc.toml
[[sources]]
key = "access_log"
connect = "file_src"
params = {
    base = "./logs",
    file = "access.log",
    encode = "text"
}
tags = ["type:access", "env:prod"]
```

### Syslog Input Example
```toml

# models/sources/wpsrc.toml
[[sources]]
key = "syslog_udp"
connect = "syslog_udp_src"
params = {
    port = 1514,
    header_mode = "parse",
    prefer_newline = true
}
tags = ["protocol:syslog", "transport:udp"]
```



### TCP Input Example (Generic TCP Line/Length Framing)
```toml

# models/sources/wpsrc.toml
[[sources]]
key = "tcp_in"
connect = "tcp_src"
enable = true
params= {
  port = 19000,
  framing = "auto",
  prefer_newline = true
}
```
