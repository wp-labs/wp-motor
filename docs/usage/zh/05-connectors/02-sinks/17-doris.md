# Doris Sink

Doris sink 通过 Doris 的 MySQL 协议入口建立连接，并使用 Stream Load 写入数据。相比 mysql sink，对 Doris 兼容性更好（mysql sink 不兼容 Doris，doris sink 兼容 MySQL）。

## 连接器定义

推荐使用仓库自带模板（位于 `connectors/sink.d/50-doris.toml`）：

```toml
[[connectors]]
id = "doris_sink"
type = "doris"
allow_override = ["endpoint", "user", "password", "database", "table", "create_table"]

[connectors.params]
endpoint = "mysql://localhost:9030?charset=utf8mb4&connect_timeout=10"
user = "root"
password = ""
database = "wp_test"
table = "events_parsed"
```

## 可用参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `endpoint` | string | Doris FE 的 MySQL 访问地址（DSN 形式），如 `mysql://host:9030?charset=utf8mb4`（必填） |
| `user` | string | Doris 用户名（必填） |
| `password` | string | Doris 密码（可为空） |
| `database` | string | 目标数据库（必填） |
| `table` | string | 目标表（必填） |
| `create_table` | string | 可选建表 SQL，库表不存在时自动执行 |
| `pool_size` | int | 连接池大小（可选，常用 4） |
| `batch_size` | int | 单批写入事件数量（可选，常用 2048） |

## 配置示例

### 基础用法

```toml
version = "2.0"

[sink_group]
name = "/sink/doris"
oml  = ["example2"]

[[sink_group.sinks]]
name = "doris_stream_load"
connect = "doris_sink"

[sink_group.sinks.params]
endpoint = "mysql://localhost:9030?charset=utf8mb4&connect_timeout=10"
database = "wp_test"
table = "events_parsed"
user = "root"
password = ""
```

### 自动建表

```toml
[[sink_group.sinks]]
name = "doris_stream_load"
connect = "doris_sink"

[sink_group.sinks.params]
endpoint = "mysql://localhost:9030?charset=utf8mb4&connect_timeout=10"
database = "wp_test"
table = "events_parsed"
create_table = """
CREATE DATABASE IF NOT EXISTS wp_test;
CREATE TABLE events_parsed (
    sn           VARCHAR(64) COMMENT '设备序列号',
    dev_name     VARCHAR(128) COMMENT '设备名称',
    sip          VARCHAR(45) COMMENT '源 IP',
    from_zone    VARCHAR(32) COMMENT '来源区域',
    from_ip      VARCHAR(45) COMMENT '来源 IP',
    requ_uri     VARCHAR(512) COMMENT '请求 URI',
    requ_status  SMALLINT COMMENT '请求状态码',
    resp_len     INT COMMENT '响应长度',
    src_city     VARCHAR(32) COMMENT '源城市'
)
    ENGINE=OLAP
    DUPLICATE KEY(sn)
COMMENT '设备请求事件解析表'
DISTRIBUTED BY HASH(sn) BUCKETS 8
PROPERTIES (
    "replication_num" = "1"
);
"""
user = "root"
password = ""
```

## 注意事项

- 确保 Doris FE 的 MySQL 端口可访问，并开启 Stream Load 能力。
- 需要为账号授予 `SELECT/INSERT` 以及 `LOAD` 权限。
- 完整端到端示例可参考 `wp-examples/extensions/doris/README.md`。
