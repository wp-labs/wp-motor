# TCP Source Configuration

This document describes how to use the generic TCP source (kind=`tcp`), framing modes, and examples of integration with TCP Sink.

## Feature Overview

- Supports three framing modes:
  - `line`: Frame by newline; trailing CR/space/Tab are stripped
  - `len`: Length prefix (RFC 6587 octet-counting): `<len><SP><payload>`
  - `auto` (default): Auto-select; defaults to `len` priority, uses line priority when `prefer_newline=true`

## Connector Definition (source.d)

```toml
# connectors/source.d/12-tcp.toml
[[connectors]]
id = "tcp_src"
type = "tcp"
# Keys allowed to override, compatible with common syslog naming
allow_override = ["addr", "port", "framing", "tcp_recv_bytes", "instances"]

[connectors.params]
addr = "0.0.0.0"
port = 9000
framing = "auto"          # auto|line|len
tcp_recv_bytes = 256000   # 256KB
# instances = 1             # Optional: parallel instances, default 1, max 16
```

## Source Configuration (wpsrc.toml)

```toml
[[sources]]
key = "tcp_in"
connect = "tcp_src"
enable = true
tags = ["source:tcp", "type:raw"]

[[sources.params]]
port = 19000
framing = "auto"
instances = 2
```


## Framing Mode Details

### line (newline)
- Use case: Text logs, manual/script pushing, simple tool (nc/tail) pipelines
- Trailing `\r`/space/Tab are stripped; senders should end each line with `\n`

### len (length prefix)
- Format: `5 hello` → indicates next payload length is 5 bytes (not including the space in the prefix)
- Use case: Scenarios where payload may contain newlines/binary (e.g., multi-line logs, stack traces, compressed fragments)
- Receiver constraints: Maximum length 10MB, prefix max 10 decimal digits, discards current attempt on error to avoid memory bloat

### auto (automatic)
- Default tries `len` first, falls back to line on parse failure
- If "length prefix in progress" is detected (read `<digits><SP>` but payload incomplete), continues waiting instead of falling back to line to avoid mis-splitting

## Integration with TCP Sink (Loopback Pipeline)

For end-to-end debugging convenience, this project provides a generic TCP Sink (kind=`tcp`):
- sink connectors: `connectors/sink.d/12-tcp.toml`
- sink parameters: `addr`/`port`/`framing(line|len)`
- Example: `wpgen` outputs to `tcp_sink`, `wparse` listens on same port via `tcp_src`, achieving local loopback

### Example wpgen (conf/wpgen.toml)

```toml
[generator]
mode = "sample"
count = 10000

[output]
connect = "tcp_sink"

[output.params]
addr = "127.0.0.1"
port = 19000
framing = "line"  # or "len"
```

## Complete Use Case (usecase/core/tcp_roundtrip)

Directory: `usecase/core/tcp_roundtrip`
- Start: `./case_verify.sh`
- Steps: Start wparse (tcp source) → wpgen push (tcp sink) → verify file output

## FAQ

- Q: Will text starting with "number+space" be misidentified as length prefix?
  - A: In `auto` mode, this is possible; use `framing="line"` to avoid
- Q: Why is `len` recommended for production?
  - A: Clear boundaries, more robust for binary/multi-line; many syslog/TCP production pipelines recommend/default to octet-counting

## Best Practices

- Text only: `framing="line"`
- Multi-line/binary: `framing="len"` (or `auto` default)
- Quick debugging: `framing="line"`, paired with `nc -lk <port>`

## Related Documentation

- [Source Configuration Basics](./01-sources_basics.md)
- [TCP Sink Configuration](../02-sinks/15-tcp_sink.md)
