# WPL Field Function 开发指南

## 概述

本指南介绍如何在 WP-Motor 中开发新的 WPL (WP Language) field function。Field function 是用于处理和转换日志字段的核心组件，包括字段选择、条件检查、数据转换等功能。

## 架构概览

WPL field function 的实现涉及以下几个核心模块：

```
crates/wp-lang/
├── src/ast/processor/
│   ├── function.rs       # 函数结构体定义
│   ├── pipe.rs           # WplFun 枚举和管道定义
│   └── mod.rs            # 模块导出
├── src/eval/builtins/
│   └── pipe_fun.rs       # FieldPipe trait 实现
└── src/parser/
    └── wpl_fun.rs        # 函数解析器（可选）
```

## 快速开始：实现一个字符串替换函数

本节以 `chars_replace` 函数为例，展示完整的开发流程。

### 第 1 步：定义函数结构体

在 `crates/wp-lang/src/ast/processor/function.rs` 中定义函数结构体：

```rust
/// 字符串替换函数
#[derive(Clone, Debug, PartialEq)]
pub struct ReplaceFunc {
    pub(crate) target: SmolStr,  // 要替换的目标字符串
    pub(crate) value: SmolStr,   // 替换后的新字符串
}
```

**命名规范：**
- 结构体名使用 PascalCase，以 `Func` 或相关后缀结尾
- 字段使用 `pub(crate)` 可见性，确保模块内可访问
- 使用 `SmolStr` 存储短字符串，节省内存

### 第 2 步：导出函数结构体

在 `crates/wp-lang/src/ast/processor/mod.rs` 中导出：

```rust
pub use function::{
    // ... 其他导出
    ReplaceFunc,
    // ...
};
```

### 第 3 步：添加到 WplFun 枚举

在 `crates/wp-lang/src/ast/processor/pipe.rs` 中：

1. 导入新函数：
```rust
use super::function::{
    // ... 其他导入
    ReplaceFunc,
};
```

2. 在 `WplFun` 枚举中添加变体：
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum WplFun {
    // ... 其他变体

    // Transformation functions
    TransJsonUnescape(JsonUnescape),
    TransBase64Decode(Base64Decode),
    TransCharsReplace(ReplaceFunc),  // 新增
}
```

**命名规范：**
- 枚举变体使用 PascalCase
- 建议按功能分类添加注释（如 Transformation functions）
- 相关功能放在一起，保持代码组织清晰

### 第 4 步：实现 FieldPipe Trait

在 `crates/wp-lang/src/eval/builtins/pipe_fun.rs` 中：

1. 导入函数结构体：
```rust
use crate::ast::processor::{
    // ... 其他导入
    ReplaceFunc,
};
```

2. 实现 `FieldPipe` trait：
```rust
impl FieldPipe for ReplaceFunc {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // 1. 检查字段是否存在
        let Some(field) = field else {
            return fail
                .context(ctx_desc("chars_replace | no active field"))
                .parse_next(&mut "");
        };

        // 2. 获取可变引用并处理
        let value = field.get_value_mut();
        if value_chars_replace(value, &self.target, &self.value) {
            Ok(())
        } else {
            fail.context(ctx_desc("chars_replace")).parse_next(&mut "")
        }
    }
}
```

3. 实现辅助函数（在同一文件的底部，`// ---------------- String Mode ----------------` 区域）：
```rust
#[inline]
fn value_chars_replace(v: &mut Value, target: &str, replacement: &str) -> bool {
    match v {
        Value::Chars(s) => {
            let replaced = s.replace(target, replacement);
            *s = replaced.into();
            true
        }
        _ => false,  // 非字符串类型返回 false
    }
}
```

**实现要点：**
- 使用 `#[inline]` 优化性能
- 错误消息使用 `ctx_desc` 包装，格式为 `"function_name | error_detail"`
- 辅助函数返回 `bool`，成功返回 `true`，失败返回 `false`
- 对于修改字段值的函数，使用 `get_value_mut()` 获取可变引用

### 第 5 步：注册到 as_field_pipe 方法

在 `crates/wp-lang/src/eval/builtins/pipe_fun.rs` 的 `impl WplFun` 块中：

```rust
impl WplFun {
    pub fn as_field_pipe(&self) -> Option<&dyn FieldPipe> {
        match self {
            // ... 其他匹配分支
            WplFun::TransJsonUnescape(fun) => Some(fun),
            WplFun::TransBase64Decode(fun) => Some(fun),
            WplFun::TransCharsReplace(fun) => Some(fun),  // 新增
        }
    }

    // 如果函数支持 auto_select（自动字段选择），还需要在这里注册
    pub fn auto_selector_spec(&self) -> Option<FieldSelectorSpec<'_>> {
        match self {
            // 仅需要 Target* 系列函数添加
            // WplFun::TransCharsReplace(fun) => fun.auto_select(),
            _ => None,
        }
    }
}
```

### 第 6 步：编写测试

