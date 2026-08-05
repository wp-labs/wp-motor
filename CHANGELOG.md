# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.25.0 Unreleased]

### Added
- **OML 内网富化**：新增 `intranet_ip`（判内/外）、`access_direct`（访问方向）、`on_fail`（失败兜底）函数；管道源扩展支持 `access_direct(a,b) | on_fail('x')`。内网网段作为知识统一由 wp-knowledge 管理（`knowdb.toml [intranet_nets]` 节），`wproj check` 可校验
- **OML IP 编码**：新增 `ip_to_biguint`（IPv4/IPv6 统一编码），`FieldQueryCache`/`compare_datafield` 支持 `BigUint`
- **Docs**：OML 语法参考、富化函数与内网网段配置文档同步

### Changed
- **Dependencies**：升级 `wp-knowledge` 0.14→0.15、`wp-model-core` 0.8→0.9、`wp-lang` 0.4→0.5、`wp-error` 0.10→0.11 等一批依赖；`wp-knowledge` 改为本地 path 依赖


## [1.23.8] - 2026-07-31

### Added
- **Parser**：新增 `wp_event_md5` 事件指纹字段；`copy_event_parse` 改为独立旁路 record 路由到目标 sink；`#[no_match]` rule 装配支持旁路路由


## [1.23.7] - 2026-07-12

### Added
- **Sink/BatchMeta**：OML 输出名与组级元信息输出策略经 `BatchMeta` 下发给 connector

### Changed
- **Dependencies**：升级 `wp-connector-api` 0.11、`wp-core-connectors` 0.7、`wp-lang` 0.4，新增 `wp-source-types`
- **Sink/Metadata**：运行时元信息改由 sink 侧渲染；`ProcMeta::Rule` 改名 `WplName` 明确 WPL/OML 区分


## [1.23.5] - 2026-07-06

### Added
- **wpgen**：`wpgen.toml` 新增 `[models]` 段指定 WPL 目录；目录无效时启动报错

### Changed
- **Sink/Factories**：启动时打印已注册 factory 列表

## [1.23.4] - 2026-07-05

### Changed
- **Dependencies**：升级 `shadow-rs` 2.0、`wp-core-connectors` 0.5
- **Generator/Sink**：动态字节预算批量下发（BatchSizePolicy），TCP 场景 CPU 显著下降

### Fixed
- **Project Init**：修复 `wproj init` 模板无效 header 与废弃 connector 引用
- **wpadm/Cli**：source 配置支持目录式扫描

## [1.23.0]

### Added
- **Knowledge/Redis**：升级 wp-knowledge 0.14，knowdb.toml 支持 `[provider.redis]` 高速查表

### Changed
- **Knowledge/Provider**：`[provider]` 拆分为 `[provider.sqldb]` 与 `[provider.redis]`

### Removed
- **Sinks/Arrow**：移除独立 arrow sink，统一到 file/tcp 的 `protocol = "arrow"`

### Fixed
- **OML/Pipe/ip4_to_int**：IPv6 输入改返回 Null，支持字符串 IPv4 解析

## [1.22.10 Unreleased]

### Changed
- **Memory Profile**：未设置 `WP_MEMORY_PROFILE` 默认回到 `standard`；提升 `standard` TCP 批量参数改善长行样本供给

## [1.22.9] - 2026-06-25

### Added
- **Benchmarks**：新增 sink record id 成功路径基准

### Changed
- **Sink Runtime**：批量错误日志简化

## [1.22.8] - 2026-06-23

### Changed
- **Memory Profile**：默认内存 profile 改为 `low`，更早背压、优先控制 RSS
- **Source Rate Limit**：固定限速不再被 pending 批数水位压低，仍受内存上限保护

## [1.22.7] - 2026-06-23

### Added
- **Source Rate Limit**：新增 source 全局限速（`rate_limit_rps`）与 AIMD 自动限速
- **Memory Profiles**：统一 `WP_MEMORY_PROFILE=standard|low|throughput`

### Changed
- **Runtime Defaults**：默认限速改自动模式；运行时缓冲/水位统一由 `wp_conf::limits` 管理

## [1.22.3] - 2026-05-19

### Added
- **SQL/Route**：SQL 按表名路由到本地 SQLite 或外部 Provider；新增 `SqlKnowledgeRoute`；支持子查询与别名

## [1.22.2] - 2026-05-13

### Added
- **Sinks/Sync**：`SinkTerminal` 批量写入方法，降低统计反压

