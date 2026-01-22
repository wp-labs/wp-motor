# Core Module - warp 数据处理核心引擎

## 目录结构
```
src/core/
├── error/            # 错误处理系统
│   ├── strategies/   # 错误处理策略
│   ├── macros.rs     # 错误转换宏
│   └── mod.rs        # 错误转换traits
├── generator/        # 数据生成器
│   └── rules.rs      # 生成规则实现
├── parser/           # 数据解析系统
│   ├── wpl/          # 规则处理核心
│   ├── indexing.rs   # 资源索引器
│   ├── setting.rs    # 解析配置
│   └── workshop.rs   # 数据处理车间
├── sinks/            # 数据输出系统
│   ├── async_sink.rs # 异步输出接口
│   └── sync_sink.rs  # 同步输出实现
├── sources/          # 数据输入系统
├── mod.rs            # 核心模块导出
└── prelude.rs        # 预导入项
```

## 核心功能

### 1. 数据处理流水线
```mermaid
graph LR
    S[Source] --> P[Parser]
    P --> G[Generator]
    G --> K[Sink]
```

### 2. 主要组件

#### 解析器系统 (Parser)
- `WplWorkshop`: 数据处理车间，管理多个解析管道
- `WplPipeline`: 单个解析规则管道
- `ParsingEngine`: 解析引擎（规则引擎/插件）

#### 生成器系统 (Generator)
- `GenRuleUnit`: 字段生成规则集合
- 支持动态加载TOML规则配置

#### 错误处理系统
- 三级策略模式：
  - `DebugPolicy`: 开发模式（严格）
  - `NormalPolicy`: 生产模式（容错）
  - `StrictPolicy`: 苛刻模式（重试）
- 错误报告生成：
  ```rust
  let report = ErrReport::new_wpl("parse failed")
      .add_code(source_code)
      .add_error(error);
  ```

#### 数据终端 (Sink)
- `SinkTerminal` 四种实现：
  ```rust
  pub enum SinkTerminal {
      Channel(DataChannel),    // 通道传输
      BlackHole(BlackHoleSink),    // 空实现
      Inspector(DebugView),   // 调试输出
      Storage(SinkBackend)    // 实际存储
  }
  ```

## 关键设计

### 1. 错误处理策略矩阵
| 错误类型       | Debug | Normal | Strict |
|----------------|-------|--------|--------|
| 数据错误       | Ignore| Ignore | Ignore |
| 资源错误       | Throw | Ignore | Retry  |
| 配置错误       | Throw | Ignore | Throw  |
| 规则语法错误   | Throw | Ignore | Ignore |

### 2. 性能优化点
- **解析器缓存**：高频规则LRU缓存
- **零拷贝处理**：`DataPacket` 支持bytes/string双模式
- **批量统计**：定时聚合上报监控数据

## 使用示例

### 基础数据处理流程
```rust
let workshop = WplWorkshop::from_code(repository, sink_agent)?;
let packet = source.recv().await?;
workshop.proc(packet, &setting)?;
```

### 自定义错误处理
```rust
let result = load_config().for_conf(ConfigError::LoadFailed);
match current_error_policy().handle_config_load(&result) {
    ErrorAction::Retry => { /*...*/ }
    ErrorAction::Ignore => { /*...*/ }
}
```

## 扩展建议

1. **新增Sink实现**：
   ```rust
   #[derive(Clone)]
   pub struct CustomSink { /*...*/ }

   impl SyncSink for CustomSink {
       fn send_to_sink(&self, data: SinkPayload) -> DeliveryResult {
           // 实现细节
       }
   }
   ```

2. **自定义解析规则**：
   - 继承`ParsingEngine` trait
   - 注册到`WplRepository`

3. **监控集成**：
   - 实现`StatRecorder` trait
   - 挂接到`ProcessingPipeline`





> 提示：生产环境建议使用`NormalPolicy`策略，配合监控系统观察`ErrReport`生成情况
