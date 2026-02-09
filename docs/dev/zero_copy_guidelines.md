# Zero-Copy Implementation Guidelines

**Version**: 1.0
**Date**: 2026-02-09
**Status**: Active

---

## Overview

This document establishes the guidelines for maintaining zero-copy optimization in the wp-oml codebase, specifically for `Arc<DataField>` variants used in static symbol references.

---

## Core Principle

**Golden Rule**: The `extract_storage` method is now REQUIRED for all FieldExtractor implementations (compile-time enforced since 2026-02-09). Any type implementing `FieldExtractor` MUST provide an explicit `extract_storage` implementation.

- Types with `Arc<DataField>` variants MUST return `FieldStorage::Shared` via direct Arc::clone without calling `extract_one`.
- Types without Arc variants MUST call `extract_one` and wrap the result with `FieldStorage::from_owned`.

### ✅ Correct Pattern

```rust
impl FieldExtractor for SomeType {
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // ✅ CORRECT: Direct Arc::clone → FieldStorage::Shared
            SomeType::FieldArc(arc) => Some(FieldStorage::from_shared(arc.clone())),

            // Regular variants
            SomeType::Field(x) => x
                .extract_one(target, src, dst)
                .map(FieldStorage::from_owned),
        }
    }
}
```

### ❌ Incorrect Pattern (Causes Performance Regression)

```rust
impl FieldExtractor for SomeType {
    fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &DataRecord,
    ) -> Option<FieldStorage> {
        match self {
            // ❌ WRONG: Calls extract_one (triggers DataField::clone)
            SomeType::FieldArc(arc) => arc
                .as_ref()
                .extract_one(target, src, dst)  // ← DataField::clone happens here
                .map(|_| FieldStorage::from_shared(arc.clone())),  // ← Discards result
        }
    }
}
```

**Why it's wrong**:
1. `extract_one()` internally clones the `DataField`
2. The cloned field is then discarded (used only for `Some/None` check)
3. Arc is cloned again
4. Result: 2× Arc operations + 1× deep clone (defeats zero-copy purpose)

---

## Implementation Checklist

When adding or modifying code involving `Arc<DataField>`:

### For New Arc Variants

- [ ] Define the Arc variant in the enum
  ```rust
  pub enum MyType {
      FieldArc(Arc<DataField>),
      // ...
  }
  ```

- [ ] Implement `extract_storage` with direct Arc::clone
  ```rust
  impl FieldExtractor for MyType {
      fn extract_storage(...) -> Option<FieldStorage> {
          match self {
              MyType::FieldArc(arc) => Some(FieldStorage::from_shared(arc.clone())),
              // ...
          }
      }
  }
  ```

- [ ] Implement `extract_one` if needed (for fallback)
  ```rust
  fn extract_one(...) -> Option<DataField> {
      match self {
          MyType::FieldArc(arc) => arc.as_ref().extract_one(target, src, dst),
          // ...
      }
  }
  ```

- [ ] Add test coverage in `crates/wp-oml/tests/zero_copy_validation.rs`

### For Existing Code Changes

- [ ] Verify no regression to old pattern (`extract_one` → `map(|_| ...)`)
- [ ] Run zero-copy validation tests: `cargo test --test zero_copy_validation`
- [ ] Run lint check: `./scripts/lint-zero-copy.sh`
- [ ] Run benchmarks to verify performance: `cargo bench --bench oml_static_block`

---

## Current Arc Variants (as of 2026-02-09)

### ✅ Field Extraction Variants (Must Have extract_storage)

| Variant | Location | Status | Notes |
|---------|----------|--------|-------|
| `PreciseEvaluator::ObjArc` | `crates/wp-oml/src/core/mod.rs` | ✅ Optimized | Static symbol references |
| `GenericAccessor::FieldArc` | `crates/wp-oml/src/core/evaluator/extract/operations/other.rs` | ✅ Optimized | Default binding path |
| `NestedAccessor::FieldArc` | `crates/wp-oml/src/language/syntax/accessors/mod.rs` | ✅ Optimized | Nested access path |

### ℹ️ Match Condition Variants (No extract_storage Needed)

