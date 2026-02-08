# MySQL Sink

MySQL sink 用于将解析后的记录写入 MySQL 表。它会根据 `columns` 生成 `INSERT IGNORE` 语句，适合幂等重试场景；仅接受 Record 数据（不支持 raw 输入）。

## 连接器定义

推荐使用仓库自带模板（位于 `connectors/sink.d/50-mysql.toml`）：

```toml
[[connectors]]
id = "mysql_sink"
type = "mysql"
allow_override = ["endpoint", "username", "password", "database", "table", "columns", "batch"]

[connectors.params]
endpoint = "localhost:3306"
username = "root"
password = "123456"
database = "wparse"
table = "nginx_logs"
columns = ["sip", "timestamp", "http/request", "status", "size", "referer", "http/agent", "wp_event_id"]
batch = 20
```

## 可用参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `endpoint` | string | MySQL 地址（`host:port`，必填） |
| `username` | string | 用户名（可选，默认 `root`） |
| `password` | string | 密码（可选） |
| `database` | string | 目标数据库（必填） |
| `table` | string | 目标表名（可选，未设置时使用 sink 名称） |
| `columns` | array | 列名列表，决定写入字段顺序；必须包含 `wp_event_id`（缺省会自动补齐） |
| `batch` | int | 批量写入条数（可选） |

## 配置示例

### 基础用法（参考 `extensions/tcp_mysql`）

```toml
version = "2.0"

[sink_group]
name = "all"
rule = ["/*"]
parallel = 8

[[sink_group.sinks]]
name = "main"
connect = "mysql_sink"

[sink_group.sinks.params]
endpoint = "localhost:3306"
username = "root"
password = "123456"
database = "wparse"
table = "nginx_logs"
columns = ["sip", "timestamp", "http/request", "status", "size", "referer", "http/agent", "wp_event_id"]
batch = 20
```

## 注意事项

- 表结构必须包含 `wp_event_id`（建议为 `BIGINT` 主键），否则写入会失败或产生重复。
- `columns` 中的字段名需与 OML 输出字段一致；缺失字段会以 `NULL` 写入。
- 可通过环境变量 `MYSQL_URL` 覆盖连接串（格式：`mysql://user:pass@host:port/db`）。
- 端到端示例可参考 `wp-examples/extensions/tcp_mysql/README.md`。
