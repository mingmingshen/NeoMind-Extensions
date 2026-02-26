# NeoMind Extensions

Official extension repository for the NeoMind Edge AI Platform.

[中文文档](README.zh.md)

## Project Description

This repository contains officially maintained extensions for the NeoMind Edge AI Platform.

### Extension Types

NeoMind supports **three extension distribution formats**:

| Format | File Extension | Description | Use Case |
|--------|---------------|-------------|----------|
| **.nep Package** | `.nep` | ZIP archive with binaries + frontend + metadata | **Recommended**: Complete extension distribution |
| **Native Binary** | `.dylib` / `.so` / `.dll` | Platform-specific dynamic libraries | Development builds, custom extensions |
| **WASM Module** | `.wasm` + `.json` | WebAssembly modules | Cross-platform without packaging |

### Why .nep Packages?

**.nep (NeoMind Extension Package)** is the recommended distribution format:
- ✅ **Single file** - Contains binaries, frontend components, and metadata
- ✅ **Multi-platform** - Includes binaries for all platforms in one package
- ✅ **Frontend support** - Can bundle React components for the dashboard
- ✅ **Checksum verification** - SHA256 validation ensures integrity
- ✅ **Easy installation** - Drag & drop in NeoMind web UI

---

## Quick Start: Installing Extensions

### Method 1: Drag & Drop (Recommended)

