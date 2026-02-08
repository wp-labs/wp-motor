# Sink Configuration


## Directory and File Organization
- sink_root: Typically `<case>/sink` within use cases
  - business.d/**/*.toml: Business group routes (scenario outputs, supports subdirectories)
  - infra.d/**/*.toml: Infrastructure group routes (default/miss/residue/intercept/error/monitor, supports subdirectories)
  - defaults.toml: Default group-level expectations [defaults.expect]

- connectors/sink.d/*.toml: Connector definitions (loader searches upward from sink_root for the nearest such directory)


## Route File Format
- Top level
  - version (optional)
  - sink_group
    - name: Group name (string)
    - oml / rule: Flat notation recommended; can be string or string array; used for matching models or rules
    - expect: Optional, group-level expectation (overrides defaults)
    - sinks: Array, each item is a single sink definition
- Single sink fields
  - name: Name of this sink (unique within group); falls back to [index] if not provided
  - connect: References connector id (backward compatible with `use`/`connector`)
  - params: Whitelist override of connector default parameters (keys must be in connector allow_override list)
  - expect: Optional, single sink expectation (only ratio/tol/min/max, mutually exclusive: ratio/tol and min/max cannot be mixed)
  - filter: Optional, interception condition file path; when evaluated to true, data is dropped from this sink and sent to intercept

## Configuration Examples:
### Infrastructure Group
```toml
version = "2.0"
[sink_group]
name = "intercept"

[[sink_group.sinks]]
name = "intercept"
connect = "file_kv_sink"
params = { base = "./out", file = "intercept.dat" }
```

### Business Group (filter)
```toml
version = "2.0"

[sink_group]
name = "/sink/filter"
oml  = ["/oml/sh*"]

[[sink_group.sinks]]
name = "all"
connect = "file_kv_sink"
params = { base = "./out/sink", file = "all.dat" }

[[sink_group.sinks]]
name = "safe"
connect = "file_kv_sink"
filter = "./sink/business.d/filter.conf"   # Match -> intercept, not written to safe
params = { base = "./out/sink", file = "safe.dat" }
```


## Notes
- Naming Rules
  - Group name: sink_group.name (e.g., /sink/example/simple)
  - Sink name: name (unique within group; falls back to [0]/[1]/... by index when not explicitly provided)
- Filter Semantics
  - filter is an "interception condition": when the expression evaluates to true, the data is not written to that sink, but forwarded to the infrastructure group intercept (framework/intercept)
  - Each sink can independently set filter; independent of expect




## Validation Tips
- Denominator determination:
  - basis = total_input: Total input
  - basis = group_input: Sum of line counts of each sink in the group (or group input in stats)
  - basis = model: Statistics by model granularity (currently substituted by sum of sink line counts within group)
- min_samples: When denominator is 0 or less than this value, group validation is ignored (prints warning, does not fail)
- When route writes to non-file type fmt, validate will indicate "fmt is determined by backend, ignored".

## Common Troubleshooting
- Connector not found: Check if corresponding id exists in connectors/sink.d; `wproj sinks list` shows reference relationships
- Override parameters not effective: Check allow_override whitelist
- Filter not effective:
  - Path resolution is relative to current working directory (recommend writing relative path from sink_root)
  - Logs will print "found path/not found filter ..."
  - Expression syntax must pass TCondParser; test with simple expressions first
