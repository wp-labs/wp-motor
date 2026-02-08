# Prometheus Sink

> **Note**: Prometheus Sink is currently **not implemented**. This document is a design document for the planned feature; please refer to the actual code implementation for current usage.

This document is aligned with the code implementation. Currently, the Prometheus Sink is a "self-exposing" Exporter that starts a local HTTP service (currently only supports Counter-type metrics) and exposes `/metrics` externally.

## Connector Definition

Use the repository template (`connectors/sink.d/40-prometheus.toml`):

```toml
[[connectors]]
id = "prometheus_sink"
type = "prometheus"
allow_override = ["endpoint", "source_key_format", "sink_key_format"]
[connectors.params]
endpoint = "127.0.0.1:35666"   # Listen address (exposes /metrics)
source_key_format = "(?P<source_type>.)_(?P<access_source>.)"
sink_key_format   = "(?P<rule>.)_(?P<sink_type>.)_sink"
```

Note: Pushgateway/custom `metric_name`/`metric_type`/`labels` are not supported. Built-in metric names are fixed:
- `wparse_receive_data` (records received from data sources, with source labels)
- `wparse_parse_success`, `wparse_parse_all` (parse success/total)
- `wparse_send_to_sink` (records sent to sink, with sink labels)

## Available Parameters (Route `params`)

- `endpoint`: Exporter listen `host:port` (e.g., `127.0.0.1:35666`).
- `source_key_format`: Optional, regex for extracting `source_type/access_source` from key (named groups).
- `sink_key_format`: Optional, regex for extracting `rule/sink_type` from key (named groups).

## Configuration Examples

Start Exporter and expose metrics:
```toml
version = "2.0"
[sink_group]
name = "/sink/prom_exporter"
oml  = ["metrics"]

[[sink_group.sinks]]
name = "prom"
connect = "prometheus_sink"
params = { endpoint = "0.0.0.0:35666" }
```

Verification: `curl http://127.0.0.1:35666/metrics`
