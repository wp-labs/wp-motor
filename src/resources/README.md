# 资源管理系统

## 概述

wparse 资源管理系统为 wparse 数据处理管道提供了集中式的数据模型、处理规则和输出端(Sink)配置管理。

## 模块结构

```
resources/
├── core/                # 核心资源管理组件
│   ├── allocator.rs     # 资源分配策略
│   ├── manager.rs       # 主资源管理器
│   ├── types.rs         # 核心类型定义
│   └── mod.rs
├── indexing/            # 索引和映射功能
│   ├── mod.rs           # 索引显示工具
│   ├── model_index.rs   # 模型-输出端索引
│   └── rule_index.rs    # 规则-输出端索引
├── sinks/               # 输出端相关资源
│   ├── resources.rs     # 输出端资源包
│   ├── null.rs          # 空/默认资源实现
│   └── mod.rs
├── utils.rs             # 工具函数
└── mod.rs               # 模块导出
```

## 核心组件

### 资源管理器 (`core/manager.rs`)

`ResManager` 结构体提供以下功能：

- 模型加载和管理
- 规则-模型关联跟踪
- 输出端配置管理
- 解析资源分配

主要特点：
- 线程安全的资源访问
- 高效的索引快速查找
- 灵活的资源分配策略

### 资源分配器 (`core/allocator.rs`)

`ParserResAlloc` trait 定义了分配解析资源的接口：

```rust
pub trait ParserResAlloc {
    fn alloc_parse_res(&self, rule_key: &RuleKey) -> RunResult<Vec<SinkGroupAgent>>;
}
```

包含实现：
- 默认分配策略
- 空/默认资源分配
- 特定输出端分配

### 类型定义 (`core/types.rs`)

系统核心类型：
- `ModelName` - 数据模型标识符
- `RuleKey` - 处理规则标识符
- `SinkID` - 输出端配置标识符

## 索引系统

### 模型-输出端索引 (`indexing/model_index.rs`)

跟踪输出端与数据模型的关联：

```rust
pub struct SinkModelIndex(HashMap<SinkID, ModelNameSet>);
```

提供：
- 模型关联/取消关联
- 按输出端高效查找模型
- 调试用的显示格式化

### 规则-输出端索引 (`indexing/rule_index.rs`)

管理规则与输出端的关系：

```rust
pub struct SinkRuleRegistry {
    rule_sink_idx: HashMap<RuleKey, (SinkID, String)>,
    sink_rule_relation: SinkRuleMapping
}
```

特点：
- 基于模式的规则-输出端匹配
- 冲突解决(最长匹配模式优先)
- 双向关系跟踪

## 输出端资源

### 输出端资源包 (`sinks/resources.rs`)

```rust
pub struct SinkResUnit {
    aggregate_mdl: Vec<DataModel>,
    additions: DataModel
}
```

表示输出端所需的数据模型集合和额外资源。

### 空资源 (`sinks/null.rs`)

提供空/默认的资源分配实现：

```rust
pub struct NullResCenter {}
pub struct AssignRes { /* ... */ }
```

## 工具函数

主要工具函数包括：
- 从文件加载模型和规则
- 管道创建工具
- 解析器生成
- 源代码加载

## 使用示例

```rust
// 初始化资源管理器
let mut res_manager = ResManager::default();

// 加载模型和规则
res_manager.load_all_wpl_code(&conf, &sources, error_sink).await?;
res_manager.load_all_model(oml_root).await?;

// 为规则分配资源
let agents = res_manager.alloc_parse_res(&rule_key)?;

// 获取输出端资源
let sink_res = res_manager.alloc_sink_res(&sink_id).await?;
```

## 测试

系统包含全面的单元测试，包括：
- 索引管理
- 资源分配
- 模型加载
- 关联规则

运行测试：
```bash
cargo test --package wparse --lib -- resources::core::manager --exact --nocapture
```

## 性能考虑

- 索引使用高效的 HashMap/HashSet 实现
- 尽可能使用延迟加载
- 通过智能指针最小化克隆操作
- 支持并行加载

## 扩展点

1. 实现自定义 `ParserResAlloc` 进行特殊资源分配
2. 添加新索引类型以支持额外关系
3. 扩展 `SinkResUnit` 支持自定义资源类型

## 最佳实践

1. **资源初始化**：在系统启动时预先加载所有必要资源
2. **索引使用**：优先使用索引进行查找操作
3. **错误处理**：妥善处理资源分配失败情况
4. **生命周期管理**：注意资源生命周期的正确管理

## 常见问题

**Q: 如何添加新的数据模型类型？**

A: 扩展 `DataModel` 枚举并实现相关 trait，然后更新模型加载逻辑。

**Q: 如何处理资源冲突？**

A: 系统采用"最长匹配"原则解决冲突，也可自定义冲突解决策略。

**Q: 是否支持热重载？**

A: 当前版本需要重启服务加载新配置，未来版本计划支持热重载。
