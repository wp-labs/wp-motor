# TCP Source Worker 重构记录

## 背景
- 现状：所有数据源共享同一个 `ActPicker::run_dispatch_loop()`；当 `speed_limit=0`（File→BlackHole 等场景）时，loop 几乎无休眠，导致用户态 CPU 极高（~80%）。
- TCP source 在最新版本中改为“源内直接轮询连接”，虽减少通道 hop，但在单 picker 模型下会阻塞其它源，吞吐降至 ~26–28W EPS。

## 诊断结论
1. `ActPicker` 仍在同一任务中轮询所有源，`try_receive()/receive()` 组合导致 busy-loop。
2. File 等非网络源也被迫执行 TCP 风格的轮询/阻塞逻辑，无法利用自身的异步 I/O 特性。
3. 用户态 CPU 占比高、EPS 降低并非源实现慢，而是调度层缺乏 per-source 隔离。

## 改进方向
- **一 SourceInstance 一 Worker**：遍历 `SourceSvcIns.sources`，为每个 `SourceHandle` 启独立的 router worker，负责 `start -> loop { receive -> 分发 } -> close`。
- `ActPicker` 仅维护 parse workers 的 RollingQueue/backoff，不再亲自 busy-loop 拉数据。
- Worker 内可继续复用当前 `read_batch_nonblocking/blocking` 逻辑，但在自身任务里运行，避免拖累其他源；必要时在无限速模式下也加入最小 idle（避免纯 CPU 自旋）。
- TCP 侧仍用 `TcpListenerLoop + TcpSource` 直读；File 等源按原实现 `receive()` 即可，在独立 worker 中公平调度。

## 后续任务
1. 设计 `SourceWorker`（或等价抽象）结构，并让 `ActPicker::dispatch_data` 仅负责 spawn worker。
2. 调整 `runtime/collector` 中 `picker_group` 的生命周期管理，确保控制指令（Stop/Isolate）可传递至各 worker。
3. 在新模型下复测 File→BlackHole、TCP sink 等场景，目标恢复至 80–90W EPS。
