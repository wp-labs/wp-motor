# WPL Sep Pattern 性能基准测试报告

> 测试环境：macOS Darwin 25.2.0, Rust 1.92, `cargo bench` (release profile)
>
> 基准工具：criterion 0.5
>
> 测试文件：`crates/wp-lang/benches/sep_pattern_bench.rs`

## 1. 构建性能 (`build_pattern`)

`build_pattern` 将 `{}` 内的原始文本解析为 `SepPattern`（包含 `SepMatcher::Literal` 或 `SepMatcher::Glob`），整个过程为单遍扫描 O(len)。

| 模式 | Matcher 类型 | 耗时 | 说明 |
|------|-------------|------|------|
| `abc` | Literal | **76 ns** | 纯字面量，无通配符 |
| `*\s(key=)` | Glob | **89 ns** | Star + Whitespace + Preserve |
| `field?:\h=\h*\n` | Glob | **113 ns** | 多段 segments（Literal + Any + HorizontalWhitespace + Star + Literal） |

**结论**：构建开销 < 120 ns，一次编译后通过 `Clone` 复用，不影响运行时热路径。

## 2. 匹配性能 (`SepPattern::find`)

`find` 在 haystack 中查找第一个匹配位置，返回 `(offset, SepMatch)`。

### 2.1 短文本匹配

| 模式 | 输入 | 输入长度 | 耗时 | 说明 |
|------|------|----------|------|------|
| `abc` (Literal) | `"xyzabcdef"` | 9 B | **14 ns** | `str::find` 快速路径 |
| `*=` (Star+Literal) | `"key=value=extra=more"` | 20 B | **60 ns** | 非贪婪匹配首个 `=` |
| `\s=` (Whitespace+Literal) | `"key   =value"` | 12 B | **47 ns** | 跳过空白定位 `=` |
| `*(key=)` (Star+Preserve) | `"hello world  key=value"` | 22 B | **30 ns** | 消费到 preserve 前 |
| `field?:` (Literal+Any+Literal) | `"fieldA:value"` | 12 B | **50 ns** | `?` 匹配单字符 |

### 2.2 长文本匹配

| 模式 | 输入长度 | 耗时 | 吞吐量 |
|------|----------|------|--------|
| `,` (Literal) | 200 B | **69 ns** | ~2.8 GB/s |
| `*\n` (Star+Literal) | 10 KB | **99 µs** | ~98 MB/s |

### 2.3 无匹配（worst case）

| 模式 | 输入 | 输入长度 | 耗时 |
|------|------|----------|------|
| `\s=` | `"no_whitespace_equals_here"` | 24 B | **96 ns** |

无匹配场景需扫描完整 haystack，耗时仍在 ns 级别。

## 3. 开头匹配性能 (`match_at_start`)

`match_at_start` 用于 `consume_sep` / `try_consume_sep`，仅在 haystack 开头尝试匹配。

| 模式 | Matcher 类型 | 耗时 |
|------|-------------|------|
| `,` | Literal | **4.4 ns** |
| `\s=` | Glob | **25 ns** |

开头匹配无需扫描，极快。

## 4. `read_until_sep` 集成对比

核心热路径对比：旧版 `SepEnum::Str`（通过 winnow `take_until`）vs 新版 `SepEnum::Pattern`（通过 `SepPattern::find`）。

### 4.1 各路径延迟

| 分隔符类型 | 模式 | 输入 | 耗时 |
|-----------|------|------|------|
| **Str** (旧版) | `,` | `"hello,world,foo,bar"` | **84 ns** |
| **Pattern Literal** (新版) | `{,}` | `"hello,world,foo,bar"` | **33 ns** |
| **Pattern Glob** (新版) | `{*=}` | `"key=value=extra"` | **66 ns** |
| **Pattern Whitespace** (新版) | `{\s=}` | `"some_key  =value"` | **78 ns** |

### 4.2 同一数据直接 A/B 对比

在输入 `"field1,field2,...,field8"` 上直接对比：

| 方式 | 耗时 | 倍数 |
|------|------|------|
| `Str(",")` — winnow `take_until` | **85 ns** | 1.0x |
| `Pattern(",")` — `str::find` | **32 ns** | **2.6x 更快** |

Pattern 的字面量路径绕过 winnow 组合器栈，直接调用 `str::find`（底层 memchr / two-way 算法），因此反而更快。

## 5. 现有 benchmark 回归检查

修改后运行完整 `wpl_bench`，各项指标无变化：

| 场景 | 耗时 | 状态 |
|------|------|------|
| bench_data_suc | 7.96 ms | 无变化 |
| bench_data_fail | 761 µs | 无变化 |
| nginx | 2.00 ms | 无变化 |
| json_deep_paths | 3.16 ms | 无变化 |
| json_large_array | 7.49 ms | 无变化 |
| json_flat_no_subs | 9.69 ms | 无变化 |
| json_flat_with_subs | 13.98 ms | 无变化 |
| json_decoded_pipe | 5.51 ms | 无变化 |
| kv_bulk | 1.78 ms | 无变化 |
| proto_text_deep | 2.98 ms | 无变化 |
| proto_text_wide | 13.62 ms | 无变化 |

**结论**：新增 `SepEnum::Pattern` 变体对现有代码路径零影响。

## 6. 总结

| 维度 | 结果 |
|------|------|
| 构建开销 | < 120 ns，一次编译多次复用 |
| 字面量匹配 | 比旧版 Str 快 **2.5x**（绕过 winnow 组合器） |
| Glob 匹配（短串） | 50 ~ 100 ns 级别 |
| Glob 匹配（10KB 长文本） | ~100 µs，线性 O(n) |
| `match_at_start` | 4 ~ 25 ns |
| 现有性能回归 | **零回退** |

### 复杂度分析

| 操作 | 复杂度 | 说明 |
|------|--------|------|
| `build_pattern` | O(len) | 单遍扫描，len 为模式串长度 |
| `find` — Literal | O(n) | `str::find` (memchr/two-way) |
| `find` — Glob 无 `*` | O(n) | 首段 Literal 用 memchr 定位，其余逐字符 |
| `find` — Glob 含 `*` | O(n·m) worst | `*` 限制最多 1 个且 m 极小，实际近似 O(n) |
| `match_at_start` | O(m) | m 为模式匹配长度 |

### 运行基准测试

```bash
# sep_pattern 专项
cargo bench -p wp-lang --bench sep_pattern_bench

# 全量回归
cargo bench -p wp-lang --bench wpl_bench
```
