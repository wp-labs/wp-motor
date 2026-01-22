# wp-parser 命名改进建议

## 概述

本文档基于 Rust 命名惯例和代码可读性最佳实践，提供 wp-parser 项目的命名改进建议。

---








## 📋 中优先级问题

### 4. **函数命名 - 一致的前缀约定**

#### ⚠️ 不一致的命名模式

```rust
// atom.rs - "take_" 前缀
pub fn take_var_name<'a>(...) -> ModalResult<&'a str>
pub fn take_json_path<'a>(...) -> ModalResult<&'a str>
pub fn take_key_pair<'a>(...) -> ModalResult<(&'a str, &'a str)>

// 但同时也有：
pub fn take_empty(input: &mut &str) -> ModalResult<()>

// symbol.rs - "symbol_" 前缀
pub fn symbol_cmp(data: &mut &str) -> ModalResult<CmpSymbol>
pub fn symbol_logic(data: &mut &str) -> ModalResult<LogicSymbol>

// sql_symbol.rs - "symbol_sql_" 前缀
pub fn symbol_sql_cmp(data: &mut &str) -> ModalResult<CmpOperator>
pub fn symbol_sql_logic(data: &mut &str) -> ModalResult<SQLogicSymbol>
```

#### ✅ 建议统一

**方案 A: 按类型组织模块**
```rust
// src/parsers/atom.rs
pub fn var_name<'a>(...) -> ModalResult<&'a str>
pub fn json_path<'a>(...) -> ModalResult<&'a str>

// src/parsers/symbol.rs
pub mod rust {
    pub fn cmp(data: &mut &str) -> ModalResult<CmpSymbol>
    pub fn logic(data: &mut &str) -> ModalResult<LogicSymbol>
}

pub mod sql {
    pub fn cmp(data: &mut &str) -> ModalResult<CmpOperator>
    pub fn logic(data: &mut &str) -> ModalResult<SQLogicSymbol>
}

// 使用
use wp_parser::parsers::{atom, symbol};
atom::var_name.parse_next(&mut input)?;
symbol::rust::cmp.parse_next(&mut input)?;
```

**方案 B: 统一前缀但保留上下文**
```rust
// 保留 take_ 前缀用于提取值
pub fn take_var_name<'a>(...)
pub fn take_json_path<'a>(...)

// 使用 parse_ 前缀用于符号
pub fn parse_cmp_symbol(...)
pub fn parse_logic_symbol(...)
pub fn parse_sql_cmp_symbol(...)
```

---

### 5. **类型别名 - 提高可读性**

#### ⚠️ 复杂的泛型签名

```rust
// cond/test.rs:57
type CondParser = WnCondParser<u32, ObjGet, RustSymbol>;
```

这很好！但还可以改进：

#### ✅ 建议

```rust
/// 使用 Rust 语法的 u32 条件解析器
pub type U32ConditionParser = ConditionParser<u32, ObjGet, RustSymbol>;

/// 使用 SQL 语法的字符串条件解析器
pub type SqlStringConditionParser = ConditionParser<String, StringGetter, SQLSymbol>;

// 更通用的别名
pub type RustCondParser<T> = ConditionParser<T, DefaultGetter, RustSymbol>;
pub type SqlCondParser<T> = ConditionParser<T, DefaultGetter, SQLSymbol>;
```

---

### 6. **Trait 命名 - 明确用途**

#### ⚠️ 当前命名

```rust
pub trait CmpParser<T, S> {
    fn cmp_exp(data: &mut &str) -> ModalResult<Comparison<T, S>>;
}


pub trait SymbolFrom<T> {
    fn op_from(value: T) -> Self;
}
```

#### ✅ 建议改进

```rust
/// 比较表达式解析器
pub trait ComparisonParser<T, S> {  // 完整单词
    fn parse_comparison(data: &mut &str) -> ModalResult<Comparison<T, S>>;
}



/// 从其他类型转换
pub trait FromSymbol<T> {  // 与 From<T> 一致的命名模式
    fn from_symbol(value: T) -> Self;
}
```

---

## 💡 低优先级建议

### 7. **变量命名 - 避免单字母**

#### ⚠️ 示例

```rust
// symbol.rs 宏定义
macro_rules! define_cmp_symbol {
    ($name:ident, $lit:expr, $label:expr, $desc:expr, $variant:expr) => {
        //                      ^^^^                    ^^^^
    };
}
```

#### ✅ 建议

```rust
macro_rules! define_cmp_symbol {
    ($name:ident, $literal:expr, $label:expr, $description:expr, $variant:expr) => {
        #[doc = concat!("Parses the `", $literal, "` comparison operator.")]
        pub fn $name(data: &mut &str) -> ModalResult<CmpSymbol> {
            multispace0.parse_next(data)?;
            literal($literal)
                .context(StrContext::Label($label))
                .context(StrContext::Expected(StrContextValue::Description($description)))
                .parse_next(data)?;
            Ok($variant)
        }
    };
}
```

