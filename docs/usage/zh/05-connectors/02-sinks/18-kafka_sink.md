# Kafka Sink

Kafka sink 用于将数据输出到 Apache Kafka。启动时会尝试创建目标 topic（使用 `num_partitions`/`replication` 作为分区与副本配置）。

## 连接器定义

推荐使用仓库自带模板（位于 `connectors/sink.d/30-kafka.toml`）：

```toml
[[connectors]]
id = "kafka_sink"
type = "kafka"
allow_override = ["topic", "config", "num_partitions", "replication", "brokers"]

[connectors.params]
brokers = "localhost:9092"
topic = "wparse_output"
num_partitions = 1
replication = 1
# config = ["compression.type=snappy", "acks=all"]
```

## 可用参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `brokers` | string | Kafka bootstrap servers（逗号分隔，必填） |
| `topic` | string | 目标 topic（必填） |
| `num_partitions` | int | 自动创建 topic 的分区数（默认 1） |
| `replication` | int | 自动创建 topic 的副本数（默认 1） |
| `config` | string/array | 生产者配置列表，`key=value` 形式（可选） |

## 配置示例

### 基础用法

```toml
version = "2.0"

[sink_group]
name = "/sink/kafka"
oml  = ["example2"]

[[sink_group.sinks]]
name = "kafka_out"
connect = "kafka_sink"

[sink_group.sinks.params]
brokers = "localhost:9092"
topic = "wp.testcase.events.parsed"
```

### 自定义生产者参数与格式

```toml
[[sink_group.sinks]]
name = "kafka_out"
connect = "kafka_sink"

[sink_group.sinks.params]
topic = "app.events"
num_partitions = 3
replication = 1
config = [
  "compression.type=snappy",
  "acks=all",
  "linger.ms=5"
]
```

## 注意事项

- `config` 参数会透传给 Kafka producer（rdkafka），格式必须是 `key=value` 字符串。
- 若集群禁用自动建 topic，请提前在 Kafka 中创建目标 topic。
- 完整示例可参考 `wp-examples/extensions/kafka/README.md`。
