# OML PipeFun 开发指南

## 概述

本指南介绍如何在 WP-Motor OML 中开发新的 PipeFun (管道函数)。PipeFun 用于在管道表达式中对字段进行链式转换处理，支持编码转换、格式化、提取等操作。

## 架构概览

OML PipeFun 的实现涉及以下核心模块：

```
crates/wp-oml/
├── src/language/syntax/functions/pipe/
│   ├── mod.rs              # PipeFun 枚举定义和模块导出
│   ├── base64.rs           # Base64 编解码函数
│   ├── escape.rs           # 转义函数
│   ├── net.rs              # 网络相关函数
│   ├── time.rs             # 时间转换函数
│   ├── other.rs            # 其他通用函数
│   └── fmt.rs              # 格式化函数
├── src/core/evaluator/transform/pipe/
│   ├── mod.rs              # ValueProcessor trait 实现分发
│   ├── base64.rs           # Base64 函数执行实现
│   ├── escape.rs           # 转义函数执行实现
│   ├── net.rs              # 网络函数执行实现
│   ├── time.rs             # 时间函数执行实现
│   └── other.rs            # 其他函数执行实现
└── src/parser/
    └── pipe_prm.rs         # PipeFun 解析器

核心 Trait:
- ValueProcessor: 定义数据转换接口 fn value_cacu(DataField) -> DataField
```

## 快速开始：实现一个 IP 转整数函数

本节以 `ip4_to_int` 函数为例，展示完整的开发流程。

### 第 1 步：定义函数结构体

在 `crates/wp-oml/src/language/syntax/functions/pipe/net.rs` 中定义函数：

```rust
use crate::language::prelude::*;

// 定义函数名常量
pub const PIPE_IP4_TO_INT: &str = "ip4_to_int";

// 定义函数结构体
#[derive(Clone, Debug, Default)]
pub struct Ip4ToInt {}

// 实现 Display trait 用于调试和日志
impl Display for Ip4ToInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_IP4_TO_INT)
    }
}
```

**关键点：**
- 定义常量存储函数名（用于解析器）
- 使用 `#[derive(Clone, Debug, Default)]` 自动派生常用 trait
- 如果函数需要参数，在结构体中添加字段
- 实现 `Display` trait 用于格式化输出

### 第 2 步：添加到 PipeFun 枚举

在 `crates/wp-oml/src/language/syntax/functions/pipe/mod.rs` 中：

**2.1 导出模块和类型：**
```rust
pub mod net;
pub use net::*;
```

**2.2 添加到枚举：**
```rust
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PipeFun {
    Base64Encode(Base64Encode),
    // ... 其他变体
    Ip4ToInt(Ip4ToInt),  // 添加新函数
}
```

**2.3 在 Display 实现中添加分支：**
```rust
impl Display for PipeFun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... 其他分支
            PipeFun::Ip4ToInt(v) => write!(f, "{}", v),
        }
    }
}
```

### 第 3 步：实现值转换逻辑

在 `crates/wp-oml/src/core/evaluator/transform/pipe/net.rs` 中实现 `ValueProcessor`：

```rust
use crate::core::prelude::*;
use crate::language::Ip4ToInt;
use wp_model_core::model::{DataField, Value};

impl ValueProcessor for Ip4ToInt {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        // 匹配输入值类型
        match in_val.get_value() {
            Value::IpAddr(ip) => {
                // 处理 IPv4 地址
                if let std::net::IpAddr::V4(v4) = ip {
                    let as_u32 = u32::from(*v4) as i64;
                    return DataField::from_digit(
                        in_val.get_name().to_string(),
                        as_u32
                    );
                }
                // IPv6 不处理，返回原值
                in_val
            }
            // 类型不匹配，返回原值
            _ => in_val,
        }
    }
}
```

**关键点：**
- `ValueProcessor::value_cacu` 接收并返回 `DataField`
- 使用 `match` 匹配输入值类型
- 不匹配的类型应返回原值（保持幂等性）
- 使用 `DataField::from_*` 方法创建正确类型的输出

**2.4 在分发器中添加分支：**

在 `crates/wp-oml/src/core/evaluator/transform/pipe/mod.rs` 中：

