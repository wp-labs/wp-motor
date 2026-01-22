# Sinks Module

这个模块包含了数据输出（Sink）相关的所有组件。

## 目录结构

```
sinks/
├── README.md              # 本文件
├── mod.rs                 # 模块声明和公共导出
├── types.rs               # 核心类型定义（SinkBackendType, ASinkHandle等）
├── backends/              # 具体的 Sink 实现
│   ├── blackhole.rs       # 空实现（用于测试）
│   ├── file.rs            # 文件输出
│   ├── syslog.rs          # Syslog 输出
│   └── tcp.rs             # TCP 网络输出
├── decorators/            # 装饰器模式实现
│   ├── stub.rs            # 存根装饰器
│   └── test_proxy.rs      # 测试代理装饰器
├── net/                   # 网络传输层
├── routing/               # 路由和分发
│   ├── dispatcher/        # 事件分发器
│   └── agent.rs           # 路由代理
├── runtime/               # 运行时管理
│   └── manager.rs         # Sink 运行时管理器
├── utils/                 # 工具组件
│   ├── buffer_monitor.rs  # 缓冲区监控
│   ├── formatter.rs       # 数据格式化器
│   └── view.rs            # 数据查看器
└── test_helpers/          # 测试辅助工具
    └── mod.rs              # MockSink 等测试工具
```

## 核心组件

### SinkBackendType
代表不同类型的 Sink 后端实现，目前只支持 `Proxy` 类型。

### ASyncSink Trait
所有 Sink 需要实现的核心 trait，包含：
- `AsyncCtrl`: 控制接口（停止、重连等）
- `AsyncRecordSink`: 处理结构化数据记录
- `AsyncRawDataSink`: 处理原始数据（字符串、字节数组）

### 批量处理方法
为了提高性能，所有 Sink 实现都支持批量处理：
- `sink_records`: 批量处理多条数据记录
- `sink_str_batch`: 批量处理多个字符串
- `sink_bytes_batch`: 批量处理多个字节数组

## 性能优化

### 文件写入 (FileSink)
- 预分配缓冲区，减少内存分配
- 批量合并写入，减少系统调用
- 保持 100 条记录刷新一次的策略

### 网络传输 (TcpSink)
- 根据 framing 模式优化批量发送
- Line 模式：合并所有字符串，确保换行符
- Length 模式：分别构建带长度前缀的消息

## 使用示例

```rust
use wp_engine::sinks::{SinkBackendType, AsyncSink};

// 创建一个文件 Sink
let file_sink = build_file_sink("output.log", TextFmt::Json).await?;

// 创建 SinkBackendType 包装器
let backend = SinkBackendType::Proxy(Box::new(file_sink));

// 使用 Sink
let mut sink = backend;
sink.sink_str("Hello, World!").await?;
```

## 测试

测试辅助工具位于 `test_helpers` 模块，包含：
- `MockSink`: 轻量级模拟实现，适用于单元测试
- 其他测试工具函数

运行测试：
```bash
cargo test --package wp-engine --lib sinks
```