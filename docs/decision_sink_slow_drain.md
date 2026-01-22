# 设计决策：Sink 慢速 drain 行为

## 背景
- 在 `wp-examples/core/slower_sink` 中，`slow_bkh` sink 通过 `sleep_ms=100` 将处理速率限制为每秒 10 条。
- batch 任务在 2 秒内运行完 source/pick/parse 并立即收到控制面的 `Stop(Immediate)`，sink 线程在 backlog 尚存时被强制退出，导致 10k 输入中仅 3840 条成功。
- daemon 模式在停机或操作指令触发 `Stop(Immediate)` 时也会提前退出，通道剩余数据被直接丢弃。

## 问题陈述
`SinkDispatcher` 将 parse 完成的数据送入容量 128 的 `tokio::sync::mpsc`。`SinkWork::async_proc` 使用 `tokio::select!` 同时监听数据和控制命令，当 `TaskController`(push Stop) 时立即 `break`，紧接着调用 `sink.proc_end()`/`sink_rt.primary.stop()`，从未继续 drain channel，造成 slow sink 在退出阶段丢数。该缺陷在 batch/daemon/all sink types（包括 infra sinks）均存在。

## 决策
1. **两阶段关停**：Stop 不再直接 `break`，而是切换到 drain 状态，仅消费 channel 直到 sender 关闭。
2. **挂账计数**：借助 `group_sink_package` 返回的条数维护 backlog 计数，只有 `recv()` 返回 `None` 且 `pending == 0` 才停止 sink runtime。
3. **统一覆盖**：`async_proc` 与 `async_proc_infra` 同步改造，所有 sink（default/miss/residue/monitor 等）都先 drain 后退出。
4. **可观测/超时**：drain 阶段输出日志（`start sink drain` / `pending_left=...`），daemon 模式可配置 drain timeout（默认 60s），超时后记录告警并强制退出。

## 方案细节
- 在 `SinkWork::async_proc` 中引入 `DrainState`，Stop 到达时只关闭 `SinkDatYSender` 并继续 `recv()`，由 `DrainState` 追踪 `pending` 与 `is_drained`。
- `DrainState` 新增 `pending.inc_by(group_size)`/`pending.dec_finished()` API，配合 sink 的 `group_sink_package` 可在批处理完成时扣减。
- `SinkDatYReceiver` 关闭后 `recv()` 会返回 `None`，此时若 `pending` 仍大于 0 则阻塞等待剩余批次完成；否则进入真正的 `proc_end()`。
- Infra sink 的 `async_proc_infra` 共享相同逻辑，保证监控/兜底 sink 也能在停机前清空队列。
- drain timeout 作为守护线程配置项（默认启用），用于防止异常 sink 无法退出。

## 影响与权衡
- **正向**：slow sink 不再因 Stop 丢数，batch 任务统计值与输入一致；daemon 停机阶段也能保证“先处理完再退出”。
- **代价**：Stop 后需等待 backlog 被 drain，慢 sink 下停机所需时间变长，需要在运维手册中提示；若 drain timeout 生效将再次 drop 剩余数据。
- **可观测性**：通过新增日志与 monitor sink/metrics 可清晰观察 drain 耗时、剩余 backlog。

## 备选方案
- **仍保持立即退出**：实现最简单，但无法满足 batch/daemon 对数据完整性的要求，被拒绝。
- **在 dispatcher 层面直接拒绝 slow sink**：无法覆盖长尾慢点或临时波动，并不能解决停机瞬间的 backlog。

## 验证
- `src/runtime/sink/act_sink.rs` 在 Stop 阶段调用 `close_channel()` 并依据 `DrainState` 判断退出时机。
- `src/runtime/sink/drain.rs` 新增单元测试覆盖“未进入 drain 直接关闭”“drain 后关闭”“receiver::close 解锁”等场景。
- 运行 `wp-examples/core/slower_sink/run.sh`，slow sink (`sleep_ms=100`) 能成功在 drain 完 backlog 后再退场，10k 数据全部成功。

## 后续计划
1. （按需）开放 drain timeout 配置：当前默认值可以覆盖大多数场景，但若某些 daemon 任务需要更短/更长的停机窗口，需要提供可配置接口，并在运维手册中说明使用建议。
2. 在 monitor sink/metrics 中输出 drain 耗时与 backlog：便于运行期观察 slow drain 行为，评估是否需要调整 sink 并发、sleep 参数或手动触发更长超时。
