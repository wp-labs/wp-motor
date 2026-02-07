# WPL 分隔符模式化与性能优化设计

## 背景

- 现有 `WplSep` 支持的分隔符只有简单字面量（`\0`、`\s` 等），解析时依赖逐字节比较或 `str::find`。
- 用户场景中常见的分隔条件不仅是固定字符串，还包括“跳过任意空白 + 特定前缀”“遇到 `key=` 才截断”等模式。
- 缺少模式描述导致：
  1. 配置冗长，需要多个 stage 组合才能表达一个分隔逻辑；
  2. runtime 需要多次 `trim`、`regex` 才能满足需求，性能大幅下降。

## 目标

1. 在分隔符层面引入 **模式语法**，便于一次性描述“空白 + 关键字”“转义字符”等需求。
2. 提前将模式编译成高效 matcher，运行时避免反复 `regex::Regex` 构建。
3. 对旧配置、现有 API 保持兼容：纯字面量仍按原逻辑处理。
4. 允许 parser 在已知模式的前提下进行更 aggressive 的扫描（memchr / SIMD）。

## 模式语法提案

新增 `{ ... }` 形式的模式字面量，可在 `chars`、`digit`、`sep` 等位置使用：

- 字面量：`{abc}` → 等价于 `"abc"`。
- 转义字符：`\n`、`\t`、`\r`、`\\`、`\{`、`\}`、`\*`。
- 空白宏：`\s`（任意空白）、`\S`（空白或制表）。
- 组合示例：`{\s*=}` 表示“跳过空白后匹配 `=`”；`{cmd\(\*\)}` 表示 `cmd(*)` 字面量。
- 允许分组 `()` 用于后续扩展（例如命名捕获），当前阶段仅保留在模式字符串里。

模式解析规则：

| 记法        | 说明                                |
|-------------|-------------------------------------|
| `a`         | 字面字符 `a`                        |
| `\xNN`     | 指定字节                             |
| `\u{1F}`   | 指定 Unicode code point              |
| `\s`       | `[ \t\r\n]`                        |
| `.`         | 任意字符（可选，默认不开启）         |
| `(...)`     | 组（当前只做标记，不捕获）           |

## 数据结构

在 `WplSep` 上新增 `PatternKind`：

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

`SepMatcher` 为轻量状态机：

- 对纯字面串使用 `LiteralMatcher { needle: Bytes, memchr_hint }`。
- 对含 `\s`、`.` 的模式降级为 `HybridMatcher`，逐字节匹配，但可根据固定前缀使用 `memchr` 加速。
- 若未来需要真正的正则，可考虑在构建期编译成 `regex_automata::dense::DFA`，但当前只支持简单模式，无需引入大依赖。

## 解析流程

1. `wpl_sep` 增加 `{` 分支：
   ```rust
   if peek_char('{') {
       let pattern = take_scope('{', '}');
       let sep = SepEnum::Pattern(build_pattern(pattern));
   }
   ```
2. `build_pattern` 负责：
   - 解析转义序列，得到 `Vec<Token>`。
   - 根据 token 特性选择 matcher 类型：
     - 只有字面量 → `Literal`。
     - 含 `\s`/`.` → `Hybrid`。
   - 存储原始字符串（供 Display / 序列化）。

## 运行时行为

- `WplSep::read_until_sep` 根据 `SepEnum` 分派：
  - `Str`：沿用原逻辑。
  - `Pattern`：调用 matcher 的 `find(&mut &str)`，返回“匹配位置 + 截断量”。
- `SepPattern` 可缓存编译结果，避免多次重复解析；`WplSep` 已经是 `Clone`，可复用 compiled 状态。
- 对支持 `infer` 的场景（上游传入字面量），若该字面量用 `{}` 定义，则 infer 也保存 `Pattern`，但 `apply_default` 时按优先级处理即可。

## 兼容性

- YAML/JSON/OML 序列化：`SepEnum::Pattern` 需要实现 `Serialize/Deserialize`，格式建议为 `{pattern:"..."}`。
- `Display`：`WplFmt` 输出时保持 `{...}`，并按解析规则反向转义，确保 round-trip。
- 旧配置无需改动。

## 性能影响

- 构建期：每个 `{}` 模式解析一次，复杂度 O(len)。
- 运行期：
  - 字面模式转 LiteralMatcher，可用 `memchr` 或 `twoway` 算法。
  - 包含空白宏的模式仍需逐字符扫描，但可利用固定前缀/后缀减少比较次数。
- 由于 matcher 可复用，整体性能优于在 DSL 里组合多条 `trim/regex`。

## 迁移计划

1. **阶段 1**：实现 `SepPattern` 基础数据结构与 parser；保持 `read_until_sep` 兼容。
2. **阶段 2**：在热点 parser（如 `take_field`, `pipe take(...)`）中改用模式分隔，写基准测试对比。
3. **阶段 3**：文档更新（WPL 使用指南、配置参考）加入 `{}` 语法说明，提供示例。
4. **阶段 4**：可选，引入更复杂的模式（例如重复、可选项），视需求扩展。

## 测试计划

- 单元测试：
  - `{abc}`, `{ab\n}`, `{\s*=}`, `{cmd\(\*\)}` round-trip。
  - `WplSep::read_until_sep` 对不同 matcher 的结果。
- 集成测试：
  - 构造 WPL 模型使用 `{}` 分隔符，验证解析输出。
  - 与旧配置比较性能，确保新 matcher 无明显回退。

