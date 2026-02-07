# Strip BOM Processor 使用示例

## 概述

`strip/bom` 处理器用于移除数据中所有位置的 BOM (Byte Order Mark) 标记。

## 支持的 BOM 类型

- UTF-8 BOM: `0xEF 0xBB 0xBF`
- UTF-16 LE BOM: `0xFF 0xFE`
- UTF-16 BE BOM: `0xFE 0xFF`
- UTF-32 LE BOM: `0xFF 0xFE 0x00 0x00`
- UTF-32 BE BOM: `0x00 0x00 0xFE 0xFF`

## 使用方法

### 在 WPL 规则中使用

```wpl
// 清除 UTF-8 BOM
field_name | strip/bom
```

### 在管道链中使用

```wpl
// 先清除 BOM，再进行其他处理
content | strip/bom | base64_decode | json_parse
```

## 代码示例

### Rust 代码

```rust
use wp_lang::eval::builtins::{ensure_builtin_pipe_units, registry};
use wp_parse_api::RawData;

// 1. 确保内置管道单元已注册
ensure_builtin_pipe_units();

// 2. 创建 BOM 清除处理器
let processor = registry::create_pipe_unit("strip/bom")
    .expect("strip/bom processor should be registered");

// 3. 准备带 BOM 的数据 (UTF-8 BOM + "Hello")
let mut data_with_bom = vec![0xEF, 0xBB, 0xBF];
data_with_bom.extend_from_slice(b"Hello");
let input = RawData::from_string(
    String::from_utf8(data_with_bom).unwrap()
);

// 4. 处理数据
let result = processor.process(input).unwrap();

// 5. 验证 BOM 已被移除
assert_eq!(
    wp_lang::eval::builtins::raw_to_utf8_string(&result),
    "Hello"
);
```

## 行为说明

### 1. 检测并移除 BOM

```rust
// 输入: "\u{FEFF}Hello World" (UTF-8 BOM + 文本)
// 输出: "Hello World"
```

### 2. 无 BOM 时保持不变

```rust
// 输入: "Hello World"
// 输出: "Hello World" (无变化)
```

### 3. 移除所有位置的 BOM

```rust
// 输入: "\u{FEFF}Hello\u{FEFF}World"
// 输出: "HelloWorld" (移除所有 BOM)
```

### 4. 容器类型保持一致

```rust
// String 输入 → String 输出
// Bytes 输入 → Bytes 输出
// ArcBytes 输入 → ArcBytes 输出
```

## 常见场景

### 场景 1: 处理用户上传的文本文件

许多 Windows 文本编辑器（如记事本）会在 UTF-8 文件开头添加 BOM。

```wpl
// 清除 BOM 后再解析
file_content | strip/bom | parse_csv
```

### 场景 2: 处理 Web API 响应

某些旧版 API 可能在 JSON 响应前添加 BOM。

```wpl
// 清除 BOM 后再解析 JSON
api_response | strip/bom | json_parse
```

### 场景 3: 日志文件处理

某些日志系统可能在每个日志文件开头添加 BOM。

```wpl
// 清除 BOM 后提取日志内容
log_line | strip/bom | syslog_parse
```

## 性能特点

- **快速检测**：仅检查当前位置的 2-4 字节
- **零拷贝优化**：无 BOM 时直接返回原数据
- **O(n) 时间复杂度**：需要扫描整个数据

## 注意事项

1. **移除所有 BOM**：数据中任意位置的 BOM 都会被移除
2. **非破坏性**：如果没有 BOM，数据完全不变
3. **类型安全**：保持输入容器类型不变

## 测试覆盖

✅ UTF-8 BOM 检测和移除
✅ UTF-16 LE/BE BOM 检测和移除
✅ UTF-32 LE/BE BOM 检测和移除
✅ 无 BOM 数据保持不变
✅ 空数据处理
✅ 只有 BOM 的数据
✅ 包含中文的 UTF-8 BOM 数据
✅ 容器类型保持一致性
✅ 数据中间的 BOM 移除
✅ 多个 BOM 移除
✅ 混合类型 BOM 移除
✅ 数据末尾的 BOM 移除
✅ 连续 BOM 移除

---

**版本**: 1.15.0
**更新日期**: 2026-02-07
