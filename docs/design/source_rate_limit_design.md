# Source 输入限速设计

## 结论

`performance.rate_limit_rps` 是当前 wparse runtime 内所有 source 共享的全局输入 EPS 上限。

- `rate_limit_rps = 0`：自动限速。runtime 根据 source pending 水位、parser 背压和 RSS 快速增长情况自动调整输入速率。
- `rate_limit_rps > 0`：固定限速。所有 source 合计不超过该输入速率，不是每个 source 各自一份额度。
- 限速点在通用 source fetch 路径，不绑定 TCP，因此适用于 TCP、syslog、UDP、file、kafka 等 source 类型。

默认推荐同时使用：

```toml
[performance]
rate_limit_rps = 0
```

```bash
WP_MEMORY_PROFILE=low
```

不设置 `WP_MEMORY_PROFILE` 时等价于 `low`。

## 用户配置

### 输入限速

配置位置：

```toml
[performance]
rate_limit_rps = 0
```

含义：

- 单位：events per second。
- 作用范围：当前 runtime 的所有 source。
- `0` 表示自动限速。
- `> 0` 表示固定全局 source 限速。

示例：

```toml
[performance]
rate_limit_rps = 1650000
```

这表示当前 runtime 所有 source 合计约 `165W EPS`。

### 内存 Profile

内存相关队列、水位和批大小由 `WP_MEMORY_PROFILE` 统一控制。生产部署优先选择 profile，不建议同时暴露多个单项环境变量给用户。

| profile | 用途 | 核心参数 |
|---|---|---|
| `low` | 默认推荐，优先控制 RSS | `parser/sink channel = 32/16`，`sink_batch_size = 256`，`picker_burst_max = 4`，`pending = 1MB` |
| `standard` | 兼顾吞吐和 RSS | `parser/sink channel = 48/24`，`sink_batch_size = 512`，`picker_burst_max = 6`，`pending = 2MB` |
| `throughput` | 复杂样本或快 sink | `parser/sink channel = 96/48`，仍保留 `burst6/pending2M` |

兼容别名仍可解析，但不作为主文档推荐：

- `small` / `tiny` / `xs` -> `low`
- `large` / `high` -> `throughput`
- `default` / `normal` / `balanced` -> `standard`

## 设计目标

1. `rate_limit_rps` 对所有 source 类型生效。
2. 多 source 共享同一个总速率上限，避免按 source 数放大。
3. 将限速点前移到 source fetch 路径，减少 pending 和 source 队列持续膨胀。
4. 固定限速路径保持轻量，不在每条 event 上竞争全局锁。
5. 自动限速只在周期采样点调节目标速率，不在热路径做复杂判断。

## 非目标

当前实现不追求：

1. 任意 1 秒滑动窗口严格硬上限。
2. source 间严格公平调度。
3. 在通用 `DataSource` 接口读取前精确知道下一批 event 数。
4. TCP 内核 socket buffer 级别的硬限流。

这些能力需要 source 接口支持按 event budget 拉取，或在具体 source 内部做 batch 切分。

## 运行时结构

限速器集中在：

```text
src/runtime/actor/limit.rs
```

核心类型：

```rust
pub struct SourceRateLimiter { ... }
pub struct SourceRateLease { ... }
```

创建位置：

```text
src/runtime/tasks/pick.rs
```

`start_picker_tasks()` 根据 `RunArgs.speed_limit` 创建一个全局 `SourceRateLimiter`，再 clone 给所有 `SourceWorker`。

使用位置：

```text
src/runtime/collector/realtime/picker/fetch.rs
src/runtime/collector/realtime/picker/dispatch.rs
src/runtime/collector/realtime/picker/worker.rs
```

自动限速控制器：

```text
src/runtime/collector/realtime/picker/auto_limit.rs
```

它不直接读 source，也不直接写 parser/sink，只根据 `SourceWorker` 在 dispatch loop 中采集到的信号调整 `SourceRateLimiter` 当前目标速率。

## 处理流程

整体路径：

```text
wparse.toml
  -> EngineConfig.performance.rate_limit_rps
  -> RunArgs.speed_limit
  -> start_picker_tasks()
  -> SourceRateLimiter::new(speed_limit)
  -> SourceWorker::new(..., shared_limiter.clone())
  -> SourceRateLimiter::new_lease()
  -> fetch_into_pending()
  -> SourceRateLease::consume(batch.len())
```

