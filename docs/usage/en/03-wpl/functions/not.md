# not() - Result Inversion Wrapper Function

## Overview

`not()` is a wrapper function that inverts the success/failure result of an inner pipe function. When the inner function succeeds, `not()` fails; when the inner function fails, `not()` succeeds.

**Syntax**:
```wpl
| not(inner_function)
```

**Parameters**:
- `inner_function`: Any field pipe function (e.g., `f_chars_has`, `has`, `chars_has`, etc.)

**Returns**:
- Inner function succeeds → `not()` fails
- Inner function fails → `not()` succeeds

**Key Features**:
- ✅ **Preserves Field Value**: `not()` only inverts the result, doesn't modify field content
- ✅ **Supports Nesting**: Can use `not(not(...))` for double negation
- ✅ **Automatic Field Selection**: Inherits field selection behavior from inner function
- ✅ **Zero Performance Overhead**: Only clones a single field for testing

## Basic Usage

### 1. String Inequality Check

```wpl
# Check dev_type is not equal to "NDS"
(chars:dev_type) | not(chars_has(NDS))

# Equivalent to
(chars:dev_type) | chars_not_has(NDS)
```

### 2. Field Non-Existence Check

```wpl
# Check field doesn't exist
| not(f_has(optional_field))
```

### 3. Using Target Field Functions

```wpl
# Check specified field not equal to value
| not(f_chars_has(status, ERROR))

# Check specified field not in list
| not(f_chars_in(level, [DEBUG, TRACE]))
```

## Advanced Usage

### Double Negation

Double negation is equivalent to positive assertion:

```wpl
# not(not(...)) is equivalent to using inner function directly
| not(not(f_chars_has(status, OK)))

# Equivalent to
| f_chars_has(status, OK)
```

### Combining Complex Conditions

```wpl
# Field exists but value doesn't equal target
| f_has(dev_type)           # Ensure field exists
| not(chars_has(NDS))       # Ensure value not equal to NDS
```

### Combining with Numeric Functions

```wpl
# Check status code not in success range
| not(f_digit_range(status, 200, 299))

# Check port not in common port list
| not(f_digit_in(port, [80, 443, 8080]))
```

### Combining with Regular Expressions

```wpl
# Check message doesn't match error pattern
| not(regex_match('(?i)error|fail|exception'))
```

## Comparison with Existing Functions

### not(chars_has) vs chars_not_has

While functionally similar, semantics are slightly different:

| Function | Field Missing | Non-Chars Type | Value Not Equal | Value Equal |
|----------|--------------|----------------|-----------------|-------------|
| `not(chars_has(X))` | ✅ Success | ✅ Success | ✅ Success | ❌ Failure |
| `chars_not_has(X)` | ✅ Success | ✅ Success | ✅ Success | ❌ Failure |

**Recommendations**:
- Simple scenarios: Use `chars_not_has` (more intuitive)
- Complex scenarios: Use `not()` wrapper (more flexible)

```wpl
# ✅ Recommended: Simple negation
| chars_not_has(ERROR)

# ✅ Recommended: Complex condition negation
| not(f_digit_range(code, 400, 499))
```

## Practical Use Cases

### Scenario 1: Filter Non-Error Logs

```wpl
rule filter_non_errors {
    # Parse log level
    (symbol(ERROR), symbol(WARN), symbol(INFO), symbol(DEBUG):level)

    # Keep only non-ERROR and non-WARN logs
    | take(level)
    | not(chars_in([ERROR, WARN]))
}
```

**Input**:
```
INFO: Application started
ERROR: Connection failed
DEBUG: Processing request
```

**Output**:
```
INFO: Application started     # ✅ Pass (non-error)
                              # ❌ Filter out ERROR
DEBUG: Processing request     # ✅ Pass (non-error)
```

### Scenario 2: Exclude Specific Device Types

```wpl
rule exclude_device_types {
    # Parse device type field
    (chars:dev_type)

    # Exclude NDS and IDS devices
    | not(f_chars_in(dev_type, [NDS, IDS]))
}
```

**Input**:
```
dev_type=FIREWALL
dev_type=NDS
dev_type=ROUTER
dev_type=IDS
```

**Output**:
```
dev_type=FIREWALL    # ✅ Pass
                     # ❌ Filter out NDS
dev_type=ROUTER      # ✅ Pass
                     # ❌ Filter out IDS
```

### Scenario 3: Non-Standard Port Check

```wpl
rule non_standard_ports {
    # Parse port number
    (digit:port)

    # Exclude standard ports 80 and 443
    | not(f_digit_in(port, [80, 443]))

    # Must also be in valid range
    | digit_range(1, 65535)
}
```

**Input**:
```
80
8080
443
9000
```