| Variant | Location | Purpose | Notes |
|---------|----------|---------|-------|
| `MatchCond::EqArc` | `crates/wp-oml/src/language/syntax/operations/matchs.rs` | Match equality | Only used in `is_match()`, not field extraction |
| `MatchCond::NeqArc` | `crates/wp-oml/src/language/syntax/operations/matchs.rs` | Match inequality | Only used in `is_match()`, not field extraction |
| `MatchCond::InArc` | `crates/wp-oml/src/language/syntax/operations/matchs.rs` | Match range | Only used in `is_match()`, not field extraction |

---

## Testing Strategy

### 1. Validation Tests (`zero_copy_validation.rs`)

Automated tests that verify zero-copy behavior in various scenarios:

- ✅ Static assignment
- ✅ Static in match branches
- ✅ Static in nested objects
- ✅ Multi-stage pipelines
- ✅ Comprehensive integration test

**Run**: `cargo test --package wp-oml --test zero_copy_validation`

### 2. Lint Tool (`scripts/lint-zero-copy.sh`)

Static analysis tool that checks:

- All Arc variants have proper implementations
- No regression to old patterns
- Files with Arc usage have extract_storage overrides

**Run**: `./scripts/lint-zero-copy.sh`

Expected output should show all FieldExtractor variants as ✅ Optimized.

### 3. Performance Benchmarks

Verify actual performance improvements:

```bash
cargo bench --package wp-oml --bench oml_static_block
```

Expected results:
- 4-stage pipeline: ~2,211 ns (with static vars)
- Static variables should be faster or equal to temporary fields
- No performance regression compared to baseline

---

## Common Pitfalls

### ❌ Pitfall 1: Forgetting extract_storage Override

**Prevented by compile-time enforcement since Version 2.0** - The trait no longer has a default implementation, so this cannot happen.

### ❌ Pitfall 2: Using extract_one in extract_storage (for Arc variants)

```rust
fn extract_storage(...) -> Option<FieldStorage> {
    match self {
        MyType::FieldArc(arc) => arc
            .as_ref()
            .extract_one(...)  // ❌ Causes clone
            .map(|_| FieldStorage::from_shared(arc.clone())),
    }
}
```

**Fix**: Skip `extract_one` entirely for Arc variants.

### ❌ Pitfall 3: Calling extract_one in delegation patterns

**Problem**: Operations that delegate to inner extractors may inadvertently call `extract_one` instead of `extract_storage`:

```rust
// ❌ WRONG: Breaks zero-copy chain
fn extract_storage(...) -> Option<FieldStorage> {
    match self {
        Operation::Match(items) => {
            for i in items {
                if i.is_match(x) {
                    // Calls extract_one instead of extract_storage!
                    return i.result().extract_one(...).map(FieldStorage::from_owned);
                }
            }
        }
    }
}

// ✅ CORRECT: Preserves zero-copy chain
fn extract_storage(...) -> Option<FieldStorage> {
    match self {
        Operation::Match(items) => {
            for i in items {
                if i.is_match(x) {
                    // Calls extract_storage to preserve Arc variants
                    return i.result().extract_storage(...);
                }
            }
        }
    }
}
```

**Impact**: Match operations with static field results (FieldArc/ObjArc) will clone instead of sharing.

**Examples affected**:
- `MatchOperation` - Fixed in Version 2.0
- Similar patterns in any operation that delegates extraction

### ❌ Pitfall 4: Inconsistent Behavior Across Variants

If a type has multiple Arc variants (e.g., GenericAccessor), ALL must use zero-copy:

```rust
match self {
    Type::FieldArc1(arc) => Some(FieldStorage::from_shared(arc.clone())),  // ✅
    Type::FieldArc2(arc) => arc.as_ref().extract_one(...).map(...),  // ❌ Inconsistent!
}
```

---

## Future Enhancements

### Completed Enhancements

1. **✅ Compile-time Enforcement** (2026-02-09): Removed default implementation from `extract_storage`, making it a required method. This provides compile-time guarantees that all implementors explicitly handle FieldStorage. No implementation can forget to consider zero-copy optimization.

### Potential Future Improvements

1. **Compile-time Enforcement**: Consider splitting `FieldExtractor` into separate traits for owned and shared extraction, making zero-copy path mandatory.

