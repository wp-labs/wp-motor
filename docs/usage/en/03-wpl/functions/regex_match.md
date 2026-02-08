# regex_match Function Guide

## Overview

`regex_match` is a regular expression matching function in WPL (WP Language) used to check whether log field string content matches a specified regular expression pattern. It uses Rust's regex engine and supports complete regular expression syntax.

## Quick Start

### Basic Syntax

```wpl
regex_match('pattern')
```

- **pattern**: Regular expression pattern (recommend using **single quotes**)
- Returns Ok on successful match, Err on failure

### Simple Examples

```wpl
# Match pure numbers
regex_match('^\d+$')

# Match email format
regex_match('^\w+@\w+\.\w+$')

# Match IP address
regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$')

# Match HTTP methods
regex_match('^(GET|POST|PUT|DELETE)$')
```

## Important Note: Quote Usage

### Recommended: Use Single Quotes (Raw Strings)

```wpl
# ✅ Recommended: Single quotes don't process escapes, suitable for regex
regex_match('^\d+$')           # \d preserved as-is
regex_match('^\w+@\w+\.\w+$')  # \w, \. preserved as-is
regex_match('^[A-Z]+\d+$')     # Works perfectly
```

### Avoid: Double Quotes Lead to Escape Issues

```wpl
# ❌ Wrong: Double quotes attempt to escape \d
regex_match("^\d+$")  # Parsing fails! \d is not a valid escape sequence

# ❌ Wrong: \w also fails
regex_match("^\w+$")  # Parsing fails!
```

**Reason**: WPL's double-quoted string parser only supports `\"`, `\\`, `\n`, `\t` escape sequences, while regex patterns like `\d`, `\w`, `\s` will cause parsing errors.

## Regular Expression Syntax

`regex_match` uses Rust's regex engine and supports the following features:

### 1. Basic Matching

```wpl
# Literal characters
regex_match('hello')          # Matches "hello"
regex_match('error')          # Matches "error"
```

### 2. Character Classes

```wpl
# Digits
regex_match('\d')             # Matches any digit [0-9]
regex_match('\d+')            # Matches one or more digits
regex_match('\d{3}')          # Matches exactly 3 digits

# Alphanumeric
regex_match('\w')             # Matches [a-zA-Z0-9_]
regex_match('\w+')            # Matches one or more word characters

# Whitespace
regex_match('\s')             # Matches space, tab, newline
regex_match('\s+')            # Matches one or more whitespace characters

# Custom character classes
regex_match('[a-z]')          # Matches lowercase letters
regex_match('[A-Z]')          # Matches uppercase letters
regex_match('[0-9]')          # Matches digits
regex_match('[a-zA-Z0-9]')    # Matches letters or digits
```

### 3. Quantifiers

```wpl
# * - Zero or more times
regex_match('a*')             # Matches "", "a", "aa", "aaa"...

# + - One or more times
regex_match('a+')             # Matches "a", "aa", "aaa"... (not empty string)

# ? - Zero or one time
regex_match('colou?r')        # Matches "color" or "colour"

# {n} - Exactly n times
regex_match('\d{4}')          # Matches 4 digits

# {n,m} - Between n and m times
regex_match('\d{2,4}')        # Matches 2 to 4 digits

# {n,} - At least n times
regex_match('\d{3,}')         # Matches 3 or more digits
```

### 4. Anchors

```wpl
# ^ - Start of string
regex_match('^\d+')           # Must start with digits

# $ - End of string
regex_match('\d+$')           # Must end with digits

# ^...$ - Complete match
regex_match('^\d+$')          # Entire string must be digits
```

### 5. Groups and Alternation

```wpl
# (...) - Grouping
regex_match('(ab)+')          # Matches "ab", "abab", "ababab"...

# | - Alternation (OR)
regex_match('cat|dog')        # Matches "cat" or "dog"
regex_match('^(GET|POST)$')   # Matches "GET" or "POST"

# (?:...) - Non-capturing group
regex_match('(?:ab)+')        # Same as (ab)+, but doesn't capture
```

### 6. Special Character Escaping

```wpl
# Escape metacharacters
regex_match('\.')             # Matches dot .
regex_match('\[')             # Matches left bracket [
regex_match('\(')             # Matches left parenthesis (
regex_match('\$')             # Matches dollar sign $
regex_match('\*')             # Matches asterisk *
```

### 7. Flags

```wpl
# (?i) - Case insensitive
regex_match('(?i)error')      # Matches "error", "ERROR", "Error"

# (?m) - Multiline mode
regex_match('(?m)^line')      # ^ matches start of each line

# (?s) - Single line mode (. matches newlines)
regex_match('(?s).*')         # . can match newlines
```

## Practical Use Cases

