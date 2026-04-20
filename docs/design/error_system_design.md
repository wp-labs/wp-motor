# WP-Motor 错误体系设计

本文档定义 `wp-motor` / `wp-proj` / `wp-cli-core` / `wp-config` 的错误体系设计原则，并结合当前工程实际使用的 `orion-error 0.6` 与 `orion_conf 0.5` 能力，给出统一约束、推荐写法和反例。

## 1. 目标

错误体系不是“把错误传出去”这么简单，而是同时满足四个目标：

- 让代码能够基于错误语义做出正确处理
- 让 CLI / API 能输出对用户有帮助的信息
- 让排障时可以追踪到上游根因
- 让跨 crate、跨模块边界保持清晰契约

对于 `wp-motor` 这类大型工程，错误设计本质上是系统契约设计的一部分。

## 2. 结论

本工程采用以下总原则：

- 核心链路必须使用结构化错误，不以 `anyhow::Result` 作为主错误模型
- 跨层转换时必须优先保留 `source`，不要过早 `to_string()`
- `reason` 负责稳定分类，`detail` 负责当前层上下文，`source` 负责上游根因
- 顶层 CLI 负责把结构错误翻译成友好且可排障的输出
- `anyhow::Result` 仅允许存在于最外层、测试代码或非常薄的辅助边界

一句话概括：

> `anyhow` 适合传播错误，结构化错误适合建模错误。

## 3. 为什么要这样设计

如果在大型项目中长期把 `anyhow::Result` 当作默认错误模型，通常会出现这些问题：

- 错误失去类型语义，调用方无法区分配置错误、运行时错误、逻辑错误、可重试错误
- 边界层容易把错误压平成字符串，根因链被截断
- CLI 往往只能输出“配置错误”“执行失败”这类空泛信息
- 调用方不得不靠字符串匹配做分支判断
- 后续补错误码、上下文、文件路径、source chain 的成本很高

因此，本工程要求核心链路从一开始就按“结构化 reason + detail + source + context”设计。

## 4. 分层模型

### 4.1 基础设施层

基础设施层面向文件、网络、序列化、数据库、第三方库。

这一层的上游错误通常来自：

- `std::io::Error`
- `serde_json::Error`
- `glob::PatternError`
- `tokio` I/O 错误
- 第三方协议/编码库错误

这一层的职责不是直接把第三方错误往上传，而是：

- 转换成当前领域的结构化错误
- 保留原始 `source`
- 用 `detail` 补足“当前正在做什么”

例如：

```rust,ignore
sock.send(bytes).await.map_err(|e| {
    SinkReason::sink("udp send failed").err_source(e)
})?;
```

```rust,ignore
let val = general_purpose::STANDARD.decode(s.trim()).map_err(|e| {
    SourceReason::SupplierError("base64 decode error".to_string()).err_source(e)
})?;
```

### 4.2 领域层

领域层负责定义稳定的错误语义，不负责展示层的“人类友好美化”。

当前工程内的典型领域错误包括：

- `ConfIOReason`
- `RunReason`
- `SinkReason`
- `SourceReason`
- `OMLRunReason`

这一层的要求：

- `reason` 要稳定、少而准
- 不要把动态上下文塞进 reason 类型定义里
- 当前层的动态说明放进 `detail`
- 上游真实根因放进 `source`

例如：

- `SourceReason::Disconnect`
- `SourceReason::SupplierError`
- `SinkReason::Sink`
- `RunReason::from_conf()`

### 4.3 应用编排层

应用编排层主要在各 crate 边界做错误归一化。

典型位置：

- `wp-config` -> `wp-proj`
- `wp-cli-core` -> `wp-proj`
- `wp-engine` runtime -> `wp_error::RunError`

这一层的职责：

- 不重新发明错误类型
- 不直接丢弃上游错误链
- 给错误补 `path` / `operation` / `context`
- 必要时把底层错误提升为更高层 reason

例如 `wp-proj` 的统一转换辅助：

```rust,ignore
result
    .owe_conf_source()
    .with(path)
    .want("read wpsrc config")
```

或者：

```rust,ignore
some_result.to_run_err_source("load project config")?;
```

### 4.4 CLI / API 展示层

展示层的职责不是“重新构造错误”，而是消费结构错误：

- `reason`
- `detail`
- `source_frames`
- `root_cause_frame`
- `target_path`
- `display_chain`

当前工程里这一职责主要在：

- `src/facade/diagnostics.rs`

该层会从 `RunError` 中提取：

