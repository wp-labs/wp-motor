# File Sink

This document describes the current configuration and capabilities of File Sinks, aligned with the code implementation.

## Overview

File Sinks write processed data to the local file system, supporting multiple output formats and flexible path configuration. Commonly used for offline validation, archiving, and debugging.

Supported output formats (`fmt`): `json`, `csv`, `kv`, `raw`, `proto`, `proto-text` (default: `json`).

## Connector Definition

It's recommended to use the repository's built-in templates (located in `connectors/sink.d/`):

```toml
# JSON
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base","file"]
[connectors.params]
fmt  = "json"
base = "./data/out_dat"
file = "default.json"

# Prototext
[[connectors]]
id = "file_proto_sink"
type = "file"
allow_override = ["base","file"]
[connectors.params]
fmt  = "proto-text"
base = "./data/out_dat"
file = "default.dat"

# Raw
[[connectors]]
id = "file_raw_sink"
type = "file"
allow_override = ["base","file"]
[connectors.params]
fmt  = "raw"
base = "./data/out_dat"
file = "default.raw"
```


## Available Parameters (Route `params`)

- `base` + `file`: Target directory and filename (recommended approach).
- `fmt`: Output format (see above).

Note: File Sinks automatically create parent directories; internally uses buffered writes with batch flushing, with no manual buffer size/sync mode parameters.

## Configuration Examples

1) Basic JSON Output
```toml
# business.d/json_output.toml
version = "2.0"
[sink_group]
name = "/sink/json_output"
oml  = ["logs"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { base = "/var/log/warpflow", file = "application.json" }
```

2) Error Log Separation (by Filter)
```toml
version = "2.0"
[sink_group]
name = "/sink/error_logs"
oml  = ["application_logs"]

[[sink_group.sinks]]
name = "all"
connect = "file_json_sink"
params = { file = "all.json" }

[[sink_group.sinks]]
name = "err"
connect = "file_json_sink"
filter = "./error_filter.wpl"
params = { file = "err.json" }
```

3) JSON Output with OML Name Metadata
```toml
version = "2.0"
[sink_group]
name = "/sink/netflow"
oml  = ["network.netflow"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "netflow.json" }
```

For records emitted by OML output, the engine passes the OML `name` to sinks as batch-level `BatchMeta.oml_name`. The sink implementation decides how to render it; current core text sinks append `wp_oml_name` for structured JSON/CSV-like output, while Arrow framed sinks write it to the frame header tag.

To disable the OML metadata payload field for text output, configure it at `sink_group` level:

```toml
[sink_group]
name = "/sink/netflow"
oml = ["network.netflow"]
wp_meta_disable = ["wp_oml_name"]
```

`wp_meta_disable` is only valid under `[sink_group]`; it is rejected from sink `params` and `wpgen.output.params`. Currently supported fields: `wp_oml_name`, `wp_event_md5`. This disables the JSON/CSV-like payload field only; it does not disable the Arrow framed frame-header tag.
