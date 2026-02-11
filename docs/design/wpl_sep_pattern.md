# WPL 分隔符模式化与性能优化设计

## 背景

- 现有 `WplSep` 支持的分隔符只有简单字面量（`\0`、`\s` 等），解析时依赖逐字节比较或 `str::find`。
- 用户场景中常见的分隔条件不仅是固定字符串，还包括"跳过任意空白 + 特定前缀""遇到 `key=` 才截断"等模式。
- 缺少模式描述导致：
  1. 配置冗长，需要多个 stage 组合才能表达一个分隔逻辑；
  2. runtime 需要多次 `trim`、`regex` 才能满足需求，性能大幅下降。

## 目标

1. 在分隔符层面引入 **Glob 模式语法**，使用 `*` `?` 通配符，便于一次性描述"空白 + 关键字""转义字符"等需求。
2. 提前将模式编译成高效 matcher，运行时避免反复构建。
3. 对旧配置、现有 API 保持兼容：纯字面量仍按原逻辑处理，`\s` `\0` 等转义保持原有语义。
4. 允许 parser 在已知模式的前提下进行更 aggressive 的扫描（memchr / SIMD）。

## 模式语法

新增 `{ ... }` 形式的模式字面量，可在 `chars`、`digit`、`sep` 等位置使用。

### 语法表

| 记法 | 说明 |
|------|------|
| `a` | 字面字符 `a` |
| `*` | 零或多个任意字符（非贪婪，匹配最短） |
| `?` | 恰好一个任意字符 |
| `\0` | 空字节 |
| `\n` | 换行 |
| `\t` | 制表符 |
| `\r` | 回车 |
| `\s` | 一个或多个连续空白 `[ \t\r\n]+` |
| `\h` | 一个或多个连续水平空白 `[ \t]+` |
| `\\` `\*` `\?` `\{` `\}` `\(` `\)` | 字面转义 |
| `(...)` | 匹配但不消费（preserve，仅限模式末尾） |

### 关键语义说明

**`*` 非贪婪匹配**：`*` 始终采用最短匹配策略，找到第一个满足后续模式的位置即停止。
例如输入 `"a=b=c"`，模式 `{*=}` 匹配 `"a="`，而非 `"a=b="`。

**`\s` 匹配空白区域**：`\s` 匹配一个或多个连续空白字符，而非恰好一个。
在分隔符场景中，"跳过一段空白"远比"恰好匹配一个空白"常见。`\h` 同理，匹配一个或多个空格/制表符。
> 兼容性：上述扩展语义仅在 `{}` 模式内部生效；`{}` 之外沿用现有 `\\s`/`\\S` 定义，确保旧配置保持原样。

**`()` 保留标记（Preserve）**：`()` 内的内容参与匹配以确认分隔位置，但不从输入流中消费。
匹配结束后，输入流的读取位置停留在 `()` 内容的起始处，供下一阶段继续读取。

### `()` 约束

- `()` 只能出现在模式 **末尾**。`{*(key=)}` 合法，`{(key)*=}` 不合法。
- `()` 不允许嵌套。
- `()` 内允许字面量、`\s` `\h` `\0` `\n` `\t` `\r`、`?` 及转义字符。
- `()` 内 **不允许 `*`**——preserve 部分应为确定长度，否则"保留多少"语义模糊。

### `*` 约束

- 模式中 `*` **最多出现一次**（`()` 内外各算独立区域，但 `()` 内不允许 `*`，所以全局最多一个）。
- 解析时遇到第二个 `*` 直接报配置错误。

### 示例

```
{abc}           → 纯字面量 "abc"，等同于 Str("abc")，走 Literal 匹配
{*=}            → 非贪婪匹配任意内容直到第一个 "="，消费含 "="
{key=*}         → 匹配 "key=" 后跟任意内容
{\s=}           → 跳过连续空白（一个或多个）后匹配 "="
{field?:}       → "field" + 一个任意字符 + ":"
{\h:\h}         → 水平空白 + ":" + 水平空白
{*(key=)}       → 消费到 "key=" 之前，"key=" 保留在输入流中
{*\s(next)}     → 消费任意内容 + 空白区域，"next" 保留
{\s=*\n}        → 空白 + "=" + 任意内容 + 换行
```

**Preserve 示例详解**：

```
输入: "hello  key=value"
模式: {*\s(key=)}

消费部分: "hello  "     ← 被截断丢弃
保留部分: "key=value"   ← 留在输入流中，下一阶段从这里开始
```

