# TCP Sink

TCP sink is used to output data to any TCP server, supporting line-based or length-prefixed (RFC6587 style) framing.

## Connector Definition
```toml
[[connectors]]
id = "tcp_sink"
type = "tcp"
allow_override = ["addr", "port", "framing"]

[connectors.params]
addr = "127.0.0.1"
port = 9000
framing = "line"   # line|len
```

## Available Parameters

- `addr`: Target server address (IP or hostname).
- `port`: Target port (1–65535), default 9000.
- `framing`: Framing mode, `line` or `len`, default `line`.

## Usage Example (wpgen Output to TCP)
```toml
# conf/wpgen.toml
[generator]
mode = "sample"
count = 10000

[output]
connect = "tcp_sink"

[output.params]
addr = "127.0.0.1"
port = 9000
framing = "line"
```

## Framing Description
- `line`: Appends `\n` as message delimiter
- `len`: Sends `<len><space><payload>` (no `\n` appended)
