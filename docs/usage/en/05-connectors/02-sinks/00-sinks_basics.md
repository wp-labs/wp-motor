# Sink Basics

This document introduces the fundamental concepts and configuration structure of data output endpoints (Sinks) in the warpparse system.

## Overview

Sinks are the data output endpoints of the warpparse system, responsible for sending processed data to various target systems. The system supports multiple output types, including file, Syslog, Prometheus, and more.

## Core Concepts

### 1. Configuration Hierarchy

The warpparse system adopts a layered configuration architecture:

```
Global Default Configuration (defaults.toml)
    ↓
Route Group Configuration (business.d/**/*.toml, infra.d/**/*.toml)
    ↓
Connector Definitions (connectors/sink.d/*.toml)
    ↓
Resolved Sink Instance (ResolvedSinkSpec)
```

### 2. Core Data Structures


## Configuration File Structure

### 1. Connector Definition (connectors.toml)

```toml
# connectors/sink.d/file_raw_sink.toml
[[connectors]]
id = "file_raw_sink"
type = "file"
allow_override = ["base", "file", "fmt"]

[connectors.params]
base = "./data/out_dat"
file = "default.dat"
fmt = "json"
```

**Key Field Descriptions**:
- `id`: Unique identifier for the connector
- `type`: Connector type (file, syslog, prometheus, etc.)
- `allow_override`: List of parameters that can be overridden by source configuration
- `params`: Connector default parameters

### 2. Route Configuration (business.d/**/*.toml, infra.d/**/*.toml)

```toml
# business.d/example.toml
version = "2.0"

[sink_group]
name = "/sink/example"
oml = ["example_pattern"]
parallel = 2
tags = ["env:production"]

[[sink_group.sinks]]
name = "example_sink"
connect = "file_raw_sink"
params = {
    base = "./output",
    file = "example.dat"
}
filter = "./filter.wpl"
tags = ["type:example"]

[sink_group.sinks.expect]
ratio = 1.0
tol = 0.01
```

### 3. Global Default Configuration (defaults.toml)

```toml
# defaults.toml
version = "2.0"

[defaults]
tags = ["env:default"]

[defaults.expect]
basis = "total_input"
min_samples = 100
mode = "error"
```

## Basic Configuration Examples

### 1. Simple File Output
```toml
# infra.d/simple_file.toml
version = "2.0"

[sink_group]
name = "simple_output"
oml = []
[[sink_group.sinks]]
connect = "file_raw_sink"
params = { file = "simple.log" }
```

### 2. Output with Filter
```toml
# business.d/filtered_output.toml
version = "2.0"

[sink_group]
name = "/sink/filtered"
oml = ["/oml/logs/*"]

[[sink_group.sinks]]
name = "all_logs"
connect = "file_json_sink"
params = { file = "all_logs.json" }

[[sink_group.sinks]]
name = "error_logs"
connect = "file_json_sink"
filter = "./error_filter.wpl"
params = { file = "error_logs.json" }
[sink_group.sinks.expect]
ratio = 0.1
tol = 0.02
```

### 3. Parallel Output Configuration (Business Groups Only)
```toml
# business.d/parallel_output.toml
version = "2.0"

[sink_group]
name = "/sink/parallel"
oml = ["high_volume"]
parallel = 4
tags = ["type:parallel"]

[[sink_group.sinks]]
name = "output_1"
connect = "file_proto_sink"
params = { file = "output_1.dat" }

[[sink_group.sinks]]
name = "output_2"
connect = "file_proto_sink"
params = { file = "output_2.dat" }
```
Note: Infrastructure groups (infra.d) do not support `parallel` and file sharding; for improved throughput and sharding, configure in business groups.

## Tag System

### 1. Tag Inheritance Hierarchy

The tag system supports three-level inheritance:
1. **Default Tags** (from defaults.toml)
2. **Group-level Tags** (from sink_group)
3. **Sink-level Tags** (from individual sink)

### 2. Tag Configuration Example
```toml
# defaults.toml
[defaults]
tags = ["env:production", "service:warpflow"]

# business.d/example.toml
[sink_group]
tags = ["region:us-west", "tier:processing"]

[[sink_group.sinks]]
tags = ["output:file", "compression:gzip"]
```

**Final Merged Tags**:
```
["env:production", "service:warpflow", "region:us-west", "tier:processing", "output:file", "compression:gzip"]
```



## Expectation Configuration (Expect)

### 1. Ratio Mode
```toml
[sink_group.sinks.expect]
ratio = 1.0    # Expected ratio 100%
tol = 0.01     # Tolerance ±1%
```

### 2. Range Mode
```toml
[sink_group.sinks.expect]
min = 0.001    # Minimum ratio 0.1%
max = 2.0      # Maximum ratio 200%
```

### 3. Global Default Expectations
```toml
[defaults.expect]
basis = "total_input"  # Calculation basis
min_samples = 100      # Minimum sample count
mode = "error"         # Violation handling mode
```

## Filter Configuration

### 1. Filter Files
Filter files use WPL (Warp Processing Language) syntax:

```wpl
# filter.wpl
# Only process error-level logs
level == "ERROR" || level == "FATAL"

# Or complex conditions
(level == "ERROR" && source == "auth") ||
(level == "WARN" && message ~= "timeout")
```

### 2. Filter Application
```toml
[[sink_group.sinks]]
name = "filtered_output"
connect = "file_json_sink"
filter = "./error_filter.wpl"    # Apply filter
params = { file = "errors.json" }
```

## Configuration Validation

### 1. Parameter Override Validation
The system strictly validates parameter overrides:
- Only parameters specified in `allow_override` can be overridden
- Nested table structure overrides are not supported

### 2. Uniqueness Validation
- Sink names within the same sink_group must be unique
- Connector IDs must be globally unique

### 3. File Existence Validation
- Filter files must exist and have correct syntax
- File path parameters must be valid
