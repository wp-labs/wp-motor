# VictoriaLogs

VictoriaLogs sink is used to output log data to the [VictoriaLogs](https://docs.victoriametrics.com/victorialogs/) log storage system via HTTP JSON Line interface.

## Connector Definition

```toml
[[connectors]]
id = "victorialog_sink"
type = "victorialogs"
allow_override = ["endpoint", "insert_path", "fmt"]

[connectors.params]
endpoint = "http://localhost:8481"
insert_path = "/insert/json"
fmt = "json"
```

## Available Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `endpoint` | string | `http://localhost:8481` | VictoriaLogs service address (required) |
| `insert_path` | string | `/insert/json` | Data insertion path |
| `create_time_field` | string | - | Custom timestamp field name, extracted from data record |
| `fmt` | string | `json` | Output format: `json`, `csv`, `kv`, `raw`, etc. |

## Data Format

The sink converts each data record to a JSON object for sending, containing the following special fields:

- `_msg`: Formatted message content (formatted according to `fmt` parameter)
- `_time`: Timestamp (nanosecond precision), preferring the field specified by `create_time_field`, otherwise using current time

## Configuration Examples

### Basic Usage

```toml
version = "2.0"

[sink_group]
name = "/sink/victorialogs"
oml  = ["logs"]

[[sink_group.sinks]]
name = "vlogs"
connect = "victorialog_sink"
params = { endpoint = "http://victorialogs:9428" }
```

### Custom Time Field

```toml
[[sink_group.sinks]]
name = "vlogs"
connect = "victorialog_sink"

[sink_group.sinks.params]
endpoint = "http://victorialogs:9428"
insert_path = "/insert/jsonline"
create_time_field = "timestamp"
fmt = "json"
```

## Notes

- The `endpoint` parameter cannot be empty, otherwise validation will fail
- HTTP request timeout is 5 seconds
- If the field specified by `create_time_field` does not exist or is not a time type, current UTC time will be used