### Scenario 1: Log Level Matching

```wpl
rule log_level_filter {
    # Select log message field
    | take(message)

    # Match messages containing ERROR or FATAL (case insensitive)
    | regex_match('(?i)(error|fatal)')
}

# Example data:
# message: "Error occurred"     → ✅ Matched
# message: "FATAL exception"    → ✅ Matched
# message: "Warning message"    → ❌ Not matched
```

### Scenario 2: Email Address Validation

```wpl
rule email_validation {
    # Select email field
    | take(email)

    # Validate email format
    | regex_match('^\w+(\.\w+)*@\w+(\.\w+)+$')
}

# Example data:
# email: "user@example.com"           → ✅ Matched
# email: "john.doe@company.co.uk"     → ✅ Matched
# email: "invalid-email"              → ❌ Not matched
# email: "@example.com"               → ❌ Not matched
```

### Scenario 3: IP Address Matching

```wpl
rule ip_address_filter {
    # Select IP address field
    | take(client_ip)

    # Match private IP (192.168.x.x)
    | regex_match('^192\.168\.\d{1,3}\.\d{1,3}$')
}

# Example data:
# client_ip: "192.168.1.1"    → ✅ Matched
# client_ip: "192.168.0.100"  → ✅ Matched
# client_ip: "10.0.0.1"       → ❌ Not matched
# client_ip: "8.8.8.8"        → ❌ Not matched
```

### Scenario 4: URL Path Filtering

```wpl
rule api_endpoint_filter {
    # Select request path
    | take(path)

    # Match API endpoints (/api/v1/...)
    | regex_match('^/api/v\d+/')
}

# Example data:
# path: "/api/v1/users"         → ✅ Matched
# path: "/api/v2/products"      → ✅ Matched
# path: "/static/image.png"     → ❌ Not matched
```

### Scenario 5: Timestamp Format Validation

```wpl
rule timestamp_validation {
    # Select timestamp field
    | take(timestamp)

    # Match ISO 8601 format (YYYY-MM-DD HH:MM:SS)
    | regex_match('^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$')
}

# Example data:
# timestamp: "2024-01-29 15:30:45"  → ✅ Matched
# timestamp: "2024-1-9 5:3:5"       → ❌ Not matched (missing leading zeros)
# timestamp: "29/01/2024 15:30"     → ❌ Not matched (different format)
```

### Scenario 6: HTTP Method Validation

```wpl
rule http_method_check {
    # Select HTTP method field
    | take(method)

    # Only accept safe HTTP methods
    | regex_match('^(GET|HEAD|OPTIONS)$')
}

# Example data:
# method: "GET"     → ✅ Matched
# method: "HEAD"    → ✅ Matched
# method: "POST"    → ❌ Not matched
# method: "DELETE"  → ❌ Not matched
```

### Scenario 7: Version Number Matching

```wpl
rule version_check {
    # Select version field
    | take(version)

    # Match semantic version (e.g., 1.2.3)
    | regex_match('^\d+\.\d+\.\d+$')
}

# Example data:
# version: "1.0.0"     → ✅ Matched
# version: "2.10.5"    → ✅ Matched
# version: "1.0"       → ❌ Not matched (missing patch version)
# version: "v1.2.3"    → ❌ Not matched (has prefix)
```

### Scenario 8: SQL Injection Detection

```wpl
rule sql_injection_detection {
    # Select user input field
    | take(user_input)

    # Detect common SQL injection patterns
    | regex_match('(?i)(union|select|insert|update|delete|drop|;|--|\|)')
}

# Example data:
# user_input: "SELECT * FROM users"  → ✅ Matched (detected)
# user_input: "'; DROP TABLE --"     → ✅ Matched (detected)
# user_input: "normal search query"  → ❌ Not matched (safe)
```

### Scenario 9: File Extension Filtering

```wpl
rule image_file_filter {
    # Select filename field
    | take(filename)

    # Only match image files
    | regex_match('\.(?i)(jpg|jpeg|png|gif|bmp|svg)$')
}

# Example data:
# filename: "photo.jpg"      → ✅ Matched
# filename: "image.PNG"      → ✅ Matched (case insensitive)
# filename: "document.pdf"   → ❌ Not matched
```

### Scenario 10: MAC Address Validation

```wpl
rule mac_address_validation {
    # Select MAC address field
    | take(mac)

    # Match MAC address format (XX:XX:XX:XX:XX:XX)
    | regex_match('^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$')
}

# Example data:
# mac: "00:1B:44:11:3A:B7"  → ✅ Matched
# mac: "AA:BB:CC:DD:EE:FF"  → ✅ Matched
# mac: "invalid-mac"        → ❌ Not matched
```

## Usage Limitations

### Type Restrictions