## 数据结构

在 `WplSep` 上新增 `Pattern` 变体：

```rust
pub enum SepEnum {
    Str(SmolStr),
    End,
    Whitespace,
    Pattern(SepPattern),
}

pub struct SepPattern {
    raw: SmolStr,
    compiled: SepMatcher,
}
```

### 与既有 `WplSep` API 的关系

- `SepEnum::Str` 与 `SepEnum::Pattern` 统一暴露 `match_next(&mut &str) -> SepMatch` 能力；原有 `sep_str()` 只在字面量场景实现，保持旧版 `Display`/`Serialize` 语义。
- 依赖确定字符串的特性（`field_sep_until`、`ups_val`、“最近结束优先”策略等）限定仅接受 `Str`。若用户在这些位置写 `{}`，解析阶段直接报错或提示“请改用固定分隔符”。
- `infer_*`、`apply_default`、`override_with` 只复制模式对象，不做降级；上游推断出的 `{...}` 可按优先级原样覆盖下游。
- 需要输出给 UI/日志时，`Pattern` 通过 `raw` 原样回显 `{...}`，而非伪造 `sep_str()`。

### 实现约束（延续 winnow 管线）

- 现有 `WplSep` 的解析、消费与 `read_until_sep` 均基于 `winnow` 组合器，新增模式也保持同样的组合式解析；禁止引入正则引擎或 `regex` crate，以免造成额外开销。
- `build_pattern` 负责把 `{}` 字面量解析成 `GlobSegment`，其语法分析同样使用 `winnow`/手写状态机，保证与 `wpl_field` 其余 DSL 共享错误上下文与性能特征。
- 运行期 matcher 使用 `memchr`/双指针遍历，不调用正则；`winnow` 只负责前置的语法解析，确保 runtime 与旧实现的热点（`read_until_sep`）同属一条代码路径。

### SepMatcher

```rust
pub enum SepMatcher {
    /// 无通配符，直接字符串匹配（memchr / twoway）
    Literal(SmolStr),
    /// 含 * 或 ? 或 \s \h，glob 匹配
    Glob(GlobPattern),
}

pub struct GlobPattern {
    /// 主体段（消费部分）
    segments: Vec<GlobSegment>,
    /// preserve 段（匹配但不消费，可选）
    preserve: Option<Vec<GlobSegment>>,
}

pub enum GlobSegment {
    /// 连续字面字符（含 \0 \n \t \r 解析后的值）
    Literal(SmolStr),
    /// * — 零或多个任意字符（非贪婪）
    Star,
    /// ? — 恰好一个任意字符
    Any,
    /// \s — 一个或多个连续空白
    Whitespace,
    /// \h — 一个或多个连续水平空白（空格/制表）
    HorizontalWhitespace,
}
```

### Matcher 分层策略

| 模式内容 | Matcher | 说明 |
|----------|---------|------|
| 纯字面量（无 `*` `?` `\s` `\h`） | `Literal` | 退化为 `SepEnum::Str`，`memchr` 查找 |
| 含 `\s` `\h` 但无 `*` `?` | `Glob`（确定结构） | 逐字符匹配，可用首个字面段 `memchr` 加速定位 |
| 含 `*` 或 `?` | `Glob`（通配） | 双指针 glob 匹配，`*` 非贪婪 |

### 匹配返回值

```rust
pub struct SepMatch {
    /// 消费的总长度（不含 preserve 部分）
    pub consumed: usize,
    /// 整个匹配的长度（含 preserve 部分，用于定位确认）
    pub matched: usize,
}
```

约定：matcher 覆盖的分隔片段完全由 `consumed` 描述，调用方始终按 `consumed` 推进输入；若存在 preserve，则 `matched - consumed` 表示“匹配但保留”的长度。`matched` 仅用于调试/告警，便于复刻命中路径。

## 解析流程

1. `wpl_sep` 增加 `{` 分支：
   ```rust
   if peek_char('{') {
       let pattern = take_scope('{', '}');
       let sep = SepEnum::Pattern(build_pattern(pattern));
   }
   ```
2. `build_pattern` 负责：
   - 解析转义序列与通配符，得到 `Vec<GlobSegment>`。
   - 检测 `()` 位置，分离 preserve 段。
   - 校验约束：`*` 最多一个、`()` 仅在末尾、`()` 内无 `*`。
   - 根据 segment 特性选择 matcher 类型：
     - 只有字面量 → `Literal`。
     - 含通配符或空白宏 → `Glob`。
   - 存储原始字符串（供 Display / 序列化）。

