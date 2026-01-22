# AGENTS 使用指南（中文）

本文件面向在本仓库中协作的工程师与自动化 Agent。除特别说明外：
- 语言约定：默认使用中文进行沟通与文档撰写；命令、代码与标识符保持英文。
- 在提交前请确保格式化、静态检查与测试均通过。

## 仓库结构与模块组织
WP Engine 是一个 Cargo workspace：
- 共享的 Rust crates 位于 `src/`；
- Docker 与服务脚手架在 `dev-facilities/`；文档、任务与工具分别在 `docs/`、`tasks/`、`tools/`。

## 构建、测试与开发命令
- `cargo build --workspace --all-features` — 全量编译所有 crates 与二进制。
- `cargo build --workspace --features core --bins wparse` — 仅核心功能与主 CLI，便于快速迭代。
- `cargo run --bin wparse -- --help` — 查看运行时参数；更换二进制名可检查其他应用。
- `cargo fmt --all && cargo clippy --workspace --all-targets --all-features` — 代码格式化与静态检查，对齐 CI 要求。
- `cargo test --workspace --all-features` — 运行单元与集成测试；追加 `-- --nocapture` 以输出详细异步日志。

## 编码风格与命名约定
- 使用 rustfmt 默认风格（四空格缩进、尾随逗号等）。
- 模块与文件使用 `snake_case`；类型与枚举使用 `UpperCamelCase`；常量使用 `SCREAMING_SNAKE_CASE`。
- 异步边界优先使用 `anyhow::Result`；并发优先使用 `tokio` 原语。
- 可选行为通过既有 Cargo features 控制（如 `core`、`kafka`、各插件开关），避免自定义 cfg 散落。

## 测试规范
- 单元测试与实现同文件，位于 `mod tests`；集成或场景测试放在 `tests/` 或相应 `usecase/` 脚本中。
- 测试命名体现行为意图，如 `process_log_handles_missing_fields`。

## 提交与 Pull Request 规范
- 提交说明（subject）使用祈使句，70 字符以内，例如：
  `Add deployment plan and static implementation docs for externalization`。
- PR 需阐述动机与方案，列出验证命令（如 `cargo test --workspace --all-features`），注明启用的 feature flags，并为新增 CLI 流程附带日志或截图。

## 安全与配置建议
- 遵循 workspace 沙箱原则；不要提交任何密钥或云凭证。
- 使用 `dev-facilities/` 的 Docker 资源在本地启动 Kafka/MySQL 等依赖；`.env` 覆盖项放在个人忽略文件中（已被 Git 忽略）。

## 最佳实践与小贴士
- 变更跨多个 crate 时，先用精简 feature（如 `--features core`）在主 CLI 上迭代，再回到全量构建验证。
- 引入新插件或后端时，优先以 feature gate 封装边界，利用编译期隔离控制依赖与耦合。
- 对外部可见的 CLI 行为变更，请同步更新 `docs/` 与相关 `usecase/` 脚本。

### 人因优先与接口简化
- 面向用户的配置尽量保持“少而稳”的开关；能用布尔/枚举解决的不要暴露细粒度 knobs（阈值/步进/采样等）。
- 对实现细节的调参，默认以代码常量+自适应策略实现，避免要求用户理解复杂内部机制。
- 示例：TCP Sink 的“发送队列感知 backoff”仅提供 `max_backoff` 一个布尔开关；仅在“无限速”场景缺省开启，有限速强制关闭；其余细节（目标水位、采样周期、退让时长）由代码内部常量与闭环自动控制。

### 文档同步要求
- 当改动影响外部可见的参数或默认行为（例如 `max_backoff` 的自动开启条件），必须同时更新：
  - 用户参考：`docs/80-reference/params/sink_tcp.md`
  - CLI 手册：`docs/cli/wparse.md`、`docs/10-user/02-config/06-wpgen.md`
  - 设计文档：`docs/30-decision/01-architecture.md`、`docs/50-dev/design/sinks_loader.md`
