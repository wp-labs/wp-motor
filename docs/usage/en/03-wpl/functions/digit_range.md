# digit_range Function Usage Guide

## Overview

`digit_range` is a numeric range checking function in WPL (WP Language) used to determine whether a log field's numeric value falls within a specified single range. This is a simple and efficient closed-interval checking function.

## Quick Start

### Basic Syntax

```wpl
digit_range(begin, end)
```

- **begin**: Lower bound of the range (scalar value)
- **end**: Upper bound of the range (scalar value)
- Check method: `begin <= value <= end` (closed interval)

### Simple Examples

```wpl
# Check if value is in [0, 100] range
digit_range(0, 100)

# Check HTTP success status codes (200-299)
digit_range(200, 299)

# Check if port number is in standard range (0-65535)
digit_range(0, 65535)
```

## Parameter Format

### 1. Single Range

Check a single continuous range:

```wpl
# Check if port number is in standard range (0-65535)
digit_range(0, 65535)

# Check HTTP success status codes (200-299)
digit_range(200, 299)

# Check if age is adult (18-150)
digit_range(18, 150)
```

### 2. Range Limitation Explanation

**Note**: Starting from version 1.13.1, `digit_range` only supports single range checking. For checking multiple discontinuous ranges, please use multiple rules or branching logic (see "Using with Branching Logic" section below).

```wpl
# ✅ Single range check
digit_range(200, 299)  # 2xx status code

# ❌ Multiple range array syntax no longer supported
# digit_range([200, 300], [299, 399])  # Old syntax, deprecated
```

### 3. Boundary Value Handling

Range checking is a **closed interval**, including boundary values:

```wpl
# [100, 200] - includes both 100 and 200
digit_range(100, 200)

# Check value 100: ✅ Pass (equals lower bound)
# Check value 200: ✅ Pass (equals upper bound)
# Check value 150: ✅ Pass (within range)
# Check value 99:  ❌ Fail (less than lower bound)
# Check value 201: ❌ Fail (greater than upper bound)
```

### 4. Negative Number Ranges

Supports negative number ranges:

```wpl
# Temperature range check (-20°C to 40°C)
digit_range(-20, 40)

# Elevation range (Dead Sea -400m to Mount Everest 8848m)
digit_range(-400, 8848)
```

## Practical Application Scenarios

### Scenario 1: HTTP Status Code Classification

```wpl
rule http_success_check {
    # Select status code field
    | take(status_code)

    # Check if it's a success status code (2xx)
    | digit_range(200, 299)
}

# Example data:
# status_code: 200  → ✅ Pass (in [200,299] range)
# status_code: 204  → ✅ Pass (in [200,299] range)
# status_code: 301  → ❌ Fail (not in range)
# status_code: 404  → ❌ Fail (not in range)

# To check multiple status code ranges (e.g., 2xx or 3xx), use branching logic:
rule http_ok_or_redirect {
    | take(status_code)
    | (digit_range(200, 299) | digit_range(300, 399))
}
```

### Scenario 2: Performance Metrics Monitoring

```wpl
rule response_time_check {
    # Select response time field (milliseconds)
    | take(response_time)

    # Check if response time is in normal range (0-500ms)
    | digit_range(0, 500)
}

# Example data:
# response_time: 50   → ✅ Pass (fast response)
# response_time: 200  → ✅ Pass (normal response)
# response_time: 1000 → ❌ Fail (timeout)
```

### Scenario 3: Port Number Validation

```wpl
rule system_port_check {
    # Select port field
    | take(port)

    # Check if it's a system reserved port (1-1023)
    | digit_range(1, 1023)
}

# Example data:
# port: 80    → ✅ Pass (HTTP default port)
# port: 443   → ✅ Pass (HTTPS default port)
# port: 8080  → ❌ Fail (user port)
```

### Scenario 4: Time Period Filtering

```wpl
rule morning_hours_check {
    # Select hour field
    | take(hour)

    # Check if it's morning working hours (9-12)
    | digit_range(9, 12)
}

# Example data:
# hour: 10  → ✅ Pass (morning working hours)
# hour: 11  → ✅ Pass (morning working hours)
# hour: 15  → ❌ Fail (afternoon time)

# Check multiple time periods, use branching logic:
rule business_hours_check {
    | take(hour)
    | (digit_range(9, 12) | digit_range(14, 18))
}
```

### Scenario 5: Age Segmentation

```wpl
rule adult_age_check {
    # Select age field
    | take(age)

    # Adult age range (18-65)
    | digit_range(18, 65)
}

# Example data:
# age: 30  → ✅ Pass (adult)
# age: 50  → ✅ Pass (adult)
# age: 15  → ❌ Fail (minor)
# age: 70  → ❌ Fail (elderly)
```

### Scenario 6: Priority Filtering