- 用户可读的主 reason
- `detail`
- 文件/位置
- TOML parse excerpt
- root cause
- hints

因此，只有底层链路保留了结构信息，CLI 层才能把排障信息完整展示出来。

## 5. `orion-error` 在本工程中的角色

`orion-error` 是本工程错误体系的核心基础设施。它提供的不只是一个错误类型，而是一整套结构化错误能力。

### 5.1 结构化错误对象

本工程主要基于：

- `StructError<R>`

其中：

- `R` 是具体领域 reason
- `StructError<R>` 持有 reason、detail、source、上下文等附加信息

### 5.2 稳定 reason 与错误码

领域 reason 通常实现：

- `ErrorCode`

这样可以为错误建立稳定错误码，用于：

- CLI exit code 映射
- 分类统计
- 监控与告警
- 对外契约

### 5.3 结构错误构造

本工程常见写法：

```rust,ignore
RunReason::from_conf().to_err()
```

```rust,ignore
ConfIOReason::from_validation().to_err()
```

```rust,ignore
SinkReason::sink("flush rescue file failed").err_source(e)
```

```rust,ignore
SourceReason::SupplierError("hex decode error".to_string()).err_source(e)
```

推荐使用：

- `to_err()` 构造空白结构错误
- `err()` / `err_detail()` / `err_source()` 这类领域辅助方法快速补充 detail/source

### 5.4 上下文附加

当前仓库中大量使用这些模式：

- `.with(path)`：附加目标对象、路径或上下文
- `.want("operation")`：附加当前想执行的动作
- `.with_detail(...)`：附加当前层语义说明
- `.with_source(e)`：保留上游根因

组合后的效果是：

- reason 稳定
- detail 可读
- source 可追踪
- path / operation 可定位

### 5.5 边界归一化

当前工程常用：

- `ErrorOwe`
- `ErrorOweSource`
- `ErrorWith`

用于把标准库错误或第三方错误统一归一化到结构错误体系里。

典型写法：

```rust,ignore
std::fs::read_to_string(path)
    .owe_conf_source()
    .with(path)
    .want("read config")
```

这类写法优于手工 `map_err(|e| ...)` 的场景包括：

- 文件读写
- 目录创建
- TOML/配置加载
- 边界层包装

原因是它更统一、更稳定，也更不容易漏掉 source/context。

### 5.6 展示与追踪

`orion-error` 还提供了面向诊断层的能力，例如：

- `display_chain()`
- `source_frames()`
- `root_cause_frame()`

这些能力直接决定了 CLI 是否能把错误拆解成：

- 主 reason
- detail
- location
- parse excerpt
- cause

因此，本工程明确要求：不要在中间层把结构错误压成普通字符串，否则这些能力会直接失效。

### 5.7 SourceFrame metadata 方向

当前 `source_frames()` 已能提供 `reason`、`want`、`path`、`detail` 等结构化信息，但对于“配置类型”这类诊断分类仍然不足。例如 CLI 需要区分 `wpsrc.toml`、`sinks/defaults.toml`、sink route 文件时，仍可能退化为基于文件名或 `Display` 文本的启发式判断。

后续建议在 `orion-error` 中独立推进 `SourceFrame` typed metadata 能力，让 `OperationContext` 能携带机器可读 metadata，并在 `SourceFrame` 中保留。详细 PR 需求与设计见：

- [Orion Error SourceFrame Metadata PR 设计](./orion_error_source_frame_metadata_pr.md)

## 6. 各 crate 的职责

### 6.1 `wp-config`

职责：

- 负责配置加载、环境变量展开、校验、路径定位
- 主错误模型应保持为 `OrionConfResult` / `StructError<ConfIOReason>`

要求：

- 所有配置加载入口优先返回结构化配置错误
- 配置错误必须能定位到文件、字段、实例、group 或 connector
- 不要在配置加载主链路返回 `anyhow::Result`

### 6.2 `wp-cli-core`

职责：

- 负责业务辅助逻辑、观测性统计、connector 浏览与校验等

要求：

- 能直接返回结构错误的函数，优先直接返回结构错误
- 若出于历史兼容暂时仍返回 `anyhow::Result`，必须在边界转换时补齐 `with_source`
- 不要在这里把配置错误降级成纯字符串

### 6.3 `wp-proj`

职责：

- 作为应用层与 CLI 层之间的归一化边界
- 对外统一收敛到 `RunResult`

要求：

