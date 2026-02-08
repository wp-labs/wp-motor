# Sinks Routing
<!-- Role: Configuration User | Last Verified: 2025-12-11 -->

Objective
- Configure routing distribution based on directory-style routes (business.d/infra.d); understand the differences between business groups and infrastructure groups and naming rules.

Core Concepts (single point definition: `../02-config/04-sinks_config.md`)
- Business group business.d: For scenario outputs.
- Infrastructure group infra.d: System-level outputs (default/miss/residue/error/monitor).

Directories and Naming
- Business groups: `$SINK_ROOT/business.d/**/*.toml`, supports recursive subdirectories; one group per route file.
- Infrastructure groups: `$SINK_ROOT/infra.d/**/*.toml`, supports recursive subdirectories; fixed group names (default/miss/...).
- Connectors: Search upward from `$WORK_ROOT/models/sinks` for the nearest `connectors/sink.d/*.toml`.

Minimal Example (business group)
```toml
version = "2.0"
[sink_group]
name = "/sink/demo"
oml  = ["/oml/example/*"]
parallel = 1

[[sink_group.sinks]]
name = "file_out"
connect = "file_json_sink"   # Connector id
params = { file = "demo.json" }
```


Infrastructure Group Example (default)
```toml
version = "2.0"
[sink_group]
name = "default"

[[sink_group.sinks]]
name = "default_sink"
connect = "file_json_sink"
```
