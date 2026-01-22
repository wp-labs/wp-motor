# Warp Flow src 开发者指南

本文面向在 `src/` 目录内开发或维护核心代码的同学，帮助快速理解目录职责、依赖约束与常见扩展点，并给出本仓通用的开发/测试建议。

## 目录总览（职责/边界）
- adapters/：与 API 层解耦的“桥接器”。仅做结构转换（如 `wp-specs::Core*Spec → wp-*-api::Resolved*Spec`），不持有业务逻辑。
- core/：规则语言与解析引擎的核心能力（错误策略、WPL 引擎、插件工厂适配等）。对外通过 facade/orchestrator 暴露。
- facade/：应用侧唯一稳定入口。apps 只依赖 facade 暴露的极少数 API（如 `WpApp`、`WpRescueApp`、`facade::enrich::*`）。
- orchestrator/：装配层（配置加载、资源构建、任务编排、常量等）。不直接耦合具体扩展件；连接器/插件由应用注册。
- resources/：运行期资源聚合与生命周期管理（WPL/OML/路由表/错误管道等）。
- runtime/：执行层（actor、采集、解析、下沉、监控任务）。不直接读取文件配置，透过 orchestrator 提供的已装配对象运行。
- sinks/：内置下沉实现与路由分发（仍建议优先使用 `extensions/sinks/` 的独立 crate）。
- sources/：内置数据源实现（如 syslog），按需注册到全局工厂。
- stat/：统计与指标聚合。
- utils/：小型通用工具（避免放置业务逻辑；已移除过期 syslog 发送端辅助）。
- types.rs：常用别名与小型类型定义。

依赖方向建议：apps → facade → orchestrator/runtime → core → crates（语言/模型/日志等）。extensions（sources/sinks/plugins）单独成 crate，经 `wp-*-api` 提供的 trait/factory 接口注入。

## 扩展点与最佳实践
- Source/Sink 连接器
  - 使用 API 层 trait：`wp-source-api`、`wp-sink-api`；实现 `SourceFactory`/`SinkFactory` 并在应用启动时注册。
  - 在 `src/sources/*` 仅保留通用实现（如 syslog）。注册入口：`wp_connector_api::SourceFactoryRegistry::global().register(...)`。
  - 组装时统一走 Resolved 结构：`wp_specs::Core*Spec → wp_conf::*::resolved::core_to_resolved[_with] → wp_*_api::Resolved*Spec`。
- 富化（Enrichment）
  - 仅通过 `wp-enrich-api` 或 `facade::enrich::*` 引用接口；禁止使用历史路径 `crate::ability::*`（已移除）。
  - 最小接口：`EnrichingAble`、`EnrichLibAble`，详见 `crates/apis/wp-enrich-api/src/lib.rs` 与 `src/facade/enrich.rs`。
- 配置与装配
  - orchestrator/config 负责解析 `conf/`（统一：[[sources]]、routes/connectors）并构建运行期对象。
  - 运行时装配在 `facade::engine` 内完成，apps 只需调用 `WpApp`。
  - 重要：仅支持统一配置；旧版回退路径已移除。见下文“统一配置与迁移”。
- 任务与并发
  - 统一使用 tokio；边界处使用 `anyhow::Result`，内部错误用 `wp_error::*` 细化原因。

## 代码风格与命名
- rustfmt 默认、四空格缩进、保留尾逗号。
- 模块/文件 `snake_case`；类型/枚举 `UpperCamelCase`；常量 `SCREAMING_SNAKE_CASE`。
- 可选行为一律用现有 features 控制（`core`、`kafka`、插件 flags）。

## 常用开发命令
- 构建：`cargo build --workspace --all-features`
- 仅主 CLI 快速迭代：`cargo build --workspace --features core --bins wparse`
- 运行参数查看：`cargo run --bin wparse -- --help`
- Lint/格式：`cargo fmt --all && cargo clippy --workspace --all-targets --all-features`
- 测试：`cargo test --workspace --all-features`（可追加 `-- --nocapture` 打印异步日志）

## 测试与验证
- 单元测试就近放在 `mod tests`；跨模块/流程放 `tests/` 或 `usecase/` 脚本。
- 变更场景流时运行 `usecase/core/getting_started/case_verify.sh`，仅在确认为期望行为变更时更新 `out/` 工件。

## 过期代码处理策略
- 尽量通过 Facade 或 API 层消除历史耦合；清理过期兼容代码前先定位调用方（`rg` 检索）。
- 不可立即删除的旧接口，使用 `#[deprecated]` 标注并在调用侧逐步迁移；确定无引用后移除。
- 本次清理：已删除 `src/utils/syslog.rs` 旧版发送端辅助；移除多处 `#![allow(dead_code)]`，鼓励尽快消除未使用项。

## 统一配置与迁移
- 现状：仅支持统一配置格式，旧版不再回退。
- 源配置（conf/wpsrc.toml）
  - 旧：`[[source_file]]/[[source_kafka]]/[[source_syslog]]` 与分散的 `source.wpl` 预解析
  - 新：统一 `[[sources]]` + connectors，syslog 由源内置解析，无需 `source.wpl`
  - 示例（file）：
    ```toml
    [[sources]]
    key = "file_1"
    type = "file"
    connect = "file_main"     # 指定 connectors/source.d 内的连接器 id
    enable = true
    tags = ["env:test"]
    params_override = { path = "./data/in_dat/gen.dat", encode = "text" }
    ```
    > 注：自 v1.5.1 起，`tags` 中的键值会在解析成功后自动写入数据记录（字段名沿用标签 key；若记录内已有同名字段则保持原值），因此可直接在 Sink 端观察/过滤标签。
  - 示例（syslog）：
    ```toml
    [[sources]]
    key = "syslog_1"
    type = "syslog"
    connect = "syslog_main"
    enable = true
    params_override = { addr = "0.0.0.0", port = 514, protocol = "UDP", strip_header = true }
    ```
- 生成配置（wpgen）
  - 旧：DataGenConf 已移除；不再提供旧格式文件与回退路径
  - 新：`WpGenConfig` 为磁盘主格式；运行期通过 `ConfManager::load_wpgen_resolved()` 解析 `output.connect` 和 `params_override`，产出可直接使用的 `SinkInstanceConf`
  - 用法示例（应用侧）：
    ```rust
    use wp_engine::facade::config::{ConfManager, WPGEN_TOML};
    let cm = ConfManager::new(".");
    let rt = cm.load_wpgen_resolved(WPGEN_TOML)?; // rt.conf + rt.out_sink
    ```
  - 失败提示：解析失败将报错并包含路径，便于定位

## 常见错误与排查
- 连接器工厂未注册：确认应用（如 `apps/wparse`）在启动前已 `register_*_factory()`。
- 规则加载失败：检查 `conf/` 路径、`WPL` 语法与 `orchestrator/config/loader` 的装配日志。
- Sink 验证失败：关注 `orchestrator::constants` 的字段命名与 utils.hints（错误打印里会给出示例）。

如需进一步完善本文档（示例代码、模块依赖图、协议时序等），请在 PR 中补充你负责的模块片段和验证命令。
