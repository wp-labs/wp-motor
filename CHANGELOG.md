# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.25.6 latest]

### Fixed
- **加载错误提示过泛**：修复引擎启动（`load-engine-res`）阶段 OML/配置解析失败时控制台仅显示 "配置错误"、无具体原因的问题——`conv_err()` 仅按 reason 转换（`From<XReason> for RunReason`），把 `Syntax/NotFound/Other` 内层详情压缩为无 detail 的配置错误。新增 `wp-error` 的 `IntoRunError` 转换（`OMLCodeError`/`ConfError`/`OrionConfError`），提取内层消息为 detail 并保留 source；OML 加载（`loading.rs`）与配置加载（`warp_helpers.rs`）改用该机制，错误提示现定位到具体文件、解析位置与 toml 语法错误

## [1.25.5] - 2026-08-10

### Fixed
- **OML 嵌套 object 成员静默丢弃**：修复嵌套 `object { ... }` 的成员解析失败时，该成员及其后的兄弟字段被静默丢弃、但模型加载仍报成功的问题。`oml_map()` 解析后不再假设 `repeat` 已消费全部 body，剩余未解析内容将作为明确语法错误返回（非法成员使整个 OML 校验失败，不再静默加载部分对象）；`oml_sub_acq` 新增 `pipe`（含省略前缀）分支，`NestedAccessor` 新增 `Pipe` 变体，嵌套 object 成员支持 `pipe read(...) | fun`；目标列表解析容忍逗号前后空白
- **OML read/take 参数静默丢弃**：同类问题——`read(...)`/`take(...)` 括号内非法参数（如 `read(option : [x] @@garbage@@)`）此前被 `repeat(0.., oml_args)` 静默忽略、加载仍成功；现确认括号内已被完整消费，存在剩余内容时整体 OML 校验失败

### Tests
- **wp-oml issue #348 回归**：新增 28 个用例——真实报告结构（`category` 后的 `rule` 不丢弃 `behavior/confidence/attacker`）、pipe 成员任意位置（首/中/末）、省略前缀 `read(x) | fun`、`take` 源 pipe、同对象多 pipe 成员、数组元素内嵌套 object 的 pipe、深层混合嵌套、`get` 管道取前序对象输出、非法成员在首/中/末位置均整体失败、static 块拒绝 pipe、多目标逗号前后空白、`read()`/`take()` 非法参数整体失败（垃圾参数在开头/中间/末尾、pipe 源内、全部垃圾、object 成员内；合法边界形式尾逗号/尾空白/多参数/json path 不受影响）、Display 往返

## [1.25.4] - 2026-08-08

### Added
- **OML/Time 时间戳函数**：新增 `Time::from_ts`/`Time::from_ts_ms`/`Time::from_ts_us`（秒/毫秒/微秒时间戳 → 时间），与 `Time::to_ts`/`to_ts_ms`/`to_ts_us` 互为逆操作；六个函数的 `zone` 参数可选、默认东8区（正东负西，0 = UTC），超 i32 范围或 `|zone| > 23`（超出 `FixedOffset` 上限）在解析期报错，zone 值非法（超范围/溢出）时原样透传

### Fixed
- **OML/Time 时间戳 zone 溢出**：`|zone|` 极大（> 596523）时 `zone * 3600` 溢出 i32，debug 构建（`cargo test`/引擎 debug）直接 panic；改用 `saturating_mul` 饱和后由 `FixedOffset::east_opt` 返回 `None` → 无效 zone 透传，避免用户 OML 源码触发崩溃

### Docs
- **OML/Time 时间戳函数文档**：函数参考（zh/en）与 pipe_functions 补充六个时间戳函数的 `zone` 语法与示例；`to_ts_zone` 标注与 `to_ts(zone)`/`to_ts_ms(zone)`/`to_ts_us(zone)` 等价；说明「互逆需使用相同 zone」

### Tests
- **wp-oml Time 时间戳函数测试**：新增 33 个用例（六函数求值、无参/正/负 zone、边界 ±23、越界/溢出拒绝、同 zone 往返互逆、跨 zone 不对称、负时间戳、非匹配类型/越界时间戳透传、Display 往返、错误消息含具体原因）

## [1.25.3] - 2026-08-05

### Added
- **OML 嵌套对象**：`object { ... }` 子项值支持嵌套 `object { ... }` 字面量，可将平铺字段组织成多层 JSON；嵌套对象可任意深度，并与 `array` 互相嵌套
- **OML 对象数组**：新增 `array { ... }` 聚合表达式，直接构造对象/值字面量数组（元素支持 `object { ... }`、嵌套 `array`、`read/take`、值、函数等表达式）；元素缺失或为 null 自动跳过，全部缺失不输出该字段
- **static 块**：静态块支持嵌套对象/数组字面量（`ensure_static_*` 校验与静态符号重写覆盖 `NestedAccessor::Map`/`ObjArray`）

### Fixed
- **引擎启动加载失败（exit 300）**：修复嵌套 object 在 `load-engine-res` 阶段解析失败的问题——`wpadm check --what oml` 只校验文件非空，真实解析发生在引擎启动时；此前 `oml_sub_acq` 不接受 `object` 作为子项值，且 `array` 关键字未注册

### Docs
- **OML 文档**：语法参考（zh/en）补充 `array_expr` 与嵌套 `object_value`；核心概念、实战指南、完整示例新增对象数组与嵌套对象示例

### Tests
- **wp-oml 集成测试**：新增 15 个嵌套对象/数组用例（解析、求值形状、JSON 端到端输出、深层混合嵌套、缺失元素跳过、数组套数组、类型声明、get 管道、static 块、Display 往返、空数组、省略分号、文档示例）

## [1.25.2] - 2026-08-05

### Changed
- **OML 内网富化输出**：`intranet_ip` 输出改为英文简写 `LAN`/`WAN`，`access_direct` 方向改为 `L2L`/`L2W`/`W2L`/`W2W`（L=LAN、W=WAN、2=to）
- **Docs**：富化函数与内网网段配置文档（zh/en）同步输出值

## [1.25.1] - 2026-08-05

### Added
- **OML 内网富化**：新增 `intranet_ip`（判内/外）、`access_direct`（访问方向）、`on_fail`（失败兜底）函数；管道源扩展（`PipeSource`）支持 `access_direct(a,b) | on_fail('x')`
- **内网网段知识管理**：内网网段作为知识统一由 wp-knowledge 管理（`knowdb.toml [intranet_nets]` 节，随 knowdb 加载注入）；默认 RFC1918 + IPv4/IPv6 loopback + IPv6 ULA，可配置扩展；项目初始化自动生成该节；`wproj check` 增加校验项；`is_intranet` 按 IPv4/IPv6 地址族分桶优化

## [1.25.0] - 2026-08-04

### Added
- **OML IP 编码**：新增 `ip_to_biguint`（IPv4/IPv6 统一编码为任意精度整数），`FieldQueryCache`/`compare_datafield` 支持 `BigUint`
- **Docs**：OML 语法参考补充 `//` 注释说明；IP 地理位置查询示例改用 `ip_to_biguint`

### Changed
- **Dependencies**：升级 `wp-knowledge` 0.14→0.15、`wp-model-core` 0.8→0.9、`wp-lang` 0.4→0.5、`wp-error` 0.10→0.11 等一批依赖


## [1.23.8] - 2026-07-31

### Added
- **Parser/Event meta**: New `wp_event_md5` field (MD5 of the event payload) stamped on every event's records, gated by the `gen_event_md5` config flag (default off, nested under `gen_msg_id`). Stamped on both main records and `copy_event_parse` side records. `wp_event_md5` is added to `SUPPORTED_WP_META_DISABLE_FIELDS` so sink groups can drop it via `wp_meta_disable`.
  中文：新增 `wp_event_md5` 字段（事件 payload 的 MD5），由 `gen_event_md5` 配置项控制（默认关，嵌在 `gen_msg_id` 下），盖在主 record 与 `copy_event_parse` 旁路 record 上；并加入 `wp_meta_disable` 白名单，sink 组可按需不输出。
- **Parser/copy_event_parse side-record routing**: `copy_event_parse` now emits the target rule's parsed record as an independent side record routed under the target's `wpl_key` to its own sink (previously merged into the main record). Side records carry the same event meta. A global parser map resolves cross-package (`pkg/rule`) and same-package bare-name references; bare names are normalized to full paths so side records route correctly.
  中文：`copy_event_parse` 改为产出独立旁路 record，按目标 `wpl_key` 路由到自己的 sink（原为并入主 record）；旁路 record 同样盖事件 meta。全局 parser map 解析跨包（`pkg/rule`）与同包裸名引用，裸名规范化为全路径以正确路由。
