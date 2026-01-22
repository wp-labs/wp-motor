# 命名改进快速参考

## 🎯 优先修复列表

### 立即可改（无破坏性）

| 当前命名 | 建议改进 | 文件位置 | 难度 |
|---------|---------|---------|------|
| `IterMode::Sleep` | `ScopeParseState::Initial` | scope2.rs:9 | 🟢 简单 |
| `IterMode::Work` | `ScopeParseState::Parsing` | scope2.rs:10 | 🟢 简单 |
| `IterMode::Fight` | `ScopeParseState::Escaped` | scope2.rs:11 | 🟢 简单 |

### 建议添加（非破坏性）

```rust
// symbol.rs - 添加扩展 trait
pub trait StrContextExt {
    fn label(s: &'static str) -> Self;
    fn literal(s: &'static str) -> Self;
    fn description(s: &'static str) -> Self;
}

impl StrContextExt for StrContext {
    fn label(s: &'static str) -> Self {
        StrContext::Label(s)
    }
    fn literal(s: &'static str) -> Self {
        StrContext::Expected(StrContextValue::StringLiteral(s))
    }
    fn description(s: &'static str) -> Self {
        StrContext::Expected(StrContextValue::Description(s))
    }
}
```

### 下个版本重构

| 当前命名 | 建议改进 | 影响范围 | 难度 |
|---------|---------|---------|------|
| `scope2.rs` | `scope/escaped.rs` | 模块导入 | 🟡 中等 |
| `WnCondParser` | `ConditionParser` | 泛型类型 | 🟡 中等 |
| `WnTake` | `ParseNext` | Trait | 🔴 复杂 |
| `wn_label/wn_desc` | `StrContext::label/description` | 辅助函数 | 🟡 中等 |
| `symbol_sql_*` | `symbols::sql::*` | 模块组织 | 🔴 复杂 |

---

## 📋 命名模式对照表

### 模块命名

| ❌ 避免 | ✅ 推荐 | 说明 |
|--------|--------|------|
| `module2.rs` | `module/variant.rs` | 避免数字后缀 |
| `mod_util.rs` | `utilities.rs` | 完整单词 |
| `my_mod.rs` | `my_module.rs` | 清晰描述 |

### 类型命名

| ❌ 避免 | ✅ 推荐 | 说明 |
|--------|--------|------|
| `WnParser` | `WinnowParser` / `Parser` | 明确缩写或不用 |
| `MyStruct2` | `EnhancedMyStruct` | 描述性后缀 |
| `DataT` | `Data` / `DataType` | 避免无意义后缀 |

### 函数命名

| ❌ 避免 | ✅ 推荐 | 说明 |
|--------|--------|------|
| `get_data()` | `data()` / `fetch_data()` | get 通常隐含 |
| `do_parse()` | `parse()` | 避免无意义 do_ |
| `parse2()` | `parse_advanced()` | 描述性区分 |

### 变量命名

| ❌ 避免 | ✅ 推荐 | 说明 |
|--------|--------|------|
| `d` / `dt` | `data` / `datetime` | 完整单词 |
| `tmp` / `temp` | `buffer` / `intermediate` | 明确用途 |
| `i` / `j` (非循环) | `index` / `position` | 有意义名称 |

### 常量命名

| ❌ 避免 | ✅ 推荐 | 说明 |
|--------|--------|------|
| `Max` | `MAX_SIZE` | 全大写+下划线 |
| `default_val` | `DEFAULT_VALUE` | 常量用大写 |
| `PI` ✅ | `Pi` ❌ | 常量必须大写 |

---

## 🔧 重构脚本示例

### 1. 重命名 IterMode

```bash
# 在 scope2.rs 中替换
sed -i '' 's/enum IterMode/enum ScopeParseState/g' src/scope2.rs
sed -i '' 's/IterMode::Sleep/ScopeParseState::Initial/g' src/scope2.rs
sed -i '' 's/IterMode::Work/ScopeParseState::Parsing/g' src/scope2.rs
sed -i '' 's/IterMode::Fight/ScopeParseState::Escaped/g' src/scope2.rs

# 验证
cargo test -p wp-parser
```

