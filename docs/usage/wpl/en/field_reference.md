# WPL Field Reference Guide

## Overview

In WPL, the `@` symbol is used to reference fields in collections. Two field name formats are supported:
- **Plain identifiers**: `@field_name`
- **Single-quoted strings**: `@'@special-field'` (for field names containing special characters)

## Quick Start

### Basic Syntax

```wpl
# Reference plain field
@field_name

# Reference field with special characters
@'special-field-name'

# Specify field type and alias
datatype@field_name: alias_name
```

### Simple Examples

```wpl
# JSON parsing - extract fields
rule parse_json {
    json(
        @src_ip: source_ip,
        @dst_ip: dest_ip,
        @message: msg
    )
}

# Use single quotes for special field names
rule parse_json_special {
    json(
        @'@client-ip': client,
        @'event.type': event,
        @'log/level': level
    )
}
```

## Plain Field References

### Supported Characters

Plain field names (without quotes) support the following characters:
- Letters and numbers (a-z, A-Z, 0-9)
- Underscore (`_`)
- Slash (`/`)
- Hyphen (`-`)
- Dot (`.`)
- Brackets (`[`, `]`) - for array indexing
- Asterisk (`*`) - for wildcards

### Examples

```wpl
# Simple field names
@user_id
@username
@ip_address

# Path-style field names
@process/name
@parent/process/pid
@network/protocol

# Array indexing
@items[0]
@data[5]/value
@process[0]/path

# Wildcards
@items[*]
@logs/*/message
```

## Single-Quoted Field References

### When to Use

Single quotes are required when field names contain the following special characters:
- `@` symbol
- Spaces
- Commas (`,`)
- Equal signs (`=`)
- Parentheses (`(`, `)`)
- Angle brackets (`<`, `>`)
- Hash symbol (`#`)
- Other non-standard characters

### Basic Syntax

```wpl
@'field name with spaces'
@'@field-with-at-sign'
@'field,with,commas'
```

### Escape Sequences

**Double-quoted strings** support the following escape sequences:

| Escape Sequence | Meaning | Example |
|----------------|---------|---------|
| `\"` | Double quote | `@"field\"name"` |
| `\\` | Backslash | `@"path\\to\\file"` |
| `\n` | Newline | `@"multi\nline"` |
| `\t` | Tab | `@"tab\tseparated"` |
| `\r` | Carriage return | `@"carriage\rreturn"` |
| `\xHH` | Hexadecimal byte | `@"hex\x41value"` |

**Single-quoted strings** are **raw strings**:
- **Only supports** `\'` to escape the single quote itself
- All other backslashes `\` are treated literally
- `\n`, `\t`, `\\`, etc. are NOT escaped

### Examples

```wpl
# Double quotes - full escape support
@"field\"name"      # Result: field"name
@"path\\file"       # Result: path\file
@"line\nbreak"      # Result: line(newline)break