- **Parser/`#[no_match]` assembly**: `#[no_match]` rules are built as pipelines with `auto_match=false` — excluded from `parse_event` auto-matching but keeping sink bindings, so `copy_event_parse` side records can route through the target's pipeline. (The `#[no_match]` annotation itself is defined in wp-lang.)
  中文：`#[no_match]` rule 装配为 `auto_match=false` 的 pipeline——不参与 `parse_event` 自动匹配，但保留 sink 绑定，供 `copy_event_parse` 旁路 record 经目标 pipeline 路由。（`#[no_match]` 注解本身在 wp-lang 定义。）

### Fixed
- **Clippy**: Fixed `for_kv_map` (wp-config) and `question_mark` (wp-oml) lints for `-D warnings` CI.
  中文：修复 `for_kv_map`（wp-config）与 `question_mark`（wp-oml）clippy 告警，满足 `-D warnings` CI。

## [1.23.7] - 2026-07-12

### Added
- **Sink/BatchMeta**: Sink runtime now passes OML output names and group-level metadata output policy to connectors through `BatchMeta.oml_name`, `BatchMeta.output_disabled`, and `sink_records_with_meta`, enabling Arrow framed sinks to use the OML `name` as the frame tag without requiring a fixed connector `tag`.
  中文：sink runtime 现在通过 `BatchMeta.oml_name`、`BatchMeta.output_disabled` 和 `sink_records_with_meta` 将 OML 输出名与组级元信息输出策略传给 connector，使 Arrow framed sink 可直接使用 OML `name` 作为 frame tag，无需固定配置 connector `tag`。

### Changed
- **Dependencies**: Upgraded `wp-connector-api` to `0.11`, `wp-core-connectors` to `0.7`, `wp-lang` to `0.4`, and added `wp-source-types` `0.1` for the updated connector metadata contract.
  中文：升级 `wp-connector-api` 到 `0.11`、`wp-core-connectors` 到 `0.7`、`wp-lang` 到 `0.4`，并新增 `wp-source-types` `0.1`，用于新的 connector 元信息契约。
- **Sink/Metadata**: Removed engine-side payload injection for `wp_stream_tag` and `wp_event_id`. Metadata rendering is now sink-owned; the engine only passes batch-level OML metadata.
  中文：移除引擎侧对 `wp_stream_tag` 与 `wp_event_id` 的 payload 注入；元信息如何输出改由 sink 自己实现，引擎只传递批次级 OML 元信息。
- **Config/Sinks**: `wp_meta_disable` is a `sink_group`-level metadata output policy for `wp_oml_name` and is rejected from sink/wpgen output params; `stream_tag_field` remains source-only.
  中文：`wp_meta_disable` 是 `sink_group` 级 `wp_oml_name` 元信息输出策略，sink/wpgen output 参数中会报错；`stream_tag_field` 仍只属于 source 配置。
- **Sink/OML**: Successful OML output records now carry the OML model `name` as internal `ProcMeta::OmlName`, so batch sinks receive the logical output name rather than the original WPL rule key.
  中文：OML 成功输出记录现在以内部 `ProcMeta::OmlName` 携带 OML 模型 `name`，批量 sink 接收到的是逻辑输出名，而不是原始 WPL rule key。
- **Sink/Runtime**: Renamed internal `ProcMeta::Rule` to `ProcMeta::WplName` to make the WPL/OML distinction explicit.
  中文：内部 `ProcMeta::Rule` 改名为 `ProcMeta::WplName`，明确区分 WPL 名称与 OML 输出名称。

### Removed
- **Benchmarks**: Removed the obsolete `sink_wp_meta` benchmark now that engine-side runtime metadata payload injection has been removed.
  中文：移除过时的 `sink_wp_meta` 基准；引擎侧运行时元信息 payload 注入逻辑已删除。

### Tests
- **Config/Sinks**: Added coverage for source-side `stream_tag_field` pass-through, group-level `wp_meta_disable`, and sink/wpgen output rejection.
  中文：补充 source 侧 `stream_tag_field` 放行、组级 `wp_meta_disable`，以及 sink/wpgen output 侧拒绝的测试。
- **Sink/Runtime**: Added coverage for `BatchMeta.oml_name` and `BatchMeta.output_disabled` dispatch, pending-buffer flushes when OML names change, and WPL rule passthrough without OML batch metadata.
  中文：补充 `BatchMeta.oml_name` 与 `BatchMeta.output_disabled` 下发、pending buffer 遇到 OML name 变化时先 flush，以及普通 WPL rule 透传不生成 OML 批次元信息的测试。
- **Parser/Annotations**: Filtered `AnnotationType::Null` before parser pipeline annotation execution.
  中文：parser pipeline 执行 annotation 前过滤 `AnnotationType::Null`。

## [1.23.5] - 2026-07-06

### Added
- **wpgen/Config**: `wpgen.toml` 新增 `[models]` 段，支持 `wpl` 字段指定 WPL 规则/样本目录，与 `wparse.toml` 的 `[models].wpl` 保持一致。配置优先级：`--wpl` CLI > `[models].wpl` > 默认 `./models/wpl/`。
- **wpgen/Validate**: `[models].wpl` 目录不存在或无 `.dat`/`.wpl` 文件时，启动阶段报错退出（不再静默 `found 0 files`）。

### Changed
- **Sink/Factories**: `register_builtin_factories()` 启动时通过 `info_ctrl!` 打印已注册的 factory 列表（BlackHole, File, Syslog, Tcp, TestRescue）。

### Tests
- **wpgen/Config**: 新增 `wpgen_config_models_wpl_parsed`、`wpgen_config_models_default_none` 两例测试 `ModelsConfig` 解析。

## [1.23.4] - 2026-07-05

### Changed
- **Dependencies**: 升级 `shadow-rs` 1.5 → 2.0，升级 `wp-core-connectors` 0.3.3 → 0.5。
- **Generator/Sink**: 合并 hotfix/1.22 的 `BatchSizePolicy`——动态字节预算批量下发。预算 = `base_rate(EPS) × avg_line_bytes(EMA) × time_window(100ms)`，clamp 到 [8KiB, 1MiB]。TCP sink 场景下 `wpgen` CPU 从 ~300% 降至 ~15%。
### Fixed
- **Project Init**: 修复 `wproj init` 生成的 infra route 模板包含无效的 `version = "2.0"` header 和已废弃的 `file_proto_sink` connector 引用（改为 `file_proto_text_sink`）。`init` 阶段新增自动检测旧格式文件并覆写的逻辑。
- **wpadm/Cli**: `wpadm data stat/validate/check` 及 `wpadm sources list/route` 硬编码了 `wpsrc.toml` 路径，不支持目录式 source 格式（每个 source 一个 `.toml` 文件）。现改为在 `wpsrc.toml` 不存在时自动扫描 `topology/sources/` 目录加载 source 配置。
- **Clippy**: 修复 `collapsible_if` 和 `unused_imports` 警告。

### Tests
- **Generator**: 新增 `BatchSizePolicy` 单元测试 6 例及速率×行长耦合集成测试 2 例，生成器测试 84 → 92。

## [1.23.0]

### Added
- **Knowledge/Redis**: 升级 `wp-knowledge` 至 0.14.0，新增 Redis 外部数据源支持。knowdb.toml 新增 `[provider.redis]` 配置段，支持 `GET`、`HGET`、`BF.EXISTS`、`SISMEMBER` 等命令，适用于弱口令 Bloom filter、威胁情报 IP 查表等高速查表场景。

### Changed
- **Knowledge/Provider**: knowdb.toml 的 `[provider]` 拆分为 `[provider.sqldb]` 和 `[provider.redis]`，旧格式自动兼容并提示迁移。

### Removed
- **Sinks/Arrow**: 移除独立的 `arrow-file` 和 `arrow-ipc` sink 后端（`ArrowFileFactory` / `ArrowIpcFactory`）。Arrow 输出功能已统一到 file/tcp sink 中，通过 `protocol = "arrow"` 参数使用。

### Fixed
- **OML/Pipe/ip4_to_int**: 修复 `ip4_to_int` 对 IPv6 地址静默透传的问题，现改为返回 Null；新增对字符串 IPv4 地址的解析支持。

## [1.22.10 Unreleased]

### Changed
- **Memory Profile**: Changed the unset `WP_MEMORY_PROFILE` default back to `standard`. The `standard` profile keeps the `low` sink, pending, UDP, and file memory watermarks while increasing parser/source-side headroom.
  中文：未设置 `WP_MEMORY_PROFILE` 时默认回到 `standard`；`standard` 保持 low 的 sink、pending、UDP 和 file 内存水位，同时提高 parser/source 侧余量。
- **TCP Source Throughput**: Raised `standard` profile TCP defaults to `WP_TCP_RECV_BYTES=2097152`, `WP_TCP_BATCH_BYTES=262144`, and `WP_TCP_BATCH_CAPACITY=256` to keep long-line TCP samples such as firewall better fed without requiring the full `throughput` profile.
  中文：提升 `standard` profile 的 TCP 默认批量参数，以改善 firewall 等长行 TCP 样本的 parser 供给和 CPU 利用率，无需切到完整 throughput profile。