1. Download a `.nep` package from [Releases](https://github.com/camthink-ai/NeoMind-Extensions/releases)
2. Open NeoMind Web UI → Extensions
3. Click **Add Extension** → Switch to **File Mode**
4. Drag & drop the `.nep` file
5. Click **Upload & Install**

### Method 2: NeoMind Marketplace

1. Open NeoMind Web UI → Extensions → Marketplace
2. Browse available extensions
3. Click **Install** on any extension
4. The extension will be automatically downloaded and installed

### Method 3: API Installation

```bash
curl -X POST http://your-neomind:9375/api/extensions/upload/file \
  -H "Content-Type: application/octet-stream" \
  --data-binary @extension-name.nep
```

---

## For Extension Developers

### Building .nep Packages

Use the provided `package.sh` script to build .nep packages:

```bash
# Build a single extension
bash scripts/package.sh -d extensions/weather-forecast-wasm

# Build with verification
bash scripts/package.sh -d extensions/weather-forecast-wasm -v

# Build for current platform only
bash scripts/package.sh -d extensions/template -p current
```

The script will:
1. Build the extension (Rust/Cargo or use pre-built WASM)
2. Create proper directory structure
3. Package binaries, frontend components, and metadata
4. Calculate SHA256 checksum
5. Output: `dist/{extension-id}-{version}.nep`

### Extension Directory Structure

```
extensions/
└── your-extension/
    ├── manifest.json       # Extension metadata (required)
    ├── Cargo.toml          # Rust project (if native/WASM)
    ├── src/                # Source code
    │   └── lib.rs
    ├── frontend/           # React components (optional)
    │   ├── src/
    │   ├── package.json
    │   └── dist/           # Built JS files
    └── README.md           # Documentation
```

### manifest.json Format

```json
{
  "format": "neomind-extension-package",
  "format_version": "1.0",
  "id": "neomind.example.extension",
  "name": "Example Extension",
  "version": "1.0.0",
  "description": "An example extension",
  "author": "Your Name",
  "license": "MIT",
  "type": "wasm",
  "binaries": {
    "wasm": "binaries/wasm/extension.wasm"
  },
  "permissions": [],
  "config_parameters": [],
  "metrics": [],
  "commands": [],
  "dashboard_components": []
}
```

See [EXTENSION_GUIDE.md](EXTENSION_GUIDE.md) for detailed documentation.

---

## Available Extensions

### Weather Forecast (WASM)
- **ID**: `neomind.weather.forecast.wasm`
- **Description**: Real-time weather data using Open-Meteo API
- **Package**: [Download](https://github.com/camthink-ai/NeoMind-Extensions/releases/latest)
- **Components**: Weather card dashboard widget

### More extensions coming soon...

---

## CI/CD

This repository uses GitHub Actions for automatic building:

- **On push** to `main`: Builds changed extensions
- **Manual trigger**: Build specific extensions or all
- **Release**: Creates GitHub releases with .nep packages

See [`.github/workflows/build-nep-packages.yml`](.github/workflows/build-nep-packages.yml)

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This repository is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.


### Manual Installation

#### Pre-built Binaries

Download pre-built binaries from [Releases](https://github.com/camthink-ai/NeoMind-Extensions/releases):

**Native Extensions (.dylib / .so / .dll)**:
```bash
# After downloading, copy to extensions directory
mkdir -p ~/.neomind/extensions
cp ~/Downloads/libneomind_extension_weather_forecast.dylib ~/.neomind/extensions/

# Restart NeoMind
```

**WASM Extensions (.wasm)**:
```bash
# Download both files:
# - my-extension.wasm (the WebAssembly module)
# - my-extension.json (metadata file)

mkdir -p ~/.neomind/extensions
cp ~/Downloads/my-extension.wasm ~/.neomind/extensions/
cp ~/Downloads/my-extension.json ~/.neomind/extensions/

# Restart NeoMind
```

#### Build from Source

**Native Extensions**:
```bash
# Clone the repository
git clone https://github.com/camthink-ai/NeoMind-Extensions.git
cd NeoMind-Extensions

# Build all extensions
cargo build --release

# Copy to extensions directory
mkdir -p ~/.neomind/extensions
cp target/release/libneomind_extension_*.dylib ~/.neomind/extensions/
```

**WASM Extensions**:
```bash
# Clone the repository
git clone https://github.com/camthink-ai/NeoMind-Extensions.git
cd NeoMind-Extensions/extensions/as-hello

# Install dependencies and build
npm install
npm run build

# Copy both files to extensions directory
mkdir -p ~/.neomind/extensions
cp build/as-hello.wasm ~/.neomind/extensions/
cp metadata.json ~/.neomind/extensions/as-hello.json
```

---

## Available Extensions

### [image-analyzer](extensions/image-analyzer/) - Stateless Streaming

Demonstrates **Stateless** streaming mode for single-chunk image processing.

| Capability | Type | Description |
|-----------|------|-------------|
| Image Analysis | Stream | Analyze JPEG/PNG/WebP images |
| Object Detection | Metric | Detected objects with bounding boxes |
| `reset_stats` | Command | Reset processing statistics |

**Streaming Mode**: Stateless (independent processing of each chunk)
**Direction**: Upload (client → extension)
**Max Chunk Size**: 10MB

**Installation**:
```bash
cargo build --release -p neomind-image-analyzer
cp target/release/libneomind_extension_image_analyzer.dylib ~/.neomind/extensions/
```

### [yolo-video](extensions/yolo-video/) - Stateful Streaming

Demonstrates **Stateful** streaming mode for session-based video processing.

| Capability | Type | Description |
|-----------|------|-------------|
| Video Processing | Stream | Process H264/H265 video frames |
| Object Detection | Stream | YOLO-based real-time detection |
| `get_session_info` | Command | Get active session statistics |

**Streaming Mode**: Stateful (maintains session context)
**Direction**: Upload (client → extension)
**Max Chunk Size**: 5MB per frame
**Max Concurrent Sessions**: 5

**Installation**:
```bash
cargo build --release -p neomind-yolo-video
cp target/release/libneomind_extension_yolo_video.dylib ~/.neomind/extensions/
```

### [as-hello](extensions/as-hello/) - WASM Example (AssemblyScript/TypeScript)

A WASM extension written in AssemblyScript (TypeScript-like language) with fast compile times and small binary size.

| Capability | Type | Description |
|-----------|------|-------------|
| `get_counter` | Command | Get the current counter value |
| `increment_counter` | Command | Increment the counter |
| `reset_counter` | Command | Reset counter to default value |
| `get_temperature` | Command | Get temperature reading (simulated) |
| `set_temperature` | Command | Set temperature value (for testing) |
| `get_humidity` | Command | Get humidity reading (simulated) |
| `hello` | Command | Say hello from AssemblyScript |
| `get_all_metrics` | Command | Get all metrics with variation |

**Metrics**: counter, temperature, humidity

**Installation**:
```bash
# Build AssemblyScript WASM extension
cd extensions/as-hello
npm install
npm run build

# Install (both files required)
cp build/as-hello.wasm ~/.neomind/extensions/
cp metadata.json ~/.neomind/extensions/as-hello.json
```

**Why AssemblyScript?**
- TypeScript-like syntax (easy for JS/TS developers)
- Very fast compile time (~1s vs ~5s for Rust WASM)
- Small binary size (~15 KB vs ~50 KB for Rust WASM)
- Single `.wasm` file for all platforms

### [template](extensions/template/) - Native Template
Template for creating native extensions.

### [weather-forecast](extensions/weather-forecast/)
Weather data and forecasts for global cities.

| Capability | Type | Description |
|-----------|------|-------------|
| `get_weather` | Command | Get current weather for any city |
| **Dashboard Component** | UI | Beautiful weather card with dynamic gradients |

**Metrics**: temperature, humidity, wind_speed

**Features**:
- Real-time weather using Open-Meteo API (free, no API key required)
- Beautiful dashboard component with gradient backgrounds that change based on weather
- City search with auto-refresh capability
- Temperature unit selection (Celsius/Fahrenheit)

**Installation**:
```bash
# Build and install
cargo build --release -p neomind-weather-forecast
cp target/release/libneomind_extension_weather_forecast.dylib ~/.neomind/extensions/
cp -r extensions/weather-forecast/frontend/dist ~/.neomind/extensions/weather-forecast/frontend/

# Or via marketplace (in NeoMind UI)
```

---

## Repository Structure

```
NeoMind-Extensions/
├── extensions/
│   ├── index.json              # Main marketplace index
│   │   # Lists all available extensions with metadata URLs
│   ├── image-analyzer/         # Stateless streaming extension
│   │   ├── Cargo.toml          # Package configuration
│   │   └── src/lib.rs          # Source code
│   ├── yolo-video/             # Stateful streaming extension
│   │   ├── Cargo.toml          # Package configuration
│   │   └── src/lib.rs          # Source code
│   ├── as-hello/               # WASM extension example (AssemblyScript) ⭐ Recommended
│   │   ├── package.json        # npm dependencies
│   │   ├── asconfig.json       # AssemblyScript compiler config
│   │   ├── metadata.json       # Extension metadata (for marketplace)
│   │   ├── README.md           # Extension documentation
│   │   └── assembly/extension.ts  # Source code
│   ├── weather-forecast/       # Native extension
│   │   ├── metadata.json       # Extension metadata (for marketplace)
│   │   ├── Cargo.toml          # Package configuration
│   │   ├── README.md           # Extension documentation
│   │   └── src/lib.rs          # Source code
│   └── template/               # Template for native extensions
│       ├── Cargo.toml
│       ├── README.md
│       └── src/lib.rs
├── EXTENSION_GUIDE.md          # Developer guide
├── USER_GUIDE.md               # User guide
├── Cargo.toml                  # Workspace configuration
├── build.sh                    # Build script
└── README.md                   # This file
```

---

## Marketplace Data Format

### extensions/index.json

Main index that lists all available extensions:

```json
{
  "version": "1.0",
  "last_updated": "2025-02-10T12:00:00Z",
  "extensions": [
    {
      "id": "weather-forecast",
      "name": "Weather Forecast",
      "description": "Global weather data and forecasts",
      "version": "0.1.0",
      "author": "CamThink",
      "license": "MIT",
      "categories": ["weather", "data"],
      "metadata_url": "https://raw.githubusercontent.com/camthink-ai/NeoMind-Extensions/main/extensions/weather-forecast/metadata.json"
    }
  ]
}
```

### extensions/{id}/metadata.json

Detailed metadata for each extension:

```json
{
  "id": "weather-forecast",
  "name": "Weather Forecast",
  "description": "...",
  "version": "0.1.0",
  "capabilities": {
    "tools": [...],
    "metrics": [...],
    "commands": [...]
  },
  "builds": {
    "darwin-aarch64": {
      "url": "https://github.com/.../download/v0.1.0/...",
      "sha256": "...",
      "size": 123456
    }
  },
  "requirements": {
    "min_neomind_version": "0.5.8",
    "network": true
  },
  "safety": {
    "timeout_seconds": 30,
    "max_memory_mb": 100
  }
}
```

---

## For Developers: Creating Extensions

See [EXTENSION_GUIDE.md](EXTENSION_GUIDE.md) for complete documentation.

### ⚠️ Critical Safety Requirements

**Extensions MUST be compiled with `panic = "unwind"` (NOT "abort")**

```toml
# In your workspace Cargo.toml
[profile.release]
opt-level = 3
lto = "thin"
panic = "unwind"  # REQUIRED for safety!
```

If your extension uses `panic = "abort"`, any panic will crash the entire NeoMind server instead of being safely caught.

### Quick Start

**Choose Your Extension Type:**

| Goal | Recommended Type |
|------|------------------|
| Cross-platform without rebuilding | WASM |
| Maximum performance | Native |
| Learning/Development | Native (template) or WASM (as-hello for JS/TS devs) |
| Production distribution | WASM |
| Fast iteration/prototyping | WASM (AssemblyScript - ~1s compile) |

**Native Extension (from template):**
```bash
cd extensions
cp -r template my-extension
cd my-extension

# Update Cargo.toml with your extension name
# Update src/lib.rs with your implementation
# Create metadata.json for marketplace listing

# Build
cargo build --release
```

**WASM Extension (from as-hello - AssemblyScript/TypeScript):**
```bash
cd extensions
cp -r as-hello my-as-extension
cd my-as-extension

# Install dependencies
npm install

# Update package.json with your extension name
# Update assembly/extension.ts with your implementation
# Update my-extension.json metadata
# Update asconfig.json if changing output file names

# Build (very fast ~1s, single binary for all platforms!)
npm run build
```

### Submitting to Marketplace

1. Fork this repository
2. Create your extension in `extensions/your-extension/`
3. Add metadata.json following the format above
4. Add your extension to `extensions/index.json`
5. Submit a pull request

After your PR is merged:
1. Build multi-platform binaries
2. Create a GitHub Release
3. Upload binaries to the Release
4. Update metadata.json with SHA256 checksums

---

## Release Process

When preparing a release:

```bash
# 1. Update version numbers
# - In each extension's Cargo.toml
# - In each extension's metadata.json
# - In extensions/index.json

# 2. Build for all platforms
./build.sh --all-platforms

# 3. Calculate SHA256
shasum -a 256 target/release/libneomind_extension_*

# 4. Create GitHub Release
gh release create v0.1.0 \
  target/release/*.dylib \
  target/release/*.so \
  target/release/*.dll

# 5. Update metadata.json with checksums
# 6. Commit and push
git add .
git commit -m "Release v0.1.0"
git push origin main
```

---

## Platform Support

| Platform | Architecture | Native Binary | WASM Binary |
|----------|--------------|---------------|-------------|
| macOS | ARM64 (Apple Silicon) | `libneomind_extension_*.dylib` | `*.wasm` (universal) |
| macOS | x86_64 (Intel) | `libneomind_extension_*.dylib` | `*.wasm` (universal) |
| Linux | x86_64 | `libneomind_extension_*.so` | `*.wasm` (universal) |
| Linux | ARM64 | `libneomind_extension_*.so` | `*.wasm` (universal) |
| Windows | x86_64 | `neomind_extension_*.dll` | `*.wasm` (universal) |

> **Note**: WASM extensions work on all platforms without recompilation - the same `.wasm` file runs everywhere!

---

## License

MIT License
---