## [1.22.1] - 2026-05-12

### Fixed
- **OML/SQL**：SQL 参数全 Null 时跳过查询；提取字段跳过 `Value::Null`；修复知识库查询 bug

## [1.22.0] - 2026-05-08

### Added
- **Diagnostics/CLI**：错误提示以 `stable_code` 为主键，支持 `WP_LANG` 中英双语
- **Config/Engine**：新增 `RepoGroupConf` 仓库组配置

### Changed
- **Dependencies**：升级 `orion-error` 0.8，适配新 derive 宏；配置解析开启 `deny_unknown_fields`

### Fixed
- **OML/SQL**：修复 `IN (...)` 参数绑定顺序、`take(field)` 字段移动顺序、SQL 解析增强（`string_agg(distinct)`、`IN (@sip,@dip)` 等）

## [1.20.7] - 2026-04-26

### Changed
- **wpgen**：统一 `wpgen.toml` 加载入口；`wproj check` 新增 wpgen 检查与结构化 warning
- **Validation**：缺失 source/sink 目录降级为非阻断 warning

### Fixed
- **wproj/check JSON**：修复 warning 污染 JSON 输出；修复测试临时文件写入源码目录问题

## [1.20.6] - 2026-04-24

### Fixed
- **Error Handling**：修复多处 `StructError` 二次挂接与无效 TOML 触发的 panic，改为结构化错误返回

## [1.20.5] - 2026-04-24

### Fixed
- **Monitoring/Hot Reload**：修复热加载后监控统计不再输出

## [1.20.4] - 2026-04-19

### Added
- **Error Handling**：结构化错误系统设计与审查文档；模板补 VictoriaLogs 示例

### Changed
- **Config/Observability**：统一配置加载与结构化诊断

## [1.20.3] - 2026-04-16

### Fixed
- **Runtime/Stats**：修复统计切片过多导致的反压

## [1.20.2] - 2026-04-16

### Changed
- **CI**：主 CI 支持 `hotfix/*` 分支

### Fixed
- **Config Schema**：严格 schema 下修复测试与生成配置

## [1.20.0] - 2026-04-11

### Added
- **Sinks/Arrow**：新增 arrow-file sink；**OML**：新增 `iequals_any`、`lookup_nocase`、`calc(...)`；**Runtime**：结构化 LoadModel 控制、`reload_timeout_ms`；**Source**：file 通配符

### Changed
- **Runtime**：重载改事件驱动协调；admin token 默认位置改 `~/.warp_parse`；OML 统一 async 路径；默认通道容量降低提前背压

### Fixed
- **Runtime**：修复重载挂起、TCP 内存增长、OML static 引用其他符号等

## [1.18.3] - 2026-03-16

### Changed
- **Event ID**：`wp_event_id` 统一用共享生成器

### Fixed
- **Event ID/Restart**：修复重启后事件 ID 回落固定种子导致重复

## [1.18.2] - 2026-03-14

### Fixed
- **wp-lang**：修复 `kv`/`kvarr` 与 `@...` 引用路径对括号类键（`()`、`[]`、`<>`、`{}`）的解析

## [1.18.1] - 2026-03-09

### Changed
- **Semantic Dict**：外部词典改默认路径探测 + 工作根注入；新增 `enabled` 开关

### Fixed
- **wp-proj/check**：按目标 work_root 解析语义词典配置

## [1.18.0 Unreleased]

### Changed
- **Error Handling**：完成 workspace 迁移到 `orion-error 0.6`/`orion_conf 0.5` API

## [1.17.8]

### Fixed
- **wp-lang**：修复 `kv`/`kvarr` 括号类键解析

## [1.17.6] - 2026-03-02

### Changed
- **Stats**：优化 `metric_set` 合并逻辑

## [1.17.5] - 2026-02-27

### Changed
- **Documentation**：更新 OML 语法文档

### Fixed
- **Sinks/Buffer**：修复 sink 批量路径 `batch_size` 行为

## [1.17.4] - 2026-02-18

### Added
- **Sinks/Config**：sink 组新增 `batch_size` 配置

## [1.17.3] - 2026-02-16

### Added
- **Sinks/Buffer**：sink 级批缓冲 + `batch_timeout_ms`（默认 300ms）

### Changed
- **Sinks/File**：移除 `BufWriter` 冗余缓冲

## [1.17.2] - 2026-02-13

### Changed
- **wp-lang**：`kv`/`kvarr` key 解析支持括号类字符

## [1.17.0] - 2026-02-12