- **Docs**: Updated source rate limit and wparse configuration docs to describe `standard` as the default profile and include the new TCP batch defaults.
  中文：更新输入限速和 wparse 配置文档，说明 `standard` 为默认 profile，并补充新的 TCP 批量默认值。

### Tests
- **Config/Limits**: Updated memory profile tests to cover the new unset default and `standard` TCP batch values.
  中文：更新 memory profile 测试，覆盖新的未设置默认值和 `standard` TCP 批量参数。

## [1.22.9] - 2026-06-25

### Added
- **Benchmarks**: Added `sink_batch_ids_success_path` under `perf-ci` to measure sink-side record-id plumbing on the success path and compare package collection with/without ids.
  中文：新增 `sink_batch_ids_success_path` 基准，用于对比 sink 成功路径上 record id 传递开销。

### Changed
- **Sink Runtime**: Simplified batch error logging in `send_records_batch` by iterating record positions directly instead of prebuilding a `Vec<u64>`.
  中文：`send_records_batch` 的错误日志改为直接按下标遍历，不再预分配 `Vec<u64>`。

## [1.22.8] - 2026-06-23

### Changed
- **Memory Profile**: Changed the default `WP_MEMORY_PROFILE` from `standard` to `low`, so unset deployments now use smaller parser/sink channels, smaller picker pending byte caps, and earlier memory backpressure by default.
  中文：默认内存 profile 从 `standard` 改为 `low`；未显式设置时默认使用更小队列和更早背压，优先控制 RSS。
- **Source Rate Limit**: Fixed-rate source limiting now bypasses picker pending-count pull watermarks while still respecting the profile-defined `WP_PICKER_PENDING_MAX_BYTES` hard cap.
  中文：固定限速不再被 picker pending 批数水位额外压低，但仍受 profile 定义的 `WP_PICKER_PENDING_MAX_BYTES` 内存上限保护。

### Tests
- **Runtime/Pickers**: Added coverage for fixed-rate pull planning and profile pending-byte cap behavior.
  中文：补充固定限速拉取计划与 profile pending byte cap 行为测试。

## [1.22.7] - 2026-06-23

### Added
- **Source Rate Limit**: Added a shared source-side global rate limiter. `performance.rate_limit_rps = 0` now enables automatic source input rate control, while `> 0` applies a fixed global EPS cap across all sources in the runtime.
  中文：新增 source 侧全局输入限速；`rate_limit_rps = 0` 表示自动限速，`> 0` 表示所有 source 共享固定 EPS 上限。
- **Auto Rate Limit**: Added AIMD-style automatic input control based on picker pending watermarks, parser backpressure, and RSS growth protection.
  中文：新增自动输入调节，根据 pending 水位、parser 背压和 RSS 快速增长保护动态升降速。
- **Memory Profiles**: Added centralized runtime memory profiles via `WP_MEMORY_PROFILE=standard|low|throughput`, with compatibility aliases for older `small/tiny/large` names.
  中文：新增统一内存 profile，收敛 parser/sink channel、batch、pending、TCP/UDP/file buffer 等内存相关参数。
- **Docs**: Added `docs/design/source_rate_limit_design.md` and updated wparse config docs with source rate limit and memory profile guidance.

### Changed
- **Runtime Defaults**: Default `performance.rate_limit_rps` changed from fixed `10000` to `0` automatic mode. `EngineConfig::init()` now uses `PerformanceConf::default()` so generated configs follow the same defaults as serde-loaded configs.
  中文：默认输入限速改为自动模式；初始化生成配置与正式加载配置使用同一默认值入口。
- **Picker/Backpressure**: Source rate lease consumption now happens before batches enter picker pending, reducing pending/RSS growth under rate limiting.
  中文：source 限速等待前移到进入 pending 之前，减少限速场景下 pending/RSS 先膨胀。
- **Runtime Buffers**: Parser/sink channels, picker burst/coalesce thresholds, pending byte cap, TCP/UDP/file batch buffers, sink record pools, debug view queue, and command channel now use centralized memory limits.
  中文：运行时队列、水位和批大小统一由 `wp_conf::limits` 管理。
- **Benchmark**: Benchmark `wparse.toml` files now use `${RATE_LIMIT_RPS:0}`; benchmark scripts derive `RATE_LIMIT_RPS` from the speed argument unless explicitly set externally.
  中文：benchmark 入口默认用输入速率同步设置 wparse 限速；传 `0` 可测试自动限速。
- **DebugView**: Debug output now uses a bounded channel to avoid unbounded RSS growth; queue overflow records dropped-line counts with sampled warnings instead of silently ignoring pressure.
  中文：DebugView 改为有界队列，队列满时记录丢弃计数并抽样告警。

### Fixed
- **Config Defaults**: Fixed generated `conf/wparse.toml` initialization bypassing profile-aware performance defaults.
- **Tests/Docs**: Converted speed profile doctests from ignored examples to compiling doctests and cleaned obsolete source-rate-limit tuning history from docs.


## [1.22.3] - 2026-05-19

### Added
- **SQL/Route**: SQL 查询按表名路由到本地 SQLite 或外部 Provider——支持配置 `knowdb.toml` 的 `[[tables]]` 和 `[provider.tables]`，解析 SQL 时自动识别 `FROM` 子句中的表名并分发查询。
- **SQL/Route**: 新增 `SqlKnowledgeRoute` 枚举（`Provider` / `Sqlite` / `Unknown`）和 `resolve_sql_route()` 路由解析函数，支持子查询、引号内关键字跳过。
- **SQL/Parser**: `sanitize_sql_body` 支持子查询和别名语法（`FROM (子查询) AS alias`）。
- **KnowDB/Config**: 新增 `uses_external_provider_only()` 判定，纯外部 provider 配置不再删除本地 authority 文件。

### Changed
- **Dependencies**: Bumped workspace version to 1.22.3.

## [1.22.2] - 2026-05-13

### Added
- **Sinks/Sync**: 为 `SinkTerminal` 实现 `send_to_sink_batch` 和 `try_send_to_sink_batch` 批量写入方法，降低统计切片过多造成的反压。

## [1.22.1] - 2026-05-12

### Fixed
- **OML/SQL**: 当 SQL 参数全部为 Null 时跳过实际查询，避免对空参数的不必要远程调用。
- **OML/Extract**: `SingleEvalExp` 提取字段时跳过 `Value::Null`，不再为 Null 值创建目标字段。
- **OML/SQL**: 修复 `#90` `#91` 知识库查询相关 bug。

### Changed
- **Knowledge Base**: 知识库查询优化。
- **Dependencies**: Bumped workspace version to 1.22.1.

## [1.22.0] - 2026-05-08

### Added
- **Diagnostics/CLI**: Error hints are now driven by `stable_code` (from `#[derive(OrionError)]`) as primary key, with bilingual Chinese/English support; language is selected via `WP_LANG` environment variable (fallback to `LANG` then `LC_ALL`).
  中文：错误提示改为以 `stable_code` 为主键索引，支持中英双语；通过 `WP_LANG` 环境变量切换。
- **CLI/Help**: Added `after_long_help` documenting `WP_LANG` and `NO_COLOR` environment variables.
  中文：在 CLI `--help` 中添加 `WP_LANG` 和 `NO_COLOR` 环境变量说明。
- **Config/Engine**: Added `RepoGroupConf` for repository group configuration support.
  中文：新增 `RepoGroupConf` 支持仓库组配置。

### Changed
- **Dependencies**: Upgraded `orion-error` from 0.6 to 0.8, adapting to the new `#[derive(OrionError)]` derive macro and updated trait paths.
  中文：升级 `orion-error` 0.6 → 0.8，适配新的 derive 宏和 trait 路径。
- **Dependencies**: Unified `wp-lang` on `0.3.1` and refreshed `orion-error` compatibility.
- **Error Diagnostics**: Refactored `collect_hints` to use `stable_code` match branches (13 categories); improved nested error reason/detail/root_cause extraction.
  中文：重构 `collect_hints` 为 13 类 `stable_code` 匹配分支；改进嵌套错误详情提取。
- **Config Schema**: 配置解析开启 `deny_unknown_fields`，拼写错误的配置键将明确报错。
- **Error Handling**: 统一 observability、config loading、project management 等链路的错误转换风格，附带路径上下文。
- **Dependencies**: 升级工作区依赖（`jieba-rs 0.9`、`lru 0.17`、`ctor 0.10` 等）。

### Fixed
- **OML/SQL**: Fixed non-deterministic SQL parameter binding for multi-parameter `IN (...)` clauses — collect `:param` values in SQL placeholder order instead of `HashMap` iteration order.
  中文：修复 SQL `IN (...)` 参数绑定顺序不稳定问题，按占位符出现顺序绑定。
