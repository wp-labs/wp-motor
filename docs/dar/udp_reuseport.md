## UDP `SO_REUSEPORT` 多实例设计评审

### 背景

曾考虑为 `syslog_udp_src` 引入 `instances` 参数，并启用 `SO_REUSEADDR` + `SO_REUSEPORT`，目标是在一个端口上启动多个 source task，消除单 socket recv loop 的瓶颈。

### 决策：不采用 `SO_REUSEPORT` 多实例方案

经过评审，决定 **不使用 `SO_REUSEPORT`，也不支持多实例**。理由如下：

#### 1. 跨平台行为不一致

| 特性 | Linux `SO_REUSEPORT` (3.9+) | BSD/macOS `SO_REUSEPORT` |
|------|---------------------------|-------------------------|
| 允许多 socket bind 同端口 | ✅ | ✅ |
| **UDP unicast 负载均衡** | ✅ 内核 hash 分发 | ❌ 无此逻辑 |
| UID 限制（防劫持） | ✅ (Linux 4.6+) | ❌ |

- **Linux**：`SO_REUSEPORT` 具备内核级负载均衡，按 4-tuple hash 分发 datagram。
- **macOS/BSD**：虽然能 `bind` 多个 socket，但内核只把流量投递给其中一个（通常是最先 bind 的），**不做负载均衡**。这是 BSD 的原始语义，不是 bug。

参考：[LWN - The SO_REUSEPORT socket option](https://lwn.net/Articles/542629/)

#### 2. 安全风险

`SO_REUSEPORT` 允许同 UID 的其他进程 bind 到相同端口并拦截流量。即使 `instances=1`，默认启用此选项也会扩大攻击面。

#### 3. 业界实践

主流项目（Nginx、HAProxy、Envoy）均采用 **opt-in** 策略，不默认启用 `SO_REUSEPORT`：

```nginx
# Nginx: 需显式配置
listen 514 udp reuseport;
```

```yaml
# HAProxy: 需显式配置
bind :514 reuseport
```

#### 4. 包顺序问题

多实例 + 内核 hash 分发意味着来自同一源的数据包可能被分配到不同实例，破坏隐式的包顺序假设。

#### 5. 调试复杂度

流量分散到多个实例后，问题排查变得困难。

### 替代方案（如未来需要扩展）

如果未来确实遇到单 socket 收包瓶颈，可考虑：

| 方案 | 优点 | 缺点 |
|------|------|------|
| **单 socket + recvmmsg 批量**（当前已实现） | 简单、安全、跨平台 | 仍有单核上限 |
| **单 socket + 多 worker channel** | 安全、可控、保序 | 用户态分发有开销 |
| **多端口 + 前端负载均衡** | 无需特殊内核支持 | 需要外部 LB |
| **显式 opt-in SO_REUSEPORT** | 用户知情选择 | 仅 Linux 有效 |

### 当前实现

- UDP source 使用单 socket
- 不设置 `SO_REUSEPORT`
- 不支持 `instances` 参数
- 保留 `recvmmsg()` 批量读取优化（Linux）

### 附录：为什么 TCP 吞吐可能超过 UDP

在高 EPS 场景下，TCP syslog 的吞吐量可能反超 UDP，这看似反直觉，但根源在于 **syscall 效率和内核队列架构** 的差异。

#### TCP 架构优势

```
┌─────────────────────────────────────────────────────────────┐
│                      TCP Source                              │
├──────────────┬──────────────┬──────────────┬────────────────┤
│ Connection 1 │ Connection 2 │ Connection 3 │ Connection N   │
│ (kernel buf) │ (kernel buf) │ (kernel buf) │ (kernel buf)   │
└──────┬───────┴──────┬───────┴──────┬───────┴────────┬───────┘
       │              │              │                │
       ▼              ▼              ▼                ▼
   1 次 read()    1 次 read()    1 次 read()      1 次 read()
   = 多条消息     = 多条消息     = 多条消息       = 多条消息
```

- **一次 `read()` 可读取数 KB ~ 数 MB 数据**（取决于 `tcp_recv_bytes` 配置）
- 多个连接 = 多个独立的内核缓冲区
- 连接间轮询，天然并行

#### UDP 架构瓶颈

```
┌─────────────────────────────────────────────────────────────┐
│                      UDP Source                              │
│                                                              │
│                   ┌──────────────┐                          │
│                   │ 单个 Socket  │                          │
│                   │ (1 个内核队列)│                          │
│                   └──────┬───────┘                          │
│                          │                                   │
│                          ▼                                   │
│                   1 次 recv_from()                          │
│                   = 1 个 datagram                            │
└─────────────────────────────────────────────────────────────┘
```

- **一次 `recv_from()` 只能读取 1 个 UDP 包**
- 即使用 `recvmmsg()`，批量上限也只有 64 个包
- 所有流量共用一个 socket 的内核队列

#### 量化对比

假设消息平均 200 字节：

| 指标 | TCP | UDP |
|------|-----|-----|
| 单次 syscall 读取数据量 | 10 MB (`tcp_recv_bytes`) | 200 B ~ 64×200 B |
| 单次 syscall 消息数 | ~50,000 条 | 1 ~ 64 条 |
| 内核队列数 | N 个（每连接一个） | 1 个 |
| CPU 利用率瓶颈 | 用户态处理 | syscall 开销 |

#### 为什么 `recvmmsg` 不够

`recvmmsg` 只解决了"一次系统调用收多个包"的问题，但：

1. **批量上限**：通常 64-128 个包/次，远小于 TCP 单次读取量
2. **内核队列仍是单点**：高流量下队列溢出 → 丢包
3. **仅 Linux 可用**：macOS 没有 `recvmmsg`

#### 结论

UDP 在极高 EPS 场景下输给 TCP，不是网络带宽问题，而是：
- **syscall 粒度太细**（每次只能拿 1 个包，或 recvmmsg 最多 64 个）
- **单队列瓶颈**（所有流量共用一个 socket 内核缓冲区）

如果 UDP 高性能是硬需求，真正的解决路径是 `SO_REUSEPORT`（Linux）、`io_uring`、或 `AF_XDP/eBPF`。当前选择不用这些方案是基于跨平台和安全性考量的权衡。

### 参考资料

- [LWN - The SO_REUSEPORT socket option](https://lwn.net/Articles/542629/)
- [LinuxJedi - Socket SO_REUSEPORT and Kernel Implementations](https://linuxjedi.co.uk/2020/04/25/socket-so_reuseport-and-kernel-implementations/)
- [GeeksforGeeks - Difference Between SO_REUSEADDR and SO_REUSEPORT](https://www.geeksforgeeks.org/linux-unix/difference-between-so_reuseaddr-and-so_reuseport/)
