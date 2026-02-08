# WPL Separator Usage Guide

## Overview

Separator is a key syntax element in WPL used to split log fields. By flexibly using separators, you can parse various formats of structured log data.

## Quick Start

### Basic Syntax

```wpl
| take(field_name) separator
```

The separator is written after the field definition to indicate where the field ends.

### Simple Examples

```wpl
# Using space separator
| take(ip) \s
| take(method) \s
| take(path)

# Using comma separator
| take(name) ,
| take(age) ,
| take(city)
```

## Built-in Separators

### 1. Space Separator `\s`

Matches a single space character.

```wpl
# Input: "192.168.1.1 GET /api/users"
rule parse_log {
    | take(ip) \s
    | take(method) \s
    | take(path)
}
```

**Use Cases**:
- Apache/Nginx access logs
- Simple space-separated logs
- Standard format log files

### 2. Tab Separator `\t`

Matches a single tab character.

```wpl
# Input: "user001\t25\tBeijing"
rule parse_tsv {
    | take(user_id) \t
    | take(age) \t
    | take(city)
}
```

**Use Cases**:
- TSV (Tab-Separated Values) files
- Database export files
- Tab-aligned logs

### 3. Generic Whitespace Separator `\S`

Matches space **or** tab (either one).

```wpl
# Input: "field1 field2\tfield3" (mixed spaces and tabs)
rule parse_flexible {
    | take(col1) \S
    | take(col2) \S
    | take(col3)
}
```

**Use Cases**:
- Inconsistent format logs (mixed spaces and tabs)
- Manually edited configuration files
- Lenient data parsing

**Behavior**:
- Stops at either space or tab
- Flexibly handles inconsistent data sources

### 4. End-of-Line Separator `\0`

Reads to the end of line (newline or end of string).

```wpl
# Input: "prefix_value some remaining text until end"
rule parse_to_end {
    | take(prefix) _
    | take(remaining) \0
}
```

**Use Cases**:
- Parsing the last field
- Reading message body
- Getting all remaining content

**Aliases**:
- `\0` and `0` are equivalent

### 5. Custom Character Separator

Use any single character as a separator.

```wpl
# Comma separated
| take(name) ,
| take(age) ,

# Pipe separated
| take(id) |
| take(status) |

# Semicolon separated
| take(key) ;
| take(value) ;
```

**Supported Characters**:
- Comma `,`
- Pipe `|`
- Semicolon `;`
- Colon `:`
- Equal sign `=`
- Slash `/`
- Any single character

### 6. Custom String Separator

Use multi-character strings as separators.

```wpl
# Using " | " as separator
| take(field1) " | "
| take(field2) " | "

# Using " :: " as separator
| take(module) " :: "
| take(function) " :: "
```

**Use Cases**:
- Formatted output logs
- Specific format protocols
- Data requiring clear boundaries

## Practical Application Scenarios

### Scenario 1: Parsing Nginx Access Logs

```wpl
# Log format: 192.168.1.1 - - [29/Jan/2024:10:30:45 +0800] "GET /api/users HTTP/1.1" 200 1234
rule nginx_access_log {
    | take(client_ip) \s
    | take(identity) \s
    | take(user) \s
    | take(timestamp) \s
    | take(request) \s
    | take(status_code) \s
    | take(bytes_sent) \0
}
```

### Scenario 2: Parsing TSV Data

```wpl
# Input: "2024-01-29\t10:30:45\tERROR\tDatabase connection failed"
rule tsv_log {
    | take(date) \t
    | take(time) \t
    | take(level) \t
    | take(message) \0
}
```

### Scenario 3: Parsing CSV Data

```wpl
# Input: "John Smith,30,New York,Engineer"
rule csv_parser {
    | take(name) ,
    | take(age) ,
    | take(city) ,
    | take(job) \0
}
```

### Scenario 4: Parsing Structured Logs

```wpl
# Input: "level=error | module=database | msg=Connection timeout"
rule structured_log {
    | take(level_prefix) =
    | take(level_value) " | "
    | take(module_prefix) =
    | take(module_value) " | "
    | take(msg_prefix) =
    | take(message) \0
}
```

### Scenario 5: Handling Mixed Whitespace Logs

```wpl
# Input: "192.168.1.1 \tGET\t /api/data" (mixed spaces and tabs)
rule flexible_whitespace {
    | take(ip) \S
    | take(method) \S
    | take(path) \0
}
```

### Scenario 6: Parsing Syslog Format

```wpl
# Input: "Jan 29 10:30:45 hostname app[1234]: Error message here"
rule syslog {
    | take(month) \s
    | take(day) \s
    | take(time) \s
    | take(hostname) \s
    | take(app_tag) ": "
    | take(message) \0
}
```

