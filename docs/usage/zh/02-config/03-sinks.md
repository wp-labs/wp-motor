# Sink 配置


## 目录与文件组织
- sink_root：用例内通常为 `<case>/sink`
  - business.d/**/*.toml：业务组路由（场景输出，支持子目录）
  - infra.d/**/*.toml：基础组路由（default/miss/residue/intercept/error/monitor，支持子目录）
  - defaults.toml：默认组级期望 [defaults.expect]

- connectors/sink.d/*.toml：连接器定义（loader 自 sink_root 向上查找最近的该目录）


## 路由文件格式
- 顶层
  - version（可选）
  - sink_group
    - name：组名（字符串）
    - oml / rule：推荐扁平写法；均可为字符串或字符串数组；用于匹配模型或规则。
    - wp_meta_disable：可选，组级元信息 payload 输出关闭列表；当前支持 `["wp_oml_name", "wp_event_md5"]`。
    - expect：可选，组级期望（覆盖 defaults）
    - sinks：数组，每项为单个 sink 定义
- 单个 sink 字段
  - name：该 sink 的名称（组内唯一）；未提供则按 [index] 回退
  - connect：引用连接器 id（兼容读取 `use`/`connector`）
  - params：对连接器默认参数的白名单覆盖（keys 必须在连接器 allow_override 列表中）
  - expect：可选，单 sink 期望（仅 ratio/tol/min/max，互斥关系：ratio/tol 与 min/max 不可混用）
  - filter：可选，拦截条件文件路径；命中 true 时丢弃该 sink 并发送至 intercept
  - drop_when_error：可选，发送失败时是否丢弃数据；默认 `false`（写救援文件，历史行为）

## 变量化示例

```toml
version = "2.0"

[sink_group]
name = "mysql_${ENV}"
oml = ["*"]

[[sink_group.sinks]]
name = "writer"
connect = "mysql_sink"
tags = ["env:${ENV}"]

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

说明：
- route 名、sink 名、tags 以及 `params` 中的字符串都可以按 `${VAR}` 变量化
- 对账号、密码、DSN、Token 这类字段，优先使用 `SEC_` 变量
- 变量来源与 `sec_key.toml` 约定见：[配置变量与安全字典（`${VAR}` / `sec_key.toml`）](08-variables_and_sec_key.md)

## 配置示例：
### 基础组
```toml
version = "2.0"
[sink_group]
name = "intercept"

[[sink_group.sinks]]
name = "intercept"
connect = "file_kv_sink"
params = { base = "./out", file = "intercept.dat" }
```

### 业务组（filter）
```toml
version = "2.0"

[sink_group]
name = "/sink/filter"
oml  = ["/oml/sh*"]

[[sink_group.sinks]]
name = "all"
connect = "file_kv_sink"
params = { base = "./out/sink", file = "all.dat" }

[[sink_group.sinks]]
name = "safe"
connect = "file_kv_sink"
filter = "./sink/business.d/filter.conf"   # 命中 -> 拦截，不写 safe
params = { base = "./out/sink", file = "safe.dat" }
```
### 文件滚动与归档

文件型 sink 可以按单文件容量滚动，并把历史文件归档成 gzip/zip：

```toml
[[sink_group.sinks]]
name = "json"
connect = "file_json_sink"
params = { file = "app.json", max_size = "10MB", max_rolls = 3, roll_compress = "gzip" }
```

- `max_size`：单文件最大字节数（`0` / 不配 = 不滚动），支持 `10MB` / `1GiB` 这类写法；
- `max_rolls`：保留的历史文件数，默认 `3`；`0` 表示滚动时丢弃旧文件；
- `roll_compress`：`none`（默认）/ `gzip` / `zip`，历史文件依次为 `app.json.1[.gz|.zip]`。

详见 [文件 Sink 配置](../05-connectors/02-sinks/12-file_sink.md)。


## 发送失败处理（drop_when_error）

`drop_when_error` 决定 sink 发送失败（连接断开、下游 5xx、超时、401/404 等）后 wp-motor 的处理方式。
每个 sink 独立配置，默认 `false`，行为与历史版本一致。

| 取值 | 行为 |
| --- | --- |
| `false`（默认/缺省） | 写救援文件 `data/rescue/<sink>/<sink>-<ts>-<seq>.dat`，并将旧 sink 交给维护线程重连；一次故障内复用同一个文件 |
| `true` | 记录丢弃计数与最近错误后丢弃本条/本批数据，**不写救援文件、不切换坏 sink**，同组后续 sink 继续投递 |

### 熔断：坏链路时不反复尝试

`drop_when_error = true` 时，第一次发送失败会打开 sink 级熔断：

- 冷却期内**不再调用 connector**，到达的数据直接丢弃并计数，因此不会每批都去撞连接超时或重试退避；
- 冷却 5s 起，连续失败按 ×2 增长，上限 60s；
- 冷却到期只放行一个批次做探测：成功则关闭熔断、恢复投递并输出恢复汇总；失败则继续退避。

| 状态 | 行为 | 代价 |
| --- | --- | --- |
| 关闭（正常） | 正常下发 | 一次 send |
| 打开（熔断） | 不调用 connector，直接丢弃并计数 | 一次计数 |
| 半开（冷却到期） | 放行一个探测批次 | 一个批次 |

熔断只绕过 wp-motor 自身的救援层，connector 自己的重试仍然生效：`HttpSink` 会对 5xx/网络错误按 `params.max_retries`（默认 3，退避 1s→2s→4s）重试。
高频链路建议配合更短的重试预算：

```toml
[[sink_group.sinks]]
name = "atlas"
connect = "http_sink"
drop_when_error = true

[sink_group.sinks.params]
max_retries = 0
```

批量发送时，一次失败的批次会整批丢弃（丢弃计数按批次内记录数累加）；`batch_size` 越大，单次丢弃的数据量越大，需要按业务容忍度设置。

### 可观测性

- 日志：首次失败立刻 WARN，之后每 60 秒最多一条；熔断期间跳过批次也会限流告警，日志会区分 `failed`（真实调用失败）与 `breaker open`（未调用）。恢复后输出一条汇总，包含故障期间累计丢弃数。
- 统计：监控窗口内会记录 `sink_dropped_total`（按 sink 名分行），`sum stat` 中为进程启动以来的累计丢弃数。
- 监控日志：每个统计窗口输出一条 `sink '<name>' dropped: window=... total=... last_error_at=... last_error=...`，错误文本截断到 200 字符。
- 被丢弃的记录在 `sink_stat` 中仍按“已处理”计数；数据是否真的落地请以 `sink_dropped_total` 为准。

### 边界说明

- 覆盖**所有发送失败**（包括 401/404 这类配置错误），启用前请确认 endpoint 正确。
- 与 `filter`（拦截到 intercept）和 `expect`（校验失败）无关，那两条路有自己的语义。
- `sink.is_ready() == false` 时的记录回收不在本开关范围内。
- 默认的救援文件目前没有自动回放（仅 `wp rescue stat` 可统计），因此 `false` 只保证“失败时落本地”，不等同于可靠投递。

### 救援模式（`drop_when_error = false`）

- **一次故障只产生一个救援文件**：维护线程重连后会切回真实 sink，但救援文件保持打开；如果下一次发送再次失败，直接复用同一个文件追加，不再新建。
- 真正投递成功后才回收：flush 并把 `.dat.lock` 重命名为 `.dat`，同时输出一条 `rescue file finalized` 日志（含路径与记录数）。
- 可观测性：救援期间每 60s 最多一条 WARN：`on rescue: file=... records=... size=...B`。
  `records` 是本次故障内已写入的记录数；`size` 是已落盘字节数，受 `BufWriter` 缓冲影响会略滞后（每 100 条或停止时 flush）。
  注意 `wp rescue stat` 只统计已回收的 `.dat`，故障期间的 `.lock` 不会出现在统计里，需要看这条日志。
- 仍然没有自动回放：救援文件需要外部工具或人工处理。
- **救援模式不做尝试限速**：维护线程重连成功后会立即切回，后续每个批次仍会尝试真实 sink；需要限速时用 `drop_when_error = true`，或把该 sink 拆到独立 `sink_group`。

### 与拆组的关系

同一组内的 sink 是串行下发的；`drop_when_error = true` 通过熔断避开了坏链路的每批重试，但单次失败（首次或探测）仍会占用一次 connector 重试预算。
如果单次失败本身就非常慢，仍建议把该 sink 拆到独立的 `sink_group`。

## 说明
- 标识规则
  - 组名：sink_group.name（例如 /sink/example/simple）
  - sink 名：name（组内唯一；未显式提供时按索引回退为 [0]/[1]/…）
- 元信息输出控制（wp_meta_disable）
  - `wp_meta_disable` 只能配置在 `[sink_group]`，作用于该组内所有 sinks。
  - 当前支持的字段：`wp_oml_name`、`wp_event_md5`，用于关闭 JSON/CSV 等结构化文本 payload 中的 OML 名称字段输出。
  - 该配置不会关闭 Arrow framed 的 frame header tag；Arrow tag 仍由 OML `name` 传递给 sink 后写入 frame header。
  - 不要把 `wp_meta_disable` 放到 `[[sink_group.sinks]].params` 或 `wpgen.output.params`；这些位置会被校验拒绝，避免运行时控制字段泄漏给 connector params。
- 过滤语义（filter）
  - filter 是“拦截条件”：表达式求值为 true 时，该条数据不写入该 sink，而是转发到基础组 intercept（framework/intercept）
  - 每个 sink 可独立设置 filter；与 expect 相互独立




## 校验提示
- 分母决定：
  - basis = total_input：总输入
  - basis = group_input：该组各 sink 行数之和（或 stats 中该组输入）
  - basis = model：按模型粒度统计（目前以组内 sinks 行数之和替代）
- min_samples：当分母为 0 或小于该值时，组校验被忽略（打印提示，不 fail）
- 当 route 为非文件类写入 fmt 时，validate 会提示“fmt 由后端决定，已忽略”。

## 常见排错
- 连接器未找到：检查 connectors/sink.d 是否存在对应 id；`wproj sinks list` 可查看引用关系
- 覆盖参数不生效：检查 allow_override 白名单
- 遇到 `${VAR}` 未替换：先检查上层程序是否已注入变量字典，再检查变量名拼写和 `sec_key.toml` / 环境变量内容
- filter 未生效：
  - 路径解析相对当前工作目录（建议写相对 sink_root 的相对路径）
  - 日志中会打印“found path/not found filter …”
  - 表达式语法需通过 TCondParser；可先用简单表达式试验