- **OML/SQL Cache**: Aligned SQL cache keys and query parameters to the same placeholder order for both sync and async evaluators.
  中文：同步/异步 SQL evaluator 缓存键与参数使用同一占位符顺序。
- **OML/SQL**: `take(field)` and `__temp_var` now correctly convert to `IN` bind parameters.
  中文：修复 `take(field)` 与临时变量在 `IN` 子句中的参数绑定。
- **OML/Take**: Fixed field move order when target and source records share field names — prioritize current target record's generated fields.
  中文：修复 `take(...)` 字段移动顺序，避免前序 OML 字段被源记录同名值覆盖。
- **OML/Take**: Fixed `take(...)` only consuming from source record; now supports consuming previously generated fields in the target record.
  中文：修复 `take(...)` 只能从源记录取值的问题，支持消费目标记录中已生成的字段。
- **OML/SQL Parser**: Extended strict SQL mode aggregation validation to support `string_agg(distinct field, ',')` and `group_concat(distinct ...)`.
  中文：扩展严格 SQL 模式聚合函数校验，支持 `string_agg(distinct ...)`、`group_concat(distinct ...)`。
- **OML/SQL Parser**: Support `IN (@sip, @dip)` and `in(@sip, @dip)` reference parameter syntax.
  中文：支持 `IN (@sip, @dip)` 等引用参数写法。
- **wp-proj/Load Semantics**: Restored `WarpProject::load()` to load existing projects only — missing `conf/wparse.toml` now fails instead of auto-creating.
  中文：恢复 `WarpProject::load()` 只加载已有工程的语义。
- **Runtime/Stats**: 修复统计切片过多导致的反压问题。
- **Error Handling/Config Loading**: 修复 `owe_conf_source` 在加载损坏 TOML 时触发 panic 的回归问题。
- **Config/Tests**: 修复 observability validate 测试与严格 config schema 的兼容性。

### Removed
- **Sinks/Rescue**: Removed unused `sink_err` helper method.
  中文：移除不再使用的 `sink_err` 辅助方法。

## [1.20.7] - 2026-04-26

### Changed
- **wpgen/Config Loading**: Unified `wpgen.toml` loading through `WpGenConfig::load_from_path`, keeping parse, environment expansion, and validation semantics consistent across runtime loading, project loading, and `wproj check`.
  中文：统一 `wpgen.toml` 加载入口到 `WpGenConfig::load_from_path`，确保运行期、项目加载和 `wproj check` 的解析、环境变量展开和校验语义一致。
- **wproj/check**: Added `wpgen` config checks and exposed semantic-dictionary empty words, duplicates, and empty categories as structured warnings instead of mixing them into the success message.
  中文：新增 `wpgen` 配置检查，并将语义词典空词、重复词和空类别改为结构化 warning，不再混入“配置有效”消息。
- **Validation/Warnings**: Downgraded missing source/sink input or output directories to non-blocking warnings so clean projects are not failed before runtime-created directories exist.
  中文：将 source/sink 输出或输入目录缺失调整为非阻断 warning，避免干净工程在目录尚未创建时被误判为配置失败。

### Fixed
- **wproj/check JSON**: Fixed warning paths that wrote directly to stdout and polluted `wproj check --json` output.
  中文：修复多个 check 路径直接写 stdout 导致 `wproj check --json` 输出被 warning 文本污染的问题。
- **wpgen/Schema**: Made missing `output.connect` invalid for `wpgen.toml`, and updated tests/examples to remove deprecated `mode` and `duration_secs` fields.
  中文：明确拒绝缺失 `output.connect` 的 `wpgen.toml`，并更新测试与示例配置，移除已废弃的 `mode` 和 `duration_secs` 字段。
- **OML/WPL Lint**: Made extra model semantic checks non-blocking lint so hand-written head parsing or empty-directory states do not override official parser results.
  中文：将额外模型语义检查调整为非阻断 lint，避免手写 head 解析或空目录状态覆盖官方 parser 的结果。
- **Tests/Temp Files**: Fixed the `wp-config` test that wrote temporary `framework.toml` under the source tree by using an isolated temp directory.
  中文：修复 `wp-config` 测试把临时 `framework.toml` 写入源码目录的问题，改用隔离临时目录。
- **Code Quality**: Cleaned clippy issues exposed by `-D warnings` in `wp-config`, `wp-cli-core`, and `wp-proj`.
  中文：清理 `wp-config`、`wp-cli-core` 和 `wp-proj` 在 `-D warnings` 下暴露的 clippy 问题。

## [1.20.6] - 2026-04-24

### Fixed
- **Error Handling/Structured Errors**: 修复多个配置与项目管理链路把 `StructError` 当作普通 source 再次挂接而触发 panic 的问题，相关路径现在按结构化错误方式转换并保留可读诊断信息。
- **Observability/KnowDB**: 修复 `wpsrc.toml` 统计、knowdb 配置解析与 source 构建等路径在遇到无效 TOML 或配置错误时可能 panic 的问题；现在会稳定返回结构化错误。
- **wp-proj/Config Loading**: 修复 `Knowledge` 错误转换、`load_warp_engine_confs()` 以及模型路径回退相关测试场景中的 panic，非法 `wparse.toml` 与缺失配置会按预期返回错误或走回退逻辑。

## [1.20.5] - 2026-04-24

### Fixed
- **Monitoring/Hot Reload**: 修复热加载后监控统计数据不再输出的问题；现在 engine reload 之后，统计与监控链路会继续正常产出数据，不会出现 reload 成功但统计面板长期无数据的情况。
- **Stats/Runtime**: 调整 monitor、service、recovery 与 rule/sample 生成相关链路，补齐热加载后统计任务继续运行所需的状态衔接。

### Changed
- **Code Quality**: 清理相关模块中的 clippy 告警，收敛统计与 dispatcher 附近的实现细节，不改变对外行为。

## [1.20.4] - 2026-04-19

### Added
- **Error Handling/Docs**: Add structured error-system design and review checklist documentation
- **wp-proj/Templates**: Add commented VictoriaLogs/VictoriaMetrics infra sink examples to generated route templates

### Changed
- **Error Handling**: Improve shared CLI diagnostics and preserve upstream source chains across config, source, sink, generator, recovery, monitor, and project-management boundaries
- **Config Loading**: Align source/sink/wpgen loading and validation with the unified loader contract, including env evaluation, path context, and structured validation details
- **Observability**: Return structured item-level diagnostics for source/sink stats and validation failures such as invalid connectors, disallowed overrides, unreadable files, and line-count errors

### Fixed
- **CLI/Error Output**: Fix terse `wpgen`, `wproj`, and `wprescue` configuration errors so they include actionable detail and source-chain context
- **Runtime**: Surface recovery checkpoint and monitor sink failures as structured errors instead of flattening or only logging them
- **Tests**: Update config, observability, source-stat, and knowledge tests to assert stable diagnostic semantics

## [1.20.3] - 2026-04-16

### Fixed
- **Runtime/Stats**: Fix backpressure caused by excessive statistical slicing

## [1.20.2] - 2026-04-16

### Changed
- **CI/GitHub Actions**: Enable the main CI workflow on `hotfix/*` branches so maintenance releases run the same workflow checks as the main release line

### Fixed
- **Config Schema/Tests**: Update generated sink defaults fixtures and test configs to match strict `deny_unknown_fields` schemas, removing invalid `version = "2.0"` headers from `defaults.toml`-style files
- **Observability Validation**: Fix observability validation tests to load sink defaults from schema-valid fixtures under the strict config layout

## [1.20.0] - 2026-04-11

### Added
- **Sinks/Arrow**: Add `arrow-file` sink for local length-prefixed Arrow IPC frame output, and add `arrow_file_sink` and `arrow_tcp_sink` templates to project initialization
- **OML/Functions**: Add `iequals_any(...)`, `lookup_nocase(dict, key, default)`, and `calc(...)` with `+ - * / %` and `abs/round/floor/ceil`
- **Runtime Control**: Add structured `LoadModel` runtime command handling with status snapshots and single-flight reload coordination for host/admin integration
- **Runtime Config**: Add `reload_timeout_ms`, available from CLI `--reload-timeout-ms` and `wparse.toml` `[performance].reload_timeout_ms`
- **Source/File**: Allow wildcard patterns in `file` while preserving matched file order
- **Monitoring/Stats**: Add fixed-label propagation across the runtime metrics pipeline and expose `wp-knowledge` reload/cache/query telemetry through `wp-stats`

