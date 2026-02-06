# Sink 连接器使用指南

## 概述

Sink 连接器用于将处理后的数据输出到各种目标系统。本指南介绍如何配置和使用 WP-Motor 的 Sink 功能。

## 支持的 Sink 类型

### 文件类

| Sink 类型 | 说明 | 文档 |
|-----------|------|------|
| **file** | 本地文件输出 | [📄 详细文档](./file_sink.md) |

### 网络类 (计划中)

| Sink 类型 | 说明 | 状态 |
|-----------|------|------|
| http | HTTP/HTTPS 输出 | 📋 规划中 |
| kafka | Kafka 消息队列 | 📋 规划中 |
| tcp | TCP Socket | 📋 规划中 |
| udp | UDP Socket | 📋 规划中 |

### 数据库类 (计划中)

| Sink 类型 | 说明 | 状态 |
|-----------|------|------|
| mysql | MySQL 数据库 | 📋 规划中 |
| postgresql | PostgreSQL 数据库 | 📋 规划中 |
| clickhouse | ClickHouse 数据库 | 📋 规划中 |

## 快速开始

### 基本配置结构

```json
{
  "name": "sink_name",
  "kind": "sink_type",
  "params": {
    // 具体参数取决于 sink 类型
  }
}
```

### 示例 1: 简单文件输出

```json
{
  "name": "output",
  "kind": "file",
  "params": {
    "fmt": "json",
    "base": "./data",
    "file": "output.json"
  }
}
```

### 示例 2: 多个 Sink

```toml
[[sinks]]
name = "json_output"
kind = "file"

[sinks.params]
fmt = "json"
file = "data.json"

[[sinks]]
name = "csv_output"
kind = "file"

[sinks.params]
fmt = "csv"
file = "data.csv"
```

## 配置参数

### 通用参数

| 参数 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | Sink 实例名称 |
| `kind` | string | ✅ | Sink 类型 |
| `params` | object | ✅ | 类型特定参数 |

### 参数说明

- **name**: 唯一标识符，用于路由配置
- **kind**: 选择 Sink 类型（如 `file`）
- **params**: 根据 Sink 类型提供相应参数

## 输出格式

### 支持的格式

| 格式 | 说明 | 适用场景 |
|------|------|----------|
| `json` | JSON 格式 | API 对接、数据分析 |
| `csv` | CSV 表格 | Excel 导入、数据交换 |
| `kv` | 键值对 | 日志系统、监控 |
| `show` | 可读格式 | 调试、展示 |
| `raw` | 原始数据 | 备份、转发 |
| `proto-text` | Protocol Buffer 文本 | 结构化数据、调试 |

### 格式对比

| 格式 | 可读性 | 体积 | 解析速度 | 适用场景 |
|------|--------|------|----------|----------|
| json | ⭐⭐⭐⭐ | 中等 | 快 | 通用 |
| csv | ⭐⭐⭐ | 小 | 快 | 表格数据 |
| kv | ⭐⭐⭐ | 中等 | 快 | 日志 |
| show | ⭐⭐⭐⭐⭐ | 大 | 慢 | 调试 |
| raw | ⭐ | 最小 | 最快 | 性能优先 |
| proto-text | ⭐⭐⭐⭐ | 中等 | 中等 | 结构化数据 |

### 格式示例

#### JSON
```json
{"time":"2026-02-07T10:00:00Z","level":"INFO","msg":"User login"}
```

#### CSV
```csv
time,level,msg
2026-02-07T10:00:00Z,INFO,User login
```

#### KV
```
time=2026-02-07T10:00:00Z level=INFO msg="User login"
```

## 使用场景

### 场景 1: 日志存档

```toml
[[sinks]]
name = "log_archive"
kind = "file"

[sinks.params]
fmt = "json"
base = "./logs"
file = "app.log"
```

**适用**: 应用日志、系统日志

### 场景 2: 数据分析

```toml
[[sinks]]
name = "analytics_data"
kind = "file"

[sinks.params]
fmt = "csv"
base = "./analytics"
file = "events.csv"
```

**适用**: BI 分析、报表生成

### 场景 3: 数据备份

```toml
[[sinks]]
name = "backup"
kind = "file"

[sinks.params]
fmt = "raw"
base = "./backup"
file = "data.dat"
sync = true
```

**适用**: 重要数据备份

### 场景 4: 多目标输出

```toml
# JSON 用于程序处理
[[sinks]]
name = "json_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "data.json"

# CSV 用于 Excel
[[sinks]]
name = "csv_sink"
kind = "file"

[sinks.params]
fmt = "csv"
file = "data.csv"
```

**适用**: 需要多种格式的场景

## 路由配置

### 基于条件的路由

```toml
[[routing]]
condition = "level == \"ERROR\""
sink = "error_sink"

[[routing]]
condition = "level == \"WARN\""
sink = "warn_sink"

[routing.default]
sink = "info_sink"

[[sinks]]
name = "error_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "error.log"
sync = true

[[sinks]]
name = "warn_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "warn.log"

[[sinks]]
name = "info_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "info.log"
```

### 多目标路由

```toml
[[routing]]
condition = "event == \"login\""
sinks = ["audit_sink", "analytics_sink"]

[[sinks]]
name = "audit_sink"
kind = "file"

[sinks.params]
fmt = "json"
file = "audit.log"
sync = true

[[sinks]]
name = "analytics_sink"
kind = "file"

[sinks.params]
fmt = "csv"
file = "analytics.csv"
```

## 最佳实践

### 1. 命名规范

