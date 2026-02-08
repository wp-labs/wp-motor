# File Source

This document provides detailed instructions on configuring and using file data sources in the Warp Parse system.

## Overview

File sources are used to read data from the local file system, supporting multiple encoding formats and flexible path configuration.

## Connector Definition

### Basic File Connector

```toml
# connectors/source.d/00-file-default.toml
[[connectors]]
id = "file_src"
type = "file"
allow_override = ["base", "file", "encode"]

[connectors.params]
base = "data/in_dat"
file = "gen.dat"
encode = "text"
```

## Supported Parameters

### Path Configuration


#### base + file Combination (Recommended)
```toml
[[sources]]
key = "file_composed"
connect = "file_src"

[[sources.params]]
base = "/var/log"
file = "access.log"
```

#### instances (Multi-instance Parallel Reading)
```toml
[[sources.params]]
instances = 4   # 1-32, default is 1
```
> When `instances > 1`, the file is split into multiple ranges at line boundaries, read in parallel by multiple instances.
>
> **Multi-instance Naming Convention**: When `instances > 1`, each instance's key automatically gets a suffix, such as `file_src-1`, `file_src-2`, etc.

### Encoding Formats

#### text Encoding (Default)
```toml
[[sources.params]]
encode = "text"
```

## Configuration Examples

### Basic File Reading
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "access_log"
connect = "file_src"

[[sources.params]]
base = "/var/log/nginx"
file = "access.log"
```

### Multiple File Sources Configuration
```toml
# wpsrc.toml
[[sources]]
enable = true
key = "nginx_access"
connect = "file_src"

[[sources.params]]
base = "/var/log/nginx"
file = "access.log"

[[sources]]
enable = true
key = "nginx_error"
connect = "file_src"

[[sources.params]]
base = "/var/log/nginx"
file = "error.log"
```

### Different Encoding Formats
```toml
# Base64 encoded file
[[sources]]
key = "base64_data"
connect = "file_src"

[[sources.params]]
base = "./data"
file = "encoded.b64"
encode = "base64"

# Hexadecimal encoded file
[[sources]]
key = "hex_data"
connect = "file_src"

[[sources.params]]
base = "./data"
file = "encoded.hex"
encode = "hex"
```


## Data Processing Features

### 1. Line-by-Line Reading
File sources use line-by-line reading mode, where each line is processed as an independent data packet.

### 2. Encoding Processing
- **text**: Read text content directly


Example:
```json
{
  "data": "Original log content",
  "tags": {
    "access_source": "/var/log/nginx/access.log",
    "env": "production"
  }
}
```

## Related Documentation

- [Source Configuration Basics](./01-sources_basics.md)
- [Connector Management](../README.md)
