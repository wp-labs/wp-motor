# Warp Flow Crates 概览

本目录存放工作区共享的 Rust 库（crates）。命名采用 kebab-case，并以 `wp-` 作为前缀统一领域归属；第三方/上游组件保留其原名。二进制应用位于 `apps/`，通常通过在 Cargo.toml 中添加 `package = "…"` 的方式引用这些库（保持现有 crate id 不变）。

- 依赖约定：语言/解析相关在 `wp-lang`、`wp-expr`、`wp-parser`、`wp-oml`；数据/类型在 `wp-data-utils`；错误在 `wp-error`；日志在 `wp-log`（基于 log/log4rs 封装）；配置在 `wp-config`；控制面在 `wp-ctrl-api`；I/O 接口在 `wp-source-api`、`wp-sink-api`；统计在 `wp-stats`；CLI 共用工具在 `wp-cli-utils`；通用设施在 `wp-common`。
- Feature 约定：Kafka 聚合开关使用 `kafka`；插件/商业能力在根工程聚合（见根 Cargo.toml 的 `features`）。

## 各 crate 简述

- `syslog-parse`
  - 第三方 syslog 解析库的 vendor/upstream（保持上游命名），供 `wp_source_syslog` 使用。

- `wp-cli-utils`
  - CLI 辅助工具库：扫描 `sink/` 结构、统计输入/输出、校验规则产物、格式化输出等。
  - 被 `apps/wproj`、`apps/wpsink`、`apps/wprescue` 等复用。

- `wp-common`
  - 通用友好型 trait/helper（new/append/conv 等），降低样板代码；被多数 crate 复用。

- `wp-config`
  - 运行配置与模型构建：数据源/数据汇声明、业务分组、期望/路由、表结构、命令行参数桥接等。
  - 连接语言/解析与数据模型：依赖 `wp-lang`、`wp-parser`、`wp-oml`、`wp-data-utils`、`wp-knowledge` 等。

- `wp-ctrl-api`
  - 控制面接口：`EnginePaths`、`RuleSyncOps`、`ControlBus`、隐私服务启动等抽象与对接点。

- `wp-data-utils`
  - 核心数据/类型与格式化：数据记录、字段/类型系统；CSV/JSON/RAW/Proto/SQL 等格式；缓存与构造工具。

- `wp-error`
  - 工作区统一错误抽象：配置/分发/运行/解析/数据源错误类型；对接 `orion-error` 的错误码/结构化细节。

- `wp-expr`
  - 逻辑与比较表达式内核：表达式 AST、构建器、求值器；SQL/Rust 风格符号提供者。
  - 为语言/解析层（`wp-lang`、`wp-parser`）提供布尔表达式基础。

- `wp-knowledge`
  - 知识库/词典：内存/SQLite（`rusqlite` + `r2d2`），统一查询接口与序列化支持。

- `wp-lang`
  - Warp DSL：AST、解析器、执行引擎（VM/runtime）、内建函数、生成器等；对外暴露 `DataPacket` 等核心类型。

- `wp-log`
- 日志配置与宏封装（`wp-log` 基于 log/log4rs）；提供域宏（`info_ctrl!` 等）与初始化 `wp_log::conf::log_init`；文件日志默认 10MB/10 份滚动（gzip）。

- `wp-oml`
  - OML 语言与隐私处理：语法/解析/执行与隐私处理流水线（脱敏/掩码/规则），与 `wp-lang`、`wp-data-utils` 融合。

- `wp-parser`
  - 轻量通用解析构件：原子/符号/SQL 符号、函数调用参数、条件表达式、网络/IP 等；桥接到 `wp-expr`。

- `wp-sink-api`
  - Sink 接口与注册表：`AsyncCtrl`、`AsyncRecordSink`、`AsyncRawDataSink`、`AsyncSink` 组合与 `SinkBuilder`/`SinkRegistry`。
- 特性门控：`kafka` 用于 Kafka 源/汇聚合；其他插件按各自 feature 门控（MySQL/CH/ES/Prometheus 等在 `extensions/`）。

- `wp-source-api`
  - Source 接口与注册表：`DataSource`、`ServiceAcceptor`、`SourceBuilder`、`SourceRegistry`。
- 特性门控：`kafka` 用于 Kafka 相关扩展；其余按各自 feature 控制。

- `wp-stats`
  - 统计与度量：维度/目标、度量单位、阶段/切片、收集器与报表；与运行时/数据汇协作打印统计。

## 依赖示例

在上层二进制或库的 Cargo.toml 中，以 `package = "…"` 映射新包名，保持已有 crate id 不变：

```
[dependencies]
wp_conf = { package = "wp-config", path = "crates/wp-config" }
wpl     = { package = "wp-lang",   path = "crates/wp-lang" }
wp-model-core  = { package = "wp-data-utils", path = "crates/wp-data-utils" }
wp_err  = { package = "wp-error",   path = "crates/wp-error" }
```

## 约定与提示
- Kafka 扩展统一通过 `kafka` 开启；插件与商业能力请参考根工程 `features` 聚合开关（`plugin-*`、`enterprise`）。
- `extensions/` 下为可选实现（sources/sinks/plugins），遵循对应 `*-api` 的接口并在应用中按 feature 挂载。
- `syslog-parse` 为上游库，命名与接口保持一致。