```toml
# ✅ 好的命名
[[sinks]]
name = "user_login_events"

[[sinks]]
name = "error_logs"

[[sinks]]
name = "audit_trail"

# ❌ 避免的命名
[[sinks]]
name = "sink1"

[[sinks]]
name = "output"

[[sinks]]
name = "temp"
```

### 2. 目录规划

```
./data/
  ├── logs/         # 普通日志
  ├── audit/        # 审计日志
  ├── exports/      # 数据导出
  ├── backup/       # 备份数据
  └── analytics/    # 分析数据
```

### 3. 文件命名

```toml
# ✅ 带时间戳
[sinks.params]
file = "app_2024-01-01.log"

# ✅ 描述性名称
[sinks.params]
file = "user_events.json"

# ❌ 通用名称
[sinks.params]
file = "output.dat"
```

### 4. 选择合适的格式

```
日志归档 → json
数据分析 → csv
高性能 → raw
调试 → show / proto-text
备份 → raw
```

### 5. 性能与安全的平衡

```
关键数据 → sync: true
普通数据 → sync: false
```

## 监控与维护

### 监控要点

- ✅ 磁盘空间使用率
- ✅ 文件大小增长
- ✅ 写入延迟
- ✅ 错误率

### 日常维护

```bash
# 检查磁盘空间
df -h ./data

# 清理旧文件（保留 30 天）
find ./data/logs -mtime +30 -delete

# 查看最新输出
tail -f ./data/logs/app.log
```

### 日志轮转

```bash
# 使用 logrotate
cat > /etc/logrotate.d/wp-motor << EOF
/var/log/wp-motor/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
}
EOF
```

## 故障排查

### 常见问题速查

| 问题 | 可能原因 | 解决方法 |
|------|---------|----------|
| 无法创建文件 | 目录不存在 | `mkdir -p` 创建目录 |
| 权限错误 | 无写权限 | `chmod` 修改权限 |
| 磁盘已满 | 空间不足 | 清理旧文件 |
| 写入慢 | `sync: true` | 改用 `sync: false` |

### 详细排查步骤

#### 1. 检查配置
```bash
# 验证配置文件
wp-engine validate config.yaml
```

#### 2. 检查权限
```bash
# 检查目录权限
ls -la ./data

# 修改权限
chmod 755 ./data
```

#### 3. 检查磁盘空间
```bash
# 查看磁盘使用
df -h

# 查看目录大小
du -sh ./data/*
```

#### 4. 查看日志
```bash
# 查看最新日志
tail -f ./logs/wp-motor.log

# 搜索错误
grep ERROR ./logs/wp-motor.log
```

## 注意事项

### 1. 磁盘空间

- ⚠️ 定期监控磁盘使用率
- ⚠️ 设置空间不足告警
- ✅ 配置日志轮转

### 2. 性能影响

- `sync: true` 降低性能
- 高频写入慎用同步模式
- SSD 性能优于机械硬盘

### 3. 文件锁定

- 写入时文件带 `.lock` 后缀
- 完成后自动重命名
- 不要手动操作锁文件

### 4. 字符编码

- 默认使用 UTF-8
- 确保数据编码一致

## 常见问题

### Q: 如何选择输出格式？

**A**: 根据使用场景：
- API 对接 → `json`
- Excel 分析 → `csv`
- 日志系统 → `kv`
- 调试查看 → `show` 或 `proto-text`
- 性能优先 → `raw`

### Q: 多个 Sink 的执行顺序？

**A**: 并行执行，不保证顺序。如需顺序，使用路由配置。

### Q: 如何处理大文件？

**A**:
1. 使用日志轮转工具
2. 按日期分割文件
3. 定期归档和压缩

### Q: 网络路径支持吗？

**A**: 支持，但需先挂载网络文件系统：
```yaml
params:
  base: /mnt/nfs/data
  file: output.json
```

### Q: 支持动态文件名吗？

**A**: 目前不支持动态文件名，建议使用外部脚本定期重命名。

## 配置模板

### 基础配置
```toml
[[sinks]]
name = "main_output"
kind = "file"

[sinks.params]
fmt = "json"
base = "./data"
file = "output.json"
```

### 完整配置
```toml
# Sink 配置
# 普通日志
[[sinks]]
name = "app_log"
kind = "file"

[sinks.params]
fmt = "json"
base = "./logs"
file = "app.log"
sync = false

# 审计日志
[[sinks]]
name = "audit_log"
kind = "file"

[sinks.params]
fmt = "json"
base = "./audit"
file = "security.log"
sync = true

# CSV 导出
[[sinks]]
name = "csv_export"
kind = "file"

[sinks.params]
fmt = "csv"
base = "./exports"
file = "data.csv"
sync = false

# 路由配置
[[routing]]
condition = "level == \"ERROR\""
sink = "error_sink"

[[routing]]
condition = "category == \"audit\""
sink = "audit_log"

[routing.default]
sink = "app_log"
```

## 性能优化建议

### 1. 格式选择
- 大数据量用 `raw`
- 需要可读性用 `json` 或 `csv`
- 结构化调试用 `proto-text`

### 2. sync 参数
- 普通日志用 `sync: false`
- 关键数据用 `sync: true`

### 3. 文件分割
- 按日期分割文件
- 控制单文件大小

### 4. 硬件优化
- 使用 SSD 存储
- 独立磁盘分区

## 相关文档

- [File Sink 详细指南](./file_sink.md)
- [配置文件格式](../config/README.md)
- [路由规则配置](../routing/README.md)

---

**版本**: 1.15.0
**更新日期**: 2026-02-07