```wpl
rule high_priority_filter {
    # Select priority field
    | take(priority)

    # Only process high priority (1-3)
    | digit_range(1, 3)
}

# Example data:
# priority: 1  → ✅ Pass (high priority)
# priority: 3  → ✅ Pass (high priority)
# priority: 5  → ❌ Fail (medium priority)
```

### Scenario 7: Data Quality Check

```wpl
rule data_quality_check {
    # Check temperature sensor data
    | take(temperature)

    # Normal temperature range (-40°C to 80°C)
    | digit_range(-40, 80)
}

# Example data:
# temperature: 25   → ✅ Pass (normal room temperature)
# temperature: -10  → ✅ Pass (winter temperature)
# temperature: 100  → ❌ Fail (abnormal data)
# temperature: -100 → ❌ Fail (sensor fault)
```

## Usage Limitations

### Type Restrictions

`digit_range` can only process **numeric type** fields:

```wpl
# ✅ Correct - field is a number
status_code: 200 -> digit_range(200, 299)

# ❌ Wrong - field is a string
level: "200" -> digit_range(200, 299)  # Will fail

# ❌ Wrong - field is an IP address
ip: 192.168.1.1 -> digit_range(192, 200)  # Will fail
```

### Parameter Requirements

1. **Parameters must be scalar values**:
   ```wpl
   # ✅ Correct - using scalar values
   digit_range(1, 10)

   # ❌ Wrong - array parameters not supported (old syntax)
   digit_range([1], [10])  # Deprecated
   ```

2. **Lower bound should be less than or equal to upper bound**:
   ```wpl
   # ✅ Correct
   digit_range(1, 10)      # 1 <= x <= 10
   digit_range(10, 10)     # x == 10 (single point)

   # ⚠️ Logical error (won't match any value)
   digit_range(10, 1)      # 10 <= x <= 1 (always false)
   ```

### Unsupported Features

1. **Does not support precise floating-point matching**:
   ```wpl
   # ⚠️ Note: Uses i64 internally, floating-point numbers are rounded
   digit_range(1, 10)  # Can only match integers
   ```

2. **Does not support infinite ranges**:
   ```wpl
   # ❌ Not supported
   digit_range(0, infinity)  # No infinite value
   ```

3. **Does not support multiple range arrays**:
   ```wpl
   # ❌ Not supported (old syntax deprecated)
   digit_range([1, 100], [10, 200])  # Use branching logic instead
   ```

## Complete Examples

### Example 1: Log Severity Filtering

```wpl
rule log_error_filter {
    # Select severity level field
    | take(severity)

    # Filter ERROR level (1-2)
    | digit_range(1, 2)

    # Further processing...
}

# Log level definitions:
# 1 = CRITICAL
# 2 = ERROR
# 3 = WARNING
# 4 = WARN
# 5 = INFO
# 6 = DEBUG
```

### Example 2: Performance Monitoring Combination

```wpl
rule performance_monitor {
    # Check response time
    | take(response_ms)
    | digit_range(0, 1000)  # 0-1000ms considered normal

    # Check status code
    | take(status)
    | digit_range(200, 299)  # 2xx

    # Both conditions must be satisfied to pass
}
```

### Example 3: Time Window Analysis

```wpl
rule weekday_check {
    # Weekday check (1=Monday, 7=Sunday)
    | take(day_of_week)
    | digit_range(1, 5)  # Monday to Friday

    # Analyze only weekday data
}
```

## Performance Notes

- **Time Complexity**: O(1) - Single comparison
- **Space Complexity**: O(1) - In-place check
- **Performance Characteristics**:
  - Nanosecond-level execution time
  - Simple integer comparison operation
  - Excellent performance, suitable for high-frequency calls

## Error Handling

### Common Errors

1. **Field does not exist**
   ```
   Error: <pipe> | not in range
   Cause: No active field currently
   Solution: Use take() to select field first
   ```

2. **Field type mismatch**
   ```
   Error: <pipe> | not in range
   Cause: Field is not a numeric type (Digit)
   Solution: Ensure field is numeric type
   ```

3. **Value not in any range**
   ```
   Error: <pipe> | not in range
   Cause: Field value doesn't satisfy any range condition
   Solution: Check if range settings are correct
   ```

## Using with Other Functions

### With Field Selectors

```wpl
# Select field first, then check range
| take(status_code)
| digit_range(200, 299)
```

### With Conditional Checks

```wpl
# Check field exists first, then check range
| has()
| digit_range(0, 100)
```

### With Conversion Functions

```wpl
# Combine for complex validation
| take(response_time)
| digit_range(0, 1000)  # Response time normal
| take(status_code)
| digit_range(200, 299)  # Status code normal
```

### With Branching Logic

```wpl
# Use alt for "or" logic - check multiple discontinuous ranges
(
    # Branch 1: Check if success status code
    | take(status)
    | digit_range(200, 299)
)
|
(
    # Branch 2: Check if redirect status code
    | take(status)
    | digit_range(300, 399)
)
```

## Best Practices

### 1. Range Design Principles

