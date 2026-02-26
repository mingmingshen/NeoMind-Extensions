#!/bin/bash
# NeoMind Extension Package Builder (.nep)
# Builds extension packages in the standard .nep (ZIP) format

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Default values
EXTENSION_DIR=""
OUTPUT_DIR="dist"
PLATFORM="all"
VERIFY=false
INCLUDE_NODE_MODULES=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--dir)
            EXTENSION_DIR="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -p|--platform)
            PLATFORM="$2"
            shift 2
            ;;
        -v|--verify)
            VERIFY=true
            shift
            ;;
        --include-node-modules)
            INCLUDE_NODE_MODULES=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -d, --dir DIR              Extension directory (default: auto-detect)"
            echo "  -o, --output DIR           Output directory (default: dist)"
            echo "  -p, --platform P           Platform: all, current, darwin_aarch64, etc."
            echo "  -v, --verify               Verify package after building"
            echo "  --include-node-modules     Include node_modules in package"
            echo "  -h, --help                 Show this help"
            echo ""
            echo "Examples:"
            echo "  $0 -d extensions/weather-forecast-wasm"
            echo "  $0 -d extensions/template -p current"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Detect platform
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            CURRENT_PLATFORM="darwin_aarch64"
        else
            CURRENT_PLATFORM="darwin_x86_64"
        fi
        ;;
    Linux)
        if [ "$ARCH" = "aarch64" ]; then
            CURRENT_PLATFORM="linux_arm64"
        else
            CURRENT_PLATFORM="linux_amd64"
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        CURRENT_PLATFORM="windows_amd64"
        ;;
esac

echo -e "${BLUE}======================================"
echo "NeoMind Extension Package Builder"
echo -e "======================================${NC}"
echo ""

