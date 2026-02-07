# WPL Field Function Development Guide

## Overview

This guide introduces how to develop new WPL (WP Language) field functions in WP-Motor. Field functions are core components for processing and transforming log fields, including field selection, condition checking, data transformation, and other functionalities.

## Architecture Overview

The implementation of WPL field functions involves the following core modules:

```
crates/wp-lang/
├── src/ast/processor/
│   ├── function.rs       # Function struct definitions
│   ├── pipe.rs           # WplFun enum and pipe definitions
│   └── mod.rs            # Module exports
├── src/eval/builtins/
│   └── pipe_fun.rs       # FieldPipe trait implementations
└── src/parser/
    └── wpl_fun.rs        # Function parser (optional)
```

## Quick Start: Implementing a String Replace Function

This section demonstrates the complete development workflow using the `chars_replace` function as an example.

### Step 1: Define the Function Struct

Define the function struct in `crates/wp-lang/src/ast/processor/function.rs`:

```rust
/// String replacement function
#[derive(Clone, Debug, PartialEq)]
pub struct ReplaceFunc {
    pub(crate) target: SmolStr,  // Target string to replace
    pub(crate) value: SmolStr,   // New replacement string
}
```

**Naming Conventions:**
- Struct names use PascalCase, ending with `Func` or related suffix
- Fields use `pub(crate)` visibility to ensure module-level accessibility
- Use `SmolStr` for short strings to save memory

### Step 2: Export the Function Struct

Export in `crates/wp-lang/src/ast/processor/mod.rs`:

```rust
pub use function::{
    // ... other exports
    ReplaceFunc,
    // ...
};
```

### Step 3: Add to WplFun Enum

In `crates/wp-lang/src/ast/processor/pipe.rs`:

1. Import the new function:
```rust
use super::function::{
    // ... other imports
    ReplaceFunc,
};
```

2. Add variant to the `WplFun` enum:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum WplFun {
    // ... other variants

    // Transformation functions
    TransJsonUnescape(JsonUnescape),
    TransBase64Decode(Base64Decode),
    TransCharsReplace(ReplaceFunc),  // New addition
}
```

**Naming Conventions:**
- Enum variants use PascalCase
- Recommend adding comments by functionality category (e.g., Transformation functions)
- Group related functions together to maintain clear code organization

### Step 4: Implement the FieldPipe Trait

In `crates/wp-lang/src/eval/builtins/pipe_fun.rs`:

1. Import the function struct:
```rust
use crate::ast::processor::{
    // ... other imports
    ReplaceFunc,
};
```

2. Implement the `FieldPipe` trait:
```rust
impl FieldPipe for ReplaceFunc {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // 1. Check if field exists
        let Some(field) = field else {
            return fail
                .context(ctx_desc("chars_replace | no active field"))
                .parse_next(&mut "");
        };

        // 2. Get mutable reference and process
        let value = field.get_value_mut();
        if value_chars_replace(value, &self.target, &self.value) {
            Ok(())
        } else {
            fail.context(ctx_desc("chars_replace")).parse_next(&mut "")
        }
    }
}
```

3. Implement helper function (at the bottom of the same file, in the `// ---------------- String Mode ----------------` section):
```rust
#[inline]
fn value_chars_replace(v: &mut Value, target: &str, replacement: &str) -> bool {
    match v {
        Value::Chars(s) => {
            let replaced = s.replace(target, replacement);
            *s = replaced.into();
            true
        }
        _ => false,  // Return false for non-string types
    }
}
```

**Implementation Key Points:**
- Use `#[inline]` for performance optimization
- Wrap error messages with `ctx_desc`, format: `"function_name | error_detail"`
- Helper functions return `bool`, `true` for success, `false` for failure
- For functions that modify field values, use `get_value_mut()` to get mutable reference

### Step 5: Register in as_field_pipe Method

In the `impl WplFun` block of `crates/wp-lang/src/eval/builtins/pipe_fun.rs`:

```rust
impl WplFun {
    pub fn as_field_pipe(&self) -> Option<&dyn FieldPipe> {
        match self {
            // ... other match arms
            WplFun::TransJsonUnescape(fun) => Some(fun),
            WplFun::TransBase64Decode(fun) => Some(fun),
            WplFun::TransCharsReplace(fun) => Some(fun),  // New addition
        }
    }

    // If the function supports auto_select (automatic field selection), register it here as well
    pub fn auto_selector_spec(&self) -> Option<FieldSelectorSpec<'_>> {
        match self {
            // Only Target* series functions need to be added
            // WplFun::TransCharsReplace(fun) => fun.auto_select(),
            _ => None,
        }
    }
}
```

### Step 6: Write Tests

Add tests in the `#[cfg(test)] mod tests` block of `crates/wp-lang/src/eval/builtins/pipe_fun.rs`:

