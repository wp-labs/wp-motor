# VictoriaLogs 

VictoriaLogs sink 用于将日志数据输出到 [VictoriaLogs](https://docs.victoriametrics.com/victorialogs/) 日志存储系统，通过 HTTP JSON Line 接口写入。

## 连接器定义

```toml
[[connectors]]
id = "victorialog_sink"
type = "victorialogs"
allow_override = ["endpoint", "insert_path", "fmt"]

[connectors.params]
endpoint = "http://localhost:8481"
insert_path = "/insert/json"
fmt = "json"
```

## 可用参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `endpoint` | string | `http://localhost:8481` | VictoriaLogs 服务地址（必填） |
| `insert_path` | string | `/insert/json` | 数据写入路径 |
| `create_time_field` | string | - | 自定义时间戳字段名，从数据记录中提取 |
| `fmt` | string | `json` | 输出格式：`json`、`csv`、`kv`、`raw` 等 |

## 数据格式

Sink 会将每条数据记录转换为 JSON 对象发送，包含以下特殊字段：

- `_msg`：格式化后的消息内容（根据 `fmt` 参数格式化）
- `_time`：时间戳（纳秒精度），优先使用 `create_time_field` 指定字段，否则使用当前时间

## 配置示例

### 基础用法

```toml
version = "2.0"

[sink_group]
name = "/sink/victorialogs"
oml  = ["logs"]

[[sink_group.sinks]]
name = "vlogs"
connect = "victorialog_sink"
params = { endpoint = "http://victorialogs:9428" }
```

### 自定义时间字段

```toml
[[sink_group.sinks]]
name = "vlogs"
connect = "victorialog_sink"

[sink_group.sinks.params]
endpoint = "http://victorialogs:9428"
insert_path = "/insert/jsonline"
create_time_field = "timestamp"
fmt = "json"
```

## 注意事项

- `endpoint` 参数不能为空，否则会校验失败
- HTTP 请求超时时间为 5 秒
- 如果 `create_time_field` 指定的字段不存在或非时间类型，将使用当前 UTC 时间
