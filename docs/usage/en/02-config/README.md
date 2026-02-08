# Configuration Guide
<!-- Role: Configuration User | Last Verified: 2025-12-21 -->

This document focuses on the configuration that wparse relies on to run. It is recommended to start with the "Main Configuration (wparse.toml)" and then read the source/sink and connector sections as needed.

Recommended Reading Order
- Wparse Runtime Configuration (Main Configuration): wparse.toml (this directory)
  - [Wparse Configuration Specification (wparse.toml)](01-wparse_config.md)
- Sources and Connectors
  - [Sources Configuration Overview](02-sources.md)
  - Connectors (source.d) - see lookup rules and examples in "Sources Configuration Overview"
- Sinks and Connectors
  - [Sinks Design and Configuration (Directory-based V2)](04-sinks_config.md)
  - [Sinks Minimal Runnable Skeleton](03-sinks_minimal.md)
  - Connectors (sink.d) and route details - see "Design and Configuration"

Related References
- Reference parameters and specifications: Source/Sink/Spec documents under docs/80-reference
- CLI: docs/cli/wparse.md (quick lookup for common options)

Tips
- Use `wproj conf init --work-root .` to initialize standard directories and templates (conf/, connectors/, and some models directories). If you need a KnowDB template, run `wproj knowdb init` separately.
- After modifying scenario workflows, it is recommended to run `usecase/core/getting_started/case_verify.sh` to verify end-to-end output.