---

### 8. **参数命名 - 统一约定**

#### ⚠️ 不一致

```rust
// 有的用 input
pub fn take_var_name<'a>(input: &mut &'a str) -> ModalResult<&'a str>

// 有的用 data
pub fn symbol_cmp(data: &mut &str) -> ModalResult<CmpSymbol>
```

#### ✅ 建议统一

**选项 1: 全部使用 input**（推荐）
```rust
pub fn take_var_name<'a>(input: &mut &'a str) -> ...
pub fn symbol_cmp(input: &mut &str) -> ...
```

**选项 2: 根据上下文**
```rust
// 解析器使用 input
pub fn take_var_name<'a>(input: &mut &'a str) -> ...

// 内部函数使用 s 或 stream
fn helper(s: &mut &str) -> ...
```

---

### 9. **模块组织建议**

#### 当前结构
```
src/
├── atom.rs          - 原子解析器
├── symbol.rs        - Rust 符号
├── sql_symbol.rs    - SQL 符号
├── scope.rs         - 基础作用域
├── scope2.rs        - 转义作用域 ❌
├── cond/            - 条件表达式
├── fun/             - 函数调用
└── ...
```

#### ✅ 建议改进

```
src/
├── parsers/
│   ├── atom.rs          - 基础原子解析
│   ├── scope/
│   │   ├── mod.rs
│   │   ├── basic.rs     - 基础作用域 ✅
│   │   └── escaped.rs   - 转义作用域 ✅
│   ├── symbols/
│   │   ├── mod.rs
│   │   ├── rust.rs      - Rust 风格符号 ✅
│   │   └── sql.rs       - SQL 风格符号 ✅
│   ├── condition.rs
│   └── function.rs
├── utils/
│   ├── context.rs       - StrContext 辅助函数 ✅
│   └── helpers.rs
└── lib.rs
```

---

## 📊 命名优先级总结

| 优先级 | 问题 | 影响范围 | 破坏性 |
|--------|------|---------|--------|
| P0 | `scope2.rs` 数字后缀 | 模块导入 | 中等 |
| P0 | `IterMode::Sleep/Work/Fight` | 代码可读性 | 低 |
| P1 | `Wn` 前缀不明确 | API 理解 | 高 |
| P1 | 函数前缀不一致 | API 一致性 | 高 |
| P2 | Trait 命名改进 | API 清晰度 | 中等 |
| P2 | 参数名不统一 | 代码一致性 | 低 |
| P3 | 变量使用缩写 | 代码可读性 | 低 |

---

## 🚀 实施建议

### 阶段 1: 非破坏性改进（立即实施）

1. **重命名内部 enum**
   ```rust
   // scope2.rs
   enum IterMode -> enum ScopeParseState
   Sleep -> Initial
   Work -> Parsing
   Fight -> Escaped
   ```

2. **添加类型别名**
   ```rust
   pub type ConditionParser<T, H, S> = WnCondParser<T, H, S>;
   pub type ParseFromInput<T> = WnTake<T>;
   ```

3. **文档注释补充**
   - 为所有公开 API 添加文档
   - 解释缩写和术语

### 阶段 2: 破坏性重构（下个大版本）

1. **模块重组**
   - `scope2.rs` -> `scope/escaped.rs`
   - 符号相关代码合并到 `symbols/` 模块

2. **统一命名约定**
   - 移除 `Wn` 前缀
   - 统一参数名为 `input`
   - 完整拼写而非缩写

3. **API 重新设计**
   - 使用 builder 模式替代复杂泛型
   - 提供更人性化的 API

### 阶段 3: 保持兼容（过渡期）

```rust
// 提供旧 API 的兼容层
#[deprecated(since = "2.1.0", note = "Use `ConditionParser` instead")]
pub type WnCondParser<T, H, S> = ConditionParser<T, H, S>;

#[deprecated(since = "2.1.0", note = "Use `context::label` instead")]
pub fn wn_label(label: &'static str) -> StrContext {
    StrContext::label(label)
}
```

---

## 📚 参考资源

- [Rust API Guidelines - Naming](https://rust-lang.github.io/api-guidelines/naming.html)
- [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/style/naming/README.html)
- [winnow 命名惯例](https://docs.rs/winnow/latest/winnow/)

---

## ✅ 检查清单

重命名时请确认：

- [ ] 新名称是自解释的
- [ ] 符合 Rust 命名惯例（snake_case, PascalCase）
- [ ] 与标准库/生态系统一致
- [ ] 添加了文档注释
- [ ] 更新了所有引用
- [ ] 运行了测试和 benchmark
- [ ] 更新了 CHANGELOG
