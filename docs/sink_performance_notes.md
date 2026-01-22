# Sink 路径性能分析记录

## 主链路（ActSink / Dispatcher）
- `src/runtime/sink/act_sink.rs:203`: `proc_fix_ex` 在 `tokio::select!` 中直接 `await`，若恢复流程涉及外部重连会阻塞主消费循环，建议改为异步任务队列或快速确认后再异步处理。
- `src/runtime/sink/act_sink.rs:157` + `src/sinks/routing/dispatcher/mod.rs:117`: `group_sink_package` 以 `ProcMeta::abstract_info()` 构造 `String` 作为哈希 key，批量内每条记录都要新建字符串与 `HashMap` entry，建议为 `ProcMeta` 提供 `Hash`/`Eq` 并缓存 rule→Vec，或直接使用 `ProcMeta`/`rule_id` 作为 key。
- `src/sinks/routing/dispatcher/oml.rs:302` 之后：fanout 阶段无条件为所有 sink 评估匹配并 clone `ProcMeta` + `DataRecord`，即便 sink 已经 `!is_ready`。可以预先过滤就绪 sink 并复用匹配 buffer，减少无效的条件计算与 clone。
- `src/sinks/runtime/manager.rs:233`: `send_package_to_sink` 在重试过程中重复 `collect::<Vec<_>>`，遇到短暂失败会多次 clone records；可缓存 `Vec<Arc<DataRecord>>` 或 `Arc<[Arc<DataRecord>]>`，在重试循环中直接引用。

## OML 处理链
- `src/sinks/routing/dispatcher/oml.rs:59`: `get_match_oml` 每条记录都遍历 `aggregate_mdl()` 并逐条 `rule.matches`，复杂度与模型数量线性相关。可在 `SinkDispatcher` 中构建 rule→model 的哈希索引或至少缓存上一次命中的模型。
- `src/sinks/routing/dispatcher/oml.rs:120`: `run_oml_pipeline_vec` 的失败分支 `let mut failed = output.clone()`，会把空输出再 clone 一份；直接接管返回值即可，并提前 `Vec::with_capacity(input.len())` 以减少扩容。
- `src/sinks/routing/dispatcher/oml.rs:349`: `evaluate_sink_matches` 每次 fanout 都重新分配 `Vec<bool>`，可以在 dispatcher 里维护一个 scratch buffer（`Vec<bool>`）循环复用并在 push 前清零，从而避免大量短生命周期分配。
- `src/sinks/routing/dispatcher/oml.rs:374`: `append_pre_tags` 每次 fanout 都 clone `DataField`（包含 `String`），在多 sink 命中的场景下开销显著；可将 tags 预编译为 `Arc<[DataField]>` 或 `SmallVec` 并在追加时共享底层字符串。

> 以上结论均针对主消费路径与 OML fanout，不含 Infra default/miss/residue 相关支路。如需细化，可在此文档继续补充实验数据或修复建议。
