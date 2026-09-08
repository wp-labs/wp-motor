# WP-Motor

<div align="center">

**A high-performance data parsing & enrichment engine, written in Rust.**

[![CI](https://github.com/wp-labs/wp-motor/workflows/CI/badge.svg)](https://github.com/wp-labs/wp-motor/actions)
[![codecov](https://codecov.io/gh/wp-labs/wp-motor/graph/badge.svg?token=6SVCXBHB6B)](https://codecov.io/gh/wp-labs/wp-motor)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Docs (zh)](https://img.shields.io/badge/docs-中文-blue)](docs/usage/zh/README.md)
[![Docs (en)](https://img.shields.io/badge/docs-English-blue)](docs/usage/en/README.md)

</div>

WP-Motor is the engine core of the [Warp Parse](https://github.com/wp-labs/warp-parse) project. It turns
raw, semi-structured data (logs, network flows, metrics …) into clean, structured records through a
declarative two-stage pipeline:

1. **WPL** — a parsing rule language that extracts typed fields from raw text;
2. **OML** — a transformation language that enriches, computes, aggregates, and optionally joins the
   parsed data with SQL backends (*KnowDB*), before it is handed to a sink.

```
raw data ──▶ WPL parse ──▶ OML enrich / SQL enrich ──▶ output sink
             (typed fields)     (structured records)      (file / tcp / kafka / …)
```

---

## Why WP-Motor

- **Two declarative DSLs with one mental model.** Rules are data — version them, review them, ship them
  without touching Rust code.
- **Fast by construction.** Tokio-based async runtime with a picker → parser → sink pipeline, batching,
  bounded back-pressure, zero-copy static symbols, and a synchronous fast-path for in-memory evaluation.
- **Real-time enrichment.** OML can run SQL-style lookups (`select … from provider.table`) against
  PostgreSQL/MySQL/SQLite with query result caching, plus IP-geo style integer-key lookups
  (`ip_to_biguint`).
- **Plug-in connectors.** Sources and sinks are registered factories (TCP, UDP, file, Kafka, syslog, …),
  so the engine core stays decoupled from protocol implementations.
- **Observable and resilient.** Per-stage statistics, monitor table output, rescue/error routing, and
  generation-scoped cache invalidation make it operable in production.
- **Enterprise option.** Commercial backend features ship behind a dedicated feature flag and license.

---

## Highlights

| Area | What you get |
|------|--------------|
| WPL parsing | `package/rule` grammar, typed fields (`ip`, `time`, `chars`, `digit`, …), separators, `@field` references, `copy_event_parse`, `#[no_match]` |
| OML transformation | `read`/`take`, `calc(...)`, pipe functions (`nth`, `map_to`, `on_fail`, time/IP/base64/URL …), nested `object {}` / `array {}`, conditions, static dictionaries, `access_direct`/`intranet_ip` network enrichment |
| SQL enrichment | `select` with `where`/`order by`/`limit`, multi-provider routing via `from <provider>.<schema>.<table>`, result caching |
| Runtime | async multi-worker pipeline, adaptive batching, back-pressure, rescue/residue outputs, graceful shutdown |
| Observability | stage statistics (`pick/parse/sink`), interval + total tables, cache hit telemetry |
| Extensibility | connector factories, OML pipe-functions, memory profiles (`low/standard/throughput`) |

A quick taste — WPL rule and its OML processor:

```wpl
package nginx {
  rule access_log {
    (ip:client_ip, time:timestamp, chars:request_uri, digit:status)
  }
}
```

```oml
name : nginx_processor
rule : /nginx/access_log        # binds to the WPL package/rule above
---
client : ip = read(client_ip) ;
status_class = calc(read(status) / 100) ;
masked = pipe read(client_ip) | intranet_replace ;   # privacy: mask intranet IPs
```

---

## Getting started

The engine is consumed by the application repositories of the Warp Parse family:

- **[warp-parse](https://github.com/wp-labs/warp-parse)** — the CLI distribution (`wparse`, `wpadm`,
  `wpgen`) and the release you probably want to run;
- **[wp-examples](https://github.com/wp-labs/wp-examples)** — runnable models and benchmark scenarios;
- **[wp-connectors](https://github.com/wp-labs/wp-connectors)** / **wp-core-connectors** — source & sink
  connector implementations.

To build and test the engine itself:

```bash
cargo build --release        # engine + workspace crates
cargo test                   # unit & integration tests
cargo clippy --all-targets --all-features -- -D warnings
```

User documentation (bilingual):

- 中文指南: [`docs/usage/zh/README.md`](docs/usage/zh/README.md)
- English docs: [`docs/usage/en/README.md`](docs/usage/en/README.md)
- Design notes & developer guides: [`docs/`](docs/README.md)
- Release history: [`CHANGELOG.md`](CHANGELOG.md)

---

## Repository layout

`wp-engine` (this workspace root) hosts the engine and its shared crates:

| Crate | Purpose |
|-------|---------|
| `src/` (`wp-engine`) | Async runtime: sources/pickers, parsers, sinks, orchestrator, facade, statistics |
| `crates/wp-oml` | OML transformation language: parser, evaluator, pipe functions |
| `crates/wp-config` | Configuration loading, validation, memory/limit profiles |
| `crates/wp-condition` | Condition parser built over the expression engine |
| `crates/wp-data-utils` | Shared data utilities |
| `crates/wp-cli-core` | CLI shared infrastructure (`wpadm`/`wproj` tooling) |
| `crates/wp-proj` | Project scaffolding & workflow helpers (incl. the `wpgen` data generator core) |
| `crates/wp-stats` | Statistics collectors |
| `crates/orion_exp` / `orion_overload` | Expression evaluation & common primitives |

Companion language & knowledge crates live in sibling repositories: **WPL** grammar ships as the
`wp-lang` crate, and the SQL/geo knowledge layer lives in
[wp-knowledge](https://github.com/wp-labs/wp-knowledge) (KnowDB).

## Feature flags

| Flag | Description |
|------|-------------|
| `default` (= `runtime-core`) | Community edition with the core runtime |
| `enterprise-backend` | Enterprise-only backend features (requires license) |
| `perf-ci` | Performance benchmarking for CI pipelines |
| `dev-tools` | Development tools and diagnostics |

---

## Contributing

Contributions are welcome — bug reports, docs, tests, and code.

1. Search existing [issues](https://github.com/wp-labs/wp-motor/issues) before opening a new one.
2. For changes, keep the style of the codebase: run `cargo fmt` and
   `cargo clippy --all-targets --all-features -- -D warnings`, add tests for new behavior, and update
   the bilingual docs and `CHANGELOG.md` where relevant.
3. Open a pull request and mention the related issue.

Questions and design discussion happen in
[GitHub Discussions](https://github.com/orgs/wp-labs/discussions).

## License

Apache License 2.0 — see [LICENSE](LICENSE).

---

# WP-Motor（Warp 解析引擎）

<div align="center">

**用 Rust 编写的高性能数据解析与富化引擎。**

</div>

WP-Motor 是 [Warp Parse](https://github.com/wp-labs/warp-parse) 的引擎内核：把原始半结构化数据（日志、
网络流量、指标等）经两级声明式流水线转为结构化记录：

1. **WPL** —— 解析规则语言，从原始文本提取带类型字段；
2. **OML** —— 转换语言，对解析结果做富化、计算、聚合，并可经 **KnowDB** 关联 SQL 查询后再输出到
   sink。

```
原始数据 ──▶ WPL 解析 ──▶ OML / SQL 富化 ──▶ 输出 sink
            （类型化字段）    （结构化记录）      （file / tcp / kafka / …）
```

## 核心能力

- **WPL 解析**：`package/rule` 文法、类型化字段（`ip`/`time`/`chars`/`digit`…）、分隔符、`@field` 引用、
  `copy_event_parse`、`#[no_match]`；
- **OML 转换**：`read`/`take`、`calc(...)`、管道函数（时间/IP/base64/URL/`map_to`/`on_fail`…）、嵌套
  `object {}`/`array {}`、条件/映射、静态字典、`access_direct`/`intranet_ip`/`intranet_replace` 网络富化；
- **SQL 富化**：`select` + `where/order by/limit`、`from <provider>.<schema>.<table>` 多库路由、查询结果
  缓存、`ip_to_biguint` 数值键查询；
- **运行时**：tokio 异步多 worker 流水线（picker→parser→sink）、批量与自适应背压、零拷贝静态符号、
  同步快路径；
- **可观测与韧性**：pick/parse/sink 分段统计、interval/total 表格、rescue/error 分流、graceful 关闭；
- **可扩展**：sink/source 连接器工厂注册、OML pipe 函数扩展、内存档位（low/standard/throughput）。

## 快速开始

引擎被 Warp Parse 家族的应用仓库消费：

- [warp-parse](https://github.com/wp-labs/warp-parse) —— CLI 发行版（`wparse`/`wpadm`/`wpgen`）；
- [wp-examples](https://github.com/wp-labs/wp-examples) —— 示例模型与压测场景；
- [wp-connectors](https://github.com/wp-labs/wp-connectors) / wp-core-connectors —— 源/输出连接器实现。

本地构建与验证引擎：

```bash
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

- 中文文档：[`docs/usage/zh/README.md`](docs/usage/zh/README.md)
- 英文文档：[`docs/usage/en/README.md`](docs/usage/en/README.md)
- 设计与开发指南：[`docs/`](docs/README.md)
- 变更记录：[`CHANGELOG.md`](CHANGELOG.md)

## 仓库结构

`wp-engine`（本 workspace 根）承载引擎与共享 crate：

| Crate | 用途 |
|-------|------|
| `src/`（`wp-engine`） | 异步运行时：源/picker、parser、sink、编排、门面、统计 |
| `crates/wp-oml` | OML 转换语言：解析、求值、管道函数 |
| `crates/wp-config` | 配置加载/校验、内存与限流档位 |
| `crates/wp-condition` | 基于表达式引擎的条件解析 |
| `crates/wp-data-utils` | 共享数据工具 |
| `crates/wp-cli-core` | CLI 公共设施（`wpadm`/`wproj`） |
| `crates/wp-proj` | 工程脚手架与工作流（含 `wpgen` 生成器内核） |
| `crates/wp-stats` | 统计收集 |
| `crates/orion_exp` / `orion_overload` | 表达式求值与公共原语 |

WPL 文法以 `wp-lang` crate 形式独立发布；SQL/地理知识层来自
[wp-knowledge](https://github.com/wp-labs/wp-knowledge)（KnowDB）。

## 特性开关

| Flag | 说明 |
|------|------|
| `default`（= `runtime-core`） | 社区版：核心运行时 |
| `enterprise-backend` | 企业版后端特性（需授权） |
| `perf-ci` | CI 性能基准 |
| `dev-tools` | 开发工具与诊断 |

## 参与贡献

欢迎以 issue、文档、测试与代码方式参与：

1. 开新 issue 前先检索现有 [issues](https://github.com/wp-labs/wp-motor/issues)；
2. 提交代码前请运行 `cargo fmt` 与 `cargo clippy --all-targets --all-features -- -D warnings`，
   为新行为补充测试，并按需更新双语文档与 `CHANGELOG.md`；
3. 提交 PR 并关联相关 issue。

问题与设计讨论见 [GitHub Discussions](https://github.com/orgs/wp-labs/discussions)。

## 许可证

Apache License 2.0 —— 见 [LICENSE](LICENSE)。

---

**Warp Parse Dev Team**
