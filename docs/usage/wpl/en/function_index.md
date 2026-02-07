# WPL Field Functions Index

This document lists all available field functions in the WP-Motor WPL language.

## Field Selectors

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `take` | `take(field_name)` | Select the specified field as the active field | - |
| `last` | `last()` | Select the last field as the active field | - |

## String Matching

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `chars_has` | `chars_has(value)` | Check if the string field equals the specified value | - |
| `chars_not_has` | `chars_not_has(value)` | Check if the string field does not equal the specified value | - |
| `chars_in` | `chars_in([value1, value2, ...])` | Check if the string field is in the value list | - |
| `f_chars_has` | `f_chars_has(target, value)` | Check if the specified field equals the specified value | - |
| `f_chars_not_has` | `f_chars_not_has(target, value)` | Check if the specified field does not equal the specified value | - |
| `f_chars_in` | `f_chars_in(target, [values])` | Check if the specified field is in the value list | - |
| `regex_match` | `regex_match('pattern')` | Match the string field using regular expression | [📖 Detailed Documentation](./regex_match.md) |

## Numeric Matching

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `digit_has` | `digit_has(value)` | Check if the numeric field equals the specified value | - |
| `digit_in` | `digit_in([value1, value2, ...])` | Check if the numeric field is in the value list | - |
| `digit_range` | `digit_range(begin, end)` | Check if the number is within the specified range (inclusive) | [📖 Detailed Documentation](./digit_range.md) |
| `f_digit_has` | `f_digit_has(target, value)` | Check if the specified field equals the specified numeric value | - |
| `f_digit_in` | `f_digit_in(target, [values])` | Check if the specified field is in the numeric value list | - |

## IP Matching

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `ip_in` | `ip_in([ip1, ip2, ...])` | Check if the IP address is in the list | - |
| `f_ip_in` | `f_ip_in(target, [ips])` | Check if the IP address of the specified field is in the list | - |

## Field Existence

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `has` | `has()` | Check if the current active field exists | - |
| `f_has` | `f_has(target)` | Check if the specified field exists | - |

## Wrapper Functions

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `not` | `not(inner_function)` | Invert the success/failure result of the inner pipe function | [📖 Detailed Documentation](./not.md) |

## String Transformation

| Function | Syntax | Description | Documentation |
|------|------|------|------|
| `json_unescape` | `json_unescape()` | Decode JSON escape characters (`\n`, `\t`, `\"`, `\\`, etc.) | - |
| `base64_decode` | `base64_decode()` | Base64 decode the string field | - |
| `chars_replace` | `chars_replace(target, replacement)` | Replace substring in the string | [📖 Detailed Documentation](./chars_replace.md) |

## Function Classification Overview

### By Functionality

#### 1. Condition Check Functions
Used to check if a field meets specific conditions without modifying the field value.

- String checks: `chars_has`, `chars_not_has`, `chars_in`, `regex_match`
- Numeric checks: `digit_has`, `digit_in`, `digit_range`
- IP checks: `ip_in`
- Existence checks: `has`

#### 2. Transformation Functions
Functions that modify field values.

- Decoding: `json_unescape`, `base64_decode`
- Replacement: `chars_replace`

#### 3. Field Selector Functions
Used to select a specific field as the active field.

- `take`: Select by name
- `last`: Select the last field

#### 4. Wrapper Functions
Wrap other functions to change their behavior.

- `not`: Invert the success/failure result of the inner function

### By Target Field Support

#### Operating on Current Active Field
- `chars_has`, `chars_not_has`, `chars_in`
- `digit_has`, `digit_in`, `digit_range`
- `ip_in`
- `has`
- `json_unescape`, `base64_decode`, `chars_replace`
- `regex_match`

#### Can Specify Target Field (with `f_` prefix)
- `f_chars_has`, `f_chars_not_has`, `f_chars_in`
- `f_digit_has`, `f_digit_in`
- `f_ip_in`
- `f_has`

## Usage Examples

### Basic Pipeline

```wpl
rule example_pipeline {
    # 1. Select field
    | take(message)

    # 2. Check condition
    | chars_has(error)

    # 3. Transform and process
    | chars_replace(error, ERROR)
}
```