`regex_match` can only process **string type** fields:

```wpl
# ✅ Correct - field is string
message: "error occurred" -> regex_match('error')

# ❌ Wrong - field is number
status_code: 404 -> regex_match('\d+')  # Will fail

# ❌ Wrong - field is IP address (non-string type)
ip: 192.168.1.1 -> regex_match('\d+')  # Will fail
```

### Performance Considerations

1. **Regex Compilation Overhead**:
   - Regex is recompiled on every call
   - Complex regex compilation may take a few microseconds

2. **Matching Performance**:
   - Simple patterns: Microsecond-level
   - Complex patterns (heavy backtracking): May be slower
   - Recommendation: Avoid overly complex regular expressions

3. **Optimization Suggestions**:
   ```wpl
   # ✅ Recommended: Simple and direct patterns
   regex_match('^\d{4}$')

   # ⚠️ Use with caution: Complex backtracking patterns
   regex_match('^(a+)+b$')  # May cause performance issues
   ```

### Unsupported Features

1. **Named Capture Groups Not Supported**:
   ```wpl
   # ❌ Not supported (cannot extract captured content)
   regex_match('(?P<year>\d{4})')
   ```

2. **Replacement Not Supported**:
   ```wpl
   # ❌ regex_match only matches, doesn't replace
   # For replacement, use chars_replace
   ```

3. **Multiple Patterns Not Supported**:
   ```wpl
   # ❌ Cannot pass multiple patterns
   regex_match('pattern1', 'pattern2')

   # ✅ Use alternation |
   regex_match('pattern1|pattern2')
   ```

## Complete Examples

### Example 1: Log Classification Pipeline

```wpl
rule log_classification {
    # Select log message
    | take(message)

    # Classify as error log
    (
        | regex_match('(?i)(error|exception|failed|fatal)')
        | tag(level, ERROR)
    )
    |
    # Or classify as warning log
    (
        | regex_match('(?i)(warn|warning|deprecated)')
        | tag(level, WARNING)
    )
    |
    # Or classify as normal log
    (
        | tag(level, INFO)
    )
}
```

### Example 2: Security Audit Filtering

```wpl
rule security_audit {
    # Check for dangerous patterns in user input
    | take(user_input)

    # Detect script injection
    | regex_match('(?i)(<script|javascript:|onerror=)')

    # Or detect SQL injection
    | regex_match('(?i)(union|select.*from|insert.*into)')

    # Or detect path traversal
    | regex_match('(\.\./|\.\.\\)')

    # Log as security event if any match
    | tag(security_event, true)
}
```

### Example 3: Structured Log Parsing

```wpl
rule structured_log_parsing {
    # Validate JSON log format
    | take(raw_message)
    | regex_match('^\{.*\}$')

    # Validate required fields are present
    | regex_match('"timestamp":\s*"\d{4}-\d{2}-\d{2}')
    | regex_match('"level":\s*"(INFO|WARN|ERROR)"')
    | regex_match('"message":\s*"[^"]+"')

    # Continue processing after all validations pass
}
```

## Performance Notes

- **Regex Compilation**: Compiled on every call, recommend using simple patterns
- **Matching Speed**:
  - Simple patterns (e.g., `^\d+$`): < 1μs
  - Medium complexity (e.g., email validation): 1-10μs
  - Complex patterns (heavy backtracking): May be > 100μs
- **Memory Overhead**: ~1-10KB per regular expression

## Error Handling

### Common Errors

1. **Invalid Regular Expression**
   ```
   Error: regex_match | invalid regex pattern
   Cause: Regex syntax error
   Solution: Check regex syntax
   ```

2. **Field Does Not Exist**
   ```
   Error: regex_match | no active field
   Cause: No active field currently
   Solution: Use take() to select field first
   ```

3. **Field Type Mismatch**
   ```
   Error: regex_match | field is not a string
   Cause: Field is not a string type
   Solution: Ensure field is Chars type
   ```

4. **Pattern Not Matched**
   ```
   Error: regex_match | not matched
   Cause: Field content doesn't match regex
   Solution: This is normal filtering logic
   ```

## Using with Other Functions

### With Field Selectors

```wpl
# Select field first, then match
| take(message)
| regex_match('error')
```

### With Conditional Checks

```wpl
# Combine multiple conditions
| take(status)
| regex_match('^[45]\d{2}$')  # 4xx or 5xx
```

### With chars_replace

```wpl
# Match first, then replace
| regex_match('error')
| chars_replace(error, ERROR)
```

### With Branch Logic

```wpl
# Different patterns take different branches
(
    | regex_match('^2\d{2}$')  # 2xx
    | tag(status_class, success)
)
|
(
    | regex_match('^4\d{2}$')  # 4xx
    | tag(status_class, client_error)
)
```