```rust
#[test]
fn chars_replace_successfully_replaces_substring() {
    let mut fields = vec![DataField::from_chars(
        "message".to_string(),
        "hello world, hello rust".to_string(),
    )];
    ReplaceFunc {
        target: "hello".into(),
        value: "hi".into(),
    }
    .process(fields.get_mut(0))
    .expect("replace ok");

    if let Value::Chars(s) = fields[0].get_value() {
        assert_eq!(s.as_str(), "hi world, hi rust");
    } else {
        panic!("message should remain chars");
    }
}

#[test]
fn chars_replace_returns_err_on_non_chars_field() {
    let mut fields = vec![DataField::from_digit("num".to_string(), 123)];
    assert!(ReplaceFunc {
        target: "old".into(),
        value: "new".into(),
    }
    .process(fields.get_mut(0))
    .is_err());
}
```

**Testing Key Points:**
- Cover at least success and error scenarios
- Use helper methods like `DataField::from_chars()` / `from_digit()` to create test data
- Name test functions with `test_` or functional descriptions that clearly express test intent

### Step 7: Verify Compilation and Tests

```bash
# Compilation check
cargo check -p wp-lang

# Run all tests
cargo test -p wp-lang

# Run specific test
cargo test -p wp-lang --lib pipe_fun::tests::chars_replace
```

## Function Types and Implementation Patterns

### 1. Condition Check Functions

Used to check if a field meets specific conditions without modifying field values.

**Examples:** `CharsHas`, `DigitHas`, `Has`

```rust
impl FieldPipe for CharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && value.as_str() == self.value.as_str()
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }
}
```

**Characteristics:**
- Return `Ok(())` when condition is met
- Return `fail` when condition is not met (triggers pipeline interruption)
- Does not modify field values

### 2. Target-based Condition Checks

Condition check functions that support specifying target fields.

**Examples:** `TargetCharsHas`, `TargetCharsIn`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsHas {
    pub(crate) target: Option<SmolStr>,  // None means current active field
    pub(crate) value: SmolStr,
}

impl FieldPipe for TargetCharsHas {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        // Implementation logic same as CharsHas
        // ...
    }

    // Key: implement auto_select method
    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}
