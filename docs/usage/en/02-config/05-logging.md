# Logging Configuration

This document provides practical guidance for users and frontline developers on enabling/adjusting logging and troubleshooting common issues in Warp Parse.

- For target domains, see "Developer Documentation: Logging Targets And Levels".
- File logging defaults to rotation: 10MB/10 files, gzip compression.

## Quick Start

### wparse / wprescue (conf/wparse.toml)

Production Recommended (file output + low noise):
```toml
[log_conf]
output = "File"   # Console|File|Both
level  = "warn,ctrl=info,dfx=info"
[log_conf.file]
path = "./data/logs"   # Directory is created automatically; filename is automatically named after executable, e.g., wparse.log
```

Local Debugging (output to console simultaneously):
```toml
[log_conf]
output = "Both"
level  = "debug"
[log_conf.file]
path = "./data/logs"
```
