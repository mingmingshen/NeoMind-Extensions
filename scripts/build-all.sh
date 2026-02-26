#!/bin/bash
# Build all .nep packages for all extensions

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}======================================"
echo "NeoMind Extensions - Build All"
echo -e "======================================${NC}"
echo ""

# Create dist directory
mkdir -p dist

# Clean old packages
echo -e "${YELLOW}Cleaning old packages...${NC}"
rm -f dist/*.nep
rm -f dist/checksums.txt

# Track results
SUCCESS_COUNT=0
FAIL_COUNT=0
PACKAGES=""

# Build each extension
for ext_dir in extensions/*/; do
    if [ ! -d "$ext_dir" ]; then
        continue
    fi

    ext_name=$(basename "$ext_dir")

    # Skip if no manifest.json
    if [ ! -f "$ext_dir/manifest.json" ]; then
        echo -e "${YELLOW}Skipping $ext_name (no manifest.json)${NC}"
        continue
    fi

    echo ""
    echo -e "${BLUE}Building: $ext_name${NC}"
    echo "----------------------------------------"

    if bash scripts/package.sh -d "$ext_dir" -o dist 2>&1 | tee /tmp/build_${ext_name}.log; then
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        # Get the package name
        PACKAGE=$(grep "Package:" /tmp/build_${ext_name}.log | tail -1 | awk '{print $2}')
        if [ -n "$PACKAGE" ]; then
            PACKAGES="$PACKAGES$PACKAGE\n"
        fi
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo -e "${RED}✗ Failed to build $ext_name${NC}"
    fi
done

# Summary
echo ""
echo -e "${BLUE}======================================"
echo "Build Summary"
echo -e "======================================${NC}"
echo -e "${GREEN}✓ Successfully built: $SUCCESS_COUNT${NC}"
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}✗ Failed: $FAIL_COUNT${NC}"
fi
echo ""

# List built packages
if ls dist/*.nep 2>/dev/null; then
    echo -e "${GREEN}Built packages:${NC}"
    ls -lh dist/*.nep | awk '{printf "  %-50s %s\n", $9, $5}'
    echo ""

    echo -e "${GREEN}Checksums:${NC}"
    if [ -f dist/checksums.txt ]; then
        cat dist/checksums.txt
    fi
else
    echo -e "${YELLOW}No packages built${NC}"
fi

echo ""
echo -e "${BLUE}Total size:${NC}"
du -sh dist 2>/dev/null || echo "N/A"

echo ""
echo "To install a package:"
echo "  1. Via Web UI: Extensions → Add Extension → File Mode → Upload"
echo "  2. Via API: curl -X POST http://localhost:9375/api/extensions/upload/file \\"
echo "             -H 'Content-Type: application/octet-stream' \\"
echo "             --data-binary @dist/package-name.nep"
echo ""
