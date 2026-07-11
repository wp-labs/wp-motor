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

组级参数：

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `sink_group.wp_meta_disable` | string array | `[]` | 对整个 sink_group 禁用固定 `wp_*` 元数据字段，例如 `["wp_stream_tag", "wp_event_id"]` |

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

### 示例 3: JSON 输出携带流标签

```toml
version = "2.0"
[sink_group]
name = "/sink/json_output"
oml = ["network.netflow"]
wp_meta_disable = ["wp_event_id"]

[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "results.json" }
```

当记录来自 OML/rule 输出时，流标签值来自运行时携带的逻辑输出身份。JSON/CSV 默认输出固定字段 `wp_stream_tag` 和 `wp_event_id`；如需关闭任一字段，设置 `sink_group.wp_meta_disable`，例如 `["wp_stream_tag"]` 或 `["wp_event_id"]`。被禁用的元数据字段在该 sink_group 输出中会被隐藏，即使输入记录里已经有同名字段也不会输出。

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

配合日志轮转工具（如 logrotate）管理文件。

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

**A**: 配合日志轮转工具：
```bash
# 使用 logrotate
logrotate /etc/logrotate.d/wp-motor
```

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