```rust
impl ValueProcessor for PipeFun {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match self {
            // ... 其他分支
            PipeFun::Ip4ToInt(o) => o.value_cacu(in_val),
        }
    }
}
```

### 第 4 步：实现解析器

在 `crates/wp-oml/src/parser/pipe_prm.rs` 中添加解析逻辑。

**情况 1：无参数函数（如 ip4_to_int）**

在解析器函数 `parse_pipe_fun` 中添加：

```rust
use crate::language::{Ip4ToInt, PIPE_IP4_TO_INT};

pub(crate) fn parse_pipe_fun(data: &mut &str) -> WResult<PipeFun> {
    let _ = ctx_desc(data, "parse_pipe_fun")?;
    let fun = alt((
        // ... 其他函数
        PIPE_IP4_TO_INT.map(|_| PipeFun::Ip4ToInt(Ip4ToInt::default())),
        // ... 更多函数
    )).parse_next(data)?;
    Ok(fun)
}
```

**情况 2：单参数函数（如 nth）**

实现 `Fun1Builder` trait：

```rust
use wp_parser::fun::fun_trait::Fun1Builder;

impl Fun1Builder for Nth {
    type ARG1 = usize;

    // 解析参数
    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let index = digit1.parse_next(data)?;
        let i: usize = index.parse::<usize>().unwrap_or(0);
        Ok(i)
    }

    // 函数名
    fn fun_name() -> &'static str {
        PIPE_NTH
    }

    // 构建函数对象
    fn build(args: Self::ARG1) -> Self {
        Nth { index: args }
    }
}
```

然后在解析器中使用：

```rust
parser::fun1::<Nth>()
    .map(|x| PipeFun::Nth(x))
    .context(StrContext::Label(PIPE_NTH)),
```

**情况 3：双参数函数（如 time_to_ts_zone）**

实现 `Fun2Builder` trait：

```rust
impl Fun2Builder for TimeToTsZone {
    type ARG1 = i32;
    type ARG2 = TimeStampUnit;

    fn fun_name() -> &'static str {
        PIPE_TIME_TO_TS_ZONE
    }

    // 解析第一个参数
    fn args1(data: &mut &str) -> WResult<i32> {
        let sign = opt("-").parse_next(data)?;
        multispace0.parse_next(data)?;
        let zone = digit1.parse_next(data)?;
        let i: i32 = zone.parse::<i32>().unwrap_or(0);
        if sign.is_some() { Ok(-i) } else { Ok(i) }
    }

    // 解析第二个参数
    fn args2(data: &mut &str) -> WResult<TimeStampUnit> {
        let unit = alt((
            "ms".map(|_| TimeStampUnit::MS),
            "us".map(|_| TimeStampUnit::US),
            "s".map(|_| TimeStampUnit::SS),
        )).parse_next(data)?;
        Ok(unit)
    }

    // 构建函数对象
    fn build(args: (i32, TimeStampUnit)) -> TimeToTsZone {
        TimeToTsZone {
            zone: args.0,
            unit: args.1,
        }
    }
}
```

然后在解析器中使用：

```rust
parser::fun2::<TimeToTsZone>()
    .map(|x| PipeFun::TimeToTsZone(x))
    .context(StrContext::Label(PIPE_TIME_TO_TS_ZONE)),
```

### 第 5 步：编写测试

在实现文件中添加测试（推荐在 evaluator 文件中）：

```rust
#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use std::net::{IpAddr, Ipv4Addr};
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};

    #[test]
    fn test_pipe_ip4_int() {
        // 准备测试数据
        let cache = &mut FieldQueryCache::default();
        let data = vec![DataField::from_ip(
            "src_ip",
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        )];
        let src = DataRecord { items: data };

        // 解析 OML 配置
        let mut conf = r#"
        name : test
        ---
        X  =  pipe  read(src_ip) | ip4_to_int ;
         "#;
        let model = oml_parse_raw(&mut conf).assert();

        // 执行转换
        let target = model.transform(src, cache);

        // 验证结果
        let expect = DataField::from_digit("X".to_string(), 2130706433);
        assert_eq!(target.field("X"), Some(&expect));
    }
}
```

