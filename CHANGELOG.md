# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Entries may be written in both English and Chinese.

## [1.21.9 Unreleased]

### Added
- **Diagnostics/CLI**: Error hints are now driven by `stable_code` (from `#[derive(OrionError)]`) as primary key, with bilingual Chinese/English support; language is selected via `WP_LANG` environment variable (fallback to `LANG` then `LC_ALL`).
  中文：错误提示改为以 `stable_code`（来自 `#[derive(OrionError)]`）为主键索引，支持中英双语；通过 `WP_LANG` 环境变量切换（fallback `LANG` → `LC_ALL`）。
- **Diagnostics/CLI**: Error output now shows `doing:` context (operation being performed when error occurred).
  中文：错误输出新增 `doing:` 上下文展示（发生错误时正在执行的操作）。
- **CLI/Help**: Added `after_long_help` documenting `WP_LANG` and `NO_COLOR` environment variables in `wparse` and `wproj` CLI help output.
  中文：在 `wparse` 和 `wproj` 的 CLI `--help` 输出中添加 `WP_LANG` 和 `NO_COLOR` 环境变量说明。

### Changed
- **Diagnostics/CLI**: Refactored `collect_hints` to use `stable_code` match branches (13 categories), with `detail` string matching only for subcategorization in 4 scenarios.
  中文：重构 `collect_hints`，使用 `stable_code` 匹配分支（13 个类别），仅在 4 种场景下通过 `detail` 字符串做二次细分。

## [1.21.8] - 2026-05-02

### Changed
- **Dependencies**: Upgraded `orion-error` from 0.6 to 0.7, adapting to the new `#[derive(OrionError)]` derive macro and updated trait paths (`ErrorOweBase`, `ToStructError`, `ContextRecord`).
  中文：升级 `orion-error` 从 0.6 到 0.7，适配新的 `#[derive(OrionError)]` 宏和更新的 trait 路径。
- **Error Handling**: Replaced `.map_err()` with idiomatic `.owe()` across sources, sinks, and runtime modules; migrated integration and unit tests from `anyhow::Result<()>` to `StructError<T>`-based Result types.
  中文：使用 `.owe()` 惯用模式替换 `.map_err()`，将测试从 `anyhow::Result` 迁移到基于 `StructError<T>` 的 Result 类型。

### Removed
- **Sinks/Rescue**: Removed unused `sink_err` helper method.
  中文：移除不再使用的 `sink_err` 辅助方法。

## [1.21.7] - 2026-04-27

### Added
- **Config/Engine**: Added `RepoGroupConf` for repository group configuration support in engine config.
  中文：新增 `RepoGroupConf` 支持引擎配置中的仓库组配置。

## [1.21.6] - 2026-04-27

### Changed
- **Release Merge**: Merged all changes from `1.20.7` into the `1.21.x` release line.
  中文：将 `1.20.7` 的全部变更合并到 `1.21.x` 发布线。

### Fixed
- **OML/SQL**: Fixed non-deterministic SQL parameter binding for multi-parameter `IN (...)` clauses by collecting `:param` values in SQL placeholder order instead of `HashMap` iteration order.
  中文：修复 SQL `IN (...)` 多参数绑定顺序不稳定的问题，现在按 SQL 中 `:param` 占位符出现顺序绑定，而不是依赖 `HashMap` 遍历顺序。
- **OML/SQL Cache**: Aligned SQL cache keys and query parameters to the same placeholder order for both sync and async evaluators, preventing partial matches such as only returning `server` while missing `db`.
  中文：同步和异步 SQL evaluator 的缓存键与查询参数现在使用同一占位符顺序，避免只命中 `server` 而漏掉 `db` 这类部分匹配结果。
- **wp-proj/Load Semantics**: Restored `WarpProject::load()` to load existing projects only; missing `conf/wparse.toml` now fails instead of being auto-created through `load_or_init`.
  中文：恢复 `WarpProject::load()` 只加载已有工程的语义；缺少 `conf/wparse.toml` 时返回错误，不再通过 `load_or_init` 自动创建配置。

## [1.21.5 2026-04-24]

### Added
- **Audit**: 新增 `.cargo/audit.toml`，忽略 RUSTSEC-2023-0071（rsa crate Marvin Attack 时序侧信道 — 仅影响 loopback TLS 且默认关闭，实际风险低，计划 2026-07-25 再评估）。

### Removed
- **Patches**: 移除未使用的 `include-flate-codegen` patch 及相关目录。


## [1.21.4] - 2026-04-22

### Fixed
- **Error Handling/Config Loading**: 修复 v1.21.3 引入的 `owe_conf_source` 在加载损坏 TOML 文件时触发 panic 的回归问题，恢复为 `err_conv` 链式错误转换。

### Changed
- **Version Control**: 从仓库中移除 `Cargo.lock`。


## [1.21.3] - 2026-04-22

### Added
- **Error System Docs**: 新增结构化错误系统设计文档与 review 清单。

### Changed
- **Error Diagnostics**: 重构错误诊断输出，改进嵌套错误的 reason/detail/root_cause 提取逻辑，支持 `ConfigError`、`Core` 等多种错误格式的详情展示，CLI 报错信息更完整可读。
- **Config Schema**: 配置解析开启 `deny_unknown_fields`，拼写错误的配置键将明确报错而非静默忽略。
- **Error Handling**: 统一 observability、config loading、project management 等链路的错误转换风格，错误信息附带路径上下文。

### Fixed
- **Runtime/Stats**: 修复统计切片过多导致的反压问题。
- **Config/Tests**: 修复 observability validate 测试与严格 config schema 的兼容性。


## [1.21.2] - 2026-04-22

### Fixed
- **OML/SQL**: 修复 SQL `IN (...)` 子句的参数绑定，`take(field)` 与临时变量可正确转换为 `IN` 绑定参数；补充对应测试用例。


## [1.21.1] - 2026-04-21

### Fixed
- **OML/Take**: 修复 `take(...)` 在目标记录与源记录同时存在同名字段时的移动顺序，优先消费当前目标记录中的已生成字段，避免前序 OML 字段被源记录同名值错误覆盖。
- **OML/SQL Parser**: 修复 SQL 参数提取对 `take(field)` 与 `__temp_var` 的识别，支持它们在 `=` 与 `IN (...)` 条件中稳定转换为绑定参数。
- **OML/SQL Parser**: 扩展严格 SQL 模式下的聚合函数校验，支持 `string_agg(distinct field, ',')` 这类合法表达式。

## [1.21.0] - 2026-04-20

### Changed
- **Dependencies**: 升级工作区依赖，包含 `jieba-rs 0.9`、`lru 0.17`、`ctor 0.10` 等版本更新。
- **Release Workflow**: 升级 GitHub Release Action 到 `softprops/action-gh-release@v3`。

### Fixed
- **OML/Take**: 修复 `take(...)` 只能从源记录取值的问题，现支持消费当前目标记录中已生成的字段，使前序 OML 字段可以被后续 `take(...)` 正确移动复用。
- **OML/SQL Parser**: 修复严格 SQL 模式下对 `group_concat(distinct ...)` 这类聚合表达式的校验与解析，并支持 `IN (@sip, @dip)`、`in(@sip, @dip)` 这类引用参数写法。

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