# Single quotes - raw string, only escapes \'
@'field\'s name'    # Result: field's name
@'path\to\file'     # Result: path\to\file (literal backslashes)
@'raw\nstring'      # Result: raw\nstring (literal \n)
@'C:\Users\test'    # Result: C:\Users\test (Windows path)
```

**Recommended use cases**:
- **Single quotes**: Windows paths, Unix paths, regular expressions, strings containing backslashes
- **Double quotes**: Scenarios requiring escape characters like newlines, tabs, etc.

## Practical Use Cases

### Scenario 1: Parsing Elasticsearch Logs

```wpl
# Elasticsearch fields typically use @ prefix
rule elasticsearch_log {
    json(
        @'@timestamp': timestamp,
        @'@version': version,
        @message: msg,
        @'log.level': level,
        @'event.action': action
    )
}
```

### Scenario 2: Parsing Fields with Spaces

```wpl
# CSV or other formats may contain column names with spaces
rule csv_with_spaces {
    (
        @'First Name': first_name,
        @'Last Name': last_name,
        @'Email Address': email,
        @'Phone Number': phone
    )
}
```

### Scenario 3: Parsing Nested JSON Fields

```wpl
# JSON field paths containing special characters
rule nested_json {
    json(
        @'user.id': uid,
        @'user.profile.name': username,
        @'event#metadata': metadata,
        @'geo.location.lat': latitude,
        @'geo.location.lon': longitude
    )
}
```

### Scenario 4: Handling Prometheus Metrics

```wpl
# Prometheus metric names contain various special characters
rule prometheus_metrics {
    (
        @'http_requests_total{method="GET"}': get_requests,
        @'http_requests_total{method="POST"}': post_requests,
        @'process_cpu_seconds_total': cpu_usage
    )
}
```

### Scenario 5: Windows Event Logs

```wpl
# Windows paths contain backslashes
rule windows_events {
    json(
        @'Event.System.Provider': provider,
        @'Event.EventData.CommandLine': cmdline,
        @'Process\\Path': process_path
    )
}
```

### Scenario 6: Mixed Use of Plain and Special Field Names

```wpl
rule mixed_fields {
    json(
        # Plain field names
        @username: user,
        @ip_address: ip,
        @timestamp: time,

        # Special field names
        @'@client-ip': client,
        @'user.email': email,
        @'event#type': event_type,
        @'log level': level
    )
}
```

### Scenario 7: KV Parsing with Special Fields

```wpl
# Key-value pairs containing special characters in keys
rule kv_special_keys {
    kv(
        @'@timestamp': time,
        @'event-type': type,
        @'user/name': username,
        @'session#id': session
    )
}
```

## take() Function Quote Support

The `take()` function is used to select the current field and also supports single and double quotes for handling field names with special characters.

### Basic Syntax

```wpl
# Plain field name
| take(field_name)

# Double-quoted field name
| take("@special-field")

# Single-quoted field name
| take('@special-field')
```

### Use Cases

#### 1. Selecting Fields with Special Characters

```wpl
rule select_special_fields {
    # Double quotes
    | take("@timestamp")
    | take("field with spaces")
    | take("field,with,commas")

    # Single quotes
    | take('@client-ip')
    | take('event.type')
    | take('log/level')
}
```

#### 2. Escape Character Support

```wpl
rule escaped_fields {
    # Escaping within double quotes
    | take("field\"name")
    | take("path\\with\\backslash")

    # Escaping within single quotes
    | take('field\'s name')
    | take('path\\to\\file')
}
```

#### 3. Practical Applications

```wpl
# Elasticsearch log processing
rule elasticsearch {
    | take("@timestamp")
    | take("@version")
    | take("log.level")
}

# CSV data processing
rule csv_processing {
    | take('First Name')
    | take('Last Name')
    | take('Email Address')
}

# Mixed usage
rule mixed_usage {
    | take(user_id)          # Plain field
    | take("@timestamp")     # Double quotes
    | take('event.type')     # Single quotes
}
```

### Supported Escape Characters

| Escape Sequence | Meaning | Double Quotes | Single Quotes |
|----------------|---------|---------------|---------------|
| `\"` | Double quote | ✅ | ❌ (literal `\"`) |
| `\'` | Single quote | ❌ (literal `\'`) | ✅ |
| `\\` | Backslash | ✅ | ❌ (literal `\\`) |
| `\n` | Newline | ✅ | ❌ (literal `\n`) |
| `\t` | Tab | ✅ | ❌ (literal `\t`) |

**Explanation**:
- **Double quotes**: Full escape support, similar to C/Java/JavaScript strings
- **Single quotes**: Raw string, only supports `\'` to escape the single quote itself, all other backslashes are literal characters

### Best Practices

```wpl
# ✅ Recommended - prefer unquoted
| take(field_name)

# ✅ Recommended - use quotes for special characters
| take("@timestamp")
| take('@client-ip')

# ✅ Recommended - choose quote type based on content
| take("field with spaces")         # Double quotes, suitable for simple strings
| take('it\'s a field')              # Single quotes, only need to escape \'
| take('C:\Windows\System32')       # Single quotes, Windows paths
| take("line\nbreak")                # Double quotes, need newline escape
```