```wpl
# ✅ Recommended: Semantically clear range
digit_range(200, 299)  # HTTP 2xx status code

# ⚠️ Avoid: Using old array syntax
# digit_range([200], [299])  # Deprecated
```

### 2. Concise and Clear Single Range

```wpl
# ✅ Recommended: Simple and direct single range
digit_range(0, 100)

# ✅ Recommended: Multiple ranges use branching logic
(digit_range(0, 50) | digit_range(100, 150))
```

### 3. Range Readability

```wpl
# ✅ Recommended: Add comments explaining range meaning
| digit_range(200, 299)  # HTTP success status codes

# ✅ Recommended: Use meaningful ranges
| digit_range(18, 65)  # Working age range
```

### 4. Boundary Value Testing

```wpl
# Test if boundary values meet expectations
digit_range(100, 200)
# Test: 100 ✅, 200 ✅, 99 ❌, 201 ❌
```

### 5. Using Branching for Discontinuous Ranges

```wpl
# ✅ Recommended: Clear branching logic
(
    digit_range(1, 10)   # First range
    |
    digit_range(50, 100) # Second range
)

# ❌ Avoid: Using deprecated array syntax
# digit_range([1, 50], [10, 100])
```

## Debugging Tips

### 1. Test Single Range

```wpl
# Start with a simple single range
| digit_range(0, 100)

# For multiple ranges, use branching logic
| (digit_range(0, 100) | digit_range(200, 300))
```

### 2. Verify Field Type

```wpl
# Use digit_has() to confirm field is numeric type
| take(my_field)
| digit_has(0)  # If it fails, it's not a numeric field
```

### 3. Check Boundary Values

```bash
# Prepare test data
echo "value: 99" | wp-motor test.wpl    # Test lower bound-1
echo "value: 100" | wp-motor test.wpl   # Test lower bound
echo "value: 200" | wp-motor test.wpl   # Test upper bound
echo "value: 201" | wp-motor test.wpl   # Test upper bound+1
```

## Frequently Asked Questions (FAQ)

### Q1: How to check a single specific value?

```wpl
# Set lower and upper bounds to the same value
digit_range(200, 200)  # Only matches 200
```

### Q2: Can the range be in reverse order?

Technically yes, but logically meaningless:

```wpl
digit_range(100, 50)  # begin > end, never matches
```

### Q3: How to implement "not in range" logic?

Pipeline failure interrupts in WPL, can use branching logic:

```wpl
# Using negation logic (complex)
# Recommendation: Use other field functions or handle in application layer
```

### Q4: Does it support floating-point numbers?

Uses `i64` internally, floating-point numbers are converted:

```wpl
# Field value 3.14 will be treated as 3
digit_range(3, 4)  # May match 3.14 (depending on parsing method)
```

### Q5: How to check multiple discontinuous ranges?

Use branching logic (alt operator):

```wpl
# Check [1,10] or [100,200] ranges
(digit_range(1, 10) | digit_range(100, 200))
# Matches: any value in [1,10] or [100,200]
```

### Q6: Is performance sufficient?

Very fast! Range checking is simple numeric comparison:
- O(1) time complexity
- Nanosecond-level execution time
- Suitable for high-frequency call scenarios

### Q7: Can the old array syntax still be used?

Not recommended, deprecated:

```wpl
# ❌ Old syntax (deprecated)
digit_range([200], [299])

# ✅ New syntax (recommended)
digit_range(200, 299)
```

## Comparison with digit_in

| Feature | digit_range | digit_in |
|---------|-------------|----------|
| Purpose | Single range check | Set membership check |
| Parameters | Two scalars (begin, end) | One array (value list) |
| Use Case | Continuous range | Discrete values |
| Example | `digit_range(0, 100)` | `digit_in([200, 404, 500])` |
| Complexity | O(1) | O(n) |

```wpl
# digit_range - suitable for continuous ranges
digit_range(200, 299)  # 200, 201, ..., 299

# digit_in - suitable for discrete values
digit_in([200, 404, 500])  # Only matches these three values

# Multiple discontinuous ranges - use branching logic
(digit_range(200, 299) | digit_range(300, 399))
```

## Additional Resources

- **Development Guide**: `docs/guide/wpl_field_func_development_guide.md`
- **Source Code**: `crates/wp-lang/src/ast/processor/function.rs`
- **Test Cases**: `crates/wp-lang/src/eval/builtins/pipe_fun.rs`

## Version History

- **1.13.1** (2026-02-02)
  - Refactored to two-parameter form: `digit_range(begin, end)`
  - Deprecated old array syntax: `digit_range([begins], [ends])`
  - Simplified to single range check, performance optimized to O(1)
  - Support for negative number ranges
  - Added complete test coverage

---

**Tip**: `digit_range` is now a simple and efficient single range checking function, suitable for handling continuous numeric range validation scenarios such as status codes, performance metrics, time periods, etc. For multiple discontinuous ranges, please use branching logic (alt operator). For discrete value checking, please use the `digit_in` function.
