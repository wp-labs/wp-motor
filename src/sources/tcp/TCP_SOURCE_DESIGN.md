# TcpSource 设计目标与实现文档

## 设计目标

### 1. 高性能零拷贝TCP数据接收
- **零拷贝架构**: 使用 `Arc<[u8]>` 避免数据复制，从网络直接到处理流水线
- **分离式架构**: 连接管理与数据读取分离，提高并发性能
- **配置驱动**: 支持动态配置监听地址、缓冲区大小、连接数限制

### 2. 生产级可靠性
- **连接生命周期管理**: 自动处理连接建立、维护、清理
- **错误恢复**: 连接断开自动检测和清理
- **超时机制**: 连接空闲超时自动回收（默认5分钟）
- **资源限制**: 最大并发连接数限制（默认1000）

### 3. 多协议帧支持
- **行帧模式**: 以 `\n` 为分隔符的消息格式
- **长度帧模式**: 长度前缀的消息格式
- **自动检测模式**: 智能检测消息格式（支持优先换行）

## 当前实现

### 架构设计

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   TcpSource     │    │ ConnectionWorker │    │ TcpConnection   │
│                 │    │                  │    │                 │
│ DataSource      │◄──►│ 监听 + 连接管理    │◄──►│ 单个TCP连接      │
│ Interface       │    │                  │    │                 │
│                 │    │ • 接受新连接      │    │ • 缓冲区管理     │
│ • recv()        │    │ • 清理死连接      │    │ • 活跃度检测     │
│ • start()       │    │ • 连接池维护      │    │ • 数据读取       │
│ • close()       │    │                  │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌──────────────────┐
                    │ ConnectionPool   │
                    │                  │
                    │ Arc<RwLock<      │
                    │ HashMap<u64,     │
                    │ TcpConnection>>>│
                    └──────────────────┘
```

### 核心组件

#### 1. TcpSource (主要接口)
```rust
pub struct TcpSource {
    // 配置参数
    key: String,
    address: String,           // 监听地址 (如 "127.0.0.1:19001")
    tcp_recv_bytes: usize,     // 接收缓冲区大小
    framing: FramingMode,      // 消息帧模式

    // 连接管理
    connection_pool: ConnectionPool,
    max_connections: usize,

    // 工作线程管理
    connection_worker: Option<JoinHandle<SourceResult<()>>>,
    stop_tx: Option<broadcast::Sender<()>>,

    // 状态
    started: bool,
}
```

**职责**:
- 实现 `DataSource` trait 接口
- 管理连接工作线程生命周期
- 从连接池中读取并解析数据
- 构造零拷贝 `SourceEvent`

#### 2. ConnectionWorker (连接管理)
```rust
struct ConnectionWorker {
    key: String,
    address: String,           // 监听地址
    max_connections: usize,
    tcp_recv_bytes: usize,
    connection_pool: ConnectionPool,
    stop_tx: broadcast::Sender<()>,
}
```

**职责**:
- 监听指定TCP端口
- 接受新连接并加入连接池
- 定期清理超时/断开的连接
- 响应停止信号

#### 3. TcpConnection (单个连接)
```rust
struct TcpConnection {
    stream: TcpStream,
    client_addr: SocketAddr,
    buffer: BytesMut,          // 接收缓冲区
    last_activity: Instant,    // 最后活跃时间
}
```

**职责**:
- 维护单个TCP连接状态
- 管理接收缓冲区
- 跟踪连接活跃度

### 数据流程

#### 1. 启动流程
```
1. TcpSource::new(address, tcp_recv_bytes, framing)
   ↓
2. TcpSource::start()
   ↓
3. 创建 ConnectionWorker + ConnectionPool
   ↓
4. ConnectionWorker::run()
   ↓
5. TcpListener::bind(address) - 开始监听指定地址
   ↓
6. 进入连接管理循环
   - 接受新连接 (非阻塞1ms超时)
   - 清理死连接
   - 响应停止信号