## Field Type Specification

You can specify data types for fields:

```wpl
# Unquoted fields
ip@source_ip: src
digit@port: port_num
time@timestamp: time

# Quoted fields
ip@'@client-ip': client
digit@'user.age': age
chars@'event message': msg
```

Supported types include:
- `ip` - IP address
- `digit` - Integer
- `float` - Floating point
- `time` - Timestamp
- `chars` - String
- `json` - JSON object
- `kv` - Key-value pair
- etc.

## Field Aliases

Use `:` to specify aliases for fields:

```wpl
# Plain field aliases
@source_ip: src
@destination_ip: dst
@user_id: uid

# Special field aliases
@'@timestamp': time
@'event.type': event
@'log/level': level

# With type and alias
ip@'@client-ip': client_ip
digit@'user.age': age
chars@'user name': username
```

## Usage Limitations

### 1. Double Quotes Not Supported

Only single quotes are supported, double quotes are not supported:

```wpl
# ✅ Correct
@'@field-name'

# ❌ Wrong
@"@field-name"
```

### 2. Escape Character Limitations

Escape characters are only valid within single-quoted strings:

```wpl
# ✅ Correct - escaping within single quotes
@'user\'s name'

# ❌ Wrong - plain field names don't support escaping
@user\'s_name
```

### 3. Empty Field Names

Field names cannot be empty:

```wpl
# ❌ Wrong
@''

# ✅ Correct
@'_'  # Use underscore as field name
```

### 4. Nested References

Single quotes don't support nesting:

```wpl
# ❌ Wrong
@'field\'nested\''

# ✅ Correct - use escaping
@'field\'nested'
```

## Performance Notes

### Parsing Performance

- **Plain field names**: Zero-copy, optimal performance
  ```wpl
  @field_name  # Direct reference, no allocation
  ```

- **Single-quoted field names**: Requires decoding escape characters
  ```wpl
  @'@field'    # No escape characters, performance close to plain fields
  @'field\'s'  # Has escape characters, requires additional processing
  ```

### Performance Comparison

| Field Name Type | Parse Time | Memory Allocation | Recommended Use |
|----------------|------------|-------------------|-----------------|
| Plain field name | ~10ns | Zero-copy | Use first |
| Single-quoted (no escaping) | ~15ns | One allocation | Special characters |
| Single-quoted (with escaping) | ~30ns | One allocation | When necessary |

### Optimization Recommendations

1. **Prefer plain field names**
   ```wpl
   # ✅ Recommended
   @user_id
   @timestamp

   # ⚠️ Only when necessary
   @'@timestamp'
   ```

2. **Avoid unnecessary escaping**
   ```wpl
   # ✅ Recommended
   @'simple-field'

   # ⚠️ Avoid
   @'field\twith\tescape'  # Only when you really need tabs
   ```

3. **Consider performance in bulk operations**
   ```wpl
   # When parsing many fields, prefer plain field names
   json(
       @user_id,      # Fast
       @username,     # Fast
       @'@metadata'   # Slightly slower
   )
   ```

## Error Handling

### Common Errors

#### 1. Field name contains special characters but no quotes used

```
Error: Parse failed, unexpected character '@'
Reason: Field name contains @ but single quotes not used
Solution: @'@field-name'
```

#### 2. Unclosed single quote

```
Error: Unterminated string
Reason: Missing closing single quote
Solution: Ensure quotes are paired @'field-name'
```

#### 3. Invalid escape sequence

```
Error: Invalid escape sequence
Reason: Used unsupported escape character
Solution: Only use supported escape sequences \', \\, \n, \t, \r, \xHH
```

#### 4. Empty field name

```
Error: Field name cannot be empty
Reason: @'' or @ with no content
Solution: Provide a valid field name
```

## Best Practices

### 1. Naming Conventions

```wpl
# ✅ Recommended - use underscore separator
@user_id
@client_ip
@event_timestamp

# ⚠️ Avoid - unless necessary
@'user id'
@'client-ip'
```

