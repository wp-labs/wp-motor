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
    - wp_meta_disable: Optional group-level metadata payload output disable list; currently supports `["wp_oml_name", "wp_event_md5"]`
    - expect: Optional, group-level expectation (overrides defaults)
    - sinks: Array, each item is a single sink definition
- Single sink fields
  - name: Name of this sink (unique within group); falls back to [index] if not provided
  - connect: References connector id (backward compatible with `use`/`connector`)
  - params: Whitelist override of connector default parameters (keys must be in connector allow_override list)
  - expect: Optional, single sink expectation (only ratio/tol/min/max, mutually exclusive: ratio/tol and min/max cannot be mixed)
  - filter: Optional, interception condition file path; when evaluated to true, data is dropped from this sink and sent to intercept
  - drop_when_error: Optional; drop the record/batch when a send fails. Defaults to `false` (write a rescue file, historical behavior)

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


## Send Failure Handling (drop_when_error)

`drop_when_error` controls what wp-motor does after a sink send failure (disconnect, downstream 5xx, timeout, 401/404, ...).
It is configured per sink and defaults to `false`, which keeps the historical behavior.

| Value | Behavior |
| --- | --- |
| `false` (default) | Write a rescue file `data/rescue/<sink>/<sink>-<ts>-<seq>.dat` and hand the old sink to the maintenance loop for reconnect; one file is reused for the whole outage |
| `true` | Count the drop, remember the last error, discard the failed record/batch, and move on. **No rescue file, no bad-sink swap**; later sinks in the same group keep receiving data |

### Circuit breaker: do not retry a broken sink on every batch

With `drop_when_error = true` the first send failure opens a per-sink circuit breaker:

- while open, the connector is **not called at all**; incoming records are dropped and counted, so a broken link no longer pays a connect timeout or retry backoff on every batch;
- cooldown starts at 5s and doubles on consecutive failures, capped at 60s;
- once the cooldown expires exactly one batch is let through as a probe: success closes the breaker, resumes delivery, and logs a recovery summary; failure reopens it with a longer cooldown.

| State | Behavior | Cost |
| --- | --- | --- |
| Closed | normal delivery | one send |
| Open | skip the connector, drop and count | one counter update |
| Half-open | let one probe batch through | one batch |

The breaker only bypasses the wp-motor layer. The connector's own retry still applies: `HttpSink` retries 5xx/network errors up to `params.max_retries` (default 3, backoff 1s→2s→4s).
For high-rate channels, pair it with a smaller retry budget:

```toml
[[sink_group.sinks]]
name = "atlas"
connect = "http_sink"
drop_when_error = true

[sink_group.sinks.params]
max_retries = 0
```

For batched sends a failed batch is dropped as a whole (the drop counter increases by the number of records in the batch); choose `batch_size` according to how much data you can afford to lose at once.

### Observability

- Logs: the first failure logs a WARN immediately, then at most one WARN per 60 seconds. Breaker skips are warned with the same throttle and distinguished from real failures (`failed` vs `breaker open`). Once delivery recovers, one summary line reports the total dropped during the outage.
- Stats: each monitor window records `sink_dropped_total` per sink name; `sum stat` shows the cumulative drop count since process start.
- Monitor log: one line per window: `sink '<name>' dropped: window=... total=... last_error_at=... last_error=...` with the error text truncated to 200 characters.
- Dropped records are still counted as handled in `sink_stat`; use `sink_dropped_total` to see whether data actually landed.

### Boundaries

- It covers **all send failures**, including configuration errors such as 401/404; verify the endpoint before enabling it.
- It is unrelated to `filter` (intercept) and `expect` (validation), which have their own semantics.
- Records discarded while `sink.is_ready() == false` are outside this switch.
- Rescue files are not replayed automatically (only `wp rescue stat` exists), so `false` means "persisted locally on failure", not guaranteed delivery.

### Rescue mode (`drop_when_error = false`)

- **One outage produces exactly one rescue file**: after the maintenance loop reconnects and swaps the real sink back, the rescue file stays open; if the next send fails again the same file is appended to instead of creating a new one.
- It is finalized only after a real delivery succeeds: flush and rename `.dat.lock` to `.dat`, logging `rescue file finalized` with the path and record count.
- Observability: at most one WARN per 60s while rescuing: `on rescue: file=... records=... size=...B`.
  `records` counts what was written during this outage; `size` is the on-disk byte count, which may lag because of the `BufWriter` (flushed every 100 records or on stop).
  Note that `wp rescue stat` only counts finalized `.dat` files, so an in-progress `.lock` is invisible there; use this log line during an outage.
- There is still no automatic replay: rescue files must be handled by an external tool or operator.
- **Rescue mode does not throttle attempts**: after a successful reconnect the real sink is swapped back and every following batch tries it again; use `drop_when_error = true` or split the sink into its own `sink_group` if you need pacing.

### Relation to splitting groups

The breaker avoids per-batch retries against a broken link, but the first failure and each probe still spend one connector retry budget.
If a single failure is very slow, keep the failing sink in its own `sink_group` as well.

## Notes
- Naming Rules
  - Group name: sink_group.name (e.g., /sink/example/simple)
  - Sink name: name (unique within group; falls back to [0]/[1]/... by index when not explicitly provided)
- Metadata Output Control (wp_meta_disable)
  - `wp_meta_disable` is valid only under `[sink_group]` and applies to all sinks in that group.
  - Currently supported fields: `wp_oml_name`, `wp_event_md5`; it disables the OML name field in structured text payloads such as JSON/CSV.
  - It does not disable the Arrow framed frame-header tag; Arrow tags still receive the OML `name` through sink batch metadata.
  - Do not put `wp_meta_disable` under `[[sink_group.sinks]].params` or `wpgen.output.params`; those locations are rejected so runtime control fields do not leak into connector params.
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
