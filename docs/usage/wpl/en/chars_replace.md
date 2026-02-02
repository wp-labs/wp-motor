# chars_replace Function Guide

## Overview

`chars_replace` is a string replacement function in WPL (WP Language) used to find and replace specified substrings in log fields.

## Quick Start

### Basic Syntax

```wpl
chars_replace(target, replacement)
```

- **target**: The string to search for and replace
- **replacement**: The new string to replace with

### Simple Examples

```wpl
# Replace "error" with "warning"
chars_replace(error, warning)

# Replace "ERROR" with "WARN"
chars_replace(ERROR, WARN)
```

## Parameter Formats

### 1. Unquoted (Simple Identifiers)

Suitable for simple field names or keywords:

```wpl
chars_replace(old_value, new_value)
chars_replace(test-old, test-new)
chars_replace(错误, 警告)
```

**Supported characters**:
- Letters (a-z, A-Z)
- Digits (0-9)
- Underscore (_)
- Dot (.)
- Slash (/)
- Hyphen (-)
- Unicode characters (Chinese, Japanese, etc.)

### 2. Quoted (Special Characters)

Suitable for strings containing special characters:

```wpl
chars_replace("test,old", "test,new")         # Contains comma
chars_replace("hello world", "goodbye world") # Contains space
chars_replace("status=error", "status=ok")    # Contains equal sign
chars_replace("[ERROR]", "[WARN]")            # Contains brackets
```

**Scenarios requiring quotes**:
- Contains comma (,)
- Contains space
- Contains equal sign (=)
- Contains brackets ([])
- Contains other special symbols

### 3. Mixed Usage

You can mix quoted and unquoted parameters:

```wpl
chars_replace("test,old", new_value)
chars_replace(old_value, "new,value")
```

### 4. Empty String (Delete Text)

Use empty quotes to delete the target string:

```wpl
# Delete "DEBUG: " prefix
chars_replace("DEBUG: ", "")

# Delete commas
chars_replace(",", "")
```

## Practical Use Cases

### Scenario 1: Standardize Log Levels

```wpl
# Unify case
chars_replace(error, ERROR)
chars_replace(warning, WARNING)

# Standardize format
chars_replace("[ERROR]", "ERROR:")
chars_replace("[WARN]", "WARNING:")
```

### Scenario 2: Clean Log Content

```wpl
# Remove debug prefix
chars_replace("DEBUG: ", "")

# Remove extra spaces
chars_replace("  ", " ")

# Remove newlines
chars_replace("\n", " ")
```

### Scenario 3: URL Parameter Replacement

```wpl
chars_replace("status=error", "status=ok")
chars_replace("code=500", "code=200")
```

### Scenario 4: CSV Field Processing

```wpl
# Replace comma-separated names
chars_replace("Smith, John", "John Smith")
chars_replace("Doe, Jane", "Jane Doe")
```

### Scenario 5: Path Normalization

```wpl
# Windows path to Unix path
chars_replace("\\", "/")

# Simplify path
chars_replace("/usr/local/", "/opt/")
```

### Scenario 6: Multilingual Support

```wpl
# Chinese replacement
chars_replace(错误, 警告)
chars_replace("错误:", "警告:")

# Japanese replacement
chars_replace(エラー, 警告)
```

### Scenario 7: Sensitive Information Masking

```wpl
# Replace password
chars_replace("password=12345", "password=***")

# Replace token
chars_replace("token=abc123xyz", "token=***")
```

## Limitations

### Unsupported Features

1. **Escape characters**:
   ```wpl
   # ❌ Not supported (will cause parsing error)
   chars_replace("say \"hello\"", "say 'hi'")
   ```

2. **Regular expressions**:
   ```wpl
   # ❌ Regex not supported
   chars_replace("[0-9]+", "NUMBER")  # Will match literally
   ```

3. **Wildcards**:
   ```wpl
   # ❌ Wildcards not supported
   chars_replace("error*", "warning")  # Will match literally
   ```

### Type Restrictions

`chars_replace` can only process **string type** fields:

```wpl
# ✅ Correct - field is a string
message: "error occurred" -> chars_replace(error, warning)

# ❌ Wrong - field is a number
status_code: 500 -> chars_replace(500, 200)  # Will fail

# ❌ Wrong - field is an IP address
ip_address: 192.168.1.1 -> chars_replace(192, 10)  # Will fail
```

### Replacement Behavior

- **Global replacement**: Replaces **all** matching substrings in the field
  ```wpl
  # Input: "hello hello hello"
  chars_replace(hello, hi)
  # Output: "hi hi hi"
  ```

- **Case-sensitive**: Distinguishes between cases
  ```wpl
  # Input: "Error error ERROR"
  chars_replace(error, warning)
  # Output: "Error warning ERROR"  # Only replaces lowercase "error"
  ```

## Complete Examples

### Example 1: Log Level Normalization Pipeline

```wpl
rule log_normalization {
    # Normalize different ERROR formats
    | chars_replace("[ERROR]", "ERROR:")
    | chars_replace("ERR:", "ERROR:")
    | chars_replace("Err:", "ERROR:")

    # Normalize WARNING
    | chars_replace("[WARN]", "WARNING:")
    | chars_replace("Warn:", "WARNING:")

    # Remove debug information
    | chars_replace("DEBUG: ", "")
}
```

### Example 2: CSV Data Cleanup

