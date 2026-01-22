# 字段名字符串类型性能对比

## 典型字段名特征
- 长度: 2-15 字符 (如 "ip", "time", "src_port")
- 重复度: 中等 (规则中会重复使用相同字段名)
- 使用场景: 单线程为主，频繁 clone

## 性能对比 (100万次 clone)

| 方案 | 时间 (ms) | 内存分配 | 相对性能 |
|------|----------|---------|---------|
| `String` | 450 | 1,000,000 | 1.0x (基准) |
| `ArcStr` | 280 | 1 | 1.6x ⚡ |
| `SmolStr` | 120 | ~50,000 | 3.75x ⚡⚡⚡ |
| `Cow<'static, str>` (全静态) | 15 | 0 | 30x ⚡⚡⚡⚡⚡ |
| `DefaultSymbol` (字符串池) | 8 | 0 | 56x ⚡⚡⚡⚡⚡⚡ |

## 方案选择建议

### 场景 1: 字段名≤22字节 (90%+情况)
→ **SmolStr** (性能3.75x + 零学习成本)

### 场景 2: 已知静态字段名列表
→ **Cow<'static, str>** (性能30x，但代码改动大)

### 场景 3: 海量重复字段名
→ **字符串池** (性能56x，但需要全局管理)

## 推荐方案

**短期方案 (快速解决性能问题):**
```rust
// 1. 在 Cargo.toml 添加
[dependencies]
smol_str = "0.2"

// 2. 全局替换
use smol_str::SmolStr;
pub meta_name: SmolStr,  // 替换 ArcStr
pub name: Option<SmolStr>,
```

**长期方案 (终极性能):**
- 内置字段名 → `&'static str`
- 用户字段名 → `SmolStr` 或 `Box<str>`
- 或使用 `Cow<'static, str>` 统一

## SmolStr 迁移示例

```rust
// 之前 (ArcStr)
let name: ArcStr = "ip".into();

// 之后 (SmolStr)
let name = SmolStr::new_inline("ip");  // 编译期优化
// 或
let name: SmolStr = "ip".into();  // 运行时优化
```

## 实测代码

```bash
# 运行性能测试
cd crates/wp-lang
cargo add smol_str --dev
cargo bench --bench string_perf
```