**Output**:
```
                # ❌ Filter out 80 (standard port)
8080            # ✅ Pass
                # ❌ Filter out 443 (standard port)
9000            # ✅ Pass
```

### Scenario 4: Exclude Test Environment Logs

```wpl
rule exclude_test_env {
    # Parse environment identifier
    (chars:env)

    # Exclude test and development environments
    | not(f_chars_in(env, [test, dev, staging]))
}
```

## Performance Considerations

### Performance Characteristics

| Operation | Performance Impact |
|-----------|-------------------|
| Single-layer `not()` | < 200ns (clone single field) |
| Double-layer `not(not())` | < 400ns (two clones) |
| Field selection inheritance | 0ns (no extra overhead) |

### Performance Optimization Tips

```wpl
# ✅ Recommended: Use dedicated function (faster)
| chars_not_has(ERROR)

# ⚠️ Acceptable: Use not() wrapper (slightly slower)
| not(chars_has(ERROR))

# ❌ Not recommended: Excessive nesting
| not(not(not(chars_has(ERROR))))  # Meaningless multiple negation
```

## Common Pitfalls

### Pitfall 1: Confusing Pipe-Level not() with Group-Level not()

```wpl
# ❌ Wrong: This is group-level not(), not pipe-level
not(symbol(ERROR):test)

# ✅ Correct: Pipe-level not() for pipe functions
(chars:status) | not(chars_has(ERROR))
```

### Pitfall 2: Expecting not() to Modify Field Value

```wpl
# ❌ Misconception: Thinking not() modifies field
(chars:status) | not(chars_has(ERROR))
# Field value remains original, not() only inverts match result

# ✅ Correct: Use transformation functions to modify value
(chars:status) | chars_replace(ERROR, OK)
```

### Pitfall 3: Wrapping Non-Field Functions with not()

```wpl
# ❌ Wrong: take is not a field pipe function
| not(take(field_name))
# Error: not() can only wrap field pipe functions

# ✅ Correct: Wrap field pipe functions
| not(f_has(field_name))
```

## Difference from Group-Level not()

WPL has two types of `not()`:

| Feature | Pipe-Level `not()` | Group-Level `not()` |
|---------|-------------------|---------------------|
| Purpose | Invert pipe function result | Invert field group match |
| Syntax Position | In pipe `\| not(...)` | Field group definition `not(...)` |
| Parameter Type | Pipe function | Field definition |
| Return Result | Success/Failure | ignore field |

**Comparison Example**:

```wpl
# Pipe-level not(): invert function result
(chars:status) | not(chars_has(ERROR))

# Group-level not(): fail when field exists
not(symbol(ERROR):error_marker)
```

## Best Practices

### 1. Prefer Dedicated Functions

```wpl
# ✅ Recommended: Use chars_not_has
| chars_not_has(ERROR)

# ⚠️ Alternative: Use not() wrapper
| not(chars_has(ERROR))
```

### 2. Use not() for Scenarios Without Dedicated Functions

```wpl
# ✅ Recommended: No digit_not_in, use not()
| not(f_digit_in(port, [80, 443]))

# ✅ Recommended: No digit_not_range, use not()
| not(digit_range(200, 299))
```

### 3. Combine Multiple Conditions

```wpl
# ✅ Recommended: Clear logical combination
| f_has(status)              # Field must exist
| not(chars_in([ERROR, FATAL]))  # And not error status
```

### 4. Avoid Excessive Nesting

```wpl
# ❌ Not recommended: Double negation is confusing
| not(not(chars_has(OK)))

# ✅ Recommended: Use positive form directly
| chars_has(OK)
```

## Troubleshooting

### Issue: not() Doesn't Invert Result

**Possible Cause**: Confusing pipe-level and group-level `not()`

```wpl
# Check if used in correct position
(chars:status) | not(chars_has(ERROR))  # ✅ Correct
not(chars:status)                        # ❌ Wrong (group-level)
```

### Issue: Error "can only wrap field pipe functions"

**Solution**: Ensure wrapping a field pipe function

```wpl
| not(take(field))        # ❌ take is a selector
| not(f_has(field))       # ✅ f_has is a pipe function
```

## Version History

- **1.15.1** (2026-02-07)
  - Added `not()` pipe-level wrapper function
  - Supports inverting any field pipe function result
  - Supports nesting and automatic field selection

## Related Documentation

- [Field Existence Check Functions](./function_index.md#field-existence)
- [String Matching Functions](./function_index.md#string-matching)
- [Group-Level not() Documentation](./groups.md)
- [Function Index](./function_index.md)

---

**Tip**: `not()` is a powerful tool, but don't overuse it. In most cases, using dedicated negation functions (like `chars_not_has`) is more intuitive and performs better.
