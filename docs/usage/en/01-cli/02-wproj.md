# Wproj

wproj is the Warp Parse project management tool, providing complete project lifecycle management functions, including project initialization and configuration management, data source checking and statistics, model management, and knowledge base creation and maintenance.

## Command Overview

```
wproj <COMMAND>

Commands:
  rule   Rule tools: management and debugging of parsing rules
  init   Initialize complete project skeleton
  check  Batch check project configuration and file integrity
  data   Data management tools: cleanup, statistics, validation
  model  Model management tools: rules, sources, sinks, knowledge base
```

---


## init - Project Initialization

Create project directory structure and default configuration with one command.

```bash
wproj init [OPTIONS]
```

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| mode | `-m` | `--mode` | `conf` | Initialization mode |

**Initialization Modes:**

| Mode | Description |
|------|-------------|
| `full` | Complete project (config+models+data+examples+connectors) |
| `normal` | Complete project (config+models+data+examples) |
| `model` | Model files only |
| `conf` | Configuration files only |
| `data` | Data directories only |

**Examples:**

```bash
# Initialize configuration (default)
wproj init -w /project

# Initialize complete project
wproj init -w /project --mode full
```

---

## check - Project Check

Batch check project configuration and file integrity.

```bash
wproj check [OPTIONS]
```

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| work_root | `-w` | `--work-root` | `.` | Root directory |
| what | - | `--what` | `all` | Check items |
| console | - | `--console` | false | Console log output |
| fail_fast | - | `--fail-fast` | false | Exit on first failure |
| json | - | `--json` | false | JSON format output |
| only_fail | - | `--only-fail` | false | Output only failed items |

**Check Items (--what):**

| Value | Description |
|-------|-------------|
| `conf` | Main configuration files |
| `connectors` | Connector configuration |
| `sources` | Data source configuration |
| `sinks` | Data sink configuration |
| `wpl` | WPL rule syntax |
| `oml` | OML model syntax |
| `all` | All checks (default) |

**Examples:**

```bash
# Comprehensive check
wproj check -w /project --what all

# Check only config and rules, exit on first failure
wproj check -w /project --what conf,wpl --fail-fast

# JSON output, show only failures
wproj check -w /project --json --only-fail
```

---

## data - Data Management

```bash
wproj data <SUBCOMMAND>

Subcommands:
  clean     Clean local output files
  check     Check data source connectivity
  stat      Statistics on data volume and performance
  validate  Validate data distribution and ratios
```

### data clean

Clean project output data.

```bash
wproj data clean
```


### data stat

Statistics on data volume.

```bash
wproj data stat
```


### data validate

Validate data distribution and ratios.

```bash
wproj data validate [OPTIONS]
```

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| input_cnt | - | `--input-cnt` | - | Total input count (denominator) |

**Examples:**

```bash
# Clean output data
wproj data clean

# Statistics for source + sink file line counts
wproj data stat

# Validate data distribution
wproj data validate
```

---

## model - Model Management
TODO!

---

## rule - Rule Tools

Offline parsing test to validate WPL rules.

```bash
wproj rule parse [OPTIONS]
```

**Examples:**

```bash
# Run offline parsing test with rules
wproj rule parse
```

---