2. **Runtime Instrumentation**: Add counters to `FieldStorage::from_owned` and `from_shared` to detect unexpected clone patterns in production.

3. **CI Integration**: Add lint check and validation tests to CI pipeline to catch regressions early.

4. **Automated Metrics**: Track Arc::clone vs DataField::clone ratio in benchmarks.

---

## References

- **Design Document**: `docs/tasks_backup/OML_Arc优化_完整方案.md`
- **Fix Report**: `docs/PR/zero_copy_fix_report.md`
- **Validation Tests**: `crates/wp-oml/tests/zero_copy_validation.rs`
- **Lint Tool**: `scripts/lint-zero-copy.sh`

---

## Changelog

### 2026-02-09 - Version 2.0.4

- **Enhanced**: FmtOperation now uses extract_storage for collecting format arguments
  - Previously: Called item.extract_one() when collecting arguments, cloning Arc variants
  - Now: Calls extract_storage() and uses FieldStorage directly in FmtVal
  - Affects: Format operations with static field references as arguments
- **Enhanced**: SqlQuery now uses extract_storage for collecting SQL parameters
  - Previously: Called acq.extract_one() when collecting parameters, cloning Arc variants
  - Now: Calls extract_storage() and converts to owned only when needed
  - Affects: SQL queries with static field references as parameters
- **Coverage**: All FieldExtractor implementations that collect sub-expressions now use extract_storage
  - Complete zero-copy coverage across the entire extraction pipeline

### 2026-02-09 - Version 2.0.3

- **Enhanced**: Added FieldStorage support to ValueProcessor trait
  - Added `value_cacu_storage` method with default implementation
  - Allows processors to preserve FieldStorage (Shared/Owned) variants
- **Enhanced**: PiPeOperation now uses extract_storage and value_cacu_storage
  - Previously: Called from().extract_one(), cloning all input fields
  - Now: Calls extract_storage() and preserves Shared variants through pipeline
  - Affects: Pipe operations on static objects and nested field extraction
- **Enhanced**: Get operation implements zero-copy for object field extraction
  - Extracts fields from Shared objects without cloning the entire object
  - Preserves FieldStorage type when extracting nested fields
- **Performance**: Single-stage static field advantage improved from 31.1% to 32.0%
- **Performance**: Multi-stage pipelines now benefit from zero-copy (2-stage: 8.2% faster, 4-stage: 6.7% faster)

### 2026-02-09 - Version 2.0.2

- **Enhanced**: MapOperation now uses extract_storage for sub-expression extraction
  - Previously: Called sub.acquirer().extract_one(), cloning FieldArc variants
  - Now: Calls extract_storage() and uses set_name() for zero-copy when Shared
  - Affects: Objects containing static field references
- **Enhanced**: RecordOperation default value now uses extract_storage
  - Previously: Called default_acq.extract_one(), cloning FieldArc variants
  - Now: Calls extract_storage() with zero-copy path for Shared variants
  - Affects: Fields with static default values
- **Performance**: Single-stage static field advantage improved from 29.7% to 31.1%

### 2026-02-09 - Version 2.0.1

- **Fixed**: MatchOperation extract_storage to call result().extract_storage() instead of result().extract_one()
  - Previously: Match branches with static fields (FieldArc/ObjArc) were cloned
  - Now: Match branches preserve zero-copy for Arc variants
  - Affects: apache_e1_static.oml and similar match-based static field mappings
- **Documentation**: Added Pitfall 3 about delegation patterns breaking zero-copy chain

### 2026-02-09 - Version 2.0

- **BREAKING**: Removed default implementation from `extract_storage` method
- Made `extract_storage` a required trait method for compile-time safety
- Added explicit implementations to all 23 FieldExtractor types (5 already had custom implementations, 18 added standard implementations)
- No behavior change - purely refactoring for compile-time safety
- All tests pass, no performance regression
- Updated golden rule to reflect mandatory implementation requirement

### 2026-02-09 - Version 1.0

- Initial guidelines document
- Established golden rule and patterns
- Created validation tests and lint tool
- Documented all current Arc variants
- Fixed zero-copy regressions in 3 locations

---

**Maintained by**: wp-motor team
**Last Review**: 2026-02-09
**Next Review**: When adding new Arc variants or major refactoring