### 2. Maintain Consistency

```wpl
# ✅ Recommended - unified style
rule consistent_naming {
    json(
        @user_id,
        @user_name,
        @user_email
    )
}

# ⚠️ Avoid - mixed styles
rule inconsistent_naming {
    json(
        @user_id,
        @'user name',
        @userEmail
    )
}
```

### 3. Document Special Fields

```wpl
# ✅ Recommended - add comments
rule documented {
    json(
        # Elasticsearch's @timestamp field
        @'@timestamp': time,

        # Log level (contains spaces)
        @'log level': level
    )
}
```

### 4. Use Type Prefixes

```wpl
# ✅ Recommended - explicitly specify types
time@'@timestamp': time
ip@'@client-ip': client
chars@'event message': msg
```

### 5. Alias Usage Guidelines

```wpl
# ✅ Recommended - use short aliases
@'very.long.nested.field.name': short_name
@'@timestamp': time
@'event.action': action

# ⚠️ Avoid - aliases too long
@'@timestamp': timestamp_value_from_elasticsearch
```

## Debugging Tips

### 1. Validate field references step by step

```wpl
# Step 1: Validate single field
rule test_single {
    json(@'@timestamp')
}

# Step 2: Add more fields
rule test_multiple {
    json(
        @'@timestamp',
        @'event.type'
    )
}

# Step 3: Add types and aliases
rule test_complete {
    time@'@timestamp': time,
    chars@'event.type': event
}
```

### 2. Check field name spelling

```bash
# Use JSON tools to view original field names
echo '{"@timestamp": "2024-01-01"}' | jq 'keys'

# Output: ["@timestamp"]
# Use in WPL: @'@timestamp'
```

### 3. Test escape characters

```wpl
# Test escape characters one by one
@'test\'quote'      # Single quote
@'test\\backslash'  # Backslash
@'test\nnewline'    # Newline
```

### 4. Use debug mode

```bash
# Use WP-Motor debug mode to view parse results
wp-motor --debug rule.wpl < test.log
```

## Frequently Asked Questions (FAQ)

### Q1: When must single quotes be used?

Single quotes are required when field names contain the following characters:
- `@`, spaces, commas, equal signs, parentheses, angle brackets, hash symbols, and other special characters

### Q2: What's the difference between single quotes and double quotes?

WPL only supports single quotes `'` for field name references. Double quotes `"` are used for other syntax elements (such as scope markers).

### Q3: How to include a single quote in a field name?

Use backslash escaping: `@'user\'s name'`

### Q4: How significant is the performance impact?

For most application scenarios, the performance impact is negligible (nanosecond-level difference). Only consider this when extremely high performance is required.

### Q5: Can I use variables as field names?

No, field names must be static literals.

### Q6: How to handle dynamic field names?

Use wildcards or field combinations:
```wpl
@items[*]/name     # Match name field of all array elements
@'prefix*'         # Match fields starting with prefix (if supported)
```

### Q7: Are Unicode characters supported?

Yes, field names can contain any Unicode characters:
```wpl
@'用户名称'
@'événement'
@'フィールド'
```

## Additional Resources

- **Separator Usage Guide**: `docs/usage/wpl/en/separator.md`
- **chars_replace Usage Guide**: `docs/usage/wpl/en/chars_replace.md`
- **WPL Field Function Development Guide**: `docs/guide/en/wpl_field_func_development_guide.md`
- **Source Code**:
  - `crates/wp-lang/src/parser/utils.rs` (take_ref_path_or_quoted)
  - `crates/wp-lang/src/parser/wpl_field.rs` (wpl_id_field)

## Version History

- **1.11.0** (2026-01-29)
  - Added single-quoted field name support (`@'@special-field'`)
  - Added `take()` function single and double quote support
  - Support for `take("@field")` and `take('@field')` syntax
  - Added escape character support (`\"`, `\'`, `\\`, `\n`, `\t`)
  - Added comprehensive test coverage

---

**Tip**: Prefer plain field names for best performance, only use quotes when field names contain special characters.