在 `crates/wp-lang/src/eval/builtins/pipe_fun.rs` 的 `#[cfg(test)] mod tests` 块中添加测试：

```rust
#[test]
fn chars_replace_successfully_replaces_substring() {
    let mut fields = vec![DataField::from_chars(
        "message".to_string(),
        "hello world, hello rust".to_string(),
    )];
    ReplaceFunc {
        target: "hello".into(),
        value: "hi".into(),
    }
    .process(fields.get_mut(0))
    .expect("replace ok");

    if let Value::Chars(s) = fields[0].get_value() {
        assert_eq!(s.as_str(), "hi world, hi rust");
    } else {
        panic!("message should remain chars");
    }
}

#[test]
fn chars_replace_returns_err_on_non_chars_field() {
    let mut fields = vec![DataField::from_digit("num".to_string(), 123)];
    assert!(ReplaceFunc {
        target: "old".into(),
        value: "new".into(),
    }
    .process(fields.get_mut(0))
    .is_err());
}
```

**测试要点：**
- 至少覆盖成功场景和错误场景
- 使用 `DataField::from_chars()` / `from_digit()` 等辅助方法创建测试数据
- 测试函数名以 `test_` 或功能描述命名，清晰表达测试意图

### 第 7 步：验证编译和测试

```bash
# 编译检查
cargo check -p wp-lang

# 运行所有测试
cargo test -p wp-lang

# 运行特定测试
cargo test -p wp-lang --lib pipe_fun::tests::chars_replace
```

## 函数类型和实现模式

### 1. 条件检查函数（Condition Check）

用于检查字段是否满足特定条件，不修改字段值。

**示例：**`CharsHas`, `DigitHas`, `Has`

```rust
impl FieldPipe for CharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && value.as_str() == self.value.as_str()
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }
}
```

**特点：**
- 条件满足返回 `Ok(())`
- 条件不满足返回 `fail`（触发管道中断）
- 不修改字段值

### 2. 带目标字段的条件检查（Target-based Condition）

支持指定目标字段的条件检查函数。

**示例：**`TargetCharsHas`, `TargetCharsIn`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsHas {
    pub(crate) target: Option<SmolStr>,  // None 表示当前活动字段
    pub(crate) value: SmolStr,
}

impl FieldPipe for TargetCharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // 实现逻辑同 CharsHas
        // ...
    }

    // 关键：实现 auto_select 方法
    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}