```

**Characteristics:**
- Struct contains `target: Option<SmolStr>` field
- Implements `auto_select()` method to support automatic field selection
- When `target` is `None`, operates on current active field

### 3. Transformation Functions

Functions that modify field values.

**Examples:** `JsonUnescape`, `Base64Decode`, `ReplaceFunc`

```rust
impl FieldPipe for JsonUnescape {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("json_unescape | no active field"))
                .parse_next(&mut "");
        };

        let value = field.get_value_mut();
        if value_json_unescape(value) {
            Ok(())
        } else {
            fail.context(ctx_desc("json_unescape")).parse_next(&mut "")
        }
    }
}
```

**Characteristics:**
- Need to check if field exists
- Use `get_value_mut()` to get mutable reference
- Return `Ok(())` for successful modification, `fail` for failure

### 4. Field Selector Functions

Used to select specific fields as active fields.

**Examples:** `TakeField`, `SelectLast`

```rust
impl FieldSelector for TakeField {
    fn select(
        &self,
        fields: &mut Vec<DataField>,
        index: Option<&FieldIndex>,
    ) -> WResult<Option<usize>> {
        if let Some(idx) = index.and_then(|map| map.get(self.target.as_str()))
            && idx < fields.len()
        {
            return Ok(Some(idx));
        }
        if let Some(pos) = fields.iter().position(|f| f.get_name() == self.target) {
            Ok(Some(pos))
        } else {
            fail.context(ctx_desc("take | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        }
    }

    fn requires_index(&self) -> bool {
        true  // Requires field index to optimize performance
    }
}
```

**Characteristics:**
- Implements `FieldSelector` trait instead of `FieldPipe`
- Returns the index position of the field in Vec
- Usually paired with `requires_index()` to optimize lookup performance

## Advanced Topics

### 1. Functions with Multiple Parameters

For functions that require multiple parameters, use struct fields to store parameters:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsIn {
    pub(crate) target: Option<SmolStr>,   // Parameter 1: target field
    pub(crate) value: Vec<SmolStr>,       // Parameter 2: list of candidate values
}
```

### 2. Parser Integration (Optional)

If you need to parse function calls from WPL syntax, you need to implement a parser in `crates/wp-lang/src/parser/wpl_fun.rs`.

**Example:**
```rust
impl Fun2Builder for TargetCharsIn {
    type ARG1 = SmolStr;
    type ARG2 = Vec<CharsValue>;

    fn args1(data: &mut &str) -> WResult<Self::ARG1> {
        multispace0.parse_next(data)?;
        let val = take_key.parse_next(data)?;
        Ok(val.into())
    }

    fn args2(data: &mut &str) -> WResult<Self::ARG2> {
        take_arr::<CharsValue>(data)
    }

    fn fun_name() -> &'static str {
        "f_chars_in"  // Function name in WPL syntax
    }

    fn build(args: (Self::ARG1, Self::ARG2)) -> Self {
        let value: Vec<SmolStr> = args.1.iter().map(|i| i.0.clone()).collect();
        Self {
            target: normalize_target(args.0),
            value,
        }
    }
}
```

### 3. Using normalize_target Utility Function

`normalize_target` is used to uniformly process target field parameters:

```rust
pub(crate) fn normalize_target(target: SmolStr) -> Option<SmolStr> {
    if target == "_" {
        None  // "_" means current active field
    } else {
        Some(target)
    }
}
```

### 4. Performance Optimization Recommendations

1. **Use `#[inline]`**: Apply `#[inline]` attribute to small functions
2. **SmolStr Optimization**: Use `SmolStr` for short strings (< 23 bytes) to reduce heap allocations
3. **Avoid Unnecessary Clones**: Prefer passing by reference
4. **Early Returns**: Use `if let` and `&&` chained conditions for early returns

```rust
// Recommended approach
if let Some(item) = field
    && let Value::Chars(value) = item.get_value()
    && value.as_str() == self.value.as_str()
{
    return Ok(());
}
```

## Common Errors and Solutions

### Error 1: Function Struct Not Exported

**Error Message:**
```
error[E0432]: unresolved import `crate::ast::processor::ReplaceFunc`
note: struct `crate::ast::processor::function::ReplaceFunc` exists but is inaccessible
```

**Solution:**
Add export in `crates/wp-lang/src/ast/processor/mod.rs`:
```rust
pub use function::{ReplaceFunc, /* ... */};
```

### Error 2: Not Registered in as_field_pipe

**Error Message:**
```
error[E0004]: non-exhaustive patterns: `WplFun::TransCharsReplace(_)` not covered
```

**Solution:**
Add a new branch in the match statement of the `as_field_pipe()` method.

### Error 3: Type Mismatch

**Error Message:**
```
error[E0277]: the trait bound `ReplaceFunc: FieldPipe` is not satisfied
```

**Solution:**
Ensure that the `FieldPipe` trait has been implemented for the struct.

## Development Checklist

Use the following checklist to ensure complete implementation:

- [ ] Define function struct in `function.rs`
- [ ] Export function struct in `mod.rs`
- [ ] Import function struct in `pipe.rs`
- [ ] Add variant in `WplFun` enum
- [ ] Import function struct in `pipe_fun.rs`
- [ ] Implement `FieldPipe` trait (or `FieldSelector`)
- [ ] Register function in `as_field_pipe()`
- [ ] If supporting target fields, implement `auto_select()` method
- [ ] If supporting target fields, register in `auto_selector_spec()`
- [ ] Write unit tests (cover at least success and failure scenarios)
- [ ] Pass `cargo check -p wp-lang`
- [ ] Pass `cargo test -p wp-lang`
- [ ] (Optional) Implement parser integration

## Reference Implementations

### Simple Transformation Functions
- `JsonUnescape` - JSON escape character decoding
- `Base64Decode` - Base64 decoding
- `ReplaceFunc` - String replacement

### Condition Check Functions
- `CharsHas` - String equality check
- `CharsNotHas` - String inequality check
- `CharsIn` - String in list check
- `DigitHas` - Numeric equality check
- `DigitIn` - Numeric in list check
- `IpIn` - IP address in list check

### Target-based Functions
- `TargetCharsHas` - String check on specified field
- `TargetCharsIn` - String list check on specified field
- `TargetDigitHas` - Numeric check on specified field
- `TargetHas` - Field existence check

### Field Selector Functions
- `TakeField` - Select field by name
- `SelectLast` - Select last field

## Debugging Tips

1. **View Parse Results**: Use `dbg!()` macro to print struct contents
2. **Test Field Processing**: Create simple `DataField` test data
3. **Check Type Conversions**: Confirm correct matching of `Value` enum variants
4. **Enable Logging**: Set `RUST_LOG=debug` to view detailed logs

## Summary

Core steps for developing WPL field functions:

1. **Define**: Define struct in `function.rs`
2. **Export**: Export in `mod.rs`
3. **Enum**: Add to `WplFun` in `pipe.rs`
4. **Implement**: Implement trait in `pipe_fun.rs`
5. **Register**: Register in `as_field_pipe()`
6. **Test**: Write unit tests to verify functionality

By following this guide and referencing existing implementations, you can efficiently develop high-quality field functions.

## Appendix: File Location Quick Reference

```
function.rs      → Define structs
mod.rs           → Export structs
pipe.rs          → Add enum variants, imports
pipe_fun.rs      → Implement traits, imports, registration, tests
wpl_fun.rs       → Parser (optional)
```

## Change Log

- 2026-01-29: Initial version, written based on `chars_replace` implementation experience