### 2. 添加类型别名（保持兼容）

```rust
// 在 cond/mod.rs 中添加
pub type ConditionParser<T, H, S> = WnCondParser<T, H, S>;
pub type ParseFromInput<T> = WnTake<T>;

// 标记为废弃（下个大版本移除）
#[deprecated(since = "2.1.0", note = "Use ConditionParser instead")]
pub use WnCondParser as WnCondParser;
```

### 3. 重组模块结构

```bash
# 创建新模块
mkdir -p src/scope
mkdir -p src/symbols

# 移动文件
mv src/scope.rs src/scope/basic.rs
mv src/scope2.rs src/scope/escaped.rs
mv src/sql_symbol.rs src/symbols/sql.rs
mv src/symbol.rs src/symbols/rust.rs

# 创建 mod.rs
cat > src/scope/mod.rs << 'EOF'
mod basic;
mod escaped;

pub use basic::ScopeEval;
pub use escaped::EscapedScopeEval;
EOF

cat > src/symbols/mod.rs << 'EOF'
pub mod rust;
pub mod sql;

pub use rust::{LogicSymbol, CmpSymbol};
EOF

# 更新 lib.rs
sed -i '' 's/pub mod scope;/pub mod scope;/' src/lib.rs
sed -i '' 's/pub mod symbol;/pub mod symbols;/' src/lib.rs
```

---

## 📝 提交信息模板

### 非破坏性改进
```
refactor(scope): rename IterMode to ScopeParseState for clarity

- Sleep -> Initial
- Work -> Parsing
- Fight -> Escaped

Improves code readability without breaking API.
```

### 破坏性更改
```
feat!: reorganize parser modules

BREAKING CHANGE: Module paths have changed

- scope2.rs → scope/escaped.rs
- Use `ScopeEval` and `EscapedScopeEval` directly
- Old imports will break

Migration guide: See NAMING_GUIDE.md
```

---

## 🚦 重构检查清单

每次重命名后：

- [ ] 搜索所有引用： `rg "OldName"`
- [ ] 更新文档注释
- [ ] 更新 examples 和 tests
- [ ] 运行完整测试： `cargo test`
- [ ] 运行 clippy： `cargo clippy`
- [ ] 检查公开 API： `cargo doc --open`
- [ ] 更新 CHANGELOG.md
- [ ] 如果破坏性，添加迁移指南

---

## 🎨 命名风格指南

### Rust 标准惯例

| 类型 | 惯例 | 示例 |
|------|------|------|
| 模块 | `snake_case` | `my_module` |
| 类型/Trait | `PascalCase` | `MyStruct`, `MyTrait` |
| 函数/方法 | `snake_case` | `do_something()` |
| 常量 | `SCREAMING_SNAKE_CASE` | `MAX_SIZE` |
| 静态变量 | `SCREAMING_SNAKE_CASE` | `GLOBAL_STATE` |
| 泛型参数 | 单字母大写或 `PascalCase` | `T`, `Item` |
| 生命周期 | 小写单字母或短词 | `'a`, `'static` |

### 项目特定约定

| 场景 | 约定 | 示例 |
|------|------|------|
| 解析器函数 | `take_*` / `parse_*` | `take_var_name`, `parse_expr` |
| 符号解析 | `symbol_*` | `symbol_cmp`, `symbol_and` |
| 作用域处理 | `*_scope` | `get_scope`, `eval_scope` |
| Builder trait | `*Builder` | `Fun1Builder`, `ExprBuilder` |
| Provider trait | `*Provider` | `SymbolProvider` |

---

## 📚 进一步学习

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)
- [Effective Rust](https://www.lurklurk.org/effective-rust/)
- [winnow Parser Patterns](https://docs.rs/winnow/latest/winnow/)

---

**最后更新**: 2025-10-05
**版本**: v2.0.0