- 将 `wp-config` / `wp-cli-core` / 标准库错误提升到 `RunError`
- 尽量使用统一 helper，例如：
  - `owe_conf_source().with(...).want(...)`
  - `to_run_err_source(...)`
- 不要在这里丢掉上游 source

### 6.4 `wp-engine`

职责：

- 运行时核心链路
- source/sink/runtime/orchestrator 领域错误传播

要求：

- source/sink 错误保持领域语义
- `SourceError` / `SinkError` / `RunError` 之间转换要保留 source
- runtime 主链路禁止把关键错误压平成 `e.to_string()`

## 7. 允许和禁止的模式

### 7.1 推荐模式

#### 模式 A：领域错误 + source

```rust,ignore
stream.shutdown().await.map_err(|e| {
    SinkReason::sink("tcp shutdown failed").err_source(e)
})?;
```

#### 模式 B：配置边界统一包装

```rust,ignore
std::fs::read_to_string(&path)
    .owe_conf_source()
    .with(&path)
    .want("read wpsrc config")?;
```

#### 模式 C：边界层提升到 `RunError`

```rust,ignore
result.to_run_err_source("load project config")?;
```

#### 模式 D：detail 表达当前动作，source 保留上游根因

```rust,ignore
RunReason::from_conf()
    .to_err()
    .with_detail(format!("parse config failed: {}", path.display()))
    .with_source(e)
```

### 7.2 不推荐模式

#### 反例 A：过早压平 source

```rust,ignore
map_err(|e| SinkError::from(SinkReason::Sink(e.to_string())))
```

问题：

- 丢失根因链
- CLI 无法提取 source frame
- 日志只剩一段字符串

#### 反例 B：核心边界直接返回 `anyhow::Result`

```rust,ignore
pub fn load_config(...) -> anyhow::Result<Config>
```

问题：

- 契约不清晰
- 调用方无法做分类处理
- 后续补 reason/code/detail/source 代价高

#### 反例 C：只给 reason，不给 detail/source

```rust,ignore
Err(RunReason::from_conf().to_err())
```

问题：

- 用户看不到有效排障信息
- 上游真实失败点丢失

## 8. `anyhow::Result` 的使用边界

本工程不把 `anyhow::Result` 视为禁用项，但必须收敛使用范围。

允许使用的地方：

- `main`
- 一次性工具
- 测试代码
- 非核心 glue code
- 临时过渡层

不应作为主错误模型的地方：

- 配置加载主链路
- runtime 主链路
- source/sink 运行边界
- CLI 诊断链路
- crate 对 crate 的稳定接口

规范结论：

- 如果函数属于核心领域接口，优先返回结构化错误
- 如果函数只是最外层调度或测试辅助，可以使用 `anyhow::Result`

## 9. 如何写 detail / reason / source

### 9.1 reason

要求：

- 稳定
- 分类性强
- 不携带过多动态上下文

正确方向：

- `Disconnect`
- `SupplierError`
- `Validation`
- `Sink`
- `Conf`

### 9.2 detail

要求：

- 描述“当前层正在做什么”
- 面向用户与排障
- 尽量简洁可读

正确示例：

- `read wpsrc config`
- `parse wpsrc config`
- `validate source 'file_1' path spec`
- `flush rescue file failed`

### 9.3 source

要求：

- 保留真实上游错误对象
- 不要轻易转换成字符串
- 由诊断层统一决定是否展示、如何展示

## 10. CLI 诊断层要求

顶层 CLI 错误输出必须尽量展示以下信息：

- 主 reason
- detail
- file/location
- parse excerpt
- root cause
- hints
- exit code

当前 `src/facade/diagnostics.rs` 已按这一方向演进，因此底层代码必须配合：

- 尽量保留 `source`
- 尽量补全 `detail`
- 路径类错误要补 `with(path)` / target

否则 CLI 再漂亮，也只能看到一层“配置错误”。

## 11. 测试要求

错误体系相关变更，测试不应只断言“报错了”，还应断言：

- 是否保留关键 detail
- 是否保留 source chain
- 是否能提取出文件路径/位置
- CLI 输出是否包含足够排障信息

建议至少覆盖：

- 缺少配置项
- 路径不存在
- TOML parse error
- source/sink I/O 失败
- 编码解码失败

## 12. 代码评审检查清单

评审错误相关代码时，至少检查以下问题：

1. 这是核心链路还是最外层辅助代码
2. 是否错误地使用了 `anyhow::Result`
3. 是否把上游错误压成了 `to_string()`
4. 是否有稳定 `reason`
5. 是否补了当前层 `detail`
6. 是否保留了 `with_source`
7. 是否补了 `path` / `operation` / `context`
8. CLI 最终是否能看到有效排障信息

