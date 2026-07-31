# Wparse配置

完整示例（推荐默认）
```toml
version = "1.0"
robust  = "normal"           # debug|normal|strict
gen_event_md5 = false        # true 则为每条事件盖 wp_event_md5（payload 的 MD5 指纹）；默认关闭

[models]
wpl     = "./models/wpl"
oml     = "./models/oml"
knowledge = "./models/knowledge"

[topology]
sources = "./topology/sources"
sinks   = "./topology/sinks"

[performance]
rate_limit_rps = 0            # 输入限速；0 表示自动限速，>0 表示固定 records/second
parse_workers  = 2            # 解析并发 worker 数
reload_timeout_ms = 10000     # reload 兜底超时（毫秒）；覆盖 graceful drain 与旧 processing 尾部清理
fetch_timeout_ms = 300 # realtime picker 每轮阻塞拉取的超时（毫秒）

[rescue]
path = "./data/rescue"        

[log_conf]
output = "File"               # Console|File|Both
level  = "warn,ctrl=info"

[log_conf.file]
path = "./data/logs"          # 文件输出目录；文件名自动取可执行名（wparse.log）

[stat]

[[stat.pick]]                 # 采集阶段统计
key    = "pick_stat"
target = "*"

[[stat.parse]]                # 解析阶段统计
key    = "parse_stat"
target = "*"

[[stat.sink]]                 # 下游阶段统计
key    = "sink_stat"
target = "*"
```

说明：
- `[models].knowledge` 是知识配置根目录，默认值为 `./models/knowledge`
- `semantic_dict.toml` 默认读取 `${models.knowledge}/semantic_dict.toml`
- `knowdb.toml` 默认读取 `${models.knowledge}/knowdb.toml`
- `rate_limit_rps` 默认 `0`；表示自动限速，会根据 source picker 水位和 parser 背压自动调整输入速率
- `reload_timeout_ms` 默认 `10000`；CLI `--reload-timeout-ms` 优先于配置文件
- `fetch_timeout_ms` 默认 `300`；用于控制 realtime picker 单轮阻塞拉取的最长等待时间
- `gen_event_md5` 默认 `false`；置 `true` 时为每条事件产出 `wp_event_md5`（payload 的 MD5 指纹）字段，出现在该事件的所有 record（含 `copy_event_parse` 旁路 record）。需配合 `gen_msg_id`（事件 meta 总开关，默认开启）。详见 [Source Meta](../05-connectors/01-sources/09-metadata.md)。

## 内存 Profile

内存相关队列、水位和批大小默认由 `WP_MEMORY_PROFILE` 统一控制。普通部署只需要选择一个 profile：

```bash
WP_MEMORY_PROFILE=low        # 更低内存
WP_MEMORY_PROFILE=standard   # 默认推荐；不设置时等价，平衡吞吐和 RSS
WP_MEMORY_PROFILE=throughput # 更宽 parser/sink channel；适合复杂样本或快 sink
```

推荐含义：

- `low`：更早施加背压，优先控制 RSS：`parser/sink channel = 32/16`、`sink_batch_size = 256`、`picker_burst_max = 4`、`tcp_batch = 32KB/32 events`、`pending = 1MB`。
- `standard`：默认生产档，保留 low 的 sink、pending、UDP 和 file 内存水位，同时提高 TCP 长行样本吞吐：`parser/sink channel = 48/16`、`sink_batch_size = 256`、`picker_burst_max = 6`、`tcp_recv = 2MB`、`tcp_batch = 256KB/256 events`、`pending = 1MB`。
- `throughput`：给解析和发送链路更大的通道余量，优先跑满复杂样本；仍保留较小 pending，避免无界堆积。

历史别名仍可用：`small/tiny/xs` 等价于 `low`，`large/high` 等价于 `throughput`，`default/normal/balanced` 等价于 `standard`。

单项环境变量仍然保留给专项调优和压测，例如 `WP_PARSER_CHANNEL_CAP`、`WP_SINK_CHANNEL_CAP`、`WP_SINK_BATCH_SIZE`、`WP_PICKER_BURST_MAX`、`WP_PICKER_PENDING_MAX_BYTES`、`WP_TCP_RECV_BYTES`、`WP_TCP_BATCH_BYTES`、`WP_TCP_BATCH_CAPACITY`。生产配置优先使用 profile，避免多个变量组合后难以解释。

## 变量化建议

`wparse.toml` 中的路径类字符串适合使用 `${VAR}` 变量化，例如：

```toml
[models]
knowledge = "${WORK_ROOT}/models/knowledge"

[rescue]
path = "${WORK_ROOT}/data/rescue"

[log_conf.file]
path = "${WORK_ROOT}/data/logs"
```

涉及外部变量文件、敏感值和 `sec_key.toml` 约定时，参考：[配置变量与安全字典（`${VAR}` / `sec_key.toml`）](08-variables_and_sec_key.md)。