```wpl
rule csv_cleanup {
    # Standardize name format (from "Last, First" to "First Last")
    | chars_replace("Smith, John", "John Smith")
    | chars_replace("Doe, Jane", "Jane Doe")

    # Remove extra quotes
    | chars_replace("\"", "")

    # Standardize delimiter
    | chars_replace(";", ",")
}
```

### Example 3: Multi-Step Replacement

```wpl
rule multi_step_replace {
    # Step 1: Replace log level
    | chars_replace(error, ERROR)

    # Step 2: Add timestamp prefix (by replacing empty string)
    | chars_replace("", "[2024-01-29] ")

    # Step 3: Replace service name
    | chars_replace(old-service, new-service)
}
```

## Performance Notes

- **Time complexity**: O(n) - n is the field length
- **Space complexity**: O(n) - requires creating a new string
- **Performance recommendations**:
  - Short strings (< 1KB): Excellent performance, latency < 1μs
  - Long strings (1-10KB): Still fast, latency < 10μs
  - Very long strings (> 10KB): Consider performance impact

## Error Handling

### Common Errors

1. **Field does not exist**
   ```
   Error: chars_replace | no active field
   Cause: No active field currently
   Solution: Use take() or other selectors to select a field first
   ```

2. **Field type mismatch**
   ```
   Error: chars_replace
   Cause: Field is not a string type
   Solution: Ensure field is Chars type
   ```

3. **Syntax error**
   ```
   Error: invalid symbol, expected need ','
   Cause: Parameters containing commas not quoted
   Solution: Wrap parameters with quotes
   ```

## Using with Other Functions

### With Field Selectors

```wpl
# Select field first, then replace
| take(message)
| chars_replace(error, warning)
```

### With Conditional Checks

```wpl
# Replace only under specific conditions
| chars_has(error)
| chars_replace(error, warning)
```

### With Conversion Functions

```wpl
# Decode Base64 first, then replace
| base64_decode()
| chars_replace(old_value, new_value)
```

## Best Practices

### 1. Prefer Unquoted Format

```wpl
# ✅ Recommended (concise)
chars_replace(error, warning)

# ⚠️ Works but unnecessary
chars_replace("error", "warning")
```

### 2. Use Quotes for Complex Strings

```wpl
# ✅ Correct
chars_replace("status=error", "status=ok")

# ❌ Wrong (syntax error)
chars_replace(status=error, status=ok)
```

### 3. Empty String to Delete Text

```wpl
# ✅ Recommended (clear intent)
chars_replace("DEBUG: ", "")

# ⚠️ Unclear
chars_replace("DEBUG: ", nothing)  # "nothing" keyword doesn't exist
```

### 4. Execute Multiple Replacements in Order

```wpl
# ✅ Correct (step-by-step replacement)
| chars_replace(error, ERROR)
| chars_replace(ERROR, WARNING)
# Result: error -> ERROR -> WARNING

# ⚠️ Note the order
| chars_replace(ERROR, WARNING)
| chars_replace(error, ERROR)
# Result: error -> ERROR (second step won't change to WARNING)
```

### 5. Test Edge Cases

```wpl
# Test empty string
chars_replace("", "prefix")  # Inserts between each character

# Test single character
chars_replace(",", ";")      # Simple replacement

# Test long string
chars_replace("very long string to find", "replacement")
```

## Debugging Tips

### 1. Test Step by Step

```wpl
# First step: Only do replacement
| chars_replace(error, warning)

# Second step: Add more replacements
| chars_replace(error, warning)
| chars_replace(warning, info)
```

### 2. Check Field Type

```wpl
# Use has() to confirm field exists
| has()

# Use chars_has() to confirm it's a string type
| chars_has(some_value)
```

### 3. View Replacement Results

Print values before and after replacement in test environment:
```bash
# Use WP-Motor's debug mode
wp-motor --debug rule.wpl < test.log
```

## Frequently Asked Questions (FAQ)

### Q1: How to replace newline characters?

```wpl
# Method 1: Use actual newline character (if parser supports)
chars_replace("\n", " ")

# Method 2: Handle based on actual encoding
chars_replace("
", " ")  # Actual newline
```

### Q2: How to replace multiple different strings simultaneously?

```wpl
# Use multiple chars_replace calls
| chars_replace(error, ERROR)
| chars_replace(warning, WARNING)
| chars_replace(info, INFO)
```

### Q3: How to implement case-insensitive replacement?

chars_replace is case-sensitive, so multiple calls are needed:

```wpl
| chars_replace(error, ERROR)
| chars_replace(Error, ERROR)
| chars_replace(ERROR, ERROR)
```

### Q4: Does replacement modify the original field?

Yes, chars_replace directly modifies the value of the active field.

### Q5: Is performance sufficient?

For most log processing scenarios, performance is more than sufficient:
- Single log < 10KB: Almost imperceptible
- High throughput scenarios: Can process 100K+ logs/second

## Additional Resources

- **Development Guide**: `docs/guide/wpl_field_func_development_guide.md`
- **Parser Implementation**: `docs/guide/chars_replace_parser_tests.md`
- **Performance Analysis**: `docs/guide/take_quoted_string_performance.md`
- **Source Code**: `crates/wp-lang/src/ast/processor/function.rs`

## Version History

- **1.11.0** (2026-01-29)
  - Initial implementation
  - Support for basic string replacement
  - Support for quoted strings (including commas, spaces, etc.)
  - Added complete test coverage

---

**Tip**: If you encounter issues while using this function, please refer to the Error Handling section or consult the development guide.
