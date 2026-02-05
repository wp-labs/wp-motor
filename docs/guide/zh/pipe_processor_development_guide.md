# PipeProcessor 开发指南

本文档指导如何为 WP-Motor 开发自定义 PipeProcessor（管道处理器）和 FieldPipe（字段管道处理器）。

## 目录

- [概述](#概述)
- [PipeProcessor 开发](#pipeprocessor-开发)
- [FieldPipe 开发](#fieldpipe-开发)
- [注册和使用](#注册和使用)
- [测试指南](#测试指南)
- [最佳实践](#最佳实践)

---

## 概述

WP-Motor 提供两种管道处理机制：

### 1. PipeProcessor（原始数据处理）

- **位置**：`crates/wp-lang/src/eval/builtins/`
- **用途**：处理原始数据（`RawData`），适用于数据解码、转换等场景
- **特点**：处理字节流、字符串等原始数据格式

### 2. FieldPipe（字段数据处理）

- **位置**：`crates/wp-lang/src/eval/builtins/pipe_fun.rs`
- **用途**：处理结构化字段（`DataField`），适用于字段验证、转换、过滤
- **特点**：处理已解析的结构化数据

---

## PipeProcessor 开发

### 核心 Trait 定义

```rust
pub trait PipeProcessor {
    /// 处理输入数据并返回转换后的结果
    fn process(&self, data: RawData) -> WparseResult<RawData>;

    /// 获取处理器名称/标识符
    fn name(&self) -> &'static str;
}
```

### 数据类型

```rust
pub enum RawData {
    String(String),           // UTF-8 字符串
    Bytes(Bytes),            // 字节序列
    ArcBytes(Arc<Vec<u8>>),  // 共享字节序列
}
```

### 开发步骤

#### 1. 创建处理器结构体

```rust
use wp_parse_api::{PipeProcessor, RawData, WparseResult};

#[derive(Debug)]
pub struct YourProcessor;
```

#### 2. 实现 PipeProcessor trait

```rust
impl PipeProcessor for YourProcessor {
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                // 处理字符串
                let result = your_transform(&s)?;
                Ok(RawData::from_string(result))
            }
            RawData::Bytes(b) => {
                // 处理字节序列
                let result = your_transform_bytes(&b)?;
                Ok(RawData::Bytes(Bytes::from(result)))
            }
            RawData::ArcBytes(b) => {
                // 处理共享字节序列
                let result = your_transform_bytes(&b)?;
                Ok(RawData::ArcBytes(Arc::new(result)))
            }
        }
    }

    fn name(&self) -> &'static str {
        "your/processor"
    }
}
```

#### 3. 关键实现要点

**✅ 保持容器类型一致**
- `String` 输入 → `String` 输出
- `Bytes` 输入 → `Bytes` 输出
- `ArcBytes` 输入 → `ArcBytes` 输出

**✅ 错误处理**
```rust
use orion_error::{ErrorOwe, ErrorWith};

// 使用 .owe_data() 和 .want() 链式处理错误
let decoded = hex::decode(s.as_bytes())
    .owe_data()
    .want("hex decode")?;

let vstring = String::from_utf8(decoded)
    .owe_data()
    .want("utf8 conversion")?;
```

**✅ 性能优化**
```rust
// 预分配容量
let mut out: Vec<u8> = Vec::with_capacity(input.len());

// 避免不必要的分配
if !s.as_bytes().contains(&b'\\') {
    return true; // 快速路径
}
```

### 完整示例：Base64 解码器

```rust
use base64::{Engine as _, engine::general_purpose};
use bytes::Bytes;
use orion_error::{ErrorOwe, ErrorWith};
use std::sync::Arc;
use wp_parse_api::{PipeProcessor, RawData, WparseResult};

#[derive(Debug)]
pub struct Base64Proc;

impl PipeProcessor for Base64Proc {
    fn process(&self, data: RawData) -> WparseResult<RawData> {
        match data {
            RawData::String(s) => {
                let decoded = general_purpose::STANDARD
                    .decode(s.as_bytes())
                    .owe_data()
                    .want("base64 decode")?;
                let vstring = String::from_utf8(decoded)
                    .owe_data()
                    .want("utf8 conversion")?;
                Ok(RawData::from_string(vstring))
            }
            RawData::Bytes(b) => {
                let decoded = general_purpose::STANDARD
                    .decode(b.as_ref())
                    .owe_data()
                    .want("base64 decode")?;
                Ok(RawData::Bytes(Bytes::from(decoded)))
            }
            RawData::ArcBytes(b) => {
                let decoded = general_purpose::STANDARD
                    .decode(b.as_ref())
                    .owe_data()
                    .want("base64 decode")?;
                Ok(RawData::ArcBytes(Arc::new(decoded)))
            }
        }
    }

    fn name(&self) -> &'static str {
        "decode/base64"
    }
}
```

---

## FieldPipe 开发

### 核心 Trait 定义

```rust
pub trait FieldPipe {
    /// 处理字段数据，返回处理结果
    fn process(&self, field: Option<&mut DataField>) -> WResult<()>;

    /// 返回自动选择器（可选）
    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        None
    }
}
```

### 开发步骤

#### 1. 定义处理器结构体

```rust
use wp_model_core::model::DataField;
use wp_parser::WResult;

#[derive(Debug, Clone)]
pub struct YourFieldProcessor {
    pub param1: String,
    pub param2: i64,
}
```

#### 2. 实现 FieldPipe trait

```rust
impl FieldPipe for YourFieldProcessor {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // 1. 检查字段是否存在
        let Some(field) = field else {
            return fail
                .context(ctx_desc("your_processor | no active field"))
                .parse_next(&mut "");
        };

        // 2. 获取字段值
        let value = field.get_value_mut();

        // 3. 处理字段值
        if process_value(value, &self.param1, self.param2) {
            Ok(())
        } else {
            fail.context(ctx_desc("your_processor | process failed"))
                .parse_next(&mut "")
        }
    }
}
```

### 常见模式

#### 模式 1：验证字段值

```rust
impl FieldPipe for CharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && value.as_str() == self.value.as_str()
        {
            return Ok(());  // 验证通过
        }
        // 验证失败，返回错误
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }
}
```

#### 模式 2：转换字段值

```rust
impl FieldPipe for Base64Decode {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("base64_decode | no active field"))
                .parse_next(&mut "");
        };

        let value = field.get_value_mut();
        if value_base64_decode(value) {
            Ok(())  // 转换成功，值已修改
        } else {
            fail.context(ctx_desc("base64_decode"))
                .parse_next(&mut "")
        }
    }
}

fn value_base64_decode(v: &mut Value) -> bool {
    match v {
        Value::Chars(s) => {
            if let Ok(decoded) = general_purpose::STANDARD.decode(s.as_bytes())
                && let Ok(vstring) = String::from_utf8(decoded)
            {
                *s = vstring.into();  // 修改字段值
                return true;
            }
            false
        }
        _ => false,
    }
}
```

#### 模式 3：范围检查

```rust
impl FieldPipe for DigitRange {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && *value >= self.begin
            && *value <= self.end
        {
            return Ok(());  // 在范围内
        }
        fail.context(ctx_desc("<pipe> | not in range"))
            .parse_next(&mut "")
    }
}
```

#### 模式 4：条件转换（转换为 ignore）

```rust
impl FieldPipe for StartsWith {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("starts_with | no active field"))
                .parse_next(&mut "");
        };

        if let Value::Chars(value) = field.get_value() {
            if value.starts_with(self.prefix.as_str()) {
                // 匹配成功，保持原字段
                Ok(())
            } else {
                // 不匹配，转换为 ignore 类型
                let field_name = field.get_name().to_string();
                *field = DataField::from_ignore(field_name);
                Ok(())
            }
        } else {
            // 非字符串类型也转换为 ignore
            let field_name = field.get_name().to_string();
            *field = DataField::from_ignore(field_name);
            Ok(())
        }
    }
}
```

### 自动选择器（可选）

如果处理器带有目标字段名，可实现自动选择：

```rust
impl FieldPipe for TargetCharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // ... 处理逻辑 ...
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}
```

---

## 注册和使用

### PipeProcessor 注册

#### 1. 创建构建函数

```rust
// crates/wp-lang/src/eval/builtins/mod.rs

fn your_processor_stage() -> PipeHold {
    Arc::new(YourProcessor)
}
```

#### 2. 注册到全局注册表

```rust
pub fn ensure_builtin_pipe_units() {
    BUILTIN_PIPE_INIT.call_once(|| {
        registry::register_pipe_unit("your/processor", your_processor_stage);
        // ... 其他处理器 ...
    });
}
```

#### 3. 使用宏注册（推荐）

```rust
use crate::register_wpl_pipe;

register_wpl_pipe!("your/processor", your_processor_stage);

// 批量注册
register_wpl_pipe!(
    "decode/base64" => decode_base64_stage,
    "decode/hex" => decode_hex_stage,
    "your/processor" => your_processor_stage,
);
```

### FieldPipe 集成

#### 1. 添加到 WplFun 枚举

```rust
// crates/wp-lang/src/ast/mod.rs

pub enum WplFun {
    // ... 现有函数 ...
    YourFieldProc(YourFieldProcessor),
}
```

#### 2. 实现 as_field_pipe 方法

```rust
impl WplFun {
    pub fn as_field_pipe(&self) -> Option<&dyn FieldPipe> {
        match self {
            // ... 现有函数 ...
            WplFun::YourFieldProc(fun) => Some(fun),
        }
    }
}
```

---

## 测试指南

### PipeProcessor 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnyResult;

    #[test]
    fn test_your_processor_string() -> AnyResult<()> {
        let data = RawData::from_string("input_data".to_string());
        let result = YourProcessor.process(data)?;
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "expected_output"
        );
        Ok(())
    }

    #[test]
    fn test_your_processor_bytes() -> AnyResult<()> {
        let data = RawData::Bytes(Bytes::from_static(b"input_data"));
        let result = YourProcessor.process(data)?;
        assert!(matches!(result, RawData::Bytes(_)));
        assert_eq!(
            crate::eval::builtins::raw_to_utf8_string(&result),
            "expected_output"
        );
        Ok(())
    }

    #[test]
    fn test_your_processor_arc_bytes() -> AnyResult<()> {
        let data = RawData::ArcBytes(Arc::new(b"input_data".to_vec()));
        let result = YourProcessor.process(data)?;
        assert!(matches!(result, RawData::ArcBytes(_)));
        Ok(())
    }

    #[test]
    fn test_your_processor_error_handling() {
        let data = RawData::from_string("invalid_input".to_string());
        assert!(YourProcessor.process(data).is_err());
    }
}
```

### FieldPipe 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wp_model_core::model::{DataField, Value, DataType};

    #[test]
    fn test_field_processor_success() {
        let mut fields = vec![DataField::from_chars(
            "test_field".to_string(),
            "test_value".to_string(),
        )];

        YourFieldProcessor { param1: "test".into(), param2: 42 }
            .process(fields.get_mut(0))
            .expect("should succeed");

        // 验证结果
        if let Value::Chars(s) = fields[0].get_value() {
            assert_eq!(s.as_str(), "expected_value");
        }
    }

    #[test]
    fn test_field_processor_validation_failure() {
        let mut fields = vec![DataField::from_chars(
            "test_field".to_string(),
            "invalid_value".to_string(),
        )];

        assert!(YourFieldProcessor { param1: "test".into(), param2: 42 }
            .process(fields.get_mut(0))
            .is_err());
    }

    #[test]
    fn test_field_processor_type_conversion() {
        let mut fields = vec![DataField::from_digit("num".to_string(), 123)];

        // 测试非预期类型
        assert!(YourFieldProcessor { param1: "test".into(), param2: 42 }
            .process(fields.get_mut(0))
            .is_err());
    }

    #[test]
    fn test_field_processor_none_field() {
        assert!(YourFieldProcessor { param1: "test".into(), param2: 42 }
            .process(None)
            .is_err());
    }
}
```

### 测试覆盖要求

**必须测试的场景**：

1. ✅ **正常路径**：有效输入的正确处理
2. ✅ **错误处理**：无效输入的错误返回
3. ✅ **边界条件**：空字符串、空字节、最大/最小值
4. ✅ **类型处理**：所有支持的 `RawData` 或 `Value` 类型
5. ✅ **None 字段**：处理 `Option<&mut DataField>` 为 None 的情况

---

## 最佳实践

### 1. 性能优化

```rust
// ✅ 快速路径：避免不必要的处理
if !s.as_bytes().contains(&b'\\') {
    return true; // 无需转义处理
}

// ✅ 预分配容量
let mut out = Vec::with_capacity(input.len());

// ✅ 使用 #[inline] 标记热点函数
#[inline]
fn value_json_unescape(v: &mut Value) -> bool {
    // ...
}
```

### 2. 错误信息清晰

```rust
// ✅ 使用描述性错误上下文
fail.context(ctx_desc("base64_decode | invalid input"))
    .parse_next(&mut "")

// ❌ 避免模糊错误
fail.parse_next(&mut "")
```

### 3. 保持容器类型

```rust
// ✅ 正确：保持输入类型
match data {
    RawData::String(s) => Ok(RawData::from_string(result)),
    RawData::Bytes(b) => Ok(RawData::Bytes(result)),
    RawData::ArcBytes(b) => Ok(RawData::ArcBytes(result)),
}

// ❌ 错误：改变容器类型
match data {
    RawData::String(s) => Ok(RawData::Bytes(result)), // 类型不一致！
    // ...
}
```

### 4. 字段修改模式

```rust
// ✅ 验证型：不修改字段
if let Value::Chars(value) = field.get_value() {
    if value == expected {
        return Ok(());
    }
}

// ✅ 转换型：修改字段值
let value = field.get_value_mut();
if let Value::Chars(s) = value {
    *s = transformed.into(); // 直接修改
    return Ok(());
}

// ✅ 过滤型：转换为 ignore
let field_name = field.get_name().to_string();
*field = DataField::from_ignore(field_name);
```

### 5. 文档注释

```rust
/// Base64 解码处理器
///
/// 将 Base64 编码的数据解码为原始字节或 UTF-8 字符串。
///
/// # 行为
/// - `String` 输入：解码并尝试 UTF-8 转换
/// - `Bytes` 输入：解码为原始字节
/// - `ArcBytes` 输入：解码为共享字节
///
/// # 错误
/// - 无效的 Base64 编码
/// - 解码后的数据不是有效 UTF-8（仅 String 输入）
#[derive(Debug)]
pub struct Base64Proc;
```

### 6. 命名约定

```rust
// PipeProcessor 命名
pub struct Base64Proc;      // ✅ 以 Proc 结尾
pub struct HexProc;         // ✅
pub struct Base64Decoder;   // ❌ 不推荐

// FieldPipe 命名
pub struct CharsHas;        // ✅ 描述性名称
pub struct DigitRange;      // ✅
pub struct StartsWith;      // ✅
```

---

## 参考示例

### 现有实现参考

**PipeProcessor 示例**：
- `Base64Proc` - Base64 解码
- `HexProc` - 十六进制解码
- `EscQuotaProc` - 引号和转义处理

**FieldPipe 示例**：
- `CharsHas` - 字符串值验证
- `DigitRange` - 数值范围检查
- `StartsWith` - 字符串前缀匹配
- `Base64Decode` - Base64 字段解码
- `ReplaceFunc` - 字符串替换
- `RegexMatch` - 正则表达式匹配

### 文件结构

```
crates/wp-lang/src/eval/builtins/
├── mod.rs              # 模块入口，注册函数
├── base64.rs           # Base64 处理器
├── hex.rs              # 十六进制处理器
├── quotation.rs        # 引号处理器
├── pipe_fun.rs         # FieldPipe 实现
└── registry.rs         # 注册表
```

---

## 快速开始清单

**开发 PipeProcessor**：

- [ ] 创建处理器结构体
- [ ] 实现 `PipeProcessor` trait
- [ ] 处理所有 `RawData` 类型
- [ ] 添加错误处理
- [ ] 创建构建函数
- [ ] 注册到全局注册表
- [ ] 编写完整测试
- [ ] 添加文档注释

**开发 FieldPipe**：

- [ ] 创建处理器结构体
- [ ] 实现 `FieldPipe` trait
- [ ] 处理 `Option<&mut DataField>`
- [ ] 实现字段值处理逻辑
- [ ] 添加到 `WplFun` 枚举
- [ ] 实现 `as_field_pipe` 方法
- [ ] 编写完整测试
- [ ] 添加文档注释

---

**版本**：1.13.4
**更新日期**：2026-02-05