### 第 6 步：编译和验证

```bash
# 编译检查
cargo check -p wp-oml

# 运行测试
cargo test -p wp-oml test_pipe_ip4_int

# 运行所有 pipe 相关测试
cargo test -p wp-oml pipe
```

## 常见函数类型示例

### 类型 1：简单转换函数（无参数）

**示例：Base64 编码**

```rust
// 定义
#[derive(Default, Debug, Clone)]
pub struct Base64Encode {}

// 实现
impl ValueProcessor for Base64Encode {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                let encode = general_purpose::STANDARD.encode(x);
                DataField::from_chars(in_val.get_name().to_string(), encode)
            }
            _ => in_val,
        }
    }
}
```

### 类型 2：带枚举参数的函数

**示例：Base64 解码（支持多种编码）**

```rust
// 定义枚举参数
#[derive(Default, Debug, Clone, Serialize, Deserialize, EnumString, Display)]
pub enum EncodeType {
    #[default]
    Utf8,
    Utf16le,
    Gbk,
    // ... 更多编码
}

// 定义函数结构体
#[derive(Default, Debug, Clone)]
pub struct Base64Decode {
    pub encode: EncodeType,
}

// 实现解析
impl Fun1Builder for Base64Decode {
    type ARG1 = EncodeType;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val: &str = alphanumeric0.parse_next(data).unwrap();
        if val.is_empty() {
            Ok(EncodeType::Utf8)
        } else {
            EncodeType::from_str(val).map_err(|_| {
                ErrMode::Cut(ContextError::default())
            })
        }
    }

    fn fun_name() -> &'static str {
        PIPE_BASE64_DECODE
    }

    fn build(args: Self::ARG1) -> Self {
        Base64Decode { encode: args }
    }
}
```

### 类型 3：提取/访问器函数

**示例：URL 解析**

```rust
// 定义提取类型
#[derive(Default, Debug, Clone, Serialize, Deserialize, EnumString, Display)]
pub enum UrlType {
    #[default]
    Default,
    #[strum(serialize = "domain")]
    Domain,
    #[strum(serialize = "host")]
    HttpReqHost,
    #[strum(serialize = "path")]
    HttpReqPath,
}

// 定义函数
#[derive(Default, Debug, Clone)]
pub struct UrlGet {
    pub key: UrlType,
}

// 实现转换
impl ValueProcessor for UrlGet {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(url_str) => {
                // 解析 URL 并根据 key 提取不同部分
                let result = match self.key {
                    UrlType::Domain => extract_domain(url_str),
                    UrlType::HttpReqHost => extract_host(url_str),
                    UrlType::HttpReqPath => extract_path(url_str),
                    // ...
                };
                DataField::from_chars(in_val.get_name().to_string(), result)
            }
            _ => in_val,
        }
    }
}
```

## DataField 类型系统

OML 使用 `DataField` 作为统一的数据容器，支持多种类型：

### 创建 DataField

```rust
use wp_model_core::model::DataField;

// 字符串
DataField::from_chars("field_name", "value")

// 整数
DataField::from_digit("field_name", 42i64)

// IP 地址
DataField::from_ip("field_name", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))

// 浮点数
DataField::from_float("field_name", 3.14f64)

// 布尔值
DataField::from_bool("field_name", true)

// 时间戳
DataField::from_ts("field_name", 1234567890i64)
```

### 读取 DataField

```rust
match in_val.get_value() {
    Value::Chars(s) => { /* 字符串处理 */ },
    Value::Digit(n) => { /* 整数处理 */ },
    Value::IpAddr(ip) => { /* IP 地址处理 */ },
    Value::Float(f) => { /* 浮点数处理 */ },
    Value::Bool(b) => { /* 布尔值处理 */ },
    Value::TimeStamp(ts) => { /* 时间戳处理 */ },
    _ => in_val, // 不支持的类型返回原值
}
```

## 最佳实践

### 1. 错误处理

```rust
impl ValueProcessor for MyFunc {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                // 尝试转换，失败时返回原值
                if let Ok(result) = try_convert(x) {
                    DataField::from_chars(in_val.get_name().to_string(), result)
                } else {
                    // 返回原值或空值
                    in_val
                }
            }
            _ => in_val,
        }
    }
}
```