```

#### 2. 数据接收流程
```
1. TcpSource::recv() 被调用
   ↓
2. 遍历所有活跃连接
   ↓
3. 对每个连接: connection.try_read()
   ↓
4. 如果有数据:
   - 更新连接活跃度
   - 调用 FramingExtractor 提取消息
   ↓
5. 如果提取到完整消息:
   - 构造零拷贝 SourceEvent
   - 设置客户端IP地址
   - 返回事件
   ↓
6. 如果无数据: 短暂休眠1ms后继续循环
```

#### 3. 消息帧处理
```rust
match self.framing {
    FramingMode::Line => FramingExtractor::extract_line_message(buffer),
    FramingMode::Len => FramingExtractor::extract_length_prefixed_message(buffer),
    FramingMode::Auto => {
        FramingExtractor::extract_length_prefixed_message(buffer)
            .or_else(|| FramingExtractor::extract_line_message(buffer))
    }
}
```

### 配置集成

#### 工厂模式配置解析
```rust
// factory.rs - TcpSourceSpec::from_params()
struct TcpSourceSpec {
    addr: String,              // 默认: "0.0.0.0"
    port: u16,                 // 默认: 9000
    tcp_recv_bytes: usize,     // 默认: DEFAULT_TCP_RECV_BYTES
    framing: FramingMode,      // 默认: Auto
}
```

#### 使用示例配置
```toml
# topology/sources/wpsrc.toml
[[sources]]
enable = true
key = "tcp_1"
connect = "tcp_src"
params = {
    port = 19001,                    # 监听端口
    addr = "127.0.0.1",             # 监听地址
    tcp_recv_bytes = 8192           # 缓冲区大小
}
```

## 实现特点

### ✅ 已实现特性

1. **分离式架构**: ConnectionWorker + TcpSource 分离连接管理和数据读取
2. **配置驱动**: 完全支持动态配置地址、端口、缓冲区大小
3. **零拷贝**: 使用 `Arc<[u8]>` 避免数据复制
4. **连接池管理**: 高效的并发连接管理和清理
5. **多帧支持**: Line/Len/Auto 三种消息帧模式
6. **错误恢复**: 自动检测和清理断开连接
7. **DataSource接口**: 完整实现标准接口

### ⚠️ 当前限制

1. **try_recv支持有限**: 由于异步/同步桥接限制，`try_recv()` 总是返回 `None`
2. **配置传递**: `address` 和 `tcp_recv_bytes` 参数通过结构体存储传递（已修复）
3. **连接超时**: 固定5分钟超时，暂不支持配置
4. **错误处理**: 连接错误时日志记录但继续处理其他连接

### 🔧 性能优化

1. **非阻塞连接接受**: 使用1ms超时避免阻塞数据读取
2. **读写分离**: 连接管理与数据读取在不同异步任务中
3. **缓冲区复用**: 使用 `BytesMut` 高效缓冲区管理
4. **连接池优化**: 使用 `Arc<RwLock<HashMap>>` 支持并发访问

## 测试验证

### 单元测试覆盖
- ✅ TcpSource 创建和配置
- ✅ 连接池操作
- ✅ 消息帧提取和事件构造
- ✅ try_recv 接口限制文档

### 集成测试场景
- ✅ Line/Len 帧模式端到端测试
- ⚠️ 需要网络环境 (WP_NET_TESTS=1)
- ⚠️ 多连接并发测试

### 性能测试
- 🔄 高并发连接性能测试
- 🔄 大数据量吞吐测试
- 🔄 内存使用优化验证

## 未来改进方向

1. **配置增强**: 支持连接超时、心跳间隔等可配置参数
2. **性能监控**: 添加连接数、吞吐量等指标
3. **SSL/TLS支持**: 支持加密连接
4. **连接池优化**: 实现连接预热、负载均衡等高级特性
5. **错误恢复**: 支持连接重试、断线重连等机制

---

**文档状态**: 当前实现 v1.1.6
**最后更新**: 2025-11-24
**维护者**: TcpSource开发团队
