# OML Match Expression Function Matching

This document describes the function matching capabilities in OML `match` expressions.

## Overview

Since version 1.13.4, OML's `match` expression supports function-based pattern matching, providing more flexible matching than simple value comparisons.

### Basic Syntax

```oml
field_name = match read(source_field) {
    function_name(arguments) => result_value,
    _ => default_value,
};
```

### Difference from Pipe Functions

| Feature | Match Functions | Pipe Functions |
|---------|---------------|---------------|
| **Purpose** | Multi-branch conditional evaluation | Binary filtering (keep/discard) |
| **Return** | Returns different values based on conditions | Match returns original value, non-match returns ignore |
| **Scenario** | Classification, routing, decision-making | Filtering, data cleaning |

**Comparison Example**:

```oml
# Match: classify by prefix into different results
EventType = match read(log) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    starts_with('[INFO]') => chars(info),
    _ => chars(other),
};

# Pipe: filter ERROR logs, others become ignore
ErrorLog = pipe take(log) | starts_with('[ERROR]');
```

## Supported Functions

### String Matching Functions

#### starts_with(prefix)

Checks if the field value starts with the specified prefix.

**Syntax**: `starts_with('prefix')`

**Parameters**:
- `prefix`: String, the prefix to match (quotes required)

**Matching Rules**:
- Field value starts with the specified prefix -> match succeeds
- Field value does not start with the specified prefix -> match fails
- Field is not a string type -> match fails
- Case-sensitive

**Example**:
```oml
EventType = match read(log_line) {
    starts_with('[ERROR]') => chars(error),
    starts_with('[WARN]') => chars(warning),
    _ => chars(info),
};
```

#### ends_with(suffix)

Checks if the field value ends with the specified suffix.

**Syntax**: `ends_with('suffix')`

**Parameters**:
- `suffix`: String, the suffix to match (quotes required)

**Matching Rules**:
- Field value ends with the specified suffix -> match succeeds
- Field value does not end with the specified suffix -> match fails
- Field is not a string type -> match fails
- Case-sensitive

**Example**:
```oml
FileType = match read(filename) {
    ends_with('.json') => chars(json),
    ends_with('.xml') => chars(xml),
    ends_with('.log') => chars(log),
    _ => chars(unknown),
};
```

#### contains(substring)

Checks if the field value contains the specified substring.

**Syntax**: `contains('substring')`

**Parameters**:
- `substring`: String, the substring to match (quotes required)

**Matching Rules**:
- Field value contains the specified substring -> match succeeds
- Field value does not contain the specified substring -> match fails
- Field is not a string type -> match fails
- Case-sensitive

**Example**:
```oml
ErrorType = match read(message) {
    contains('exception') => chars(exception),
    contains('timeout') => chars(timeout),
    contains('failed') => chars(failure),
    _ => chars(normal),
};
```

#### regex_match(pattern)

Matches the field value against a regular expression.

**Syntax**: `regex_match('pattern')`

**Parameters**:
- `pattern`: String, the regex pattern (quotes required)

**Matching Rules**:
- Field value matches the regex -> match succeeds
- Field value does not match the regex -> match fails
- Invalid regex syntax -> match fails with a warning logged
- Field is not a string type -> match fails

**Note**: Uses standard Rust regex syntax

**Example**:
```oml
EventPattern = match read(log_message) {
    regex_match('^\[\d{4}-\d{2}-\d{2}') => chars(timestamped),
    regex_match('^ERROR:.*timeout') => chars(error_timeout),
    regex_match('^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}') => chars(ip_address),
    _ => chars(unmatched),
};
```

#### is_empty()

Checks if the field value is an empty string.

**Syntax**: `is_empty()`

**Parameters**: None

**Matching Rules**:
- Field value is an empty string -> match succeeds
- Field value is non-empty -> match fails
- Field is not a string type -> match fails

**Example**:
```oml
Status = match read(field_value) {
    is_empty() => chars(missing),
    _ => chars(present),
};
```

#### iequals(value)

Case-insensitive comparison of the field value.

**Syntax**: `iequals('value')`

**Parameters**:
- `value`: String, the value to compare (quotes required)

**Matching Rules**:
- Field value equals the parameter value (case-insensitive) -> match succeeds
- Field value does not equal the parameter value -> match fails
- Field is not a string type -> match fails

**Example**:
```oml
NormalizedStatus = match read(status) {
    iequals('success') => chars(ok),
    iequals('error') => chars(fail),
    iequals('warning') => chars(warn),
    _ => chars(unknown),
};
```

### Numeric Comparison Functions

#### gt(value)

Checks if the field value is greater than the specified value.

**Syntax**: `gt(100)` (numeric parameters do not need quotes)

**Parameters**:
- `value`: Numeric, the threshold for comparison

**Matching Rules**:
- Field value > parameter value -> match succeeds
- Field value <= parameter value -> match fails
- Field is not a numeric type -> match fails
- Supports integers (digit) and floats (float)

**Example**:
```oml
Level = match read(count) {
    gt(1000) => chars(critical),
    gt(500) => chars(high),
    gt(100) => chars(medium),
    _ => chars(low),
};
```

#### lt(value)

Checks if the field value is less than the specified value.

**Syntax**: `lt(60)` (numeric parameters do not need quotes)