### Changed
- **Runtime/Reload Flow**: Replace fixed-wait reload with event-driven drain coordination and return structured reload results for runtime/admin control flows
- **Source/File Contract**: Standardize runtime file-source configuration on `base + file`, keep wildcard support limited to `file`, and process matched files sequentially
- **Admin API/Auth**: Move the default bearer token location to `${HOME}/.warp_parse/admin_api.token`, with work-root fallback when `HOME` is unavailable
- **Config/Knowledge**: Add `[models].knowledge` as the configurable root for knowdb and semantic dictionary files while preserving legacy semantic dictionary fallback
- **OML/Async**: Switch OML model loading and async transform execution onto the async evaluator path consistently
- **Runtime/Backpressure**: Lower default parser and sink channel capacities to `128` and `64` so backpressure applies earlier under sustained load
- **Monitoring/Tags**: Standardize metric tag naming across pipeline, sources, sinks, and stats collectors
- **wp-proj/Demo Template**: Update generated demo source configuration from `gen.dat` to `gen*.dat` so project templates match both single-file and sharded wpgen output

### Fixed
- **Runtime/Reload Stability**: Prevent reload and batch shutdown from hanging by draining only started workers, cleaning up detached old processing tails, and disconnecting parse routing correctly during shutdown
- **Realtime/TCP Memory**: Bound realtime picker backlog by bytes and reduce TCP source-side pending buffering to curb memory growth under high EPS input
- **Admin API/Auth Loading**: Apply the standard `env_eval + conf_absolutize` pipeline to all engine-config loading paths so `${HOME}` in `admin_api.auth.token_file` resolves correctly
- **Source/File Validation**: Align source statistics and total-input calculation with the runtime-only `base + file` contract, preserve `Some(0)` for empty files, and fail explicitly on wildcard no-match or unreadable files
- **wpgen/Clean**: Make `wpgen data clean` remove sharded outputs such as `gen-r*.dat`
- **wp-proj/Bootstrap**: Ensure bootstrap directories are created before writing config files and fix generated OML examples so initialization no longer leaves merge artifacts or intermittent save failures
- **Sinks/Runtime**: Fix sink runtime behavior around disconnect handling, raw input validation, output path resolution, and duplicate factory registration
- **OML/Static**: Reject `static { ... }` blocks that reference other static symbols during parsing
- **OML/Diagnostics**: Make `oml-diag` collection task-aware under async execution
- **OML/Calc**: Normalize invalid arithmetic cases in `calc(...)` to `ignore`, including integer overflow, non-finite floats, and large-integer rounding edge cases

## [1.18.3] - 2026-03-16

### Changed
- **Event ID/Runtime**: Switch `wp_event_id` generation to the shared `wp-model-core::event_id::next_wp_event_id()` implementation so all sources use one unified generator

### Fixed
- **Event ID/Restart**: Prevent `wp_event_id` from falling back to a process-local fixed seed path after restart, avoiding duplicate IDs in Docker and other time-fragile runtime environments

## [1.18.2] - 2026-03-14

### Fixed
- **wp-lang/kv+kvarr**: Fix WPL engine/runtime parsing for keys containing `()`, `[]`, `<>`, and `{}` (for example `protocal(80)`, `arr[0]`, `list<int>`, `set{a}`)
- **wp-lang/ref-path**: Fix `@...` field reference path parsing so bracket-style keys are accepted without consuming outer WPL syntax delimiters

## [1.18.1] - 2026-03-09

### Changed
- **Semantic Dict/Loader**: Switch external dictionary discovery to default path probing (`models/knowledge/semantic_dict.toml` then `knowledge/semantic_dict.toml`) and support work-root path injection from engine startup
- **Semantic Dict/Config**: Add `enabled` switch for external dictionary config (also accepts legacy `enable` key) so external merge can be disabled while keeping builtin dictionary
- **Observability**: Add startup logs for semantic analysis toggle and semantic dictionary load status
- **Documentation**: Update Chinese/English semantic dictionary docs to reflect default-path loading and `enabled` usage

### Fixed
- **wp-proj/check**: Resolve semantic dictionary config under target `work_root` during project checks instead of relying on process environment
- **Semantic Dict/Validation**: Treat missing auto-detected external config as builtin fallback and skip validation success output when external config is explicitly disabled

## [1.18.0 Unreleased]

### Changed
- **Error Handling/Deps**: Complete workspace migration to `orion-error 0.6`/`orion_conf 0.5` API surface
  - Replace legacy `Uvs*From` traits with `UvsFrom`
  - Update `from_validation/from_conf/from_logic` call patterns and structured error detail attachment
  - Align `UvsReason` matching with 0.6 enum shape and update `RawData` imports to new public path
- **wp-proj/Runtime**: Refactor error conversion flow to `owe/want` style and align generators/loaders with unified error construction

### Fixed
- **Build**: Fix upgrade-induced compile breaks across `wp-config`, `wp-cli-core`, `wp-proj`, `wp-oml`, and `wp-engine` after dependency bump
- **Tests**: Repair integration/runtime test paths impacted by error API migration

## [1.17.8 ]

### Fixed
- **wp-lang**: Fix WPL engine/runtime parsing for `kv` and `kvarr` keys containing `()`, `[]`, `<>`, and `{}` (for example `protocal(80)`, `arr[0]`, `list<int>`, `set{a}`)
- **wp-lang**: Fix `@...` field reference path parsing to support bracket-style keys without consuming outer WPL syntax delimiters

## [1.17.6] - 2026-03-02

### Changed
- **Stats**: Refine `metric_set` merge logic and simplify conditional flow

## [1.17.5] - 2026-02-27

### Changed
- **Documentation/OML**: Update OML grammar docs

### Fixed
- **Sinks/Buffer**: Fix `batch_size` behavior in sink batch path

## [1.17.4] - 2026-02-18

### Added
- **Sinks/Config**: Add `batch_size` configuration to sink groups

### Changed
- **Sinks/Runtime**: Read and apply `batch_size` directly from `sink_group` configuration


## [1.17.3] - 2026-02-16

### Added
- **Sinks/Buffer**: Add sink-level batch buffer with configurable `batch_size` parameter
  - Small packages (< batch_size) enter pending buffer, flushed periodically or when buffer is full
  - Large packages (>= batch_size) automatically bypass pending buffer for reduced overhead (zero-copy direct path)
  - New `flush()` public API for manual buffer flush
- **Sinks/Config**: Add `batch_timeout_ms` configuration to sink group (default 300ms), controls periodic buffer flush interval

### Changed
- **Sinks/File**: Remove `BufWriter` and `proc_cnt` periodic flush from `AsyncFileSink`, write directly to `tokio::fs::File`; upstream batch assembly makes userspace buffering redundant

### Fixed
- **wp-oml**: Fix llvm-cov warnings in parser and test modules

## [1.17.2 ] - 2026-02-13
### Changed
- **wp-lang**: `kv`/`kvarr` key 解析支持括号类字符 `()`、`<>`、`[]`、`{}`，新增专用 `take_kv_key` 函数，不影响 WPL 语法层面其他模块的 key 解析

## [1.17.0 ] - 2026-02-12


### Added
- **OML Match**: Add OR condition syntax `cond1 | cond2 | ...` for match expressions
  - Supports single-source and multi-source match
  - Compatible with both value matching and function matching
- **OML NLP**: Add `extract_main_word` and `extract_subject_object` pipe functions for Chinese text analysis
- **OML NLP**: Add configurable NLP dictionary system, support custom dictionary via `NLP_DICT_CONFIG` environment variable
- **Engine Config**: Add `[semantic]` section in `wparse.toml` to control NLP semantic dictionary loading (`enabled = false` by default, saves ~20MB memory when disabled)

### Changed
- **OML Match**: Multi-source match now supports any number of source fields (no longer limited to 2/3/4)
- **Documentation**: Update OML documentation (Chinese and English) for match OR syntax and multi-source support


## [1.16.2] - 2026-02-11

### Fixed
- **wp-lang**: Fix kvarr pattern separator parsing


## [1.16.1] - 2026-02-11

### Changed
- **wp-lang**: Extend separator pattern syntax with `\S` and `\H` matchers


## [1.16.0] - 2026-02-11

### Added
- **wp-lang**: Add separator pattern syntax `{…}` with wildcards (`*`, `?`), whitespace matchers (`\s`, `\h`, `\S`, `\H`) and preserve groups `(…)` for expressing complex separator logic in a single declaration


## [1.15.5] - 2026-02-10

### Changed
- **wp-oml**: Enhanced FieldRead with zero-copy FieldStorage preservation


## [1.15.4] - 2026-02-10

### Added
- **wp-oml**: Add zero-copy validation test suite and lint tool
- **Documentation**: Add zero-copy implementation guidelines

### Changed
- **wp-oml**: Refactor FieldExtractor trait to require explicit extract_storage implementation
- **wp-oml**: Enhanced zero-copy support across MapOperation, RecordOperation, PiPeOperation, FmtOperation, SqlQuery, and FieldRead

### Fixed
- **wp-oml**: Fix MatchOperation to preserve zero-copy for Arc variants in match branches


## [1.15.3] - 2026-02-09

