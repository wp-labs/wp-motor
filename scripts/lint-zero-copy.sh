#!/bin/bash
# Zero-Copy Lint Tool
# Checks that all Arc<DataField> variants have proper extract_storage implementations

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OML_SRC="$SCRIPT_DIR/../crates/wp-oml/src"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🔍 Zero-Copy Lint Check"
echo "======================="
echo ""

ERRORS=0
WARNINGS=0

# Check 1: Find all Arc<DataField> enum variants
echo "📋 Checking Arc<DataField> variants..."
ARC_VARIANTS=$(rg "^\s*([\w]+)\(Arc<DataField>\)" "$OML_SRC" --only-matching --replace '$1' --no-filename | sort -u)

if [ -z "$ARC_VARIANTS" ]; then
    echo "  ℹ️  No Arc<DataField> variants found"
else
    echo "  Found Arc variants:"
    echo "$ARC_VARIANTS" | while read variant; do
        echo "    - $variant"
    done
    echo ""
fi

# Check 2: Verify each Arc variant has extract_storage handling
echo "🔎 Verifying extract_storage implementations..."
echo ""

while IFS= read -r variant; do
    if [ -n "$variant" ]; then
        # Search for extract_storage implementations mentioning this variant
        IMPL_COUNT=$(rg "$variant\(arc\)|$variant\(.*Arc" "$OML_SRC" --type rust -A 5 | grep -c "extract_storage\|FieldStorage::from_shared" || true)

        if [ "$IMPL_COUNT" -eq 0 ]; then
            echo -e "  ${RED}✗${NC} $variant: Missing extract_storage optimization"
            ERRORS=$((ERRORS + 1))
        else
            echo -e "  ${GREEN}✓${NC} $variant: Has extract_storage handling"
        fi
    fi
done <<< "$ARC_VARIANTS"

echo ""

# Check 3: Find potential problematic patterns
echo "⚠️  Checking for problematic patterns..."
echo ""

# Pattern 1: extract_one followed by map(|_| ...)
PATTERN1=$(rg "\.extract_one\(.*\).*\.map\(\|_\|" "$OML_SRC" --type rust -l || true)
if [ -n "$PATTERN1" ]; then
    echo -e "  ${YELLOW}⚠${NC} Found 'extract_one().map(|_| ...)' pattern (may discard cloned data):"
    echo "$PATTERN1" | while read file; do
        echo "    - $file"
        WARNINGS=$((WARNINGS + 1))
    done
    echo ""
fi

# Pattern 2: Arc with extract_one but not in extract_storage context
PATTERN2=$(rg "Arc.*extract_one" "$OML_SRC" --type rust -l | \
    while read file; do
        # Check if the file has proper extract_storage override
        if ! rg -q "fn extract_storage.*FieldStorage::from_shared" "$file"; then
            echo "$file"
        fi
    done)

if [ -n "$PATTERN2" ]; then
    echo -e "  ${YELLOW}⚠${NC} Files with Arc + extract_one but missing extract_storage override:"
    echo "$PATTERN2" | while read file; do
        echo "    - $file"
        WARNINGS=$((WARNINGS + 1))
    done
    echo ""
fi

# Check 4: Verify FieldArc/ObjArc patterns
echo "🎯 Checking specific Arc variant patterns..."
echo ""

for variant in "FieldArc" "ObjArc"; do
    # Find definitions
    DEFS=$(rg "$variant\(Arc<DataField>\)" "$OML_SRC" --type rust -l || true)

    if [ -n "$DEFS" ]; then
        echo "  Checking $variant:"
        echo "$DEFS" | while read def_file; do
            # Find corresponding FieldExtractor impl
            IMPL_FILE=$(dirname "$def_file")

            # Check for extract_storage with from_shared
            if rg -q "$variant.*=>\s*Some\(FieldStorage::from_shared\(arc\.clone\(\)\)\)" "$OML_SRC" --type rust; then
                echo -e "    ${GREEN}✓${NC} $variant: Zero-copy path verified"
            else
                # Check if it's in extract_one (acceptable)
                if rg -q "$variant.*=>.*\.as_ref\(\)\.extract_one" "$OML_SRC" --type rust; then
                    echo -e "    ${GREEN}✓${NC} $variant: Extract pattern found (verify extract_storage exists)"
                else
                    echo -e "    ${RED}✗${NC} $variant: No zero-copy pattern found"
                    ERRORS=$((ERRORS + 1))
                fi
            fi
        done
        echo ""
    fi
done

# Check 5: Verify no regression to old pattern
echo "🚫 Checking for zero-copy regression patterns..."
echo ""

OLD_PATTERN=$(rg "\.extract_one\(target, src, dst\).*\.map\(\|_\|.*FieldStorage::from_shared" "$OML_SRC" --type rust -l || true)
if [ -n "$OLD_PATTERN" ]; then
    echo -e "  ${RED}✗${NC} Found OLD PATTERN (extract_one + discard + Arc::clone):"
    echo "$OLD_PATTERN" | while read file; do
        echo "    - $file"
        ERRORS=$((ERRORS + 1))
    done
    echo ""
else
    echo -e "  ${GREEN}✓${NC} No zero-copy regressions found"
    echo ""
fi

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📊 Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━"

if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠ $WARNINGS warning(s) found${NC}"
    echo ""
    echo "Please review the warnings above."
    exit 0
else
    echo -e "${RED}✗ $ERRORS error(s) and $WARNINGS warning(s) found${NC}"
    echo ""
    echo "Please fix the errors above to ensure zero-copy optimization is correctly implemented."
    exit 1
fi
