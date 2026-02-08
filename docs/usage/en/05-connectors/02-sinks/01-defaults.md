# Defaults

Objective
- Configure expectations (ratio/threshold/window) and understand denominator basis and ignore rules; validate in CLI.

Core Concepts (authoritative reference: `../02-config/04-sinks_config.md`)
- defaults: Default expectations for groups, defined in `[defaults.expect]` within `$SINK_ROOT/defaults.toml`; can be overridden by group/individual sink.
- group expect: `[sink_group].expect`; takes priority over defaults.
- sink expect: `[[sink_group.sinks]].expect`; only supports local fields (ratio/tol or min/max, mutually exclusive).
- Denominator basis: `total_input | group_input | model` (see authoritative documentation for specific definitions).

Example (defaults)
```toml
version = "2.0"
[defaults]
tags = ["env:dev"]
[defaults.expect]
basis = "group_input"
min_samples = 100
mode = "warn"
```

Example (group-level and individual sink)
```toml
[sink_group]
name = "/sink/demo"
[sink_group.expect]
basis = "group_input"
mode  = "fail"

[[sink_group.sinks]]
name = "ff"
connect = "file_json_sink"
[sink_group.sinks.expect]
ratio = 0.98   # Mutually exclusive with min/max
tol   = 0.01
```