### Added
- **WP-OML Batch Processing**: Add record-level batch processing API to DataTransformer trait
  - New methods: `transform_batch()` and `transform_batch_ref()` for processing Vec<DataRecord>
  - Default implementation provides backward compatibility (processes records one by one)
  - Optimized ObjModel implementation reuses FieldQueryCache across all records
  - Performance improvement: 12-17% faster when compared to creating fresh cache per record
    - 100 records: 42.6µs → 37.3µs (12.4% faster with shared cache)
    - 10 records: 4.45µs → 3.76µs (15.5% faster with shared cache)
  - Additional 5% improvement in multi-stage pipelines with 100+ records
  - Provides standardized batch API to prevent cache misuse patterns

### Changed
- **Dependencies**: Upgrade wp-model-core 0.8.3 → 0.8.4
  - Introduces FieldRef<'a> wrapper type for zero-copy, cur_name-aware field access
  - DataRecord::get_field() now returns Option<FieldRef<'_>> instead of Option<&Field<Value>>
  - Tests updated to use get_field_owned() where owned fields are needed
- **WP-OML Performance**: Enable conditional zero-copy optimization in eval_proc
  - Shared variants use cur_name overlay without cloning Arc (zero-copy)
  - Owned variants or type conversions apply name to underlying field
  - Performance improvement: 14-17% faster in multi-stage pipelines
    - 2-stage: 1,151ns → 956ns (16.9% faster)
    - 4-stage: 2,641ns → 2,277ns (13.8% faster)


## [1.15.2] - 2026-02-08

### Added
- **Documentation**: Add complete English WPL grammar reference documentation
  - Comprehensive syntax reference for all WPL language features
  - Examples and usage patterns for field operations


## [1.15.1] - 2026-02-07

### Added
- **WPL Pipe Functions**: Add `not()` wrapper function for inverting pipe function results
  - Syntax: `| not(f_chars_has(dev_type, NDS))` succeeds when dev_type ≠ NDS
  - Supports wrapping any field pipe function (f_has, f_chars_has, chars_has, etc.)
  - Preserves field value - only inverts success/failure result
  - Supports nested negation: `not(not(...))` for double negation logic

### Changed
- **Sinks/Logging**: Unify event ID naming across the codebase for end-to-end tracing

### Fixed
- **WP-OML Tests**: Fix `DataRecord` initialization for compatibility with wp-model-core 0.7.2
- **WP-OML Zero-Copy**: Fix FieldStorage zero-copy optimization for wp-model-core 0.8.3 migration
  - Correctly distinguish Shared vs Owned variants in eval_proc implementation
  - Shared variants use cur_name overlay for zero-copy field name modification
  - Owned variants directly modify underlying field to avoid name inconsistencies
  - Performance improvement: 17-20% faster in multi-stage pipelines (2,730ns → 2,255ns for 4-stage)
- **WPL Pipe Functions**: Fix `f_chars_not_has` and `chars_not_has` type checking bug
  - Previously: Non-Chars fields (e.g., Digit) incorrectly returned FALSE
  - Now: Non-Chars fields correctly return TRUE (they are "not the target Chars value")
  - Semantics: Missing field OR non-Chars type OR value ≠ target → TRUE; value == target → FALSE
  - Previously: `extract_storage()` called `extract_one()` which cloned DataField, then discarded result
  - Now: Direct `Arc::clone()` for PreciseEvaluator::ObjArc, GenericAccessor::FieldArc, NestedAccessor::FieldArc
  - Each static field per stage: eliminated 1× DataField::clone + reduced to single Arc::clone
  - Performance improvement: 4-stage pipeline 2,277ns → 2,211ns (3.3% faster)
  - Static variables now consistently faster than temporary fields (6.3% advantage in 4-stage pipeline)
  - Zero-copy optimization now truly effective as designed
- **WP-OML Tests**: Fix `DataRecord` initialization for compatibility with wp-model-core 0.7.2
- **WP-OML Zero-Copy**: Fix FieldStorage zero-copy optimization for wp-model-core 0.8.3 migration
  - Correctly distinguish Shared vs Owned variants in eval_proc implementation
  - Shared variants use cur_name overlay for zero-copy field name modification
  - Owned variants directly modify underlying field to avoid name inconsistencies
  - Performance improvement: 17-20% faster in multi-stage pipelines (2,730ns → 2,255ns for 4-stage)
- **WPL Pipe Functions**: Fix `f_chars_not_has` and `chars_not_has` type checking bug
  - Previously: Non-Chars fields (e.g., Digit) incorrectly returned FALSE
  - Now: Non-Chars fields correctly return TRUE (they are "not the target Chars value")
  - Semantics: Missing field OR non-Chars type OR value ≠ target → TRUE; value == target → FALSE


## [1.15.0] - 2026-02-07

### Added
- **Sinks/File**: Add `sync` parameter to control immediate disk flushing
  - `sync: false` (default): High-performance mode with buffered writes, suitable for large data volumes
  - `sync: true`: Real-time disk writes for data safety, suitable for critical data
- **WPL not() Group**: Add `not()` group wrapper for negative assertion in field parsing
- **OML Static Blocks**: Introduce `static { ... }` sections for model-scoped constants and template caching
  - Static expressions are executed only once during model loading, results stored in constant pool for reuse across records, avoiding repeated `object { ... }` construction
  - Static symbols can be directly used in assignments, `match` branches, `object { field = tpl; }`, default values `{ _ : tpl }`, and other scenarios
- **OML Enable Configuration**: Add `enable` configuration option to support disabling OML models

### Changed
- **Sinks/Infrastructure**: Optimize infrastructure sink data flow to maintain batch processing
- **Sinks/File**: Remove proto binary format support
- **Sinks/File**: Supported output formats: json, csv, kv, show, raw, proto-text

### Fixed
- **Sinks/File**: Fix `sync` parameter not forcing data to disk
  - Now calls `sync_all()` after `flush()` when `sync: true` to ensure data is physically written to disk
  - Previously only flushed to OS buffer, which didn't guarantee immediate disk writes
- **Benchmarks**: Fix compilation errors in OML benchmarks
  - Fix dereferencing issue in `DataField::from_chars` calls
  - Update import paths from `wp_conf` to `wp_config`
  - Add missing dev-dependencies: orion-variate, wp_config


## [1.14.1] - 2026-02-05

### Added
- **WPL Pipe Processor**: Add `strip/bom` processor for removing BOM (Byte Order Mark) from data
  - Supports UTF-8, UTF-16 LE/BE, and UTF-32 LE/BE BOM detection and removal
  - Fast O(1) detection by checking only first 2-4 bytes
  - Preserves input container type (String → String, Bytes → Bytes, ArcBytes → ArcBytes)


## [1.14.0] - 2026-02-04

### Added
- **WPL Functions**: Add `starts_with` pipe function for efficient string prefix matching
  - Checks if a string field starts with a specified prefix
  - More performant than regex for simple prefix checks
  - Case-sensitive matching
  - Converts to ignore field when prefix doesn't match
- **OML Pipe Functions**: Add `starts_with` pipe function for OML query language
  - Supports same prefix matching functionality as WPL
  - Returns ignore field when prefix doesn't match
  - Usage: `pipe take(field) | starts_with('prefix')` or `take(field) | starts_with('prefix')`
- **OML Pipe Functions**: Add `map_to` pipe function for type-aware conditional value assignment
  - Replaces field value when field is not ignore
  - Supports multiple types with automatic type inference: string, integer, float, boolean
  - Preserves ignore fields unchanged
  - Usage examples:
    - `pipe take(field) | map_to('string')` - map to string
    - `pipe take(field) | map_to(123)` - map to integer
    - `pipe take(field) | map_to(3.14)` - map to float
    - `pipe take(field) | map_to(true)` - map to boolean
- **OML Match Expression**: Add function-based pattern matching support
  - Enables using functions like `starts_with` in match conditions
  - Syntax: `match read(field) { starts_with('prefix') => result, _ => default }`
  - More flexible than simple value comparison
  - Useful for log parsing, URL routing, and content classification
  - Supported functions:
    - **String matching**:
      - `starts_with(prefix)` - Check if string starts with prefix
      - `ends_with(suffix)` - Check if string ends with suffix
      - `contains(substring)` - Check if string contains substring
      - `regex_match(pattern)` - Match string against regex pattern
      - `is_empty()` - Check if string is empty (no arguments)
      - `iequals(value)` - Case-insensitive string comparison
    - **Numeric comparison**:
      - `gt(value)` - Check if numeric field > value
      - `lt(value)` - Check if numeric field < value
      - `eq(value)` - Check if numeric field equals value (with floating point tolerance)
      - `in_range(min, max)` - Check if numeric field is within range [min, max]
- **OML Parser**: Add quoted string support for `chars()` and other value constructors
  - Supports single quotes: `chars('hello world')`
  - Supports double quotes: `chars("hello world")`
  - Enables strings containing spaces and special characters
  - Escape sequence support: `\n`, `\r`, `\t`, `\\`, `\'`, `\"`
  - Backward compatible with unquoted syntax: `chars(hello)`
  - Works in all contexts: field assignments, match expressions, etc.
