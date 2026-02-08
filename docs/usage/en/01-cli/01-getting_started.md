# Getting Started

## Prerequisites
- [Download wparse](https://github.com/wp-labs/warp-parse/releases)
- Copy to an executable path, such as /usr/local/bin or /${HOME}/bin

## 1. Initialize Working Directory
- Clean and initialize configuration and templates
  ```bash
  wproj init --mode full
  wproj check
  ```

After execution, the working directory will contain:
```
├── conf
│   ├── wparse.toml
│   └── wpgen.toml
├── connectors
│   ├── sink.d
│   └── source.d
├── data
│   ├── in_dat
│   ├── logs
│   ├── out_dat
│   └── rescue
├── models
│   ├── knowledge
│   ├── oml
│   └── wpl
└── topology
    ├── sinks
    └── sources
```

## 2. Generate Data and Cleanup
```bash
wproj data clean
wpgen data clean

# Generate samples (example: 3000 lines, 3-second stats interval)
wpgen sample -n 3000 --stat 3
```

## 3. Run Parsing
```bash
# Batch processing (-n specifies count, -p prints statistics; check ./logs/ for failures)
wparse batch --stat 3 -p
```

## 4. Statistics and Validation
```bash
# Statistics for both sources and file-type sinks
wproj data stat
```
