# WPL Group Logic

WPL provides multiple group wrappers to control the logical behavior of field parsing. Groups can contain one or more fields and determine parsing success or failure based on different logical semantics.

## Group Types

### seq - Sequence (Default)

The default group type requires all fields to be parsed successfully in order.

**Syntax:**
```wpl
(field1, field2, field3)
seq(field1, field2, field3)
```

**Behavior:**
- Parses all fields in sequence
- All fields must succeed
- The entire group fails if any field fails

**Example:**
```wpl
(digit:id, chars:name, ip:addr)
```

### opt - Optional

Marks a group as optional, so failure does not affect overall parsing.

**Syntax:**
```wpl
opt(field1, field2)
```

**Behavior:**
- Attempts to parse all fields
- Does not return an error on failure
- Returns parsed results on success, skips on failure

**Example:**
```wpl
opt(symbol([DEBUG]):level), chars:msg
```

### alt - Alternative

Tries multiple parsing options, succeeding if any one succeeds.

**Syntax:**
```wpl
alt(field1, field2, field3)
```

**Behavior:**
- Tries each field in sequence
- Uses the first successful field
- The group fails if all fields fail

**Example:**
```wpl
alt(ip:addr, chars:addr)  # Try to parse as IP, fall back to string
```

### some_of - Partial Match

Requires at least one field to succeed.

**Syntax:**
```wpl
some_of(field1, field2, field3)
```

**Behavior:**
- Attempts to parse all fields
- Succeeds if at least one field succeeds
- The group fails if all fields fail

**Example:**
```wpl
some_of(digit:port, chars:service)
```

### not - Negative Assertion

Reverse logic: succeeds when the inner field fails to parse.

**Syntax:**
```wpl
not(field)
```

**Behavior:**
- Attempts to parse the inner field
- `not()` succeeds when the inner field fails
- `not()` fails when the inner field succeeds
- Returns an `ignore` type field on success

**Input Consumption:**
- `not(symbol(...))` - Consumes input (symbol may consume whitespace on failure)
- `not(peek_symbol(...))` - Does not consume input (peek_symbol never consumes)

**Examples:**
```wpl
# Ensure ERROR keyword is not present
not(symbol(ERROR):check)

# Use with peek_symbol to avoid consuming input
not(peek_symbol(ERROR):check), (chars:msg)
```

## Use Cases

### 1. Conditional Parsing

```wpl
# Parse optional debug information
opt(symbol([DEBUG]):level), chars:msg
```

### 2. Format Compatibility

```wpl
# Support multiple IP address formats
alt(ip:addr, chars:addr)
```

### 3. Negative Filtering

```wpl
# Process only non-error logs
not(symbol(ERROR)), (chars:msg)
```

### 4. Lenient Matching

```wpl
# Match at least port or service name
some_of(digit:port, chars:service)
```

## Combining Groups

Groups can be nested and combined to implement complex parsing logic:

```wpl
# Optional IP or domain
opt(alt(ip:addr, chars:domain))

# Ensure not ERROR, then parse message
not(peek_symbol(ERROR)), (alt(json, kv, chars):msg)
```

## Important Notes

1. **Groups cannot be nested inside other groups**
   ```wpl
   # ❌ Wrong: nesting not supported
   (chars, (digit, chars))

   # ✓ Correct: use multiple parallel groups
   (chars), (digit, chars)
   ```

2. **not() can only contain a single field**
   ```wpl
   # ✓ Correct
   not(symbol(ERROR):check)

   # ❌ Wrong
   not(symbol(ERROR), symbol(FATAL))
   ```

3. **Input consumption depends on the inner parser**
   - Using non-consuming parsers like `peek_symbol` enables lookahead assertions
   - Using consuming parsers like `symbol`, `digit` will change the input position