- **OML Transformer**: Add automatic temporary field filtering with performance optimization
  - Fields with names starting with `__` are automatically converted to ignore type after transformation
  - Parse-time detection: checks for temporary fields during OML parsing (one-time cost ~50-500ns)
  - Runtime optimization: skips filtering entirely when no temporary fields exist (~99% cost reduction)
  - Enables using intermediate/temporary fields in calculations without polluting final output
  - Example: `__temp = chars(value); result = pipe take(__temp) | base64_encode;`
  - The `__temp` field will be marked as ignore in the final output
  - Performance: ~1ns overhead for models without temp fields, ~500ns for models with temp fields

### Changed
- **OML Syntax**: `pipe` keyword is now optional in pipe expressions
  - Both `pipe take(field) | func` and `take(field) | func` are supported
  - Simplified syntax improves readability
  - Display output always includes `pipe` for consistency

### Fixed
- **OML Match Parser**: Fixed `in_range` function parsing failure in match expressions
  - Issue: `kw_in` consumed prefix `in` before `cond_fun` could parse `in_range`
  - Fix: Reordered `match_cond1` alternatives to try `cond_fun` before `cond_in`
  - Now `match read(x) { in_range(0, 10) => ... }` parses correctly
- **OML map_to Parser**: Fixed large integer precision loss during parsing
  - Issue: Parsing integers via f64 caused precision loss for values > 2^53 (e.g., 9007199254740993)
  - Fix: Try parsing as i64 first, only fall back to f64 for actual floats
  - Preserves exact integer values up to i64::MAX
- **OML Display Output**: Fixed round-trip parsing compatibility for strings
  - Issue: Display output was not parseable by `quot_str` due to escaping mismatch
  - Fix: Removed extra escaping in Display implementations since `quot_str` preserves raw escape sequences
  - Display output now stable across multiple round-trips (parse -> display -> parse -> display)


## [1.13.3] - 2026-02-03

### Fixed
- **WPL Parser**: Fix compilation errors in pattern parser implementations by adding missing `event_id` parameter to all trait methods
- **Runtime**: Remove unused `debug_data` import in vm_unit module


## [1.13.2] - 2026-02-03

### Added
- **WPL Parser**: Add support for `\t` (tab) and `\S` (non-whitespace) separators in parsing expressions
- **WPL Parser**: Add support for quoted field names with special characters (e.g., `"field.name"`, `"field-name"`) #16
- **WPL Functions**: Add `chars_replace` function for character-level string replacement #13
- **WPL Functions**: Add `regex_match` function for regex pattern matching
- **WPL Functions**: Add `digit_range` function for numeric range validation
- **Documentation**: Add multi-language documentation structure for WPL guides

### Changed
- **Logging**: Optimize high-frequency log paths with `log_enabled!` guard to eliminate loop overhead when log level is filtered
- **Logging**: Add `event_id` to debug messages for better traceability
- **WPL Parser**: Add `event_id` parameter to `PatternParser` trait for improved event tracing across all parser implementations

### Fixed
- **Miss Sink**: Remove base64 encoding from raw data display to show actual content
- **Data Rescue**: Fix lost rescue data problem #19

### Removed
- **Syslog UDP Source**: Remove `SO_REUSEPORT` multi-instance support
  - Security risk: allows same-UID processes to intercept traffic
  - Cross-platform inconsistency: macOS/BSD doesn't provide kernel-level load balancing
  - See `docs/dar/udp_reuseport.md` for detailed design rationale


## [1.11.0] - 2026-01-28

### Added
- **Syslog UDP Source**: Added `udp_recv_buffer` configuration parameter to control UDP socket receive buffer size (default 8MB)
  - Helps prevent packet loss under high throughput conditions
  - Uses `socket2` crate for buffer configuration before socket binding
- **Syslog UDP Source**: Added batch receiving (up to 128 packets per `receive()` call) for better throughput
- **Syslog UDP Source**: Added `fast_strip` optimization (previously TCP-only)
  - Skip full syslog parsing when `header_mode = "skip"` and only stripping header
  - Fast path for RFC3164 (find `: `) and RFC5424 (skip fixed structure) formats
  - Reduces CPU overhead significantly at high EPS
- **Syslog UDP Source**: Added Linux `recvmmsg()` syscall support for batch receiving
  - Receive up to 64 datagrams in a single syscall on Linux
  - Reduces syscall overhead by ~60x compared to per-packet `recv_from()`
  - Automatically falls back to standard loop on non-Linux platforms
- **Syslog UDP Source**: Changed payload from `Bytes::copy_from_slice` to `Arc<[u8]>`
  - Zero-copy sharing downstream reduces memory allocation overhead
  - More consistent with TCP source's `ZcpMessage` pattern

### Changed
- **Syslog Architecture**: Major refactoring to eliminate duplicate parsing and unify UDP/TCP processing
  - Removed `SyslogDecoder` dependency from UDP source (now uses raw UDP socket)
  - UDP source passes raw bytes to `SourceEvent`, syslog processing happens in preprocessing hook
  - Unified preprocessing logic between UDP and TCP sources
  - `header_mode = "raw"` now correctly preserves full syslog message including header
  - Eliminated redundant `normalize_slice()` calls (was parsing twice: in decoder + preproc hook)
- **Syslog UDP Source**: Optimized preprocessing hook to be created once and reused via `Arc::clone()` instead of per-message allocation
- **Syslog header_mode**: Renamed configuration values for clarity with backward compatibility
  - `raw` (保留原样) - previously `keep`
  - `skip` (跳过头部) - previously `strip`
  - `tag` (提取标签) - previously `parse`
  - Legacy values (`keep`/`strip`/`parse`) remain supported as aliases
  - Default changed from `strip` to `skip`

### Removed
- **Syslog Protocol**: Removed `SyslogDecoder` and `SyslogFrame` from `protocol::syslog` module
  - No longer needed after UDP source refactoring
  - Syslog encoding (`SyslogEncoder`, `EmitMessage`) retained for sink usage
- **Benchmarks**: Replaced deprecated `criterion::black_box` with `std::hint::black_box` across all benchmark files
  - `crates/wp-stats/benches/wp_stats_bench.rs`
  - `crates/orion_exp/benches/or_we_bench.rs`
  - `crates/wp-oml/benches/oml_sql_bench*.rs`
  - `crates/wp-parser/benches/*.rs`
  - `crates/wp-lang/benches/nginx_10k.rs`
  - `crates/wp-knowledge/benches/read_bench.rs`
  - `src/sources/benches/normalize_bench.rs`
- **Documentation**: Updated Syslog source documentation with comprehensive configuration guide
  - Added UDP vs TCP protocol selection guide
  - Added performance tuning recommendations
  - Updated `wp-docs/10-user/02-config/02-sources.md`
  - Updated `wp-docs/10-user/05-connectors/01-sources/04-syslog_source.md`

### Fixed
- **Syslog RFC3164 Parser**: Implemented strict validation to prevent misidentification of non-standard formats
  - Added month name validation (Jan-Dec only)
  - Added strict timestamp format validation (HH:MM:SS with colons)
  - Added mandatory space validation after month, day, and time fields
  - Non-standard formats (e.g., ISO timestamps, invalid month names) now correctly fallback to passthrough
  - Examples that now correctly reject:
    - `<11>2025-07-07 09:42:43,132 sentinel - ...` (ISO format)
    - `<158>Jul23 17:18:36 skyeye ...` (missing space after month)
    - `<34>Xyz 11 22:14:15 host ...` (invalid month)
- **Clippy**: Fixed `bool_assert_comparison` warnings in syslog tests (`src/sources/syslog/mod.rs`)


## [1.10.4] - 2026-01-27

### Changed
- **Dependencies**: Updated `sysinfo` requirement from 0.37 to 0.38
- **License**: Changed license from Elastic License 2.0 to Apache 2.0
- **Support Links**: Updated support links to point to organization discussions

### Fixed
- **Monitoring**: Repaired monitoring statistics and examples for MetricCollectors


## [1.10.0] - 2026-01-22

### Added
- **KvArr Parser** (`crates/wp-lang/src/eval/value/parser/protocol/kvarr.rs`): New parser for key=value array format
  - Supports both `=` and `:` as key-value separators (e.g., `key=value` or `key:value`)
  - Flexible delimiter support: comma-separated, space-separated, or mixed
  - Automatic type inference for values (bool, integer, float, string)
  - Quoted and unquoted string values (e.g., `"value"` or `value`)
  - Duplicate key handling with automatic array indexing (e.g., `tag=alpha tag=beta` → `tag[0]`, `tag[1]`)
  - Subfield configuration support with type mapping and meta field ignoring (`_@name`)
  - Nested parser invocation through sub-parser context
  - WPL syntax: `kvarr(type@field1, type@field2, ...)`
