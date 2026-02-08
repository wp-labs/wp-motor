# Wparse Configuration

Complete Example (Recommended Defaults)
```toml
version = "1.0"
robust  = "normal"           # debug|normal|strict

[models]
wpl     = "./models/wpl"
oml     = "./models/oml"

[topology]
sources = "./topology/sources"
sinks   = "./topology/sinks"

[performance]
rate_limit_rps = 10000        # Rate limit (records/second)
parse_workers  = 2            # Number of concurrent parsing workers

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