```

**特点：**
- 结构体包含 `target: Option<SmolStr>` 字段
- 实现 `auto_select()` 方法，支持自动字段选择
- `target` 为 `None` 时操作当前活动字段

### 3. 转换函数（Transformation）

修改字段值的函数。

**示例：**`JsonUnescape`, `Base64Decode`, `ReplaceFunc`

```rust
impl FieldPipe for JsonUnescape {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("json_unescape | no active field"))
                .parse_next(&mut "");
        };

        let value = field.get_value_mut();
        if value_json_unescape(value) {
            Ok(())
        } else {
            fail.context(ctx_desc("json_unescape")).parse_next(&mut "")
        }
    }
}
```

**特点：**
- 需要检查字段是否存在
- 使用 `get_value_mut()` 获取可变引用
- 修改成功返回 `Ok(())`，失败返回 `fail`

### 4. 字段选择函数（Field Selector）

用于选择特定字段作为活动字段。

**示例：**`TakeField`, `SelectLast`

```rust
impl FieldSelector for TakeField {
    fn select(
        &self,
        fields: &mut Vec<DataField>,
        index: Option<&FieldIndex>,
    ) -> WResult<Option<usize>> {
        if let Some(idx) = index.and_then(|map| map.get(self.target.as_str()))
            && idx < fields.len()
        {
            return Ok(Some(idx));
        }
        if let Some(pos) = fields.iter().position(|f| f.get_name() == self.target) {
            Ok(Some(pos))
        } else {
            fail.context(ctx_desc("take | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        }
    }

    fn requires_index(&self) -> bool {
        true  // 需要字段索引优化性能
    }
}
```

**特点：**
- 实现 `FieldSelector` trait 而非 `FieldPipe`
- 返回字段在 Vec 中的索引位置
- 通常配合 `requires_index()` 优化查找性能

## 高级主题

### 1. 支持多个参数的函数

对于需要多个参数的函数，使用结构体字段存储参数：

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsIn {
    pub(crate) target: Option<SmolStr>,   // 参数1：目标字段
    pub(crate) value: Vec<SmolStr>,       // 参数2：候选值列表
}
```

### 2. 解析器集成（可选）

如果需要从 WPL 语法中解析函数调用，需要在 `crates/wp-lang/src/parser/wpl_fun.rs` 中实现解析器。

**示例：**
```rust
impl Fun2Builder for TargetCharsIn {
    type ARG1 = SmolStr;
    type ARG2 = Vec<CharsValue>;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        take_arr::<CharsValue>(data)
    }

    fn fun_name() -> &'static str {
        "f_chars_in"  // WPL 语法中的函数名
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        let value: Vec<SmolStr> = args.1.iter().map(|i| i.0.clone()).collect();
        Self {
            target: normalize_target(args.0),
            value,
        }
    }
}
```

### 3. 使用 normalize_target 工具函数

`normalize_target` 用于统一处理目标字段参数：

```rust
pub(crate) fn normalize_target(target: SmolStr) -> Option<SmolStr> {
    if target == "_" {
        None  // "_" 表示当前活动字段
    } else {
        Some(target)
    }
}
```

### 4. 性能优化建议

1. **使用 `#[inline]`**：对小函数使用 `#[inline]` 属性
2. **SmolStr 优化**：短字符串（< 23 字节）使用 `SmolStr` 减少堆分配
3. **避免不必要的克隆**：尽量使用引用传递
4. **提前返回**：使用 `if let` 和 `&&` 链式条件提前返回

```rust
// 推荐写法
if let Some(item) = field
    && let Value::Chars(value) = item.get_value()
    && value.as_str() == self.value.as_str()
{
    return Ok(());
}
```

## 常见错误和解决方案

### 错误 1：未导出函数结构体

**错误信息：**
```
error[E0432]: unresolved import `crate::ast::processor::ReplaceFunc`
note: struct `crate::ast::processor::function::ReplaceFunc` exists but is inaccessible
```

**解决方案：**
在 `crates/wp-lang/src/ast/processor/mod.rs` 中添加导出：
```rust
pub use function::{ReplaceFunc, /* ... */};
```

### 错误 2：未注册到 as_field_pipe

**错误信息：**
```
error[E0004]: non-exhaustive patterns: `WplFun::TransCharsReplace(_)` not covered
```

**解决方案：**
在 `as_field_pipe()` 方法的 match 语句中添加新的分支。

### 错误 3：类型不匹配

**错误信息：**
```
error[E0277]: the trait bound `ReplaceFunc: FieldPipe` is not satisfied
```

**解决方案：**
确保已为结构体实现 `FieldPipe` trait。

## 开发检查清单

使用以下清单确保实现完整：

- [ ] 在 `function.rs` 中定义函数结构体
- [ ] 在 `mod.rs` 中导出函数结构体
- [ ] 在 `pipe.rs` 中导入函数结构体
- [ ] 在 `WplFun` 枚举中添加变体
- [ ] 在 `pipe_fun.rs` 中导入函数结构体
- [ ] 实现 `FieldPipe` trait（或 `FieldSelector`）
- [ ] 在 `as_field_pipe()` 中注册函数
- [ ] 如果支持目标字段，实现 `auto_select()` 方法
- [ ] 如果支持目标字段，在 `auto_selector_spec()` 中注册
- [ ] 编写单元测试（至少覆盖成功和失败场景）
- [ ] 运行 `cargo check -p wp-lang` 通过
- [ ] 运行 `cargo test -p wp-lang` 通过
- [ ] （可选）实现解析器集成

## 参考实现

### 简单转换函数
- `JsonUnescape` - JSON 转义字符解码
- `Base64Decode` - Base64 解码
- `ReplaceFunc` - 字符串替换

### 条件检查函数
- `CharsHas` - 字符串相等检查
- `CharsNotHas` - 字符串不相等检查
- `CharsIn` - 字符串在列表中检查
- `DigitHas` - 数值相等检查
- `DigitIn` - 数值在列表中检查
- `IpIn` - IP 地址在列表中检查

### 带目标字段的函数
- `TargetCharsHas` - 指定字段的字符串检查
- `TargetCharsIn` - 指定字段的字符串列表检查
- `TargetDigitHas` - 指定字段的数值检查
- `TargetHas` - 指定字段存在性检查

### 字段选择函数
- `TakeField` - 选择指定名称的字段
- `SelectLast` - 选择最后一个字段

## 调试技巧

1. **查看解析结果**：使用 `dbg!()` 宏打印结构体内容
2. **测试字段处理**：创建简单的 `DataField` 测试数据
3. **检查类型转换**：确认 `Value` 枚举的变体匹配正确
4. **启用日志**：设置 `RUST_LOG=debug` 查看详细日志

## 总结

开发 WPL field function 的核心步骤：

1. **定义**：在 `function.rs` 中定义结构体
2. **导出**：在 `mod.rs` 中导出
3. **枚举**：在 `pipe.rs` 中添加到 `WplFun`
4. **实现**：在 `pipe_fun.rs` 中实现 trait
5. **注册**：在 `as_field_pipe()` 中注册
6. **测试**：编写单元测试验证功能

遵循本指南和参考现有实现，可以高效地开发出高质量的 field function。

## 附录：文件位置速查

```
function.rs      → 定义结构体
mod.rs           → 导出结构体
pipe.rs          → 添加枚举变体、导入
pipe_fun.rs      → 实现 trait、导入、注册、测试
wpl_fun.rs       → 解析器（可选）
```

## 更新记录

- 2026-01-29：初始版本，基于 `chars_replace` 实现经验编写