### Scenario 7: Parsing Key-Value Logs

```wpl
# Input: "user=admin;action=login;ip=192.168.1.1;status=success"
rule kv_log {
    | take(user_key) =
    | take(user_value) ;
    | take(action_key) =
    | take(action_value) ;
    | take(ip_key) =
    | take(ip_value) ;
    | take(status_key) =
    | take(status_value) \0
}
```

## Separator Priority

WPL supports three levels of separator settings:

### 1. Field-level Separator (Priority 3, Highest)

```wpl
| take(field1) ,  # This field uses comma
| take(field2) \s # This field uses space
```

### 2. Group-level Separator (Priority 2)

```wpl
group {
    sep = \t  # All fields in the group use tab by default
    | take(field1)
    | take(field2)
}
```

### 3. Inherited Separator (Priority 1, Lowest)

Default separator inherited from upstream rules.

### Priority Rules

Field-level > Group-level > Inherited-level

```wpl
group {
    sep = \t         # Group-level: tab
    | take(f1)       # Uses \t
    | take(f2) ,     # Uses , (field-level overrides group-level)
    | take(f3)       # Uses \t
}
```

## Separator Behavior

### Global Replacement vs Single Match

Separators only match once at the current field's end position:

```wpl
# Input: "hello,world,test"
| take(first) ,   # Reads "hello", consumes first comma
| take(second) ,  # Reads "world", consumes second comma
| take(third) \0  # Reads "test"
```

### Separator Consumption Behavior

By default, separators are **consumed** (removed from input):

```wpl
# Input: "a,b,c"
| take(x) ,  # Reads "a", consumes ",", remaining "b,c"
| take(y) ,  # Reads "b", consumes ",", remaining "c"
```

### When Separator Not Found

If the end of string is reached without finding the separator, reads to the end:

```wpl
# Input: "field1 field2"
| take(f1) ,  # Comma not found, reads entire "field1 field2"
```

## Advanced Usage

### Combining Multiple Separators

```wpl
# Input: "192.168.1.1:8080/api/users?id=123"
rule url_parse {
    | take(ip) :
    | take(port) /
    | take(api_path) /
    | take(resource) ?
    | take(query_string) \0
}
```

### Handling Optional Fields

```wpl
# Input may be: "user,30,city" or "user,,city" (age is empty)
rule optional_fields {
    | take(name) ,
    | take(age) ,      # May be empty string
    | take(city) \0
}
```

### Skipping Unwanted Fields

```wpl
# Only extract fields 1 and 3
rule skip_fields {
    | take(field1) ,
    | take(_skip) ,    # Temporary variable, not saved
    | take(field3) \0
}
```

## Usage Limitations

### 1. Separators Do Not Support Regular Expressions

```wpl
# ❌ Regular expressions not supported
| take(field) [0-9]+

# ✅ Use fixed strings
| take(field) \s
```

### 2. Separators Are Case-Sensitive

```wpl
# "ABC" and "abc" are different separators
| take(field1) ABC
| take(field2) abc
```

### 3. Empty String Cannot Be Used as Separator

```wpl
# ❌ Not supported
| take(field) ""

# ✅ Use \0 to read to end
| take(field) \0
```

### 4. Escape Character Limitations

Currently supported escape characters:
- `\s` - space
- `\t` - tab
- `\S` - space or tab
- `\0` - end of line

Other escape characters (such as `\n`, `\r`) need to use actual characters.

## Performance Notes

### Single Character Separators

Best performance, recommended for priority use:

```wpl
| take(f1) ,
| take(f2) \s
| take(f3) \t
```

- Time complexity: O(n)
- Scan speed: ~500 MB/s

### Multi-character Separators

Slightly lower performance, but still efficient:

```wpl
| take(f1) " | "
| take(f2) " :: "
```

- Time complexity: O(n × m), where m is the separator length
- Scan speed: ~300-400 MB/s

### Generic Whitespace Separator `\S`

Requires character-by-character checking, performance between the two:

```wpl
| take(f1) \S
```

- Time complexity: O(n)
- Scan speed: ~400 MB/s

## Error Handling

### Common Errors

#### 1. Separator Not Found

```
Error: Separator ',' not found
Cause: Input string does not contain the specified separator
Solution: Check input format or use \0 to read to end
```

#### 2. Separator Syntax Error

```
Error: invalid separator
Cause: Used unsupported separator syntax
Solution: Refer to this document for correct separator format
```

#### 3. Field Order Error

