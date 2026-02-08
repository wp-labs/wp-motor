# Wpgen

wpgen is the WarpParse data generator, used to generate test data based on WPL rules or sample files.

## Command Overview

```
wpgen <COMMAND>

Commands:
  rule    Generate data by rule
  sample  Generate data from sample files
  conf    Configuration commands
  data    Data management commands
```

## Subcommand Details

### rule - Rule-based Generation

Generate test data based on WPL rules, supporting rule validation and performance analysis.

```bash
wpgen rule [OPTIONS]
```

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| wpl_dir | - | `--wpl` | - | WPL rules directory override |
| conf_name | `-c` | `--conf` | `wpgen.toml` | Configuration filename |
| stat_print | `-p` | `--print_stat` | false | Periodically print statistics |
| line_cnt | `-n` | - | - | Total line count override |
| speed | `-s` | - | - | Generation speed (lines/sec) override |
| stat_sec | - | `--stat` | 1 | Statistics output interval (seconds) |

### sample - Sample-based Generation

Generate test data based on sample files (sample.dat).

```bash
wpgen sample [OPTIONS]
```


### conf - Configuration Management

```bash
wpgen conf <SUBCOMMAND>

Subcommands:
  init   Initialize generator configuration (conf/wpgen.toml)
  clean  Clean generator configuration
  check  Check configuration validity
```

### data - Data Management

```bash
wpgen data <SUBCOMMAND>

Subcommands:
  clean  Clean generated output data according to wpgen config
  check  Not supported; reserved for future use
```

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| conf_name | `-c` | `--conf` | `wpgen.toml` | Configuration filename |

## Runtime Semantics

### count (Total Output Count)

At startup, precisely divided among workers according to `parallel`, with remainders distributed to earlier workers. Each worker exits after completing its local workload; total count strictly equals `count`.

### speed (Global Rate)

- `speed = 0`: No limit (no waiting)
- `speed > 0`: Per-worker rate is `floor(speed / parallel)`

### parallel (Parallelism)

Number of parallel generation workers. For `blackhole_sink` consumers, parallelism also applies; other sinks default to single consumer.

## Usage Examples

```bash
# Configuration initialization
wpgen conf init -w .
wpgen conf check -w .

# Generate 10000 records based on rules
wpgen sample -n 10000 -p

# Custom rules directory and generation speed
wpgen rule \
    --wpl nginx \
    -c custom.toml \
    -s 1000 \
    --stat 2 \
    -p

# Generate based on sample file
wpgen sample -n 50000 -s 5000 --stat 5 -p

# Clean generated data
wpgen data clean -c wpgen.toml --local
```

## FAQ

**Q: Output less than expected?**

A: `count` is precisely allocated to each worker. Check if `limit : …` in the logs matches expectations.

## Configuration File

Default configuration file path: `conf/wpgen.toml`

Main configuration items:

```toml
[generator]
count = 10000      # Total generation count
speed = 1000       # Generation speed (lines/sec), 0 means unlimited
parallel = 4       # Number of parallel workers

[output]
# Output configuration...
```

Generated files are typically located in `./data/in_dat/`; the target path can be adjusted in the configuration.