## 运行时行为

- `WplSep::read_until_sep` 根据 `SepEnum` 分派：
  - `Str`：沿用原逻辑。
  - `Pattern`：调用 matcher 的 `find(haystack)`，返回 `SepMatch`。
    - 无论是否存在 preserve，`read_until_sep` 都仅推进 `consumed` 字节；`matched` 仅用于日志/调试定位。
- `SepPattern` 可缓存编译结果，避免多次重复解析；`WplSep` 已经是 `Clone`，可复用 compiled 状态。
- 对支持 `infer` 的场景（上游传入字面量），若该字面量用 `{}` 定义，则 infer 也保存 `Pattern`，但 `apply_default` 时按优先级处理即可。

### 匹配流程与语义保护

1. 依序处理：`is_to_end` → `Whitespace` → `ups_val` / 单字符快路径 → 引号/原始字符串跳过 → 模式/字面 matcher。
2. `Pattern` 仅在 `ups_val.is_none()` 时生效；若用户同时配置 `{}` 与次级结束符，解析阶段报错提示“请选择固定分隔符”。
3. `find(haystack)` 只在“已排除引号段”的切片上执行，避免把引号内的分隔符提前截断，保持与旧逻辑相同的保护级别。
4. matcher 返回的 `SepMatch` 由 `read_until_sep` 统一转换为 `String`，因此调用方无需感知是 `Str` 还是 `Pattern`。

## 兼容性

- YAML/JSON/OML 序列化：`SepEnum::Pattern` 需要实现 `Serialize/Deserialize`，格式建议为 `{pattern:"..."}`。
- `Display`：`WplFmt` 输出时保持 `{...}`，并按解析规则反向转义，确保 round-trip。
- 旧配置无需改动：`\s` `\0` 等在 `{}` 外的原有行为不变，`{}` 内的转义语义一致。

## 性能影响

- 构建期：每个 `{}` 模式解析一次，复杂度 O(len)。
- 运行期：
  - 纯字面模式走 `Literal`，可用 `memchr` 或 `twoway` 算法，O(n)。
  - 含 `\s` `\h` 无 `*` 的模式，逐字符匹配但长度确定，可用首字面段 memchr 加速。
  - 含 `*` 的模式，最坏 O(n·m)，但 `*` 限制为一个且模式串短（m 极小），实际近似 O(n)。
- 由于 matcher 可复用，整体性能优于在 DSL 里组合多条 `trim/regex`。

## 迁移计划

1. **阶段 1**：实现 `GlobSegment`、`GlobPattern`、`SepMatcher` 数据结构与 `build_pattern` 解析器；保持 `read_until_sep` 兼容。
2. **阶段 2**：在热点 parser（如 `take_field`, `pipe take(...)`）中改用模式分隔，写基准测试对比。
3. **阶段 3**：文档更新（WPL 使用指南、配置参考）加入 `{}` 语法说明，提供示例。
4. **阶段 4**：可选，视需求扩展（例如字符类 `[a-z]`、可选项等）。

## 测试计划

- 单元测试：
  - 纯字面量：`{abc}`, `{ab\n}`, `{ab\0}` round-trip。
  - 通配符：`{*=}`, `{key=*}`, `{field?:}` 匹配正确性。
  - 空白匹配：`{\s=}`, `{\h:\h}` 对连续空白的匹配行为。
  - Preserve：`{*(key=)}`, `{*\s(next)}` 消费/保留边界验证。
  - 兼容性：`\\s` 在 `{}` 外仍解析为单空格，`\\S` 保持“空白宏”语义。
  - 约束校验：多个 `*` 报错、`()` 非末尾报错、`()` 内含 `*` 报错。
  - `SepMatch` 的 `consumed` / `matched` 值正确性。
- 集成/性能测试：
  - 构造 WPL 模型使用 `{}` 分隔符，验证解析输出。
  - 与旧配置比较性能，确保新 matcher 无明显回退。
  - 基准：对 `take_field`、`pipe take(...)`、`wparse` CLI 等热点路径分组 benchmark，比较旧版 winnow 字面分隔符与新 `Pattern` 的性能，确保不出现回退并记录阈值。
  - `*` 非贪婪行为：`"a=b=c"` + `{*=}` → 匹配 `"a="`。
  - 空白区域匹配：`"key  \t  =val"` + `{\s=}` → 空白段正确消费。
