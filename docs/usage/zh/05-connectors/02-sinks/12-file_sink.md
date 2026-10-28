# File Sink 使用指南

## 概述

File Sink 用于将处理后的数据写入本地文件。支持多种数据格式，可根据需求选择性能模式或安全模式。

## 配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `fmt` | string | `json` | 输出格式 |
| `base` | string | `./data/out_dat` | 输出目录 |
| `file` | string | `out.dat` | 输出文件名 |
| `sync` | bool | `false` | 是否立即刷新到磁盘 |
| `max_size` | int/string | `0` | 单文件最大容量（字节，或 `10MB`/`1GiB` 写法）；`0` 表示不限容量、不滚动 |
| `max_rolls` | int | `3` | 保留的滚动文件个数（不含当前文件）；`0` 表示滚动时直接丢弃旧文件 |
| `roll_compress` | string/bool | `none` | 归档压缩方式：`none` / `gzip` / `zip`（`true` 等价 `gzip`） |

## 容量限制与滚动

配置 `max_size` 后，写入前会检查当前文件大小：本次写入会超过上限时先滚动，
再把记录写进新文件（单条记录不会被拆开，所以单个文件可能略超上限）。

- 历史文件按序号命名：`default.json`（当前）→ `default.json.1`（最新历史）→ `default.json.2` → `default.json.3`；
- `roll_compress` 可把历史文件归档成压缩包：`gzip` → `default.json.1.gz`（可直接 `gunzip`/`zcat`），`zip` → `default.json.1.zip`（标准 zip，内含单个同名条目）；默认 `none` 不压缩；
- `max_rolls` 控制保留的历史文件个数，默认 `3`，即最多保留 `.1` ~ `.3`；超出后最旧的文件被删除；
- 容量按**落盘字节**计算：开启 `compression` / `encryption` 时统计的是压缩/加密后的字节，且压缩流以 64KiB 块输出，触发粒度较粗；
- 滚动时会对当前文件做收尾（压缩流 flush、`sync_all`），并为新文件重建压缩/加密流，因此每个滚动文件都能独立解密解压；
- 归档压缩在独立阻塞线程中完成，失败时自动回退为不压缩归档，不会丢数据；
- `max_rolls` / `roll_compress` 必须与 `max_size > 0` 一起使用，否则视为配置错误；
- 重启续写已有文件时，容量从文件当前大小起算；
- 滚动只在写入时按容量触发，进程停止时不会额外滚动（最后一批数据留在当前文件）；
- `wparse stat sink-file` / `validate sink-file` 只统计当前活动文件的行数，不含历史归档；
- 目前只有 `protocol = "txt"`（默认）的文件 Sink 支持滚动，`protocol = "arrow"` 配 `max_size` / `max_rolls` / `roll_compress` 会报错。

使用前需确认连接器 `allow_override` 中包含 `max_size` / `max_rolls`：

```toml
[[connectors]]
id = "file_json_sink"
type = "file"
allow_override = ["base", "file", "max_size", "max_rolls", "roll_compress"]
[connectors.params]
fmt = "json"
base = "./data/out_dat"
file = "default.json"
```

> ⚠️ 不要把 `max_size` / `max_rolls` / `roll_compress` 写进 `[connectors.params]` 当默认值：
> 覆盖参数时会做类型一致性校验（覆盖值类型必须与默认值相同），默认成整数 `0` 后
> `max_size = "10MB"` 会被拒绝。不配默认值时的行为等价于不滚动（`max_size` 无限）。

按 10MB 滚动、保留 3 份历史、历史文件 zip 归档：

```toml
[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "app.json", max_size = "10MB", max_rolls = 3, roll_compress = "zip" }
```

## 支持的输出格式

| 格式 | 说明 | 适用场景 |
|------|------|----------|
| `json` | JSON 格式（每行一个对象） | API 对接、数据分析 |
| `csv` | CSV 格式 | Excel 导入、数据交换 |
| `kv` | 键值对格式 | 日志系统、监控 |
| `show` | 人类可读格式 | 调试、查看 |
| `raw` | 原始数据 | 备份、转发 |
| `proto-text` | Protocol Buffer 文本格式 | 结构化数据、调试 |

## 配置示例

### 示例 1: 基础 JSON 输出