**Parameters**:
- `value`: Numeric, the threshold for comparison

**Matching Rules**:
- Field value < parameter value -> match succeeds
- Field value >= parameter value -> match fails
- Field is not a numeric type -> match fails
- Supports integers and floats

**Example**:
```oml
Grade = match read(score) {
    lt(60) => chars(fail),
    lt(70) => chars(pass),
    lt(85) => chars(good),
    _ => chars(excellent),
};
```

#### eq(value)

Checks if the field value equals the specified numeric value.

**Syntax**: `eq(5)` (numeric parameters do not need quotes)

**Parameters**:
- `value`: Numeric, the target value for comparison

**Matching Rules**:
- Field value equals parameter value -> match succeeds (float comparison tolerance 1e-10)
- Field value does not equal parameter value -> match fails
- Field is not a numeric type -> match fails
- Supports integers and floats

**Example**:
```oml
Status = match read(level) {
    eq(0) => chars(disabled),
    eq(5) => chars(max_level),
    eq(1) => chars(minimum),
    _ => chars(normal),
};
```

#### in_range(min, max)

Checks if the field value is within the specified range.

**Syntax**: `in_range(20, 30)` (numeric parameters do not need quotes)

**Parameters**:
- `min`: Numeric, the minimum value of the range
- `max`: Numeric, the maximum value of the range

**Matching Rules**:
- min <= field value <= max -> match succeeds
- Field value < min or field value > max -> match fails
- Field is not a numeric type -> match fails
- Supports integers and floats
- Uses closed interval [min, max]

**Example**:
```oml
TempZone = match read(temperature) {
    lt(0) => chars(freezing),
    in_range(0, 10) => chars(cold),
    in_range(10, 20) => chars(cool),
    in_range(20, 30) => chars(comfortable),
    gt(30) => chars(warm),
    _ => chars(unknown),
};
```

## OR Condition Syntax

### Single-source OR Matching

Use `|` to separate multiple alternative conditions within a single branch. Any match succeeds:

```oml
tier = match read(city) {
    chars(bj) | chars(sh) | chars(gz) => chars(tier1),
    chars(cd) | chars(wh) => chars(tier2),
    _ => chars(other),
};
```

OR syntax also works with function matching:

```oml
EventType = match read(log_line) {
    starts_with('[ERROR]') | starts_with('[FATAL]') => chars(critical),
    starts_with('[WARN]') => chars(warning),
    _ => chars(info),
};
```

### Multi-source + OR Matching

Each condition position in a multi-source match supports OR syntax:

```oml
priority = match (read(city), read(level)) {
    (chars(bj) | chars(sh), chars(high)) => chars(priority),
    (chars(gz), chars(low) | chars(mid)) => chars(normal),
    _ => chars(default),
};
```

## Notes

### 1. Parameter Quoting Rules

```oml
# String parameters require quotes
starts_with('prefix')
iequals('value')

# Numeric parameters do not need quotes
gt(100)
eq(5)
in_range(20, 30)

# Incorrect examples
starts_with(prefix)   # missing quotes
gt('100')             # should not have quotes
```

### 2. Case Sensitivity

```oml
# Most string functions are case-sensitive
starts_with('ERROR')  # will not match 'error:'

# Use iequals for case-insensitive matching
iequals('success')    # matches 'SUCCESS', 'Success', 'success'
```

### 3. Match Order

```oml
# match evaluates top to bottom, first matching branch is executed
Grade = match read(score) {
    gt(90) => chars(A),        # check > 90 first
    gt(80) => chars(B),        # then > 80
    gt(70) => chars(C),        # then > 70
    _ => chars(F),
};

# If score = 95, only the first branch (A) matches
```

## Best Practices

### 1. Prefer Simple Functions

```oml
# Recommended: use simple starts_with
match read(url) {
    starts_with('https://') => chars(secure),
    _ => chars(insecure),
}

# Avoid: unnecessary regex
match read(url) {
    regex_match('^https://') => chars(secure),  # worse performance
    _ => chars(insecure),
}
```

### 2. Organize Match Order Properly

```oml
# Recommended: from specific to general
match read(log) {
    starts_with('[ERROR]') => chars(error),     # most specific
    starts_with('[WARN]') => chars(warning),
    contains('exception') => chars(exception),   # broader
    _ => chars(other),                          # default
}
```

### 3. Use iequals for User Input

```oml
# Recommended: use iequals for case-uncertain input
Status = match read(user_input) {
    iequals('yes') => chars(confirmed),
    iequals('no') => chars(rejected),
    _ => chars(invalid),
};
```

## Version History

- **1.16.3** (Unreleased)
  - Added OR condition syntax: `cond1 | cond2 | ...`, express alternative conditions within a single branch
  - Multi-source match supports any number of source fields (no longer limited to 2/3/4)
  - Multi-source match condition positions support OR syntax

- **1.13.4** (2026-02-04)
  - Added match expression function matching support
  - String matching: `starts_with`, `ends_with`, `contains`, `regex_match`, `is_empty`, `iequals`
  - Numeric comparison: `gt`, `lt`, `eq`, `in_range`

---

**Tip**: Match functions are for multi-branch conditional evaluation, while Pipe functions are for binary filtering. Choose the appropriate function type based on your scenario for cleaner code.
