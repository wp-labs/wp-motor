# WPL Usage Guide

This directory contains usage documentation for the WP-Motor WPL (WP Language).

## 📚 Documentation Index

### Core Concepts

- **[Function Index](./function_index_en.md)** - Complete list of all available field functions
- **[Field Reference](./field_reference_en.md)** - Field access and reference guide
- **[Separator](./separator_en.md)** - Log separator configuration guide

### Detailed Function Documentation

#### String Processing

- **[chars_replace](./chars_replace_en.md)** - String replacement function
  - Basic usage: `chars_replace(target, replacement)`
  - Supports quoted strings (handles special characters like commas, spaces, etc.)
  - Global replacement, case-sensitive

- **[regex_match](./regex_match_en.md)** - Regular expression matching function ⭐ New
  - Basic usage: `regex_match('pattern')`
  - Supports full Rust regex syntax
  - **Important**: Use single quotes to avoid escaping issues
  - Suitable for complex pattern matching scenarios

#### Numeric Processing

- **[digit_range](./digit_range_en.md)** - Numeric range checking function ⭐ New
  - Basic usage: `digit_range(begin, end)`
  - Simple and efficient closed interval checking
  - Suitable for HTTP status codes, port numbers, performance metrics, etc.

### Example Code

- **[take_quoted_demo.wpl](../take_quoted_demo.wpl)** - Quoted string processing example

## 🚀 Quick Start

### Basic Pipeline Structure

```wpl
rule my_rule {
    # 1. Select field
    | take(field_name)

    # 2. Condition check
    | chars_has(expected_value)

    # 3. Transform processing
    | chars_replace(old, new)
}
```

### Common Patterns

#### Pattern 1: Log Level Filtering

```wpl
rule error_filter {
    | take(level)
    | chars_in([ERROR, FATAL, CRITICAL])
}
```

#### Pattern 2: Status Code Validation

```wpl
rule status_validation {
    | take(status_code)
    | digit_range(200, 299)  # 2xx success status codes
}
```

#### Pattern 3: Content Pattern Matching

```wpl
rule content_match {
    | take(message)
    | regex_match('(?i)(error|exception|failed)')
}
```

#### Pattern 4: Multiple Condition Combination

```wpl
rule complex_filter {
    # Check status code
    | take(status)
    | digit_range(200, 299)

    # Check response time
    | take(response_time)
    | digit_range(0, 1000)  # Less than 1 second

    # Check path
    | take(path)
    | regex_match('^/api/v\d+/')
}
```

## 📖 Function Selection Guide

### String Matching

| Need | Recommended Function | Example |
|------|----------|------|
| Exact match single value | `chars_has` | `chars_has(ERROR)` |
| Match multiple fixed values | `chars_in` | `chars_in([ERROR, FATAL])` |
| Complex pattern matching | `regex_match` | `regex_match('(?i)error:\d+')` |
| String replacement | `chars_replace` | `chars_replace(old, new)` |

### Numeric Matching

| Need | Recommended Function | Example |
|------|----------|------|
| Exact match single value | `digit_has` | `digit_has(200)` |
| Match multiple discrete values | `digit_in` | `digit_in([200, 404, 500])` |
| Range check | `digit_range` | `digit_range(200, 299)` |

### Performance Priority

1. **Fastest**: `chars_has`, `digit_has` (< 100ns)
2. **Fast**: `chars_in`, `digit_in`, `digit_range` (< 1μs)
3. **Medium**: `chars_replace`, `base64_decode` (1-10μs)
4. **Slower**: `regex_match` (1-100μs, depends on pattern complexity)

**Recommendation**: Prioritize simple dedicated functions, use regular expressions only when needed.

## ⚠️ Common Pitfalls

### 1. regex_match Quote Issues

```wpl
# ✅ Correct: Use single quotes
regex_match('^\d+$')

# ❌ Wrong: Double quotes will cause parsing failure
regex_match("^\d+$")  # \d is not a valid escape sequence
```

### 2. Missing Field Selection

```wpl
# ❌ Wrong: No field selected
chars_replace(old, new)  # Will fail

# ✅ Correct: Select field first
| take(message)
| chars_replace(old, new)
```

### 3. Special Characters Without Quotes

```wpl
# ❌ Wrong: Contains comma without quotes
chars_replace(hello, world, hi)  # Syntax error

# ✅ Correct: Use quotes
chars_replace("hello, world", hi)
```

## 🔧 Debugging Tips

### 1. Use Debug Mode

```bash
# View detailed processing
wp-motor --debug rule.wpl < test.log
```

### 2. Test Step by Step

```wpl
# Test single condition first
| chars_has(error)

# Gradually add more conditions
| chars_has(error)
| regex_match('error:\d+')
```

### 3. Verify Field Type

```wpl
# Use has() to confirm field exists
| has()

# Use type-specific checks to confirm type
| chars_has(any_value)  # Confirm it's a string
| digit_has(0)          # Confirm it's a number
```

## 📝 Development Guide

If you want to develop new field functions, please refer to:

- **[Field Function Development Guide](../../../guide/wpl_field_func_development_guide_en.md)**
  - Complete development workflow
  - Code examples
  - Testing methods
  - Best practices

## 🆕 Latest Updates

### v1.13.1 (2026-02-02)

- ⭐ **Added** `digit_range` function - Numeric range checking
- ⭐ **Added** `regex_match` function - Regular expression matching
- 📖 Improved usage documentation and examples

### v1.11.0 (2026-01-29)

- ⭐ **Added** `chars_replace` function - String replacement
- 📖 Improved quoted string processing documentation

## 📞 Getting Help

- **Issues**: https://github.com/wp-labs/wp-motor/issues
- **Documentation**: `/docs`
- **Examples**: `/examples`

## Related Links

- [Main Documentation](../../../README.md)
- [Development Guide](../../../guide/)
- [Configuration Reference](../../../dar/)

---

**Tip**: Start with the [Function Index](./function_index_en.md) to quickly learn about all available functions.