```toml
[[sinks]]
name = "json_output"
kind = "file"

[sinks.params]
fmt = "json"
base = "./data/output"
file = "results.json"
```

输出文件路径: `./data/output/results.json`

### 示例 2: CSV 格式输出

```toml
[[sinks]]
name = "csv_export"
kind = "file"

[sinks.params]
fmt = "csv"
base = "./exports"
file = "data.csv"
```

### 示例 3: JSON 输出携带 OML 输出名

```toml
version = "2.0"
[sink_group]
name = "/sink/json_output"
oml = ["network.netflow"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "results.json" }
```

当记录来自 OML 输出时，引擎会把 OML `name` 作为批次级 `BatchMeta.oml_name` 传给 sink。具体输出形式由 sink 实现决定；当前核心文本 sink 会在 JSON/CSV 等结构化文本输出中追加 `wp_oml_name` 字段，Arrow framed sink 会把它写入 frame header tag。

如需关闭文本 payload 中的 OML 元字段，可在 `sink_group` 级配置：

```toml
[sink_group]
name = "/sink/json_output"
oml = ["network.netflow"]
wp_meta_disable = ["wp_oml_name"]
```

`wp_meta_disable` 只能放在 `[sink_group]`，不能放入 sink `params` 或 `wpgen.output.params`。当前支持的字段：`wp_oml_name`、`wp_event_md5`；该配置只控制 JSON/CSV 等文本 payload 字段输出，不关闭 Arrow framed 的 frame header tag。

### 示例 4: 启用同步模式

```toml
[[sinks]]
name = "critical_sink"
kind = "file"

[sinks.params]
fmt = "json"
base = "./data/critical"
file = "important.json"
sync = true
```

**说明**: 设置 `sync = true` 可确保数据实时写入磁盘，适合关键数据。

### 示例 5: 多个输出文件

```toml
# JSON 格式
[[sinks]]
name = "json_output"
kind = "file"

[sinks.params]
fmt = "json"
base = "./data/json"
file = "output.json"

# CSV 格式
[[sinks]]
name = "csv_output"
kind = "file"

[sinks.params]
fmt = "csv"
base = "./data/csv"
file = "output.csv"

# 原始格式备份
[[sinks]]
name = "raw_backup"
kind = "file"

[sinks.params]
fmt = "raw"
base = "./backup"
file = "backup.dat"
sync = true
```

## sync 参数说明

### sync: false (默认)

**特点**:
- ✅ 高性能，适合大量数据输出
- ✅ 系统资源占用低
- ⚠️ 异常退出时可能丢失少量数据

**适用场景**:
- 日志归档
- 批量数据导出
- 非关键数据存储

### sync: true

**特点**:
- ✅ 数据实时写入磁盘
- ✅ 最大程度避免数据丢失
- ⚠️ 性能较低，不适合高频写入

**适用场景**:
- 审计日志
- 金融交易记录
- 关键业务数据
- 调试环境

### 性能对比

| 模式 | 吞吐量 | 数据安全性 | 推荐场景 |
|------|--------|-----------|----------|
| `sync: false` | 🚀 高 | ⚠️ 中等 | 普通日志、批量导出 |
| `sync: true` | 📊 中等 | ✅ 高 | 审计日志、关键数据 |

## 输出格式示例

### JSON 格式
```json
{"timestamp":"2026-02-07T10:30:00Z","level":"INFO","message":"User login"}
{"timestamp":"2026-02-07T10:31:00Z","level":"WARN","message":"API timeout"}
```

### CSV 格式
```csv
timestamp,level,message
2026-02-07T10:30:00Z,INFO,User login
2026-02-07T10:31:00Z,WARN,API timeout
```

### KV 格式
```
timestamp=2026-02-07T10:30:00Z level=INFO message="User login"
timestamp=2026-02-07T10:31:00Z level=WARN message="API timeout"
```

## 使用场景

### 场景 1: 应用日志收集

```toml
[[sinks]]
name = "app_log"
kind = "file"

[sinks.params]
fmt = "json"
base = "./logs"
file = "app.log"
sync = false
```

**推荐配置**: 使用 `sync = false` 获得最佳性能。

### 场景 2: 审计日志

```toml
[[sinks]]
name = "audit_log"
kind = "file"

[sinks.params]
fmt = "json"
base = "./audit"
file = "security.log"
sync = true
```

