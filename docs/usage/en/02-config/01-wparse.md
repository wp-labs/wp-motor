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
rate_limit_rps = 10000        # Rate limit (records/second)
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
- `reload_timeout_ms` defaults to `10000`; CLI `--reload-timeout-ms` overrides the config value
- `fetch_timeout_ms` defaults to `300`; it controls how long a realtime picker waits for one blocking fetch round
