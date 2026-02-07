# OML Usage Documentation

WP-Motor OML (Object Mapping Language) usage documentation is available in multiple languages.

## 📚 Documentation by Language

### 🇨🇳 中文文档 (Chinese)
Complete documentation in Simplified Chinese.

**Start here**: [中文文档 README](./zh/README.md)

### 🇬🇧 English Documentation
Coming soon.

---

## 📖 Documentation Contents

The Chinese version includes:

- **README** - Usage guide overview and quick start
- **Function Index** - Complete reference of all pipe functions
- **Match Functions** - Function-based pattern matching in match expressions ⭐ New
- **starts_with** - String prefix matching function
- **map_to** - Type-aware conditional value assignment function
- **static Blocks** - Model-scoped constants & template caching ⭐ New

## 🚀 Quick Links

### Chinese (中文)
- [函数索引](./zh/function_index.md)
- [Match 表达式函数](./zh/match_functions.md) - ⭐ New
- [starts_with 使用指南](./zh/starts_with.md)
- [map_to 使用指南](./zh/map_to.md)
- [`static` 块常量](./zh/static_blocks.md) - ⭐ New

### English
- [`static` Blocks](./static_blocks.md) - New

## 📝 Function Categories

OML provides two types of functions:

### Match Expression Functions ⭐ New
Functions used within `match` expressions for pattern matching:

#### String Matching
- `starts_with(prefix)` - Check if string starts with prefix
- `ends_with(suffix)` - Check if string ends with suffix
- `contains(substring)` - Check if string contains substring
- `regex_match(pattern)` - Match against regex pattern
- `is_empty()` - Check if string is empty
- `iequals(value)` - Case-insensitive string comparison

#### Numeric Comparison
- `gt(value)` - Greater than comparison
- `lt(value)` - Less than comparison
- `eq(value)` - Numeric equality check
- `in_range(min, max)` - Range check

### Pipe Functions
Functions used in pipe chains for data transformation:

#### Field Accessors
- `take(field)` - Extract field from input
- `get(key)` - Get nested field value
- `nth(index)` - Get array element

### String Matching
- `starts_with(prefix)` - Check string prefix

### Value Transformation
- `map_to(value)` - Map to specified value with type inference
- `to_str` - Convert to string
- `to_json` - Convert to JSON

### Encoding/Decoding
- Base64: `base64_encode`, `base64_decode`
- HTML: `html_escape`, `html_unescape`
- JSON: `json_escape`, `json_unescape`

### Time Conversion
- `Time::to_ts*` - Timestamp conversion
- `Time::to_ts_zone` - Timezone conversion

### Network
- `ip4_to_int` - IPv4 to integer conversion
- `url(type)` - URL parsing
- `path(type)` - Path parsing

---

**Version**: 1.13.4
**Last Updated**: 2026-02-04

**What's New in 1.13.4**:
- ⭐ Match expression function-based pattern matching
- String matching functions: `starts_with`, `ends_with`, `contains`, `regex_match`, `is_empty`, `iequals`
- Numeric comparison functions: `gt`, `lt`, `eq`, `in_range`
- Pipe functions: `starts_with`, `map_to`
- ⭐ Temporary fields with `__` prefix - automatic filtering after transformation
- ⭐ Quoted string support: `chars('hello world')`
- `pipe` keyword is now optional

## 💡 Temporary Fields

OML supports temporary fields for intermediate calculations with automatic filtering.

### Naming Convention

Fields starting with `__` (double underscore) are treated as temporary fields:

```oml
name : example
---
# Temporary fields (auto-filtered)
__protocol = take(url) | starts_with('https://') | map_to('https');
__is_secure = match read(__protocol) {
    chars(https) => chars(true),
    _ => chars(false),
};

# Final output field
security_level = match read(__is_secure) {
    chars(true) => chars(high),
    _ => chars(low),
};
```

**Output**:
- `__protocol` → ignore type (filtered)
- `__is_secure` → ignore type (filtered)
- `security_level` → normal output

### Performance

OML uses **parse-time detection + runtime conditional filtering**:

| Scenario | Overhead | Description |
|----------|----------|-------------|
| No temp fields | **~1ns** | Skip filtering (99%+ cost reduction) |
| With temp fields | ~500ns | Execute filtering logic |
| Parse-time check | ~50-500ns | One-time cost (negligible) |

### Use Cases

1. **Complex logic decomposition** - Break down complex rules into steps
2. **Intermediate state** - Store intermediate results for reuse
3. **Avoid duplication** - Cache common calculations
4. **Readability** - Name intermediate values for clarity
>>>>>>> main