固定限速：

```text
rate_limit_rps > 0
  -> 所有 source worker 共享同一条全局 deadline
  -> source worker 通过本地 lease 批量申请额度
  -> 额度不足时才访问全局 limiter
```

自动限速：

```text
rate_limit_rps = 0
  -> limiter 以保守初始速率启动
  -> pending 低水位且 dispatch 有进展时升速
  -> pending 上升、parser channel full 或 RSS 快速增长时降速
```

## 固定限速算法

`SourceRateLimiter` 内部维护一个共享 deadline：

```text
next_deadline
```

每次申请 `N` 个 event 额度时：

```text
wait = max(next_deadline - now, 0)
next_deadline = max(next_deadline, now) + N / rate_limit_rps
```

如果 `wait > 0`，当前 source worker sleep；否则立即获得额度。

本地 lease 用于减少锁竞争：source worker 先消耗本地剩余额度，额度不足时才向全局 limiter 申请一段新额度。这样高 EPS 场景不会变成每个 event 一次全局锁。

## 自动限速算法

自动限速使用 AIMD 风格控制：

```text
pending 低水位且本轮有投递进展
  -> 升速

pending bytes 达到高水位
  -> 降速

pending bytes 持续增长并超过低水位
  -> 降速

parser channel full / reloading 导致分发侧 pending
  -> 降速

RSS 在采样窗口内快速增长
  -> 降速
```

默认从 `1W EPS` 起步，不随 worker 数直接放大。原因是合理初始值取决于样本复杂度、规则成本、CPU、sink 类型和远端状态；MySQL/HTTP 等慢 sink 可能连 `10W EPS` 都无法承受。

启动后的前几秒允许更快探测上限；进入稳定期后降低升速幅度，减少长期震荡和 RSS 波动。

RSS 只作为保护信号，不作为默认目标水位。内存目标应通过输入速率和 `WP_MEMORY_PROFILE` 控制，避免把某个样本或机器上的 RSS 阈值固化成通用默认。

## 准确性与性能

长期平均速率接近配置值；瞬时速率是 batch/lease 粒度的软限速。

误差来源：

1. 通用 `DataSource` 读取前不知道下一批 `SourceBatch` 的 event 数。
2. 单个 batch 较大时会形成 batch 级突发。
3. 多个 source 各自持有本地 lease，瞬时突发可能叠加。
4. `tokio::time::sleep` 和系统调度会引入毫秒级抖动。

性能开销：

- 固定限速主要开销是本地 lease 不足时的一次 mutex、deadline 计算和必要 sleep。
- 自动限速只在采样窗口到期时调整目标速率，不在每条 event 上做控制判断。
- 已进入 pending 的数据优先继续向 parser 投递，不再叠加旧 picker/post 阶段 soft throttle。

## 当前调优结论

`standard` profile 来自 nginx 165W/8 worker 低内存调优：

```text
parser/sink channel = 48/24
sink_batch_size = 512
picker_burst_max = 6
picker_pending_max_bytes = 2MB
tcp_batch_bytes = 64KB
```

在该测试条件下，稳定窗口 RSS 约 `192-204 MB`，且可以跑满 `165W EPS` 固定限速。

多样本测试结论：

- nginx：`standard` 是默认推荐。
- 默认部署：`low` 是默认 profile，优先避免背压时 RSS 被放大。
- APT/mix：在更复杂样本或更高目标速率下，可切换到 `throughput`。
- AWS ELB：单条日志和规则成本更高，吞吐上限明显低于 nginx；优先通过 `rate_limit_rps` 控制输入速率，不建议单纯放大 channel 追速。

## Benchmark 约定

`wp-examples/benchmark` 下的 `wparse.toml` 使用：

```toml
rate_limit_rps = ${RATE_LIMIT_RPS:0}
```

benchmark 脚本默认将输入速率同步给 `RATE_LIMIT_RPS`。例如：

```bash
./run.sh nginx 2500000 -w 8 -c 100000000
```

表示：

- `wpgen` 目标输入速率为 `250W EPS`
- `wparse` 固定 source 限速也为 `250W EPS`

如果要测试自动限速，使用：

```bash
./run.sh nginx 0 -w 8 -c 100000000
```

## 内部诊断

代码中保留少量内部诊断开关，用于开发、压测或故障定位。它们不是稳定用户接口，不写入部署文档，也不建议作为生产配置暴露。