# Auto-detect extension directory if not specified
if [ -z "$EXTENSION_DIR" ]; then
    # Check if we're in an extension directory
    if [ -f "manifest.json" ] || [ -f "Cargo.toml" ]; then
        EXTENSION_DIR="."
    elif [ -d "extensions" ]; then
        # Find first extension with manifest.json
        for dir in extensions/*/; do
            if [ -f "$dir/manifest.json" ]; then
                EXTENSION_DIR="$dir"
                break
            fi
        done
    fi

    if [ -z "$EXTENSION_DIR" ]; then
        echo -e "${RED}Error: Cannot find extension directory${NC}"
        echo "Please specify with -d/--dir"
        exit 1
    fi
fi

# Normalize path
EXTENSION_DIR=$(cd "$EXTENSION_DIR" && pwd)

# Get extension directory name
EXT_NAME=$(basename "$EXTENSION_DIR")

echo -e "${BLUE}Extension: ${EXT_NAME}${NC}"
echo -e "${BLUE}Directory: ${EXTENSION_DIR}${NC}"
echo ""

# Find manifest file
MANIFEST_FILE=""
if [ -f "$EXTENSION_DIR/manifest.json" ]; then
    MANIFEST_FILE="$EXTENSION_DIR/manifest.json"
elif [ -f "$EXTENSION_DIR/metadata.json" ]; then
    MANIFEST_FILE="$EXTENSION_DIR/metadata.json"
else
    echo -e "${RED}Error: manifest.json not found in ${EXTENSION_DIR}${NC}"
    exit 1
fi

echo -e "${BLUE}Found manifest: ${MANIFEST_FILE}${NC}"

# Read extension metadata
if command -v jq >/dev/null 2>&1; then
    EXT_ID=$(jq -r '.id // .name // "unknown"' "$MANIFEST_FILE")
    EXT_VERSION=$(jq -r '.version // "0.1.0"' "$MANIFEST_FILE")
    EXT_TYPE=$(jq -r '.type // .runtime // "native"' "$MANIFEST_FILE")
    EXT_RUNTIME=$(jq -r '.runtime // .type // "native"' "$MANIFEST_FILE")
else
    # Fallback to grep
    EXT_ID=$(grep -o '"id"[[:space:]]*:[[:space:]]*"[^"]*"' "$MANIFEST_FILE" | head -1 | cut -d'"' -f4)
    EXT_VERSION=$(grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$MANIFEST_FILE" | head -1 | cut -d'"' -f4)
    EXT_TYPE=$(grep -o '"type"[[:space:]]*:[[:space:]]*"[^"]*"' "$MANIFEST_FILE" | head -1 | cut -d'"' -f4)
    EXT_RUNTIME=$(grep -o '"runtime"[[:space:]]*:[[:space:]]*"[^"]*"' "$MANIFEST_FILE" | head -1 | cut -d'"' -f4)
fi

# Use runtime as type if type is not "wasm"
if [ "$EXT_TYPE" != "wasm" ] && [ -n "$EXT_RUNTIME" ]; then
    EXT_TYPE="$EXT_RUNTIME"
fi

echo -e "${BLUE}ID: ${EXT_ID}${NC}"
echo -e "${BLUE}Version: ${EXT_VERSION}${NC}"
echo -e "${BLUE}Type: ${EXT_TYPE}${NC}"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR_ABS=$(cd "$OUTPUT_DIR" && pwd)

# Create temporary build directory
BUILD_DIR=$(mktemp -d)
trap "rm -rf $BUILD_DIR" EXIT

echo -e "${BLUE}Step 1: Building extension...${NC}"

# Check if this is a Rust extension
if [ -f "$EXTENSION_DIR/Cargo.toml" ]; then
    echo "Building Rust extension..."

    # Get library name from Cargo.toml
    # First check for [lib] section, fall back to [package] section
    LIB_NAME=$(grep -A2 '^\[lib\]' "$EXTENSION_DIR/Cargo.toml" | grep -o '^name = ".*"' | cut -d'"' -f2)
    if [ -z "$LIB_NAME" ]; then
        LIB_NAME=$(grep -o '^name = ".*"' "$EXTENSION_DIR/Cargo.toml" | head -1 | cut -d'"' -f2)
    fi

    # Convert hyphens to underscores (Rust naming convention for library files)
    LIB_NAME=$(echo "$LIB_NAME" | tr '-' '_')

    if [ "$EXT_TYPE" = "wasm" ]; then
        # Build WASM
        echo "Building WASM target..."

        # Ensure wasm32 target is installed
        echo "Checking for wasm32 target..."
        rustup target add wasm32-unknown-unknown 2>/dev/null || echo "wasm32 target already installed or failed to add"

        # Try to build, but fall back to pre-built file if it fails
        if cargo build --release --target wasm32-unknown-unknown --manifest-path="$EXTENSION_DIR/Cargo.toml" 2>/dev/null; then
            echo "WASM build succeeded"
        else
            echo -e "${YELLOW}WASM build failed, trying pre-built file...${NC}"
        fi

        # Find built WASM file
        WASM_FILE="$EXTENSION_DIR/target/wasm32-unknown-unknown/release/${LIB_NAME}.wasm"
        if [ ! -f "$WASM_FILE" ]; then
            WASM_FILE="${EXTENSION_DIR}/${LIB_NAME}.wasm"
        fi

        if [ ! -f "$WASM_FILE" ]; then
            # Search for any .wasm file
            WASM_FILE=$(find "$EXTENSION_DIR/target" -name "*.wasm" 2>/dev/null | head -1)
        fi

        # If still not found, check for pre-built files
        if [ -z "$WASM_FILE" ] || [ ! -f "$WASM_FILE" ]; then
            if [ -f "$EXTENSION_DIR/weather_forecast_wasm.wasm" ]; then
                WASM_FILE="$EXTENSION_DIR/weather_forecast_wasm.wasm"
            elif [ -f "$EXTENSION_DIR/extension.wasm" ]; then
                WASM_FILE="$EXTENSION_DIR/extension.wasm"
            fi
        fi

        if [ -z "$WASM_FILE" ] || [ ! -f "$WASM_FILE" ]; then
            echo -e "${RED}Error: WASM file not found${NC}"
            exit 1
        fi

        # Copy WASM to build directory
        mkdir -p "$BUILD_DIR/binaries/wasm"
        cp "$WASM_FILE" "$BUILD_DIR/binaries/wasm/extension.wasm"

        # Also check for extension.json (for WASM extensions)
        if [ -f "$EXTENSION_DIR/extension.json" ]; then
            cp "$EXTENSION_DIR/extension.json" "$BUILD_DIR/binaries/wasm/"
        fi

        echo -e "${GREEN}✓ WASM packaged: ${WASM_FILE}${NC}"
    else
        # Build native extension
        cargo build --release --manifest-path="$EXTENSION_DIR/Cargo.toml"

        # Determine library extension
        case "$OS" in
            Darwin)
                EXT="dylib"
                ;;
            Linux)
                EXT="so"
                ;;
            MINGW*|MSYS*|CYGWIN*)
                EXT="dll"
                ;;
        esac

        # Find built library
        # Try extension's target directory first
        LIB_FILE="$EXTENSION_DIR/target/release/lib${LIB_NAME}.${EXT}"
        if [ ! -f "$LIB_FILE" ]; then
            LIB_FILE="${EXTENSION_DIR}/target/release/${LIB_NAME}.${EXT}"
        fi

        # Try workspace target directory (for workspace builds)
        if [ ! -f "$LIB_FILE" ]; then
            WORKSPACE_ROOT=$(cd "$EXTENSION_DIR/.." && pwd)
            LIB_FILE="$WORKSPACE_ROOT/target/release/lib${LIB_NAME}.${EXT}"
        fi
        if [ ! -f "$LIB_FILE" ]; then
            WORKSPACE_ROOT=$(cd "$EXTENSION_DIR/../.." && pwd)
            LIB_FILE="$WORKSPACE_ROOT/target/release/lib${LIB_NAME}.${EXT}"
        fi

        # Search for any library in common locations
        if [ ! -f "$LIB_FILE" ]; then
            LIB_FILE=$(find "$EXTENSION_DIR/target/release" -name "*${LIB_NAME}*.${EXT}" 2>/dev/null | head -1)
        fi
        if [ -z "$LIB_FILE" ] || [ ! -f "$LIB_FILE" ]; then
            WORKSPACE_ROOT=$(cd "$EXTENSION_DIR/../.." && pwd)
            LIB_FILE=$(find "$WORKSPACE_ROOT/target/release" -name "*${LIB_NAME}*.${EXT}" 2>/dev/null | head -1)
        fi

        if [ -z "$LIB_FILE" ] || [ ! -f "$LIB_FILE" ]; then
            echo -e "${RED}Error: Library file not found${NC}"
            exit 1
        fi

        # Copy to build directory
        mkdir -p "$BUILD_DIR/binaries/$CURRENT_PLATFORM"
        cp "$LIB_FILE" "$BUILD_DIR/binaries/$CURRENT_PLATFORM/extension.${EXT}"

        echo -e "${GREEN}✓ Native library built: ${LIB_FILE}${NC}"
    fi
fi

# Check if this is a WASM extension with pre-built file
if [ -f "$EXTENSION_DIR/weather_forecast_wasm.wasm" ] || [ -f "$EXTENSION_DIR/extension.wasm" ]; then
    WASM_FILE="$EXTENSION_DIR/weather_forecast_wasm.wasm"
    if [ ! -f "$WASM_FILE" ]; then
        WASM_FILE="$EXTENSION_DIR/extension.wasm"
    fi

    mkdir -p "$BUILD_DIR/binaries/wasm"
    cp "$WASM_FILE" "$BUILD_DIR/binaries/wasm/extension.wasm"
    echo -e "${GREEN}✓ Using pre-built WASM: ${WASM_FILE}${NC}"
fi

echo ""
echo -e "${BLUE}Step 2: Creating package manifest...${NC}"

# Create standardized manifest for .nep package
NEP_MANIFEST="$BUILD_DIR/manifest.json"

# Copy original manifest and update with package format info
if command -v jq >/dev/null 2>&1; then
    # Ensure manifest has required package fields
    # Add binaries entry based on extension type
    # Convert old-style string metrics to MetricDescriptor format
    # Convert old-style string commands to CommandDescriptor format
    if [ "$EXT_TYPE" = "wasm" ]; then
        cat "$MANIFEST_FILE" | jq "
            .format = \"neomind-extension-package\" |
            .format_version = \"1.0\" |
            if .binaries then . else .binaries = {} end |
            .binaries.wasm = \"binaries/wasm/extension.wasm\" |
            if .capabilities.metrics then
                if (.capabilities.metrics | type) == \"array\" and (.capabilities.metrics | length) > 0 and (.capabilities.metrics[0] | type) == \"string\" then
                    .metrics = [.capabilities.metrics[] | {name: ., display_name: (if . == \"images_processed\" then \"Images Processed\"
                                        elif . == \"avg_processing_time_ms\" then \"Avg Processing Time (ms)\"
                                        elif . == \"total_detections\" then \"Total Detections\"
                                        else (.[0:1] | ascii_upcase) + .[1:] end),
                                        data_type: \"number\"}] |
                    del(.capabilities.metrics)
                else
                    .
                end
            else . end |
            if .capabilities.commands then
                if (.capabilities.commands | type) == \"array\" and (.capabilities.commands | length) > 0 and (.capabilities.commands[0] | type) == \"string\" then
                    .commands = [.capabilities.commands[] | {name: ., display_name: (if . == \"reset_stats\" then \"Reset Stats\"
                                        elif . == \"start_stream\" then \"Start Stream\"
                                        elif . == \"stop_stream\" then \"Stop Stream\"
                                        elif . == \"detect\" then \"Detect Objects\"
                                        else (.[0:1] | ascii_upcase) + .[1:] end)}] |
                    del(.capabilities.commands)
                else
                    .
                end
            else . end
        " > "$NEP_MANIFEST"
    else
        cat "$MANIFEST_FILE" | jq "
            .format = \"neomind-extension-package\" |
            .format_version = \"1.0\" |
            if .binaries then . else .binaries = {} end |
            .binaries.$CURRENT_PLATFORM = \"binaries/$CURRENT_PLATFORM/extension.${EXT}\" |
            if .capabilities.metrics then
                if (.capabilities.metrics | type) == \"array\" and (.capabilities.metrics | length) > 0 and (.capabilities.metrics[0] | type) == \"string\" then
                    .metrics = [.capabilities.metrics[] | {name: ., display_name: (if . == \"images_processed\" then \"Images Processed\"
                                        elif . == \"avg_processing_time_ms\" then \"Avg Processing Time (ms)\"
                                        elif . == \"total_detections\" then \"Total Detections\"
                                        elif . == \"frames_processed\" then \"Frames Processed\"
                                        elif . == \"detection_count\" then \"Detection Count\"
                                        elif . == \"avg_fps\" then \"Average FPS\"
                                        else (.[0:1] | ascii_upcase) + .[1:] end),
                                        data_type: \"number\"}] |
                    del(.capabilities.metrics)
                else
                    .
                end
            else . end |
            if .capabilities.commands then
                if (.capabilities.commands | type) == \"array\" and (.capabilities.commands | length) > 0 and (.capabilities.commands[0] | type) == \"string\" then
                    .commands = [.capabilities.commands[] | {name: ., display_name: (if . == \"reset_stats\" then \"Reset Stats\"
                                        elif . == \"start_stream\" then \"Start Stream\"
                                        elif . == \"stop_stream\" then \"Stop Stream\"
                                        elif . == \"detect\" then \"Detect Objects\"
                                        else (.[0:1] | ascii_upcase) + .[1:] end)}] |
                    del(.capabilities.commands)
                else
                    .
                end
            else . end
        " > "$NEP_MANIFEST"
    fi
else
    # Fallback: copy original manifest
    cp "$MANIFEST_FILE" "$NEP_MANIFEST"
fi

echo -e "${GREEN}✓ Package manifest created${NC}"

echo ""
echo -e "${BLUE}Step 3: Adding frontend components...${NC}"

# Copy frontend directory if exists
if [ -d "$EXTENSION_DIR/frontend" ]; then
    # Only copy essential frontend files
    FRONTEND_BUILD="$BUILD_DIR/frontend"
    mkdir -p "$FRONTEND_BUILD"

    # Copy dist folder if exists (contains built JS)
    if [ -d "$EXTENSION_DIR/frontend/dist" ]; then
        cp -r "$EXTENSION_DIR/frontend/dist" "$FRONTEND_BUILD/"
        COMPONENT_COUNT=$(find "$FRONTEND_BUILD/dist" -name "*.js" 2>/dev/null | wc -l)
        echo -e "${GREEN}✓ Frontend dist added (${COMPONENT_COUNT} JS files)${NC}"
    fi

    # Copy node_modules only if explicitly requested
    if [ "$INCLUDE_NODE_MODULES" = true ] && [ -d "$EXTENSION_DIR/frontend/node_modules" ]; then
        cp -r "$EXTENSION_DIR/frontend/node_modules" "$FRONTEND_BUILD/"
        echo -e "${YELLOW}⚠ node_modules included (large package size)${NC}"
    else
        echo -e "${GREEN}✓ Frontend components added (node_modules excluded)${NC}"
    fi
else
    echo -e "${YELLOW}No frontend directory found${NC}"
fi

echo ""
echo -e "${BLUE}Step 4: Creating .nep package...${NC}"

# Create package filename
PACKAGE_NAME="${EXT_ID}-${EXT_VERSION}.nep"
PACKAGE_PATH="${OUTPUT_DIR_ABS}/${PACKAGE_NAME}"

# Create ZIP package
(cd "$BUILD_DIR" && zip -qr "${PACKAGE_PATH}" . -x "*/node_modules/*" "*/.git/*" "*/target/*" "*/.DS_Store")

