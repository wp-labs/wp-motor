# no() Group Implementation

## Overview

Implemented a `no()` group wrapper that provides negative assertion for any parser in WPL.

## Usage Examples

```wpl
// Negative assertion for symbol
no(symbol(ERROR))

// Negative assertion for digit
no(digit)

// With field name
no(symbol(ERROR)):my_field
```

## Implementation

### 1. AST Changes (`crates/wp-lang/src/ast/field/types.rs`)

Added `negation: bool` field to `WplField` struct:

```rust
pub struct WplField {
    // ... existing fields ...
    pub negation: bool, // no() group: invert matching logic
}
```

### 2. Parser Changes (`crates/wp-lang/src/parser/wpl_field.rs`)

Added parsing logic for `no()` group in `wpl_field_impl`:

- Check for `no(` prefix
- Set `conf.negation = true`
- Parse inner field
- Close `)`  after parsing

### 3. Evaluation Changes (`crates/wp-lang/src/eval/runtime/field.rs`)

Modified `FieldEvalUnit::parse()` to handle negation:

**Negation behavior:**
- Save current data position
- Try to parse wrapped field
- If parse succeeds → Return error (backtrack)
- If parse fails → Return `DataField::from_ignore()` with success

## Tests

Added tests in `crates/wp-lang/src/parser/wpl_field.rs`:

```rust
#[test]
fn test_no_group_parsing() {
    // Test no(symbol(ERROR))
    let conf = WplField::try_parse("no(symbol(ERROR))").assert();
    assert!(conf.negation);

    // Test no(digit)
    let conf = WplField::try_parse("no(digit)").assert();
    assert!(conf.negation);
}

#[test]
fn test_no_group_with_name() {
    // Test no(symbol(ERROR)):my_field
    let conf = WplField::try_parse("no(symbol(ERROR)):my_field").assert();
    assert!(conf.negation);
    assert_eq!(conf.name, Some("my_field".into()));
}
```

## Behavior

| Scenario | Wrapped Parser | no() Result |
|----------|---------------|-------------|
| Match fails | Fails | Success (returns ignore field) |
| Match succeeds | Succeeds | Failure (backtrack) |

## Examples

```wpl
// Log line starts with ERROR - parse fails
no(symbol(ERROR)) + "ERROR: something wrong"  // ❌ Fails

// Log line does NOT start with ERROR - parse succeeds
no(symbol(ERROR)) + "INFO: all good"  // ✅ Returns ignore field

// Useful for negative lookahead
no(symbol(<!--)) + chars  // Match chars that don't start with <!--
```

## Integration

- ✅ Works with all existing parsers (symbol, digit, chars, etc.)
- ✅ Can be combined with field naming `:field_name`
- ✅ Follows existing WPL parser patterns
- ✅ Zero runtime overhead when negation = false

## Version

- **Implemented**: 2026-02-07
- **Version**: 1.15.0
