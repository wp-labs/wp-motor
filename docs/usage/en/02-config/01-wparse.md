# Wparse Configuration

Complete Example (Recommended Defaults)
```toml
version = "1.0"
robust  = "normal"           # debug|normal|strict

[models]
wpl     = "./models/wpl"
oml     = "./models/oml"
knowledge = "./models/knowledge"

[topology]
sources = "./topology/sources"
sinks   = "./topology/sinks"

[performance]
rate_limit_rps = 0            # Input rate limit; 0 means auto, >0 means fixed records/second
parse_workers  = 2            # Number of concurrent parsing workers
reload_timeout_ms = 10000     # Reload fallback timeout in milliseconds; shared by graceful drain and old-processing tail cleanup
fetch_timeout_ms = 300 # Blocking source fetch timeout in milliseconds for each picker round

[rescue]
path = "./data/rescue"

[log_conf]
output = "File"               # Console|File|Both
level  = "warn,ctrl=info"

[log_conf.file]
path = "./data/logs"          # File output directory; filename automatically takes executable name (wparse.log)

[stat]

[[stat.pick]]                 # Pickup stage statistics
key    = "pick_stat"
target = "*"

[[stat.parse]]                # Parsing stage statistics
key    = "parse_stat"
target = "*"

[[stat.sink]]                 # Sink stage statistics
key    = "sink_stat"
target = "*"
```

Notes:
- `[models].knowledge` is the root directory for knowledge-related config, defaulting to `./models/knowledge`
- `semantic_dict.toml` is loaded from `${models.knowledge}/semantic_dict.toml` by default
- `knowdb.toml` is loaded from `${models.knowledge}/knowdb.toml` by default
- `rate_limit_rps` defaults to `0`; wparse automatically adjusts source input rate from picker watermarks and parser backpressure
- `reload_timeout_ms` defaults to `10000`; CLI `--reload-timeout-ms` overrides the config value
- `fetch_timeout_ms` defaults to `300`; it controls how long a realtime picker waits for one blocking fetch round

## Memory Profile

Memory-related queues, watermarks, and batch sizes are controlled by `WP_MEMORY_PROFILE`. Most deployments should choose one profile instead of tuning many individual variables:

```bash
WP_MEMORY_PROFILE=low        # Lower memory
WP_MEMORY_PROFILE=standard   # Recommended default; also used when unset
WP_MEMORY_PROFILE=throughput # Wider parser/sink channels for complex samples or fast sinks
```

Profile meanings:

- `low`: applies backpressure earlier and prioritizes lower RSS: `parser/sink channel = 32/16`, `sink_batch_size = 256`, `picker_burst_max = 4`, `tcp_batch = 32KB/32 events`, `pending = 1MB`.
- `standard`: default production profile; keeps the `low` sink, pending, UDP, and file memory watermarks while raising TCP throughput for long-line samples: `parser/sink channel = 48/16`, `sink_batch_size = 256`, `picker_burst_max = 6`, `tcp_recv = 2MB`, `tcp_batch = 256KB/256 events`, `pending = 1MB`.
- `throughput`: gives parser and sink dispatch more channel headroom for heavier samples while still keeping pending bounded.

Historical aliases remain accepted: `small/tiny/xs` map to `low`, `large/high` maps to `throughput`, and `default/normal/balanced` map to `standard`.

Individual environment variables remain available for benchmark and targeted tuning, for example `WP_PARSER_CHANNEL_CAP`, `WP_SINK_CHANNEL_CAP`, `WP_SINK_BATCH_SIZE`, `WP_PICKER_BURST_MAX`, `WP_PICKER_PENDING_MAX_BYTES`, `WP_TCP_RECV_BYTES`, `WP_TCP_BATCH_BYTES`, and `WP_TCP_BATCH_CAPACITY`. Production deployments should prefer a profile so the memory behavior stays explainable.