```
Error: Field parsing failed
Cause: Field definition order does not match actual data
Solution: Adjust field order to match input format
```

## Best Practices

### 1. Prefer Built-in Separators

```wpl
# ✅ Recommended
| take(f1) \s
| take(f2) \t

# ⚠️ Avoid (unless necessary)
| take(f1) " "
| take(f2) "\t"
```

### 2. Explicitly Specify Last Field Separator

```wpl
# ✅ Recommended (explicit to end of line)
| take(message) \0

# ⚠️ Unclear
| take(message)  # Relies on default behavior
```

### 3. Use `\S` for Non-standard Data

```wpl
# ✅ Recommended (better compatibility)
| take(field1) \S
| take(field2) \S

# ⚠️ May fail (if spaces and tabs are mixed)
| take(field1) \s
| take(field2) \s
```

### 4. Use Multi-character Separators for Complex Formats

```wpl
# ✅ Clear and accurate
| take(level) " | "
| take(message) " | "

# ⚠️ May mismatch
| take(level) |
| take(message) |
```

### 5. Combine Field-level and Group-level Separators

```wpl
# ✅ Recommended (reduces repetition)
group {
    sep = ,
    | take(f1)
    | take(f2)
    | take(f3) \0  # Last field uses different separator
}
```

## Debugging Tips

### 1. Validate Field by Field

```wpl
# First parse the first field
| take(field1) ,

# After confirming success, add the second
| take(field1) ,
| take(field2) ,

# Add incrementally...
```

### 2. Use Temporary Fields to View Intermediate Results

```wpl
| take(field1) ,
| take(_debug) ,    # Temporary field, view remaining content
| take(field2) \0
```

### 3. Print Separator Positions

In test environment, use debug mode to view separator matching:

```bash
wp-motor --debug rule.wpl < test.log
```

### 4. Verify Separator Characters

For invisible characters (like tabs), use hex viewer to confirm:

```bash
# View tabs in file
cat -A test.log
# or
hexdump -C test.log | head
```

## Frequently Asked Questions (FAQ)

### Q1: What's the difference between `\s` and `\S`?

- `\s`: Only matches space
- `\S`: Matches space or tab (space OR tab)

```wpl
# Input: "a b"
| take(x) \s  # ✅ Match successful

# Input: "a\tb"
| take(x) \s  # ❌ Match failed (this is a tab, not a space)
| take(x) \S  # ✅ Match successful
```

### Q2: How to handle consecutive separators?

WPL treats consecutive separators as multiple empty fields:

```wpl
# Input: "a,,c"
| take(f1) ,  # Reads "a"
| take(f2) ,  # Reads "" (empty string)
| take(f3) \0 # Reads "c"
```

### Q3: Do separators affect performance?

Single-character separators have the best performance, multi-character separators are slightly slower, but the impact is negligible for most scenarios.

### Q4: How to parse nested structures?

Use multi-level separators:

```wpl
# Input: "k1=v1;k2=v2|k3=v3;k4=v4"
rule nested {
    | take(group1) |
    | take(group2) \0
}
# Then parse each group internally with ; and =
```

### Q5: Can separators be Chinese characters?

Yes, Unicode characters are supported:

```wpl
# Using Chinese comma as separator
| take(field1) ，
| take(field2) ，
```

### Q6: Is there a difference between `\0` and omitting the separator?

It's recommended to explicitly use `\0` for clearer semantics:

```wpl
# ✅ Recommended (explicit)
| take(message) \0

# ⚠️ Works but unclear
| take(message)
```

### Q7: How to handle separators within quotes?

For complex formats with quotes, it's recommended to use specialized parsers (such as JSON, KV parser):

```wpl
# Complex CSV (with quotes)
# Input: "field1","field with , comma","field3"
# Recommend using CSV parser instead of manual separators
```

## Additional Resources

- **WPL Syntax Reference**: `docs/guide/wpl_syntax.md`
- **Parser Development Guide**: `docs/guide/wpl_field_func_development_guide.md`
- **chars_replace Usage Guide**: `docs/usage/wpl/chars_replace.md`
- **Source Code**: `crates/wp-lang/src/ast/syntax/wpl_sep.rs`

## Version History

- **1.11.0** (2026-01-29)
  - Added `\t` tab separator support
  - Added `\S` generic whitespace separator (space or tab)
  - Optimized Whitespace separator performance
  - Added complete test coverage

- **1.10.x** and earlier versions
  - Support for `\s` (space) and `\0` (end of line)
  - Support for custom character and string separators

---

**Tip**: Separators are the core of WPL parsing. Choosing the right separator can greatly simplify log parsing rules.