## 13. 编码规范版

本节给出可以直接用于日常开发与 code review 的硬性规则。

### 13.1 强制规则

以下规则在核心链路中视为必须遵守：

1. 公共领域接口不得默认返回 `anyhow::Result`
2. 标准库 / 第三方错误跨模块边界时必须优先保留 `source`
3. 禁止在核心链路中使用 `map_err(|e| e.to_string())`
4. 禁止在核心链路中使用 `SinkReason::Sink(format!("{}", e))` 这类压平根因的写法
5. 配置加载错误必须尽量附带 `path`、`group`、`instance` 或字段定位
6. CLI 可见错误必须至少包含有效的 `reason` 与 `detail`
7. 运行时 source/sink 边界错误必须能追到真实 I/O 或编码错误
8. 如果 `orion-error` helper 能表达同样语义，优先使用 helper，而不是散落的手写包装

### 13.2 允许例外

以下情况允许不完全遵守上述规则，但要有明确理由：

- 测试代码
- 临时迁移适配层
- `main` 入口和极薄的 CLI glue code
- 只在本模块内部使用、不会跨边界暴露的短生命周期辅助函数

即便如此，也应遵守两个底线：

- 不要无意义丢失 source
- 不要把未来必然还会往上抛的错误先压平

### 13.3 review 处理等级

代码评审时，错误设计问题按以下等级处理：

#### 必须修改

- 核心接口返回 `anyhow::Result`
- 关键边界把错误压平成字符串
- 用户可见错误只有“配置错误”“执行失败”且无 detail
- 配置错误无法定位到文件或操作
- 运行时 I/O 失败丢失 source

#### 建议修改

- 已有结构错误，但 detail 过于笼统
- 可以使用 `owe_conf_source().with(...).want(...)` 却手写重复样板
- `to_run_err(...)` 可提升为 `to_run_err_source(...)`
- 存在多个近似包装风格，影响一致性

#### 可接受

- 测试中使用 `anyhow::Result`
- 临时迁移期间的薄包装
- 不跨模块暴露的小型辅助逻辑

### 13.4 选择矩阵

遇到错误转换时，优先按下表选择：

#### 场景 A：标准库 / 第三方错误 -> 配置错误

优先：

```rust,ignore
result
    .owe_conf_source()
    .with(path)
    .want("read config")
```

适用：

- 文件读取
- 目录创建
- TOML 加载
- 配置解析

#### 场景 B：标准库 / 第三方错误 -> 运行时领域错误

优先：

```rust,ignore
map_err(|e| SinkReason::sink("write failed").err_source(e))
```

或：

```rust,ignore
map_err(|e| SourceReason::Disconnect("tcp recv failed".to_string()).err_source(e))
```

适用：

- socket I/O
- sink flush/write
- source recv/read
- 编码解码

#### 场景 C：结构错误 -> `RunError`

优先：

```rust,ignore
result.to_run_err_source("load project config")
```

如果上游错误不满足 `StdError`，退化为：

```rust,ignore
result.to_run_err("load project config")
```

#### 场景 D：需要补当前层 detail，但上游已是结构错误

优先：

```rust,ignore
RunReason::from_conf()
    .to_err()
    .with_detail("...")
    .with_source(e)
```

### 13.5 helper 使用约束

在当前工程里，优先使用以下 helper：

- `owe_conf_source().with(...).want(...)`
- `to_run_err_source(...)`
- `to_run_err_with_source(...)`
- `reason.err_source(...)`
- `reason.err_detail(...)`

使用建议：

- 如果错误来自 `std::io::Error` / `serde_json::Error` / `glob` / 解码库，优先 `err_source(...)`
- 如果只是补业务语义且无上游错误对象，使用 `err_detail(...)`
- 如果是配置 I/O 和加载边界，优先 `owe_conf_source().with(...).want(...)`

### 13.6 detail 写法规范

`detail` 的推荐格式：

- 动词 + 对象 + 必要限定

例如：

- `read wpsrc config`
- `parse wpsrc config`
- `validate source 'file_1' path spec`
- `flush rescue file failed`
- `count lines for source 'nginx' at /tmp/a.log`

不要写成：

- `error`
- `failed`
- `配置错误`
- `something went wrong`

### 13.7 `with(path)` / `want(...)` 使用规范

以下场景应优先补 `with(path)`：

