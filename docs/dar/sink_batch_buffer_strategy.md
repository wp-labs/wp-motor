## Sink `batch_size` 与 pending 缓冲路径决策记录

### 背景

- `SinkRuntime` 原本固定走 pending 缓冲（累计到 `batch_size` 再 flush）。
- 在 `blackhole` 这类无真实 I/O 成本的 sink 上，pending 缓冲会引入额外开销（push/flush/缓冲管理），收益不明显。
- 压测诉求是：同一实现内对比“走缓冲路径”和“直通路径”，并避免引入额外用户配置复杂度。

### 问题陈述

- 当入站包大小接近或超过 `batch_size` 时，继续进入 pending 再 flush 通常没有收益。
- 若新增显式模式开关（如 `mode=on/off`），会增加配置复杂度，也不符合“按输入自适应”的目标。

### 决策

1. **不新增 mode 配置**，仅基于 `batch_size` 与入站包大小自动选择路径。
2. 在 `send_package_to_sink` 内采用如下策略：
   - 若 `pending_records` 为空且 `package.len() >= batch_size`：**直接直通发送**（绕过 pending 缓冲）。
   - 否则：进入 `pending_records`，累计到阈值后 flush。
3. `batch_size` 配置做下限保护，最小为 `1`。

### 设计细节

#### 路由规则

- 直通条件：`pending_records.is_empty() && package.len() >= batch_size`
- 缓冲条件：其余情况全部走 pending 路径

该规则满足：

- 大包快速路径：减少无意义缓冲开销
- 小包聚合路径：保留 pending 的聚合作用
- 已有积压优先：当 pending 非空时，继续按缓冲语义处理，避免行为跳变

#### 错误处理语义

- 抽象统一批发送函数，确保两条路径共享同一套错误处理策略。
- 对于缓冲路径的 `Throw` 分支，保留“回填 pending”语义，确保可恢复性。

### 命名整理（可读性改进）

- `batch_pending` → `pending_records`
- `flush_pending_records` → `flush_pending_buffer`
- `send_package_direct` → `send_package_bypass_buffer`
- `send_record_batch` → `send_records_batch`
- `put_back_on_throw` → `requeue_on_throw`

目标是让“数据结构是什么、函数做什么、参数语义是什么”一眼可读。

### 验证与结果

#### 单测

- `small_package_stays_in_pending_buffer_until_flush`
- `large_package_bypasses_pending_buffer`

分别验证：

- 小包未达阈值时不会立即下发，需 `flush()` 才发送；
- 大包满足条件时直接直通，后续 `flush()` 不会重复发送。

#### 基准（BlackHole）

- 基准文件：`benches/sink_batch_pending_blackhole.rs`
- 两条路径：
  - `buffered_path_bsz_*`：强制 `batch_size > package_size`，走缓冲路径
  - `bypass_path_bsz_*`：`batch_size <= package_size`，触发自动直通

一次样例结果（`WF_BENCH_LINES=1024 WF_BENCH_BATCH_SIZE=512`）：

- buffered: ~`30.07µs`
- bypass: ~`28.53µs`

说明：在 BlackHole 场景下，直通路径更快，符合“无 I/O 时缓冲开销占主导”的预期。

### 影响面

- 对外配置：不新增参数；`batch_size` 仍是唯一相关配置项。
- 运行行为：由“固定缓冲”变为“按输入大小自适应缓冲/直通”。
- 可维护性：通过命名统一降低后续理解成本。

### 后续建议

- 在真实 I/O sink（如 file/tcp）上复用同一基准方法做对比，确认自动策略在不同后端下的收益边界。
- 若后续出现新的路径判定需求，优先扩展内部策略逻辑，继续避免暴露复杂用户开关。