# Calculate checksum
if command -v shasum >/dev/null 2>&1; then
    CHECKSUM=$(shasum -a 256 "$PACKAGE_PATH" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
    CHECKSUM=$(sha256sum "$PACKAGE_PATH" | awk '{print $1}')
else
    CHECKSUM="unknown"
fi

# Get file size
SIZE=$(stat -f%z "$PACKAGE_PATH" 2>/dev/null || stat -c%s "$PACKAGE_PATH" 2>/dev/null)

echo -e "${GREEN}✓ Package created: ${PACKAGE_PATH}${NC}"
echo -e "${BLUE}Size: ${SIZE} bytes$(numfmt --to=iec-i --suffix=B "$SIZE" 2>/dev/null || echo " (~$(numfmt --to=iec-i --suffix=B "$SIZE" 2>/dev/null)")${NC}"
echo -e "${BLUE}SHA256: ${CHECKSUM}${NC}"

# Save checksum info
echo "${PACKAGE_NAME}|${CHECKSUM}|${SIZE}" >> "${OUTPUT_DIR}/checksums.txt"

# Verify package if requested
if [ "$VERIFY" = true ]; then
    echo ""
    echo -e "${BLUE}Step 5: Verifying package...${NC}"

    # Use Python to verify ZIP structure
    if command -v python3 >/dev/null 2>&1; then
        python3 - <<EOF
import zipfile
import json

with zipfile.ZipFile('$PACKAGE_PATH', 'r') as zf:
    # List contents
    print("Package contents:")
    for name in zf.namelist():
        size = zf.getinfo(name).file_size
        print(f"  {name} ({size} bytes)")

    # Verify manifest
    print("\nVerifying manifest...")
    try:
        with zf.open('manifest.json') as mf:
            manifest = json.load(mf)
            print(f"  ✓ Format: {manifest.get('format', 'legacy')}")
            print(f"  ✓ Extension: {manifest.get('name', 'Unknown')} ({manifest.get('version', '0.0.0')})")

            # Check binaries
            binaries = manifest.get('binaries', {})
            if binaries:
                print(f"  ✓ Binaries: {list(binaries.keys())}")
    except Exception as e:
        print(f"  ✗ Error reading manifest: {e}")

print("\n✓ Package verification complete")
EOF
    else
        echo -e "${YELLOW}Python3 not found, skipping verification${NC}"
    fi
fi

echo ""
echo -e "${GREEN}======================================"
echo "Package build complete!"
echo -e "======================================${NC}"
echo -e "${GREEN}Package: ${PACKAGE_PATH}${NC}"
echo -e "${GREEN}SHA256: ${CHECKSUM}${NC}"
echo ""
echo "To install via API:"
echo "  curl -X POST http://localhost:9375/api/extensions/upload/file \\"
echo "    -H 'Content-Type: application/octet-stream' \\"
echo "    --data-binary @${PACKAGE_PATH}"
echo ""
echo "Or use the NeoMind web UI to upload the package."
echo ""
