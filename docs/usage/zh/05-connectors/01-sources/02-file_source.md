# File Source

本文档详细介绍如何配置和使用 Warp Parse 系统的文件数据源。

## 概述

文件源用于从本地文件系统读取数据，支持多种编码格式和灵活的路径配置。

## 连接器定义

### 基础文件连接器

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

## 支持的参数

### 路径配置


#### base + file 组合（推荐）
```toml
[[sources]]
key = "file_composed"
connect = "file_src"

[[sources.params]]
base = "/var/log"
file = "access.log"
```

#### instances（多实例并行读取）
```toml
[[sources.params]]
instances = 4   # 1-32，默认为 1
```
> 当 `instances > 1` 时，文件会被按行边界分割为多个范围，由多个实例并行读取。
>
> **多实例命名规则**：当 `instances > 1` 时，每个实例的 key 会自动添加后缀，如 `file_src-1`、`file_src-2` 等。

### 编码格式

#### text 编码（默认）
```toml
[[sources.params]]
encode = "text"
```

## 配置示例

### 基础文件读取
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

### 多文件源配置
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

### 不同编码格式
```toml
# Base64 编码的文件
[[sources]]
key = "base64_data"
connect = "file_src"

[[sources.params]]
base = "./data"
file = "encoded.b64"
encode = "base64"

# 十六进制编码的文件
[[sources]]
key = "hex_data"
connect = "file_src"

[[sources.params]]
base = "./data"
file = "encoded.hex"
encode = "hex"
```


## 数据处理特性

### 1. 逐行读取
文件源采用逐行读取模式，每行作为独立的数据包处理。

### 2. 编码处理
- **text**: 直接读取文本内容


示例：
```json
{
  "data": "原始日志内容",
  "tags": {
    "access_source": "/var/log/nginx/access.log",
    "env": "production"
  }
}
```

## 相关文档

- [源配置基础](./01-sources_basics.md)
- [连接器管理](../README.md)