### Complex Condition Combinations

```wpl
rule complex_filter {
    # Check status code range
    | take(status)
    | digit_range(200, 299)  # 2xx success status codes

    # Check message content
    | take(message)
    | regex_match('(?i)(success|ok|complete)')
}
```

### Branching Logic

```wpl
rule branching_logic {
    # Branch 1: Error logs
    (
        | take(level)
        | chars_in([ERROR, FATAL])
    )
    |
    # Branch 2: Warning logs
    (
        | take(level)
        | chars_in([WARN, WARNING])
    )
}
```

## Performance Reference

| Function Type | Typical Performance | Notes |
|----------|----------|------|
| Field Selection | < 100ns | Extremely fast, based on index lookup |
| String Matching | < 1μs | Simple string comparison |
| Numeric Matching | < 100ns | Simple numeric comparison |
| Range Check | < 500ns | Linear scan of multiple ranges |
| Regex Matching | 1-100μs | Depends on pattern complexity |
| Base64 Decode | 1-10μs | Depends on string length |
| String Replacement | 1-10μs | Depends on string length |

## Best Practices

### 1. Use Appropriate Function Types

```wpl
# ✅ Recommended: Use chars_has for simple matching
| chars_has(error)

# ⚠️ Overuse: Don't use regex for simple matching
| regex_match('^error$')  # Poor performance
```

### 2. Prioritize Specialized Functions

```wpl
# ✅ Recommended: Use digit_range for numeric ranges
| digit_range(200, 299)

# ⚠️ Not recommended: Using regex to match numbers
| regex_match('^2\d{2}$')  # Poor performance
```

### 3. Select Field Before Processing

```wpl
# ✅ Correct
| take(message)
| chars_replace(old, new)

# ❌ Wrong: No active field
| chars_replace(old, new)  # Will fail
```

### 4. Combine Condition Functions

```wpl
# ✅ Recommended: Use simple conditions first, then complex ones
| chars_has(error)          # Fast filtering
| regex_match('error:\d+')  # Precise matching
```

## Function Comparison

### chars_has vs regex_match

| Feature | chars_has | regex_match |
|------|-----------|-------------|
| Purpose | Exact string matching | Pattern matching |
| Performance | Extremely fast | Relatively slow |
| Flexibility | Low | High |
| Use Case | Known fixed values | Complex patterns |

```wpl
# Simple matching: Use chars_has
| chars_has(ERROR)

# Complex matching: Use regex_match
| regex_match('(?i)error:\s*\d+')
```

### digit_in vs digit_range

| Feature | digit_in | digit_range |
|------|----------|-------------|
| Purpose | Discrete value check | Range check |
| Parameters | Value list | Range list |
| Use Case | Specific values (e.g., status codes) | Continuous ranges |

```wpl
# Discrete values: Use digit_in
| digit_in([200, 404, 500])

# Continuous range: Use digit_range
| digit_range(200, 299)
```

### Target Field Functions vs Active Field Functions

| Feature | Active Field Functions | Target Field Functions |
|------|-------------|-------------|
| Prefix | None | `f_` |
| Field Selection | Requires take first | Automatic selection |
| Performance | Slightly faster | Slightly slower (requires lookup) |
| Convenience | Requires extra step | One-step operation |

```wpl
# Active field function
| take(status)
| digit_has(200)

# Target field function (more concise)
| f_digit_has(status, 200)
```

## Related Documentation

- **Development Guide**: [WPL Field Function Development Guide](../../guide/wpl_field_func_development_guide.md)
- **Field Reference**: [Field Reference](./field_reference.md)
- **Separator**: [Separator Guide](./separator.md)

## Version History

- **1.15.1** (2026-02-07)
  - Added `not()` wrapper function
  - Fixed `f_chars_not_has` and `chars_not_has` type checking bug

- **1.13.1** (2026-02-02)
  - Added `digit_range` function
  - Added `regex_match` function
  - Improved documentation system

- **1.11.0** (2026-01-29)
  - Added `chars_replace` function
  - Improved Base64 and JSON escape support

---

**Tip**: Choosing the right function type can significantly improve performance. Prioritize simple specialized functions, and only use regular expressions when complex pattern matching is needed.