**推荐配置**: 使用 `sync = true` 确保审计记录不丢失。

### 场景 3: 数据导出到 Excel

```toml
[[sinks]]
name = "excel_export"
kind = "file"

[sinks.params]
fmt = "csv"
base = "./exports"
file = "report.csv"
sync = false
```

**提示**: CSV 格式可直接用 Excel 打开。

### 场景 4: 调试输出

```toml
[[sinks]]
name = "debug_output"
kind = "file"

[sinks.params]
fmt = "show"
base = "./debug"
file = "trace.log"
sync = true
```

**推荐配置**: 调试时使用 `sync = true` 便于实时查看。

### 场景 5: 多格式备份

```toml
# JSON - 用于程序处理
[[sinks]]
name = "json_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "data.json"

# CSV - 用于 Excel 分析
[[sinks]]
name = "csv_sink"
kind = "file"

[sinks.params]
fmt = "csv"
file = "data.csv"

# 原始格式 - 用于完整备份
[[sinks]]
name = "raw_backup"
kind = "file"

[sinks.params]
fmt = "raw"
file = "backup.dat"
sync = true
```

## 文件名规范

### 建议的命名方式

```toml
# ✅ 带日期
[sinks.params]
file = "app_2024-01-01.log"
```

```toml
# ✅ 描述性名称
[sinks.params]
file = "access.log"
```

```toml
# ❌ 通用名称
[sinks.params]
file = "out.dat"
```

### 目录规划示例

```
./data/
  ├── logs/           # 普通日志
  ├── audit/          # 审计日志
  ├── exports/        # 数据导出
  ├── backup/         # 备份数据
  └── debug/          # 调试输出
```

## 最佳实践

### 1. 根据数据重要性选择 sync 模式

```
关键数据 → sync: true
普通数据 → sync: false
```

### 2. 使用描述性文件名

```toml
# ✅ 清晰明了
[sinks.params]
file = "user_login_2024-01.log"
```

### 3. 合理规划目录结构

按数据类型或重要性分目录存储。

### 4. 定期清理旧文件

优先使用 Sink 内置滚动（`max_size` + `max_rolls`，默认保留 3 份历史），
需要长周期归档时再配合 `logrotate` 等外部工具。

### 5. 监控磁盘空间

设置告警，避免磁盘写满。

## 注意事项

### 1. 目录权限

- ✅ 确保输出目录存在
- ✅ 确保有写入权限
- ⚠️ 检查 SELinux/AppArmor 策略

### 2. 磁盘空间

- ⚠️ 监控磁盘使用率
- ⚠️ 配置空间不足告警
- ✅ 定期清理历史文件

### 3. 性能影响

- `sync: true` 会降低性能
- 高频写入场景慎用 `sync: true`
- 使用 SSD 可改善性能



## 常见问题

### Q: 如何查看输出文件？

**A**: 直接使用文本编辑器或命令行工具：
```bash
# 查看 JSON 文件
cat ./data/output/results.json

# 实时查看（追加模式）
tail -f ./data/output/results.json
```

### Q: 文件过大如何处理？

**A**: 用 Sink 自带的容量滚动（推荐），无需依赖外部工具：

```toml
params = { file = "app.json", max_size = "100MB", max_rolls = 3, roll_compress = "gzip" }
```

历史文件会归档成 `app.json.1.gz`、`app.json.2.gz`、`app.json.3.gz`。

也可以在项目外部配合 `logrotate` 等工具做长周期归档。

### Q: sync 参数如何选择？

**A**: 参考以下原则：
- 关键数据（审计、交易）→ `sync: true`
- 普通数据（日志、统计）→ `sync: false`
- 不确定时使用默认值 `false`

### Q: 支持哪些字符编码？

**A**: 默认使用 UTF-8 编码。


## 总结

File Sink 是最常用的输出方式，适合各种场景：

- ✅ 配置简单，参数清晰
- ✅ 支持多种格式
- ✅ 灵活的性能/安全平衡
- ✅ 可靠的写入机制

**记住**:
- 普通数据用 `sync: false`（高性能）
- 关键数据用 `sync: true`（高安全）

---

**版本**: 1.15.0
**更新日期**: 2026-02-07