### 2. 保持字段名

转换后的字段应保持原字段名：

```rust
// ✅ 正确
DataField::from_digit(in_val.get_name().to_string(), value)

// ❌ 错误
DataField::from_digit("new_name".to_string(), value)
```

### 3. 性能考虑

- 避免不必要的克隆
- 对于大数据量，考虑使用引用
- 复杂计算考虑使用缓存

```rust
impl ValueProcessor for MyFunc {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        match in_val.get_value() {
            Value::Chars(x) => {
                // ✅ 直接使用引用
                let result = process_string(x);
                DataField::from_chars(in_val.get_name().to_string(), result)
            }
            _ => in_val,
        }
    }
}
```

### 4. 类型安全

使用 enum 而非字符串表示选项：

```rust
// ✅ 推荐
#[derive(EnumString, Display)]
pub enum MyOption {
    OptionA,
    OptionB,
}

// ❌ 不推荐
pub struct MyFunc {
    option: String,  // 容易出错
}
```

### 5. 文档注释

为公开结构体添加文档：

```rust
/// 将 IPv4 地址转换为整数表示
///
/// # 示例
/// ```oml
/// X = pipe read(src_ip) | ip4_to_int ;
/// # 输入: 127.0.0.1
/// # 输出: 2130706433
/// ```
#[derive(Clone, Debug, Default)]
pub struct Ip4ToInt {}
```

## 调试技巧

### 1. 添加日志

```rust
use tracing::debug;

impl ValueProcessor for MyFunc {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        debug!("Processing field: {} = {:?}",
               in_val.get_name(), in_val.get_value());
        // ... 处理逻辑
    }
}
```

### 2. 单元测试模式

创建小型测试验证单个场景：

```rust
#[test]
fn test_edge_case() {
    let input = DataField::from_chars("test", "");
    let func = MyFunc::default();
    let output = func.value_cacu(input);
    assert_eq!(output.get_value(), &Value::Chars("expected".into()));
}
```

### 3. 使用 `--nocapture` 查看输出

```bash
cargo test test_my_func -- --nocapture
```

## 常见问题

### Q: 如何处理多种输入类型？

A: 在 match 中添加多个分支：

```rust
match in_val.get_value() {
    Value::Chars(x) => { /* ... */ },
    Value::Digit(n) => { /* ... */ },
    _ => in_val,
}
```

### Q: 函数需要外部依赖怎么办？

A: 在结构体中存储必要的状态：

```rust
pub struct QueryFunc {
    cache: Arc<QueryCache>,
}
```

### Q: 如何支持可选参数？

A: 使用 `Option` 类型：

```rust
pub struct MyFunc {
    param: Option<String>,
}

impl Fun1Builder for MyFunc {
    type ARG1 = Option<String>;
    // ...
}
```

### Q: 测试失败如何定位？

A: 按步骤检查：
1. 解析器是否正确解析（打印解析结果）
2. 枚举分发是否添加
3. 值转换逻辑是否正确
4. 测试数据是否符合预期

## 完整清单

添加新的 PipeFun 需要修改以下文件：

- [ ] `src/language/syntax/functions/pipe/*.rs` - 定义结构体
- [ ] `src/language/syntax/functions/pipe/mod.rs` - 添加到枚举和 Display
- [ ] `src/core/evaluator/transform/pipe/*.rs` - 实现 ValueProcessor
- [ ] `src/core/evaluator/transform/pipe/mod.rs` - 添加分发分支
- [ ] `src/parser/pipe_prm.rs` - 实现解析器
- [ ] 添加测试用例
- [ ] 运行 `cargo test` 验证
- [ ] 更新文档（如需要）

## 参考资源

- [ValueProcessor trait 定义](../../crates/wp-oml/src/core/value_processor.rs)
- [DataField API](../../crates/wp-model-core/src/model/field.rs)
- [Parser combinators](../../crates/wp-parser/)
- [现有 PipeFun 实现示例](../../crates/wp-oml/src/language/syntax/functions/pipe/)

---

如有疑问，请参考现有实现或联系项目维护者。