### Added
- **OML Match**：OR 条件语法 `cond1 | cond2`；多源 match 不限数量
- **OML NLP**：新增 `extract_main_word`/`extract_subject_object`；`wparse.toml [semantic]` 控制语义词典加载（默认关，省内存）

## [1.16.2] - 2026-02-11

### Fixed
- **wp-lang**：修复 kvarr 模式分隔符解析

## [1.16.1] - 2026-02-11

### Changed
- **wp-lang**：分隔符模式扩展 `\S`、`\H`

## [1.16.0] - 2026-02-11

### Added
- **wp-lang**：分隔符模式 `{...}` 支持通配符、空白匹配与保留组

## [1.15.5] - 2026-02-10

### Changed
- **wp-oml**：FieldRead 零拷贝 FieldStorage 保留

## [1.15.4] - 2026-02-10

### Changed
- **wp-oml**：多操作（Map/Pipe/Fmt/Sql/FieldRead）增强零拷贝支持

## [1.15.3] - 2026-02-09

### Added
- **WP-OML**：DataTransformer 批量处理 API（`transform_batch`），性能提升 12-17%

### Changed
- **Dependencies**：升级 `wp-model-core` 0.8.4；启用条件零拷贝优化

## [1.15.2] - 2026-02-08

### Added
- **Documentation**：完整英文 WPL 语法参考

## [1.15.1] - 2026-02-07

### Added
- **WPL**：新增 `not()` 包装函数反转管道结果

### Fixed
- **wp-oml**：修复 FieldStorage 零拷贝、`f_chars_not_has` 类型检查 bug

## [1.15.0] - 2026-02-07

### Added
- **Sinks/File**：`sync` 参数控制即时落盘；**OML**：`static { ... }` 常量块、`enable` 配置

### Changed
- **Sinks/File**：移除 proto 二进制格式，支持 json/csv/kv/show/raw/proto-text

## [1.14.1] - 2026-02-05

### Added
- **WPL**：`strip/bom` 处理器移除 BOM

## [1.14.0] - 2026-02-04

### Added
- **OML**：`starts_with`/`map_to` 管道函数；match 函数式匹配；`pipe` 关键字可选；`__` 临时字段自动过滤

### Fixed
- **OML**：修复 `in_range` 解析、`map_to` 大整数精度、Display round-trip

## [1.13.3] - 2026-02-03

### Fixed
- **WPL Parser**：修复 trait 方法 `event_id` 参数编译错误

## [1.13.2] - 2026-02-03

### Added
- **WPL**：`\t`/`\S` 分隔符、引号字段名、`chars_replace`/`regex_match`/`digit_range` 函数

### Fixed
- **Rescue**：修复救援数据丢失问题

### Removed
- **Syslog UDP**：移除 `SO_REUSEPORT` 多实例支持（安全风险）

## [1.11.0] - 2026-01-28

### Added
- **Syslog UDP**：`udp_recv_buffer` 配置、批量接收、Linux `recvmmsg()`、`fast_strip`、`Arc<[u8]>` 零拷贝

### Changed
- **Syslog**：消除重复解析，统一 UDP/TCP 预处理；`header_mode` 值改名（raw/skip/tag，兼容旧值）

## [1.10.4] - 2026-01-27

### Changed
- **Dependencies**：升级 `sysinfo` 0.38；License 改 Apache 2.0

## [1.10.0] - 2026-01-22

### Added
- **KvArr Parser**：键值对数组解析（`=`/`:` 分隔、类型推断、重复键索引）；Unicode 字符串解析

## [1.9.0] - 2026-01-16

### Added
- **Generator**：动态速度控制（`SpeedProfile`）；**Rescue**：救援数据统计模块；**BlackHole**：`sink_sleep_ms`

## [1.8.2] - 2026-01-14

### Changed
- **wp-oml**：`oml_parse` 改名 `oml_parse_raw`；移除废弃管道函数

## [1.8.1] - 2024-01-11

### Added
- **ConfigLoader/ComponentBase**：统一配置加载与组件架构；集中知识库操作

### Changed
- **EnvDict**：配置加载函数统一要求 `EnvDict` 参数（依赖注入）

## [1.8.0] - 2024-01-05

### Added
- **Config**：环境变量模板 `${VAR}` 支持（`orion-variate`）、`EnvDict` 三值解析

### Changed
- **Dependencies**：升级 `orion_conf` 0.4；License 改 SLv2

### Removed
- `Cargo.lock` 移出版本控制
