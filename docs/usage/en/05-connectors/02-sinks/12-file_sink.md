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
- `sync`: fsync after every write (default `false`).
- `max_size`: Max bytes per file (integer, or a size string like `10MB` / `1GiB`); `0` (default) disables rolling.
- `max_rolls`: Number of rotated files to keep, excluding the active file (default `3`; `0` drops old files on roll).
- `roll_compress`: Compress rotated files: `none` (default), `gzip`, or `zip` (`true` is an alias for `gzip`).

When `max_size` is set, the sink checks the current file size before each write: if the
incoming bytes would exceed the limit, it rotates first, then writes. A single record is
never split, so one file may slightly exceed the limit.

- Rotated files are numbered: `default.json` (active) → `default.json.1` (newest) → `default.json.2` → `default.json.3`;
- With `roll_compress`, rotated files become `default.json.1.gz` (`gunzip`-able) or `default.json.1.zip` (a standard zip with a single entry named after the active file);
- Only `max_rolls` newest rotated files are kept; older ones are deleted;
- The limit counts bytes written to disk (after `compression` / `encryption`); compression emits 64KiB blocks, so rolling granularity is coarse;
- On rotation the current file is flushed (`sync_all`) and the codec stream is rebuilt, so every rotated file can be decompressed/decrypted independently;
- Archive compression runs on a blocking thread; if it fails the sink falls back to an uncompressed archive, so no data is lost;
- `max_rolls` / `roll_compress` must be used together with `max_size > 0`, otherwise it is a config error;
- When resuming an existing file, the size starts from its current length;
- Rolling is triggered only by writes (size threshold); shutdown does not force an extra roll, so the last batch stays in the active file;
- `wparse stat sink-file` / `validate sink-file` count only the active file, not rotated archives;
- Rolling is only supported for `protocol = "txt"` (default); `protocol = "arrow"` with `max_size` / `max_rolls` / `roll_compress` is rejected.

The connector must whitelist the new params:

```toml
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "max_size", "max_rolls", "roll_compress"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.json"
max_size = 0
roll_compress = "none"
```

## Configuration Examples

0) Rolling file output (rotated files zipped)
```toml
[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "app.json", max_size = "10MB", max_rolls = 3, roll_compress = "zip" }
```

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