- 读写文件
- 解析配置文件
- 清理目录
- 加载模板
- 处理 connector / topology / project 文件

以下场景应优先补 `want(...)`：

- 当前操作语义需要明确展示给用户
- 同一底层 API 被多个操作复用，单纯 `source` 不足以说明问题
- CLI 希望给出“正在做什么”而不是只给出“哪里报错了”

示例：

```rust,ignore
std::fs::write(path, body)
    .owe_conf_source()
    .with(path)
    .want("write generated topology")?;
```

### 13.8 禁止样板

以下写法在核心链路中直接视为反模式：

```rust,ignore
map_err(|e| anyhow::anyhow!("{}", e))
```

```rust,ignore
map_err(|e| RunReason::from_conf().to_err().with_detail(e.to_string()))
```

```rust,ignore
map_err(|e| SinkReason::Sink(format!("send error: {}", e)).to_err())
```

```rust,ignore
let _ = some_result.ok();
```

### 13.9 迁移优先级

当需要从旧风格迁移到新错误体系时，优先级如下：

1. CLI 用户直接可见的错误链路
2. 配置加载主链路
3. source/sink 运行时边界
4. crate 对 crate 的公共接口
5. 内部辅助函数
6. 测试代码

这意味着：

- 先修复“看不出原因”的用户错误
- 再修复“根因丢失”的运行时错误
- 最后再处理纯内部一致性问题

### 13.10 新代码默认模板

新代码写错误处理时，可优先从以下模板出发。

#### 配置加载模板

```rust,ignore
let body = std::fs::read_to_string(&path)
    .owe_conf_source()
    .with(&path)
    .want("read config")?;
```

#### 运行时 I/O 模板

```rust,ignore
socket.readable().await.map_err(|e| {
    SourceReason::Disconnect("wait socket readable failed".to_string()).err_source(e)
})?;
```

#### sink 写入模板

```rust,ignore
self.writer.flush().await.map_err(|e| {
    SinkReason::sink("flush sink writer failed").err_source(e)
})?;
```

#### 应用层转换模板

```rust,ignore
result.to_run_err_source("load project config")?;
```

## 14. 当前工程的推荐准则

可以把本工程错误体系压缩成以下几条规则：

1. 核心领域接口优先返回结构化错误
2. 标准库和第三方错误在边界层统一映射到结构错误
3. 跨层转换必须优先保留 `source`
4. `detail` 描述当前动作，`reason` 描述稳定分类
5. CLI 层负责把结构错误翻译成友好输出，不在底层拼展示文案
6. 配置错误必须尽量携带文件、字段、group、实例定位信息
7. 测试要验证错误链是否可追踪，而不是只验证失败本身

## 15. 推荐写法速查

### 文件读取

```rust,ignore
let content = std::fs::read_to_string(&path)
    .owe_conf_source()
    .with(&path)
    .want("read config")?;
```

### 运行时 I/O

```rust,ignore
socket.recv_from(&mut buf).await.map_err(|e| {
    SourceReason::Disconnect("udp recv_from failed".to_string()).err_source(e)
})?;
```

### sink 写入

```rust,ignore
self.writer.flush().await.map_err(|e| {
    SinkReason::sink("flush rescue file failed").err_source(e)
})?;
```

### 边界提升为 `RunError`

```rust,ignore
result.to_run_err_source("load project config")?;
```

## 16. 反模式速查

以下写法在核心链路中应视为设计退化信号：

- `Result<T, String>`
- `anyhow::Result<T>` 直接作为核心领域接口返回值
- `map_err(|e| e.to_string())`
- `SinkReason::Sink(format!("{}", e))`
- `SourceReason::Disconnect(e.to_string())`
- CLI 最终只输出“配置错误”“执行失败”

## 17. 总结

本工程的错误体系不是“所有错误都做成复杂枚举”，而是强调：

- 分类稳定
- 语义清晰
- 根因可追
- 边界统一
- CLI 可诊断

`orion-error` 为这一体系提供了统一基础设施：

- `StructError`
- `ErrorCode`
- `ToStructError`
- `ErrorOwe` / `ErrorOweSource`
- `ErrorWith`
- `display_chain()`
- `source_frames()`
- `root_cause_frame()`

项目规范可以归纳为一句话：

> 核心链路必须使用结构化错误表达语义；跨层转换必须保留 source；CLI 负责把结构错误翻译成友好且可排障的输出；`anyhow::Result` 仅限最外层和非核心辅助场景。
