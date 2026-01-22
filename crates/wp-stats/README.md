# WP Stat - 统计收集与分析库

## 概述

WP Stat 是一个高性能的 Rust 统计收集与分析库，提供灵活的统计维度定义、实时数据收集和报告生成功能。

## 主要特性

- **多维度统计**：支持基于任意数据维度的统计收集
- **灵活配置**：可配置统计目标和收集规则
- **高效缓存**：使用 LRU 缓存管理统计记录
- **多种统计阶段**：支持生成(Gen)、采集(Pick)、解析(Parse)、输出(Sink)等不同阶段
- **类型安全**：基于 Rust 强类型系统保证统计安全

## 快速开始

### 添加依赖

```toml
[dependencies]
wp_stat = { path = "crates/wp_stat" }
```

### 基本用法

```rust
use wp_stat::{
    StatCollector, StatReq,
    StatTarget, StatStage,
    DataDim
};

// 创建统计收集器
let req = StatReq {
    stage: StatStage::Pick,
    name: "example".into(),
    target: StatTarget::All,
    collect: vec!["key1".into(), "key2".into()],
    max: 100,
};
let mut collector = StatCollector::new("target".into(), req);

// 记录统计事件
collector.record_begin("rule1", DataDim::new("data1"));
collector.record_end("rule1", DataDim::new("data1"));

// 生成统计报告
let report = collector.collect_stat();
println!("{}", report);
```

## 核心组件

### 主要结构体

- `StatCollector`: 统计收集器，核心工作组件
- `StatReq`: 统计请求配置
- `StatReport`: 统计报告
- `DataDim`: 统计维度数据
- `MeasureUnit`: 度量单位实现

### 主要 Trait

- `StatRecorder`: 统计记录能力
- `SliceMetrics`: 切片度量接口
- `Mergeable`: 可合并接口
- `SlicesMetadata`: 切片元数据

## 模块结构

```
wp_stat/
├── collector/   # 统计收集器实现
├── model/       # 数据模型
│   ├── dimension.rs  # 统计维度
│   ├── measure.rs    # 度量单位
│   ├── record.rs     # 记录结构
│   ├── request.rs    # 统计请求
│   └── stat_dim.rs   # 统计维度构建
├── report/      # 报告生成
├── traits/      # 特征定义
└── lib.rs       # 主模块
```

## 高级用法

### 自定义统计标签

```rust
use wp_stat::{SlicesMetadata, SlicesType};

#[derive(Clone, Default, Debug)]
struct CustomTag;

impl SlicesMetadata for CustomTag {
    fn slices_type() -> SlicesType {
        SlicesType::Diy
    }
    fn slices_name() -> String {
        "custom".into()
    }
}
```

### 合并统计报告

```rust
let mut report1 = collector1.collect_stat();
let report2 = collector2.collect_stat();

report1.merge(report2);
```

## 性能建议

1. 合理设置 LRU 缓存大小
2. 批量处理统计记录
3. 使用 `record_complete` 替代分开的 begin/end 调用

## 贡献指南

欢迎提交 Issue 和 PR。请确保:
1. 所有测试通过
2. 添加适当的文档
3. 保持代码风格一致

## 许可证

MIT