## Best Practices

### 1. Prefer Single Quotes

```wpl
# ✅ Recommended
regex_match('^\d+$')

# ❌ Avoid (will cause parsing errors)
regex_match("^\d+$")
```

### 2. Use Anchors to Clarify Match Scope

```wpl
# ✅ Recommended: Complete match
regex_match('^\d{4}$')  # Exactly 4 digits

# ⚠️ May not match expectations: Partial match
regex_match('\d{4}')    # Contains 4 digits
```

### 3. Simplify Regular Expressions

```wpl
# ✅ Recommended: Simple and clear
regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$')

# ⚠️ Overly complex
regex_match('^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$')
```

### 4. Document Complex Patterns with Comments

```wpl
# Match email: username@domain.suffix
regex_match('^\w+@\w+\.\w+$')
```

### 5. Test Edge Cases

```wpl
# Test pattern boundaries
regex_match('^\d{2,4}$')
# Test: 1 ❌, 12 ✅, 123 ✅, 1234 ✅, 12345 ❌
```

## Regular Expression Testing

### Online Testing Tools

1. **regex101.com**
   - Select Rust flavor
   - Test your regular expressions
   - View match details and performance

2. **regexr.com**
   - Visualize matching process
   - Provides cheat sheet

### Command Line Testing

```bash
# Test with WP-Motor
echo "test_value: 12345" | wp-motor test.wpl

# View match results
wp-motor --debug test.wpl < test_data.log
```

## Frequently Asked Questions (FAQ)

### Q1: Why must I use single quotes?

Because WPL's double-quoted string parser only supports limited escape sequences (`\"`, `\\`, `\n`, `\t`), while regular expressions need `\d`, `\w`, `\s`, etc., which will cause parsing failures.

### Q2: How do I match a dot (.)?

```wpl
# Use backslash to escape
regex_match('\.')  # Matches literal dot
```

### Q3: How do I implement case-insensitive matching?

```wpl
# Use (?i) flag
regex_match('(?i)error')  # Matches error, ERROR, Error
```

### Q4: Does regex match completely or partially?

Default is **partial matching**. Use `^` and `$` for complete matching:

```wpl
# Partial match
regex_match('\d+')     # "abc123def" → ✅ Matched

# Complete match
regex_match('^\d+$')   # "abc123def" → ❌ Not matched
```

### Q5: How do I match multiline text?

```wpl
# Use (?m) multiline mode
regex_match('(?m)^ERROR')  # Matches ERROR at start of any line

# Use (?s) single line mode to make . match newlines
regex_match('(?s)start.*end')  # Match across lines
```

### Q6: What about performance?

- Simple patterns: Very fast (microsecond-level)
- Complex patterns: May be slower
- Recommendation: Avoid overly complex backtracking patterns

### Q7: Can I extract matched content?

Not supported. `regex_match` only performs match testing, doesn't extract content.

## Regular Expression Quick Reference

### Common Patterns

| Pattern | Description | Example |
|------|------|------|
| `\d` | Digit | `\d+` matches "123" |
| `\w` | Word character | `\w+` matches "hello" |
| `\s` | Whitespace | `\s+` matches "   " |
| `.` | Any character | `.*` matches anything |
| `^` | Start of line | `^start` must match at beginning |
| `$` | End of line | `end$` must match at end |
| `*` | Zero or more | `a*` matches "", "a", "aa" |
| `+` | One or more | `a+` matches "a", "aa" |
| `?` | Zero or one | `a?` matches "", "a" |
| `{n}` | Exactly n times | `\d{4}` matches "2024" |
| `{n,m}` | n to m times | `\d{2,4}` matches "12", "123" |
| `[abc]` | Character set | `[aeiou]` matches vowels |
| `[^abc]` | Negated set | `[^0-9]` matches non-digits |
| `\|` | Alternation | `cat\|dog` matches "cat" or "dog" |
| `()` | Group | `(ab)+` matches "ab", "abab" |

## Additional Resources

- **Rust Regex Documentation**: https://docs.rs/regex/
- **Development Guide**: `docs/guide/wpl_field_func_development_guide.md`
- **Source Code**: `crates/wp-lang/src/ast/processor/function.rs`
- **Test Cases**: `crates/wp-lang/src/eval/builtins/pipe_fun.rs`

## Version History

- **1.13.1** (2026-02-02)
  - Initial implementation
  - Support complete Rust regex syntax
  - Support all standard regular expression features
  - Add comprehensive test coverage

---

**Tip**: While `regex_match` is powerful, it may impact performance. For simple string matching, consider using `chars_has` or `chars_in`; for numeric range checks, use `digit_range` or `digit_in`. Regular expressions are suitable for complex pattern matching scenarios.
