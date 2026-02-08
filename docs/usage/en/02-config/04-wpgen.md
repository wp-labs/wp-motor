# Wpgen Configuration

wpgen is a data generation tool for generating test data based on rules or samples.

## Basic Configuration

Configuration file path: `conf/wpgen.toml`

```toml
version = "1.0"

[generator]
mode = "sample"          # Generation mode: rule | sample
count = 1000             # Total number of records to generate (optional)
duration_secs = 60       # Generation duration in seconds (optional, mutually exclusive with count)
speed = 1000             # Constant rate (rows/sec), 0 for unlimited
parallel = 1             # Parallelism
rule_root = "./rules"    # Rules directory (used when mode=rule)
sample_pattern = "*.txt" # Sample file matching pattern (used when mode=sample)

[output]
# References connector id in connectors/sink.d
connect = "file_kv_sink"
name = "gen_out"
# Override connector parameters (only keys in allow_override whitelist)
params = { base = "./src_dat", file = "gen.dat" }

[logging]
level = "warn"
output = "file"
file_path = "./data/logs/"
```

## Dynamic Speed Profiles

In addition to using the `speed` field for constant rate, you can use `speed_profile` to configure dynamic speed variation models.
When `speed_profile` is present, the `speed` field will be ignored.

### Constant Rate (constant)

Generate data at a fixed rate.

```toml
[generator.speed_profile]
type = "constant"
rate = 5000              # Rows per second
```

### Sinusoidal (sinusoidal)

Rate fluctuates periodically following a sine curve, simulating cyclical load changes.

```toml
[generator.speed_profile]
type = "sinusoidal"
base = 5000              # Base rate (rows/sec)
amplitude = 2000         # Fluctuation amplitude (rows/sec)
period_secs = 60.0       # Period length (seconds)
```

Rate range: `[base - amplitude, base + amplitude]`, i.e., 3000-7000 rows/sec in the above example.

### Stepped (stepped)

Rate changes according to predefined step sequences, suitable for simulating phased load testing.

```toml
[generator.speed_profile]
type = "stepped"
# Format: [[duration(sec), rate], ...]
steps = [
    [30.0, 1000],        # First 30 seconds: 1000 rows/sec
    [30.0, 5000],        # Next 30 seconds: 5000 rows/sec
    [30.0, 2000]         # Last 30 seconds: 2000 rows/sec
]
loop_forever = true      # Whether to loop (default false)
```

### Burst Mode (burst)

Randomly trigger high-speed bursts on top of the base rate, simulating burst traffic scenarios.

```toml
[generator.speed_profile]
type = "burst"
base = 1000              # Base rate (rows/sec)
burst_rate = 10000       # Rate during burst (rows/sec)
burst_duration_ms = 500  # Burst duration (milliseconds)
burst_probability = 0.05 # Probability of triggering burst per second (0.0-1.0)
```

### Ramp Mode (ramp)

Rate changes linearly from start value to target value, suitable for progressive stress testing.

```toml
[generator.speed_profile]
type = "ramp"
start = 100              # Starting rate (rows/sec)
end = 10000              # Target rate (rows/sec)
duration_secs = 300.0    # Ramp duration (seconds)
```

The rate will be maintained at the target value after reaching it. Supports both forward (increasing) and reverse (decreasing) ramps.

### Random Walk (random_walk)

Rate fluctuates randomly around the base value, simulating irregular load.

```toml
[generator.speed_profile]
type = "random_walk"
base = 5000              # Base rate (rows/sec)
variance = 0.3           # Fluctuation range (0.0-1.0), 0.3 means ±30%
```

Rate range: `[base * (1 - variance), base * (1 + variance)]`

### Composite Mode (composite)

Combine multiple speed profiles with various combination methods.

```toml
[generator.speed_profile]
type = "composite"
combine_mode = "average" # Combination method: average | max | min | sum

# Sub-profile list
[[generator.speed_profile.profiles]]
type = "sinusoidal"
base = 5000
amplitude = 2000
period_secs = 60.0

[[generator.speed_profile.profiles]]
type = "random_walk"
base = 5000
variance = 0.1
```

Combination methods:
- `average`: Average of all sub-profile rates (default)
- `max`: Maximum of all sub-profile rates
- `min`: Minimum of all sub-profile rates
- `sum`: Sum of all sub-profile rates

## Configuration Examples

### Example 1: Simple Constant Rate

```toml
version = "1.0"

[generator]
mode = "sample"
count = 10000
speed = 5000
parallel = 2

[output]
connect = "file_json_sink"
params = { base = "./data", file = "output.dat" }

[logging]
level = "info"
output = "file"
file_path = "./logs"
```

### Example 2: Progressive Stress Test

```toml
version = "1.0"

[generator]
mode = "rule"
duration_secs = 600      # Run for 10 minutes
parallel = 4
rule_root = "./rules"

[generator.speed_profile]
type = "ramp"
start = 100
end = 20000
duration_secs = 300.0    # Ramp from 100 to 20000 in 5 minutes

[output]
connect = "kafka_sink"
params = { topic = "test-topic" }

[logging]
level = "warn"
output = "file"
file_path = "./logs"
```

### Example 3: Simulating Real Business Load

```toml
version = "1.0"

[generator]
mode = "sample"
duration_secs = 3600     # Run for 1 hour
parallel = 8

[generator.speed_profile]
type = "composite"
combine_mode = "average"

# Base periodic fluctuation (simulating day/night traffic difference)
[[generator.speed_profile.profiles]]
type = "sinusoidal"
base = 10000
amplitude = 5000
period_secs = 300.0

# Overlay random noise
[[generator.speed_profile.profiles]]
type = "random_walk"
base = 10000
variance = 0.15

[output]
connect = "tcp_sink"
params = { host = "127.0.0.1", port = 9000 }

[logging]
level = "info"
output = "both"
file_path = "./logs"
```

## Runtime Rules

- When `wpgen` loads `conf/wpgen.toml`, if it detects `[output].connect`:
  - It searches upward from `ENGINE_CONF.sink_root` for the nearest `connectors/sink.d/` directory
  - Reads the target connector and merges with `params` (only keys in `allow_override` are allowed)

- When `parallel > 1` is configured, the speed profile will automatically distribute by parallelism to ensure the total rate meets expectations

- `count` and `duration_secs` are mutually exclusive:
  - When `count` is set, generation stops after the specified number of records
  - When `duration_secs` is set, generation stops after the specified duration
  - When neither is set, generation continues until manually stopped