- **Unicode-friendly string parsing**: Added `take_string` helper for general text arguments (e.g. 汉字) without changing the legacy `take_path` semantics (`crates/wp-parser/src/atom.rs`).
- **WPL Documentation Updates**:
  - Added `kvarr` to builtin types in grammar specification (`wp-docs/docs/10-user/03-wpl/04-wpl_grammar.md`)
  - New "KvArr 类型（键值对数组）" section in basics guide with syntax and examples (`wp-docs/docs/10-user/03-wpl/01-wpl_basics.md`)
  - New "2.1 KvArr 键值对数组解析" section in examples guide with 5 practical use cases (`wp-docs/docs/10-user/03-wpl/02-wpl_example.md`)

### Fixed
- **KvArr Parser**: Fixed meta fields being ignored in sub-parser context (`crates/wp-lang/src/eval/value/parser/protocol/kvarr.rs`)
- **Module Export**: Fixed missing `validate_groups` function export in `wp-cli-core::utils::validate` module (`crates/wp-cli-core/src/utils/validate/mod.rs`)
- **Single-quoted strings**: `single_quot_str_impl` now rejects raw `'` and accepts `\'` escapes, aligning behavior with double-quoted parser (`crates/wp-lang/src/parser/utils.rs`).
- **Chars* fun args**: `chars_has`/`chars_in` families switched to `take_string`, restoring `take_path` for identifiers while keeping Unicode support for free-form arguments (`crates/wp-lang/src/parser/wpl_fun.rs`).


## [1.9.0] - 2026-01-16

### Added
- `BlackHoleSink` now supports `sink_sleep_ms` parameter to control sleep delay per sink operation (0 = no sleep)
- `BlackHoleFactory` reads `sleep_ms` from `SinkSpec.params` to configure sleep behavior
- **Dynamic Speed Control Module** (`src/runtime/generator/speed/`): New module for variable data generation speed
  - `SpeedProfile` enum with multiple speed models:
    - `Constant` - Fixed rate generation
    - `Sinusoidal` - Sine wave oscillation (day/night cycles)
    - `Stepped` - Step-wise rate changes (business peak/off-peak)
    - `Burst` - Random burst spikes (traffic surges)
    - `Ramp` - Linear ramp up/down (load testing)
    - `RandomWalk` - Random fluctuations (natural jitter)
    - `Composite` - Combine multiple profiles (Average/Max/Min/Sum)
  - `DynamicSpeedController` - Calculates target rate based on elapsed time and profile
  - `DynamicRateLimiter` - Token bucket rate limiter with dynamic rate updates
- `GenGRA.speed_profile` field for configuring dynamic speed models in generators
- **wpgen.toml Configuration Support** (`crates/wp-config/src/generator/`):
  - `SpeedProfileConfig` - TOML-parseable configuration for speed profiles
  - `GeneratorConfig.speed_profile` - New optional field to configure dynamic speed in wpgen.toml
  - Helper methods: `base_speed()`, `get_speed_profile()`, `is_constant_speed()`
  - Backward compatible: Falls back to `speed` field when `speed_profile` is not set
- **Rescue Statistics Module** (`crates/wp-cli-core/src/rescue/`): New module for rescue data statistics
  - `RescueFileStat` - Single rescue file statistics (path, sink_name, size, line_count, modified_time)
  - `RescueStatSummary` - Aggregated statistics with per-sink breakdown
  - `SinkRescueStat` - Per-sink statistics (file_count, line_count, size_bytes)
  - `scan_rescue_stat()` - Scan rescue directory and generate statistics report
  - Multiple output formats: table, JSON, CSV
  - Supports nested directory scanning and `.dat` file filtering

### Changed
- **Rescue stat functionality migrated to wp-cli-core**: Rescue statistics is now a standalone CLI utility in `wp-cli-core::rescue` module, decoupled from wp-engine runtime

### Removed
- `WpRescueCLI` enum removed from wp-engine (rescue CLI should be defined in application layer)
- `RescueStatArgs` struct removed from wp-engine facade
- `run_rescue_stat()` function removed from wp-engine facade


## [1.8.2] - 2026-01-14

### Changed
- **Breaking**: Renamed `oml_parse` to `oml_parse_raw` for clarity (crates/wp-oml/src/parser/mod.rs)
- Removed deprecated pipe functions from OML language module

### Refactored
- **wp-oml**: Extracted nested functions from `oml_sql` to module level for improved readability (crates/wp-oml/src/parser/sql_prm.rs)
  - `is_sql_ident`, `sanitize_sql_body`, `rewrite_lhs_fn_eq_literal`, `to_sql_piece`, `fast_path_ip4_between_eq_one`
- **wp-oml**: Unified OML parser error contexts using shared helpers (`ctx_desc`, `ctx_literal`)
  - Affected files: keyword.rs, oml_aggregate.rs, oml_conf.rs, pipe_prm.rs, sql_prm.rs, utils.rs

### Fixed
- `wp_log::conf::LogConf` construction in wpgen configuration (crates/wp-config/src/generator/wpgen.rs)

## [1.8.1] - 2024-01-11

### Added
- **P0-3**: `ConfigLoader` trait to unify configuration loading interface (crates/wp-config/src/loader/traits.rs)
- **P0-4**: `ComponentBase` trait system to standardize component architecture across wp-proj
- **P0-5**: Unified API consistency with new `fs` utilities module in wp-proj
- **P0-2**: Error conversion helpers module (`error_conv`, `error_handler`) to simplify error handling
- **P0-1**: Centralized knowledge base operations in wp-cli-core to eliminate duplication
- Comprehensive documentation comments for ConfigLoader trait
- Path normalization for log directory display to remove redundant `./` components (crates/wp-proj/src/utils/log_handler.rs:48-76)
- Test case `normalize_path_removes_current_dir_components` to verify path normalization

### Changed
- **Breaking**: EnvDict parameter now required in all configuration loading functions
  - `validate_routes(work_root: &str, env_dict: &EnvDict)` (wp-cli-core/src/business/connectors/sinks.rs:18)
  - `collect_sink_statistics(sink_root: &Path, ctx: &Ctx, dict: &EnvDict)` (wp-cli-core/src/business/observability/sinks.rs:21)
  - `load_warp_engine_confs(work_root: &str, dict: &EnvDict)` (src/orchestrator/config/models/warp_helpers.rs:17)
  - And 13 more functions across wp-proj and wp-cli-core
- **Architecture**: Enforced top-level EnvDict initialization pattern
  - EnvDict must be created at application entry point (e.g., `load_sec_dict()` in warp-parse)
  - Crate-level functions only accept `dict: &EnvDict` parameter, never create instances
  - This follows dependency injection pattern for better testability and clarity
- Source and sink factories now return multiple connector definitions instead of single instance
- Improved table formatting in CLI output for better readability

### Fixed
- Default sink path resolution now works correctly
- Engine configuration path normalization to handle `.` and `..` components properly
- Empty stat fields are now skipped during serialization
- Project initialization bug resolved
- Documentation test closure parameter issues in error_conv module
- Log directory paths now display correctly without `././` in output messages (crates/wp-proj/src/utils/log_handler.rs:96,102)
- Clippy warning `field_reassign_with_default` in wpgen configuration (crates/wp-config/src/generator/wpgen.rs:125)

### Refactored
- **wp-proj Stage 1**: Extracted common patterns to reduce code duplication
- **wp-proj Stage 2**: Implemented Component trait system for models, I/O, and connectors
- **wp-proj Stage 3**: Documented standard error handling patterns
- **wp-proj Stage 4**: Merged `check` and `checker` modules to eliminate responsibility overlap
- Knowledge base operations delegated from wp-proj to wp-cli-core

### Removed
- `EnvDictExt` trait removed from wp-config as it violated architectural separation
  - App layer (warp-parse, wpgen) is responsible for EnvDict creation
  - Crate layer (wp-engine, wp-proj, wp-config) only receives and uses EnvDict
- Documentation files: `envdict-ext-usage.md`, `envdict-ext-quickref.md`

## [1.8.0] - 2024-01-05

### Added
- Environment variable templating support via `orion-variate` integration
- `EnvDict` type for managing environment variables during configuration loading
- Environment variable substitution in configuration files using `${VAR}` syntax
- Three-level variable resolution: dict → system env → default value
- Tests for environment variable substitution in config loading
- Path resolution for relative configuration paths

### Changed
- Updated `orion_conf` dependency to version 0.4
- Updated `wp-infras` dependencies to track main branch
- License changed from MIT to SLv2 (Server License v2)
- Work root resolution now uses `Option<String>` for better API clarity
- Configuration loading functions now accept `EnvDict` parameter
- Replaced direct `toml::from_str` calls with `EnvTomlLoad::env_parse_toml`

### Fixed
- Work root validation issue (#56) - invalid work-root paths now properly handled
- Partial parsing handling improved with residue tracking and error logging

### Removed
- `Cargo.lock` removed from version control
- Unnecessary `provided_root` parameter removed from path resolution functions

## Version Comparison Links
