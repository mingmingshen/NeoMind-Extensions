# NeoMind Extension Development Guide V2

This guide explains how to develop, build, and install extensions for the NeoMind Edge AI Platform using the **V2 Extension API**.

[中文指南](EXTENSION_GUIDE.zh.md)

---

## Table of Contents

1. [Overview](#overview)
2. [V2 Extension Architecture](#v2-extension-architecture)
3. [Project Structure](#project-structure)
4. [Core Concepts](#core-concepts)
5. [V2 API Reference](#v2-api-reference)
6. [Building and Installation](#building-and-installation)
7. [Testing](#testing)
8. [Best Practices](#best-practices)
9. [Dashboard Components (Frontend)](#dashboard-components-frontend)
10. [Migration from V1](#migration-from-v1)

---

## Overview

A NeoMind Extension is a dynamic library (`.dylib`, `.so`, `.dll`) that extends the platform's capabilities. Extensions can provide:

| Capability | Description |
|-----------|-------------|
| **Metrics** | Time-series data points (e.g., temperature, humidity) |
| **Commands** | RPC-style commands for AI agents and direct API calls |

### Key V2 Changes

| V1 | V2 |
|----|----|
| `capabilities()` method | Separate `metrics()` and `commands()` methods |
| `execute_tool()` | `execute_command()` (async) |
| ABI Version 1 | ABI Version 2 |
| `neomind-extension-sdk` | `neomind-core::extension::system` |

---

## Extension Types: Native vs WASM

NeoMind supports two types of extensions, each with different trade-offs:

### Native Extensions (.dylib / .so / .dll)

Native extensions are platform-specific dynamic libraries loaded via FFI (Foreign Function Interface).

**Pros:**
- Maximum performance (no runtime overhead)
- Full system access (file system, network, hardware)
- Wide language support via C FFI (Rust, C, C++, Go, etc.)

**Cons:**
- Must compile for each platform separately
- Complex build setup for cross-platform distribution
- Security concerns (full system access)

### WASM Extensions (.wasm)

WASM extensions are WebAssembly modules that run in a sandboxed environment using Wasmtime.

**Pros:**
- **Write once, run anywhere** - single binary for all platforms
- Sandboxed execution (safe, controlled resource access)
- Small file size (typically < 100KB)
- Multi-language support (Rust, AssemblyScript, Go, C/C++)

**Cons:**
- ~10-30% performance overhead
- Limited system access (via host API only)
- Requires WASM target (`wasm32-wasi`)

### Which Type to Choose?

| Use Case | Recommended Type |
|----------|------------------|
| Production distribution | WASM (cross-platform) |
| Performance-critical operations | Native |
| Learning/Development | Native (easier debugging) |
| Untrusted extensions | WASM (sandboxed) |
| Hardware/OS integration | Native |

---

## V2 Extension Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    NeoMind Server                        │
├─────────────────────────────────────────────────────────┤
│  Extension Registry                                      │
│  ├─ Discovery (scan ~/.neomind/extensions)              │
│  ├─ Loading (dlopen + symbol resolution)                │
│  └─ Lifecycle Management                                 │
├─────────────────────────────────────────────────────────┤
│  Extension Safety (V2)                                   │
│  ├─ Circuit Breaker (5 failures → open)                  │
│  ├─ Panic Isolation (catch_unwind)                       │
│  └─ Health Monitoring                                    │
├─────────────────────────────────────────────────────────┤
│  Your Extension (Dynamic Library)                        │
│  └─ Implements Extension Trait (V2)                     │
│      ├─ metadata()                                       │
│      ├─ metrics() → &[MetricDescriptor]                  │
│      ├─ commands() → &[ExtensionCommand]                 │
│      ├─ execute_command() (async)                        │
│      ├─ produce_metrics()                                │
│      └─ health_check() (async)                           │
└─────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
my-extension/
├── Cargo.toml          # Package configuration
├── build.rs            # Optional build script
├── src/
│   └── lib.rs          # Extension implementation
└── README.md           # Extension documentation
```

### Minimal Cargo.toml

```toml
[package]
name = "neomind-my-extension"
version = "0.1.0"
edition = "2021"

[lib]
name = "neomind_extension_my_extension"
crate-type = ["cdylib"]   # Important: produces dynamic library

[dependencies]
# Use neomind-core from the NeoMind project
neomind-core = { path = "../NeoMind/crates/neomind-core" }
neomind-devices = { path = "../NeoMind/crates/neomind-devices" }

serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
once_cell = "1.19"   # For static metrics/commands
semver = "1.0"       # For version metadata
tokio = { version = "1", features = ["sync", "rt-multi-thread", "macros"] }
```

### ⚠️ CRITICAL: Panic Settings

**Extensions MUST be compiled with `panic = "unwind"` (NOT `"abort"`)**

This is **required** for the main server to safely catch panics from extensions. If your extension uses `panic = "abort"`, any panic will crash the entire NeoMind server instead of being caught gracefully.

#### Workspace Cargo.toml Example

```toml
[workspace]
members = ["extensions/*"]

# CRITICAL: Must match main project's panic setting
[profile.release]
opt-level = 3
lto = "thin"
panic = "unwind"  # NOT "abort" - required for safety!
```

#### Why This Matters

| Setting | Behavior | Safety |
|---------|----------|--------|
| `panic = "unwind"` | Panics unwind stack, can be caught | ✅ Server continues running |
| `panic = "abort"` | Panics immediately terminate process | ❌ Server crashes |

The NeoMind server wraps all FFI calls with `std::panic::catch_unwind()` to isolate extension panics. This only works if extensions are compiled with `panic = "unwind"`.

---

## Core Concepts

### 1. Extension Trait (V2)

Every extension must implement the `Extension` trait from `neomind_core::extension::system`:

```rust
use async_trait::async_trait;
use neomind_core::extension::system::{
    Extension, ExtensionMetadata, ExtensionError,
    MetricDescriptor, ExtensionCommand,
    ExtensionMetricValue, ParamMetricValue,
    MetricDataType, ABI_VERSION, Result,
};
use serde_json::Value;
use once_cell::sync::Lazy;
use std::sync::Arc;

pub struct MyExtension {
    metadata: ExtensionMetadata,
    state: Arc<MyState>,
}

// IMPORTANT: Use #[async_trait::async_trait] macro
#[async_trait::async_trait]
impl Extension for MyExtension {
    // Returns reference to metadata (not owned value)
    fn metadata(&self) -> &ExtensionMetadata {
        &self.metadata
    }

    // Returns slice of metric descriptors (use static to avoid lifetime issues)
    fn metrics(&self) -> &[MetricDescriptor] {
        &METRICS
    }

    // Returns slice of command descriptors (use static to avoid lifetime issues)
    fn commands(&self) -> &[ExtensionCommand] {
        &COMMANDS
    }

    // Async command execution
    async fn execute_command(&self, command: &str, args: &Value) -> Result<Value> {
        match command {
            "my_command" => {
                // Handle command
                Ok(json!({ "result": "success" }))
            }
            _ => Err(ExtensionError::CommandNotFound(command.to_string())),
        }
    }

    // Synchronous metric production (dylib compatibility)
    fn produce_metrics(&self) -> Result<Vec<ExtensionMetricValue>> {
        Ok(vec![
            ExtensionMetricValue {
                name: "my_metric".to_string(),
                value: ParamMetricValue::Float(42.0),
                timestamp: current_timestamp(),
            },
        ])
    }

    // Async health check
    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}
```

### 2. Static Metrics and Commands

**Critical**: Use `once_cell::sync::Lazy` or `lazy_static` for metrics and commands to avoid lifetime issues:

```rust
use once_cell::sync::Lazy;

/// Static metric descriptors
static METRICS: Lazy<[MetricDescriptor; 1]> = Lazy::new(|| [
    MetricDescriptor {
        name: "temperature_c".to_string(),
        display_name: "Temperature".to_string(),
        data_type: MetricDataType::Float,
        unit: "°C".to_string(),
        min: Some(-50.0),
        max: Some(60.0),
        required: false,
    },
]);

/// Static command descriptors
static COMMANDS: Lazy<[ExtensionCommand; 1]> = Lazy::new(|| [
    ExtensionCommand {
        name: "query_weather".to_string(),
        display_name: "Query Weather".to_string(),
        payload_template: r#"{"city": "{{city}}"}"#.to_string(),
        parameters: vec![],
        fixed_values: Default::default(),
        samples: vec![],
        llm_hints: "Query current weather for any city.".to_string(),
        parameter_groups: vec![],
    },
]);
```

### 3. Error Handling

```rust
pub enum ExtensionError {
    NotFound(String),           // Resource not found
    InvalidInput(String),       // Invalid parameters
    ExecutionFailed(String),    // Runtime error
    CommandNotFound(String),    // Unknown command (V2)
    IoError(String),            // I/O error
    Serialization(String),      // JSON serialization error
}
```

---

## V2 API Reference

### Extension Metadata

```rust
pub struct ExtensionMetadata {
    pub id: String,                  // Unique ID (e.g., "my.company.extension")
    pub name: String,                // Display name
    pub version: semver::Version,    // SemVer version (not String!)
    pub description: Option<String>, // Short description
    pub author: Option<String>,      // Author name
    pub homepage: Option<String>,    // Project URL
    pub license: Option<String>,     // License identifier
    pub file_path: Option<String>,   // Set by loader
}
```

### Metric Descriptor

```rust
pub struct MetricDescriptor {
    pub name: String,           // Metric ID (e.g., "temperature_c")
    pub display_name: String,   // Display name
    pub data_type: MetricDataType,  // Float, Integer, String, Boolean
    pub unit: String,           // Unit (e.g., "°C", "%", "km/h")
    pub min: Option<f64>,       // Minimum value
    pub max: Option<f64>,       // Maximum value
    pub required: bool,         // Whether this metric is required
}

pub enum MetricDataType {
    Float,
    Integer,
    String,
    Boolean,
}
```

### Extension Command

```rust
pub struct ExtensionCommand {
    pub name: String,                  // Command ID
    pub display_name: String,          // Display name
    pub payload_template: String,      // JSON template for parameters
    pub parameters: Vec<Parameter>,    // Parameter definitions
    pub fixed_values: HashMap<String, Value>,  // Fixed parameter values
    pub samples: Vec<Value>,           // Example inputs
    pub llm_hints: String,             // AI agent hints
    pub parameter_groups: Vec<ParameterGroup>,  // Parameter groups
}
```

### Extension Trait Methods (V2)

| Method | Return Type | Async | Description |
|--------|-------------|-------|-------------|
| `metadata()` | `&ExtensionMetadata` | No | Return extension metadata reference |
| `metrics()` | `&[MetricDescriptor]` | No | List all provided metrics |
| `commands()` | `&[ExtensionCommand]` | No | List all supported commands |
| `execute_command()` | `Result<Value>` | Yes | Execute a command |
| `produce_metrics()` | `Result<Vec<ExtensionMetricValue>>` | No | Return current metric values |
| `health_check()` | `Result<bool>` | Yes | Health check |

---

## FFI Exports (Required)

Your extension must export these C-compatible functions:

```rust
use std::ffi::CString;
use std::sync::RwLock;

/// ABI version (must be 2 for V2)
#[no_mangle]
pub extern "C" fn neomind_extension_abi_version() -> u32 {
    ABI_VERSION  // = 2
}

/// C-compatible metadata
#[no_mangle]
pub extern "C" fn neomind_extension_metadata() -> CExtensionMetadata {
    use std::ffi::CStr;

    let id = CString::new("my.extension").unwrap();
    let name = CString::new("My Extension").unwrap();
    let version = CString::new("0.1.0").unwrap();
    let description = CString::new("Does something useful").unwrap();
    let author = CString::new("Your Name").unwrap();

    CExtensionMetadata {
        abi_version: ABI_VERSION,
        id: id.as_ptr(),
        name: name.as_ptr(),
        version: version.as_ptr(),
        description: description.as_ptr(),
        author: author.as_ptr(),
        metric_count: 1,   // Number of metrics
        command_count: 1,  // Number of commands
    }
}

/// Create extension instance
#[no_mangle]
pub extern "C" fn neomind_extension_create(
    config_json: *const u8,
    config_len: usize,
) -> *mut RwLock<Box<dyn Extension>> {
    // Parse config
    let config = if config_json.is_null() || config_len == 0 {
        serde_json::json!({})
    } else {
        unsafe {
            let slice = std::slice::from_raw_parts(config_json, config_len);
            let s = std::str::from_utf8_unchecked(slice);
            serde_json::from_str(s).unwrap_or(serde_json::json!({}))
        }
    };

    // Create extension
    match MyExtension::new(&config) {
        Ok(ext) => {
            let boxed: Box<dyn Extension> = Box::new(ext);
            let wrapped = Box::new(RwLock::new(boxed));
            Box::into_raw(wrapped)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Destroy extension instance
#[no_mangle]
pub extern "C" fn neomind_extension_destroy(
    instance: *mut RwLock<Box<dyn Extension>>
) {
    if !instance.is_null() {
        unsafe {
            let _ = Box::from_raw(instance);
        }
    }
}
```

---

## Building and Installation

### 1. Build the Extension

```bash
cd ~/NeoMind-Extension
cargo build --release
```

Output location:
- macOS: `target/release/libneomind_extension_my_extension.dylib`
- Linux: `target/release/libneomind_extension_my_extension.so`
- Windows: `target/release/neomind_extension_my_extension.dll`

### Cross-Platform Native Compilation

Native extensions are platform-specific. To distribute to multiple platforms, you need to compile for each:

#### macOS

```bash
# Apple Silicon (M1/M2/M3)
cargo build --release --target aarch64-apple-darwin

# Intel Mac
cargo build --release --target x86_64-apple-darwin

# Universal Binary (both architectures)
lipo -create \
  target/aarch64-apple-darwin/release/libneomind_extension_my_extension.dylib \
  target/x86_64-apple-darwin/release/libneomind_extension_my_extension.dylib \
  -output target/universal/libneomind_extension_my_extension.dylib
```

#### Linux

```bash
# x86_64 (most common)
cargo build --release --target x86_64-unknown-linux-gnu

# ARM64 (Raspberry Pi, servers)
cargo build --release --target aarch64-unknown-linux-gnu

# Cross-compilation from macOS (requires cross)
cargo install cross
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target aarch64-unknown-linux-gnu
```

#### Windows

```bash
# From Linux/macOS (requires cross)
cross build --release --target x86_64-pc-windows-gnu

# From Windows directly
cargo build --release --target x86_64-pc-windows-msvc
```

#### Using `cross` for Cross-Compilation

The easiest way to cross-compile for different platforms is using `cross`:

```bash
# Install cross
cargo install cross

# Install Docker (required for cross)
# macOS: brew install --cask docker
# Linux: sudo apt install docker.io

# Build for all platforms
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target x86_64-pc-windows-gnu
```

#### Cross-Compilation Summary Table

| Platform | Target Triple | Cross-Compile | Native |
|----------|--------------|---------------|--------|
| macOS ARM | `aarch64-apple-darwin` | ❌ | ✅ |
| macOS Intel | `x86_64-apple-darwin` | ✅ (from ARM Mac) | ✅ |
| Linux x86 | `x86_64-unknown-linux-gnu` | ✅ (cross) | ✅ |
| Linux ARM | `aarch64-unknown-linux-gnu` | ✅ (cross) | ✅ |
| Windows | `x86_64-pc-windows-gnu` | ✅ (cross) | ✅ |

### 2. Install the Extension

```bash
# Create extensions directory if it doesn't exist
mkdir -p ~/.neomind/extensions

# Copy the compiled extension
cp target/release/libneomind_extension_my_extension.* ~/.neomind/extensions/
```

### 3. Verify Installation

```bash
# List loaded extensions via API
curl http://localhost:9375/api/extensions

# Check specific extension health
curl http://localhost:9375/api/extensions/my.extension/health
```

---

## Testing

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_creation() {
        let ext = MyExtension::new(&json!({})).unwrap();
        assert_eq!(ext.metadata().id, "my.extension");
    }

    #[tokio::test]
    async fn test_command_execution() {
        let ext = MyExtension::new(&json!({})).unwrap();
        let result = ext.execute_command(
            "my_command",
            &json!({"param": "value"})
        ).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_metrics_production() {
        let ext = MyExtension::new(&json!({})).unwrap();
        let metrics = ext.produce_metrics().unwrap();
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_health_check() {
        let ext = MyExtension::new(&json!({})).unwrap();
        let healthy = ext.health_check().await.unwrap();
        assert!(healthy);
    }
}
```

### Run Tests

```bash
cargo test
```

---

## Best Practices

### 1. Panic Safety (CRITICAL)

**Extensions MUST be compiled with `panic = "unwind"` to allow the host to catch panics.**

#### Workspace Cargo.toml Configuration

```toml
# In your workspace Cargo.toml (NOT individual extension)
[profile.release]
opt-level = 3
lto = "thin"
panic = "unwind"  # REQUIRED: Must be "unwind", NOT "abort"
```

> ⚠️ **Warning**: Using `panic = "abort"` will cause the entire NeoMind server to crash if your extension panics. Always use `panic = "unwind"` for extensions.

#### Code-Level Panic Prevention

**Never let panics escape from your extension!**

```rust
async fn execute_command(&self, command: &str, args: &Value) -> Result<Value> {
    // Bad: unwrap() will panic
    // let city = args.get("city").unwrap().as_str().unwrap();

    // Good: handle errors properly
    let city = args.get("city")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtensionError::InvalidArguments("city required".to_string()))?;
    // ...
}
```

#### Host Safety Mechanisms

NeoMind provides multiple safety layers to protect against extension failures:

```
┌─────────────────────────────────────────────────────┐
│ Safety Layer 1: panic = "unwind" (compile-time)     │
├─────────────────────────────────────────────────────┤
│ Safety Layer 2: catch_unwind (FFI call protection)  │
├─────────────────────────────────────────────────────┤
│ Safety Layer 3: Circuit Breaker (5 failures → open) │
├─────────────────────────────────────────────────────┤
│ Safety Layer 4: Panic Tracking (3 panics → disable) │
├─────────────────────────────────────────────────────┤
│ Safety Layer 5: Timeout (30s command timeout)       │
└─────────────────────────────────────────────────────┘
```

**Why panic = "unwind" is Required:**

The host uses `std::panic::catch_unwind()` to catch panics from FFI calls. This only works if:
1. The extension is compiled with `panic = "unwind"` (NOT "abort")
2. The panic originates from Rust code (not C/FFI code)

If your extension is compiled with `panic = "abort"`, any panic will immediately terminate the entire NeoMind server process!

### 2. HTTP Client Selection

**Important**: When making HTTP requests from native extensions:

- **Use `ureq`** (blocking client) for simple HTTP calls
- **Avoid `reqwest`** if possible, as it brings Tokio runtime dependencies

```toml
# Recommended: ureq (blocking, lightweight)
[dependencies]
ureq = { version = "2.9", features = ["json"] }

# NOT recommended for extensions: reqwest (brings Tokio runtime)
# reqwest = { version = "0.11", features = ["json"] }
```

```rust
// Example: Using ureq for HTTP calls
fn fetch_data(&self, url: &str) -> Result<MyData> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| ExtensionError::ExecutionFailed(format!("HTTP error: {}", e)))?;

    let data: MyData = response
        .into_json()
        .map_err(|e| ExtensionError::Serialization(format!("JSON error: {}", e)))?;

    Ok(data)
}
```

**Why ureq?**
- No Tokio runtime dependency (avoids conflicts with host)
- Simpler FFI boundary
- Smaller binary size
- Works well in blocking contexts (command execution)

### 3. Thread Safety

Extensions may be called concurrently. Use appropriate synchronization:

```rust
use std::sync::{Arc, RwLock};

pub struct MyExtension {
    state: Arc<RwLock<MyState>>,
}
```

### 4. Static Metrics/Commands

Always use `once_cell::sync::Lazy` for metrics and commands:

```rust
// Good: static with Lazy
static METRICS: Lazy<[MetricDescriptor; 1]> = Lazy::new(|| [
    MetricDescriptor { /* ... */ },
]);

fn metrics(&self) -> &[MetricDescriptor] {
    &METRICS
}

// Bad: returning reference to local array
fn metrics(&self) -> &[MetricDescriptor] {
    &[
        MetricDescriptor { /* ... */ },  // ❌ Lifetime error!
    ]
}
```

### 4. Resource Cleanup

```rust
impl Drop for MyExtension {
    fn drop(&mut self) {
        // Clean up resources (close connections, etc.)
    }
}
```

### 5. Idempotent Operations

Design commands to be idempotent where possible:

```rust
// Good: calling multiple times has same effect
async fn refresh(&self) -> Result<Value, ExtensionError> {
    // Refresh logic with cache check
}
```

---

## Migration from V1

| V1 API | V2 API |
|--------|--------|
| `use neomind_extension_sdk::prelude::*;` | `use neomind_core::extension::system::*;` |
| `fn metadata() -> ExtensionMetadata` | `fn metadata() -> &ExtensionMetadata` |
| `fn capabilities()` | `fn metrics() + fn commands()` |
| `fn execute_tool()` | `fn execute_command()` (async) |
| `NEO_EXT_ABI_VERSION = 1` | `ABI_VERSION = 2` |
| `neomind_ext_version()` | `neomind_extension_abi_version()` |
| Returns owned values | Returns references (use static) |

### Migration Steps

1. Update `Cargo.toml`: Change dependency to `neomind-core`
2. Add `async-trait` and `once_cell` dependencies
3. Change `capabilities()` to separate `metrics()` and `commands()`
4. Convert `execute_tool()` to async `execute_command()`
5. Use `once_cell::sync::Lazy` for static descriptors
6. Update FFI export names and ABI version
7. Add `#[async_trait::async_trait]` to impl block

---

## Complete Example

See the `extensions/weather-forecast/` directory for a complete working example:

```bash
cat ~/NeoMind-Extension/extensions/weather-forecast/src/lib.rs
```

This extension demonstrates:
- Static metrics and commands with `once_cell::sync::Lazy`
- Async command execution
- Metric production
- Health checks
- Proper FFI exports

---

## WASM Extension Development

WASM extensions provide cross-platform compatibility with a single build artifact. They run in a sandboxed environment using Wasmtime.

### WASM Target Systems

NeoMind supports multiple WASM targets for different use cases:

| Target | Description | Use Case |
|--------|-------------|----------|
| `wasm32-wasi` | WASI (WebAssembly System Interface) | **Recommended** - Full system access, file I/O, network |
| `wasm32-unknown-unknown` | Pure WebAssembly | Browser-only, no system access |

> **Note**: NeoMind primarily uses `wasm32-wasi` for extensions because it provides system-level capabilities needed for IoT and automation.

### Installing WASM Targets

```bash
# Install WASI target (recommended)
rustup target add wasm32-wasi

# Install pure WebAssembly target (optional)
rustup target add wasm32-unknown-unknown

# Verify installation
rustup target list --installed | grep wasm
```

### Cross-Platform WASM Compilation

One of the main advantages of WASM is **write once, run anywhere**. A single `.wasm` file works on:
- macOS (Apple Silicon and Intel)
- Linux (x86_64 and ARM64)
- Windows (x86_64)
- Web browsers

```bash
# Build WASM extension (works on ALL platforms!)
cargo build --release --target wasm32-wasi

# Output: target/wasm32-wasi/release/my_extension.wasm
```

### WASM Build Configuration

#### Cargo.toml for WASM

```toml
[package]
name = "my-wasm-extension"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Required for WASM

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Optional: WASM-specific optimizations
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
panic = "abort"      # Smaller binary (WASM doesn't support unwinding)
strip = true         # Strip symbols
```

#### .cargo/config.toml for WASM

Create `.cargo/config.toml` for WASM-specific settings:

```toml
[target.wasm32-wasi]
# Use WASI SDK if installed (optional, for better optimization)
# rustflags = ["-C", "linker=clang"]

[build]
# Default target for this extension
# target = "wasm32-wasi"
```

### WASI System Interface

WASI (WebAssembly System Interface) provides access to system resources:

| Capability | WASI Support | Notes |
|------------|--------------|-------|
| File I/O | ✅ Full | Read/write files in sandboxed directories |
| Network | ✅ Partial | HTTP client via host API |
| Environment | ✅ Full | Read environment variables |
| Arguments | ✅ Full | Command-line arguments |
| Random | ✅ Full | Cryptographic random number generator |
| Clock | ✅ Full | System time access |
| Threads | ⚠️ Limited | Experimental in some runtimes |

### WASM Extension Structure

```
my-wasm-extension/
├── Cargo.toml          # Package configuration
├── my-extension.json   # Metadata sidecar file
├── README.md           # Extension documentation
└── src/
    └── lib.rs          # WASM extension implementation
```

### Minimal Cargo.toml for WASM

```toml
[package]
name = "my-wasm-extension"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### WASM Extension Implementation

WASM extensions use a different approach than native extensions:

```rust
use serde::Serialize;

/// Simple metric value
#[derive(Debug, Clone, Serialize)]
struct MetricValue {
    name: String,
    value: f64,
    unit: String,
}

/// Command response
#[derive(Debug, Clone, Serialize)]
struct CommandResponse {
    success: bool,
    message: String,
    data: Option<MetricValue>,
}

/// Get a metric value
#[no_mangle]
pub extern "C" fn get_my_metric() -> f64 {
    42.0
}

/// Process a command and return JSON response
#[no_mangle]
pub extern "C" fn neomind_execute(
    command_ptr: *const u8,
    _args_ptr: *const u8,
    result_buf_ptr: *mut u8,
    result_buf_len: usize,
) -> usize {
    // Read command string (null-terminated)
    let command = unsafe {
        let mut len = 0;
        while *command_ptr.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(command_ptr, len)
    };

    let command_str = std::str::from_utf8(command).unwrap_or("unknown");

    // Match command and generate response
    let response = match command_str {
        "get_metric" => CommandResponse {
            success: true,
            message: "Metric retrieved".to_string(),
            data: Some(MetricValue {
                name: "my_metric".to_string(),
                value: get_my_metric(),
                unit: "count".to_string(),
            }),
        },
        "hello" => CommandResponse {
            success: true,
            message: "Hello from WASM!".to_string(),
            data: None,
        },
        _ => CommandResponse {
            success: false,
            message: format!("Unknown command: {}", command_str),
            data: None,
        },
    };

    // Serialize to JSON and write to buffer
    let json = serde_json::to_string(&response).unwrap_or_default();
    let json_bytes = json.as_bytes();
    let write_len = std::cmp::min(json_bytes.len(), result_buf_len.saturating_sub(1));

    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), result_buf_ptr, write_len);
        *result_buf_ptr.add(write_len) = 0; // Null-terminate
    }

    write_len
}

/// Health check
#[no_mangle]
pub extern "C" fn health() -> i32 {
    1 // 1 = healthy
}
```

### Metadata Sidecar File

WASM extensions use a JSON sidecar file for metadata:

```json
{
    "id": "my-wasm-extension",
    "name": "My WASM Extension",
    "version": "0.1.0",
    "description": "A cross-platform WASM extension",
    "author": "Your Name",
    "homepage": "https://github.com/your/repo",
    "license": "MIT",
    "metrics": [
        {
            "name": "my_metric",
            "display_name": "My Metric",
            "data_type": "float",
            "unit": "count",
            "min": null,
            "max": null,
            "required": false
        }
    ],
    "commands": [
        {
            "name": "get_metric",
            "display_name": "Get Metric",
            "description": "Get the current metric value",
            "payload_template": "{}",
            "parameters": [],
            "fixed_values": {},
            "samples": [],
            "llm_hints": "Returns the current metric value",
            "parameter_groups": []
        },
        {
            "name": "hello",
            "display_name": "Hello",
            "description": "Say hello from WASM",
            "payload_template": "{}",
            "parameters": [],
            "fixed_values": {},
            "samples": [],
            "llm_hints": "Returns a greeting message",
            "parameter_groups": []
        }
    ]
}
```

### Building WASM Extensions

For AssemblyScript WASM (recommended):
```bash
# Install dependencies
cd ~/NeoMind-Extension/extensions/as-hello
npm install

# Build extension
npm run build

# Output: build/as-hello.wasm
```

### Installing WASM Extensions

```bash
# Copy both files to extensions directory
mkdir -p ~/.neomind/extensions
cp build/as-hello.wasm ~/.neomind/extensions/
cp metadata.json ~/.neomind/extensions/as-hello.json

# Restart NeoMind or use extension discovery
```

### WASM vs Native: Key Differences

| Aspect | Native | WASM |
|--------|--------|------|
| **Metadata** | Embedded in code via FFI | Separate JSON file |
| **Exports** | `neomind_extension_*` functions | `neomind_execute`, `health`, custom functions |
| **State** | Can use `Arc<RwLock<T>>` | Limited to WASM memory |
| **System Access** | Full access | Via host API only |
| **Distribution** | Platform-specific binaries | Single `.wasm` file for all platforms |

### WASM Best Practices

1. **Keep it simple**: WASM extensions work best for simple operations
2. **Avoid external dependencies**: Many crates don't work with `wasm32-wasi`
3. **Use JSON for responses**: The sandbox handles JSON serialization
4. **Test on target platform**: WASM behavior can differ from native
5. **Provide good metadata**: The JSON file is your primary documentation

### WASM Host API

WASM extensions can access external resources through host functions provided by NeoMind's sandbox. Currently supported:

| Host Function | Description | Parameters | Return |
|---------------|-------------|------------|--------|
| `host_http_request` | Make HTTP requests | method, url | response JSON |

#### HTTP Requests from WASM

WASM extensions can make HTTP requests using the `host_http_request` host function:

```rust
// Import the host function
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_http_request(
        method_ptr: *const u8,
        method_len: i32,
        url_ptr: *const u8,
        url_len: i32,
        result_ptr: *mut u8,
        result_max_len: i32,
    ) -> i32;
}

// Helper function to make HTTP GET requests
fn http_get(url: &str) -> Result<String, String> {
    let method = b"GET";
    let url_bytes = url.as_bytes();
    let mut result_buffer = vec![0u8; 65536]; // 64KB buffer

    let result_len = unsafe {
        host_http_request(
            method.as_ptr(),
            method.len() as i32,
            url_bytes.as_ptr(),
            url_bytes.len() as i32,
            result_buffer.as_mut_ptr(),
            result_buffer.len() as i32,
        )
    };

    if result_len < 0 {
        return Err("HTTP request failed".to_string());
    }

    // Find null terminator
    let end = result_buffer.iter().position(|&b| b == 0).unwrap_or(result_len as usize);
    String::from_utf8(result_buffer[..end].to_vec())
        .map_err(|e| format!("Invalid UTF-8 response: {}", e))
}
```

#### Declaring Permissions

Extensions that make HTTP requests must declare the allowed URLs in their manifest:

```json
{
  "id": "my.extension",
  "permissions": [
    "https://api.example.com",
    "https://geocoding-api.open-meteo.com",
    "https://api.open-meteo.com"
  ]
}
```

### Example: Weather Forecast WASM Extension

See `extensions/weather-forecast-wasm/` for a complete WASM extension that:
- Uses the HTTP host API to fetch weather data
- Calls Open-Meteo's free geocoding and weather APIs
- Demonstrates proper error handling in WASM
- Shows how to parse JSON responses

```bash
# Build the WASM extension
cd extensions/weather-forecast-wasm
cargo build --target wasm32-unknown-unknown --release

# Output: weather_forecast_wasm.wasm (~170 KB)
```

### Example: as-hello Extension (AssemblyScript)

See the `extensions/as-hello/` directory for a complete working example:

```bash
cat ~/NeoMind-Extension/extensions/as-hello/assembly/extension.ts
cat ~/NeoMind-Extension/extensions/as-hello/metadata.json
```

This extension demonstrates:
- TypeScript-like syntax for WASM
- Fast compile times (~1s)
- Small binary size (~15 KB)
- Metric exports and command execution
- Health check implementation
- Metadata sidecar file

---

### AssemblyScript WASM Extensions

AssemblyScript is a TypeScript-like language that compiles to WebAssembly. It's ideal for JavaScript/TypeScript developers who want to create WASM extensions with fast compile times.

#### Why AssemblyScript?

| Feature | AssemblyScript | Rust WASM |
|---------|----------------|-----------|
| **Syntax** | TypeScript-like | Rust |
| **Learning Curve** | Low (for JS/TS devs) | High |
| **Compile Time** | ~1s | ~5s |
| **Binary Size** | ~15 KB | ~50 KB |
| **Performance** | 90-95% native | 85-90% native |

#### Project Structure

```
my-as-extension/
├── package.json          # npm dependencies
├── asconfig.json         # AssemblyScript compiler config
├── metadata.json         # Extension metadata sidecar
├── README.md             # Extension documentation
└── assembly/
    └── extension.ts      # AssemblyScript source
```

#### Minimal package.json

```json
{
  "name": "my-as-extension",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "build": "asc assembly/extension.ts --target release --outFile build/my-extension.wasm -O3z"
  },
  "devDependencies": {
    "assemblyscript": "^0.27.29"
  }
}
```

#### Minimal asconfig.json

```json
{
  "extends": "assemblyscript/std/assembly.json",
  "entry": "assembly/extension.ts",
  "targets": {
    "release": {
      "binaryFile": "build/my-extension.wasm",
      "textFile": "build/my-extension.wat",
      "optimizeLevel": 3,
      "shrinkLevel": 0,
      "runtime": "minimal"
    }
  }
}
```

#### AssemblyScript Implementation

```typescript
// ABI Version - must be 2 for NeoMind V2 API
const ABI_VERSION: u32 = 2;

// Extension state
let counter: i32 = 0;

// Export ABI version
export function neomind_extension_abi_version(): u32 {
  return ABI_VERSION;
}

// Get metric value
export function get_counter(): i32 {
  return counter;
}

// Increment counter
export function increment_counter(): i32 {
  counter = counter + 1;
  return counter;
}

// Execute command
export function neomind_execute(
  command_ptr: usize,
  args_ptr: usize,
  result_buf_ptr: usize,
  result_buf_len: usize
): usize {
  // Read command string
  const command = getString(command_ptr);

  // Generate response based on command
  let response: string;

  switch (command) {
    case "get_counter":
      response = JSON.stringify({
        success: true,
        message: "Counter retrieved",
        data: {
          name: "counter",
          value: counter,
          unit: "count"
        }
      });
      break;

    case "increment_counter":
      counter = counter + 1;
      response = JSON.stringify({
        success: true,
        message: "Counter incremented",
        data: {
          name: "counter",
          value: counter,
          unit: "count"
        }
      });
      break;

    default:
      response = JSON.stringify({
        success: false,
        message: "Unknown command",
        error: `Command '${command}' not found`
      });
      break;
  }

  // Write response to buffer
  return writeString(result_buf_ptr, result_buf_len, response);
}

// Health check
export function health(): i32 {
  return 1; // 1 = healthy
}

// Helper: Read null-terminated string from memory
@inline
function getString(ptr: usize): string {
  if (ptr === 0) return "";

  let len = 0;
  while (load<u8>(ptr + len) !== 0) {
    len++;
  }

  return String.UTF8.decode(ptr, len);
}

// Helper: Write string to memory buffer
@inline
function writeString(ptr: usize, maxLen: usize, str: string): usize {
  const encoded = String.UTF8.encode(str);
  const len = encoded.length;
  const writeLen = min(len, maxLen - 1);

  memory.copy(ptr, changetype<usize>(encoded), writeLen);
  store<u8>(ptr + writeLen, 0);

  return writeLen;
}
```

#### Building AssemblyScript Extensions

```bash
# Install dependencies
cd ~/NeoMind-Extension/extensions/as-hello
npm install

# Build the extension
npm run build

# Output: build/as-hello.wasm
```

#### Installing AssemblyScript Extensions

```bash
# Copy both files to extensions directory
mkdir -p ~/.neomind/extensions
cp build/as-hello.wasm ~/.neomind/extensions/
cp metadata.json ~/.neomind/extensions/as-hello.json

# Restart NeoMind or use extension discovery
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Extension not loading | Check ABI version returns 2, not 1 |
| Lifetime errors in metrics()/commands() | Use `once_cell::sync::Lazy` for static data |
| Panic on load | Ensure no panics in `new()` constructor |
| Commands failing | Check circuit breaker hasn't opened due to failures |
| Metrics not appearing | Verify `produce_metrics()` returns valid data |
| Wrong SDK reference | Use `neomind-core`, not `neomind-extension-sdk` |

### WASM-Specific Issues

| Issue | Solution |
|-------|----------|
| WASM extension not loading | Ensure both `.wasm` and `.json` files are present |
| `wasm32-wasi` target not found | Run `rustup target add wasm32-wasi` |
| Dependencies not compiling for WASM | Some crates don't support `wasm32-wasi`; check compatibility |
| Extension loads but commands fail | Check that function names match exactly (case-sensitive) |
| JSON metadata not recognized | Verify JSON is valid and matches the schema |
| Metrics not showing | Ensure `data_type` in JSON is valid (`integer`, `float`, `string`, `boolean`) |

---

## Dashboard Components (Frontend)

Extensions can provide custom **Dashboard Components** that render in the NeoMind web UI. These are React-based components bundled as IIFE (Immediately Invoked Function Expression) modules.

### Overview

```
┌─────────────────────────────────────────────────────────┐
│                    NeoMind Dashboard                     │
├─────────────────────────────────────────────────────────┤
│  Component Library                                       │
│  ├─ Built-in Components (gauge, chart, status, etc.)    │
│  └─ Extension Components (dynamically loaded)           │
├─────────────────────────────────────────────────────────┤
│  Extension Component Loading                             │
│  ├─ manifest.json defines dashboard_components          │
│  ├─ Frontend bundle loaded via script injection         │
│  └─ Component rendered with props from config           │
└─────────────────────────────────────────────────────────┘
```

### Project Structure

```
my-extension/
├── Cargo.toml                    # Rust backend
├── src/lib.rs                    # Extension implementation
├── manifest.json                 # Extension metadata + dashboard components
└── frontend/                     # Frontend dashboard component
    ├── package.json              # npm dependencies
    ├── vite.config.ts            # Vite build configuration
    └── src/
        └── index.tsx             # React component
```

### 1. manifest.json Configuration

Add a `dashboard_components` array to your `manifest.json`:

```json
{
  "id": "my.extension",
  "name": "My Extension",
  "version": "0.1.0",
  "main": "libneomind_extension_my_extension.dylib",
  "dashboard_components": [
    {
      "type": "my-component",
      "name": "My Component",
      "description": "A custom dashboard component",
      "category": "custom",
      "icon": "Activity",
      "bundle_path": "/frontend/dist/my-dashboard-components.js",
      "export_name": "MyComponent",
      "size_constraints": {
        "min_w": 2,
        "min_h": 2,
        "default_w": 4,
        "default_h": 4,
        "max_w": 8,
        "max_h": 6
      },
      "has_data_source": true,
      "has_display_config": true,
      "has_actions": false,
      "max_data_sources": 1,
      "config_schema": {
        "type": "object",
        "properties": {
          "refreshInterval": {
            "type": "number",
            "description": "Auto-refresh interval in milliseconds",
            "default": 30000,
            "minimum": 0,
            "maximum": 3600000
          },
          "showDetails": {
            "type": "boolean",
            "description": "Show detailed information",
            "default": true
          }
        }
      },
      "default_config": {
        "refreshInterval": 30000,
        "showDetails": true
      },
      "variants": ["default", "compact"],
      "data_binding": {
        "extension_command": "get_data",
        "required_fields": []
      }
    }
  ]
}
```

### 2. Frontend Component Implementation

Create a React component in `frontend/src/index.tsx`:

```tsx
/**
 * My Extension Dashboard Component
 */
import { forwardRef, useEffect, useState, useCallback } from 'react'

// Data source interface (provided by dashboard)
export interface DataSource {
  type: string
  extensionId?: string
  [key: string]: any
}

// Component props interface
export interface MyComponentProps {
  title?: string
  dataSource?: DataSource
  className?: string
  editMode?: boolean
  // Config props from config_schema
  refreshInterval?: number
  showDetails?: boolean
}

// Extension ID for API calls
const EXTENSION_ID = 'my.extension'

export const MyComponent = forwardRef<HTMLDivElement, MyComponentProps>(
  function MyComponent(props, ref) {
    const {
      title = 'My Component',
      dataSource,
      className = '',
      refreshInterval = 30000,
      showDetails = true
    } = props

    const [data, setData] = useState<any>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    const extensionId = dataSource?.extensionId || EXTENSION_ID

    // Fetch data from extension
    const fetchData = useCallback(async () => {
      setLoading(true)
      setError(null)
      try {
        const response = await fetch(`/api/extensions/${extensionId}/command`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            command: 'get_data',
            args: {}
          })
        })

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`)
        }

        const result = await response.json()
        if (result.success && result.data) {
          setData(result.data)
        } else {
          throw new Error(result.error || 'Failed to fetch data')
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error')
      } finally {
        setLoading(false)
      }
    }, [extensionId])

    // Initial fetch and auto-refresh
    useEffect(() => {
      fetchData()
      if (refreshInterval > 0) {
        const interval = setInterval(fetchData, refreshInterval)
        return () => clearInterval(interval)
      }
    }, [fetchData, refreshInterval])

    return (
      <div ref={ref} className={`p-4 bg-white rounded-lg shadow ${className}`}>
        <h3 className="text-lg font-semibold mb-2">{title}</h3>

        {loading && !data && (
          <div className="text-gray-500">Loading...</div>
        )}

        {error && (
          <div className="text-red-500">Error: {error}</div>
        )}

        {data && (
          <div>
            {/* Render your data here */}
            <pre>{JSON.stringify(data, null, 2)}</pre>
          </div>
        )}
      </div>
    )
  }
)

MyComponent.displayName = 'MyComponent'

// Export default for IIFE compatibility
export default { MyComponent }
```

### 3. package.json Configuration

```json
{
  "name": "my-dashboard-components",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "build": "vite build",
    "dev": "vite build --watch"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "vite": "^6.0.11"
  }
}
```

### 4. vite.config.ts Configuration

**Critical**: The component must be built as an IIFE bundle with React externalized:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [
    react({
      // Use classic JSX runtime to avoid jsx-runtime dependency
      jsxRuntime: 'classic'
    })
  ],
  define: {
    // Replace process.env.NODE_ENV to avoid "process is not defined" error
    'process.env.NODE_ENV': JSON.stringify('production')
  },
  build: {
    lib: {
      entry: './src/index.tsx',
      name: 'MyDashboardComponents',  // Global variable name
      fileName: () => `my-dashboard-components.js`,
      formats: ['iife']  // IIFE assigns to global variable
    },
    rollupOptions: {
      // Externalize React to use the host app's React
      external: ['react', 'react-dom'],
      output: {
        // Use 'named' exports for named exports availability
        exports: 'named',
        // Provide global variables for external dependencies
        globals: {
          'react': 'React',
          'react-dom': 'ReactDOM'
        }
      }
    }
  }
})
```

### 5. Building and Installing

```bash
# Build the Rust backend
cd ~/NeoMind-Extension/extensions/my-extension
cargo build --release

# Build the frontend component
cd frontend
npm install
npm run build

# Install extension
mkdir -p ~/.neomind/extensions/my-extension
cp ../target/release/libneomind_extension_my_extension.dylib ~/.neomind/extensions/
cp manifest.json ~/.neomind/extensions/my-extension/
cp -r dist ~/.neomind/extensions/my-extension/frontend/

# Restart NeoMind
```

### Component Props Reference

Extension components receive these props from the dashboard:

| Prop | Type | Description |
|------|------|-------------|
| `title` | `string?` | Component title from dashboard config |
| `dataSource` | `DataSource?` | Data source binding configuration |
| `className` | `string?` | Additional CSS classes |
| `editMode` | `boolean?` | True when dashboard is in edit mode |
| `...config` | `any` | Props from `config_schema` properties |

### Global Variable Naming Convention

The frontend loader maps component types to global variable names:

| Component Type | Global Variable Name |
|----------------|---------------------|
| `weather-card` | `WeatherDashboardComponents` |
| `image-analyzer` | `ImageAnalyzerDashboardComponents` |
| `yolo-video-display` | `YoloVideoDashboardComponents` |
| Custom (`my-component`) | `MyDashboardComponents` |

**Convention**: `{PascalCase(component-type)}DashboardComponents`

### Complete Example: Weather Card

See `extensions/weather-forecast/` for a complete working example:

```
weather-forecast/
├── Cargo.toml
├── manifest.json                 # Defines weather-card component
├── src/lib.rs                    # Rust backend with get_weather command
└── frontend/
    ├── package.json
    ├── vite.config.ts
    └── src/
        └── index.tsx             # WeatherCard React component
```

Features demonstrated:
- Beautiful gradient backgrounds based on weather conditions
- City search with editable input
- Auto-refresh capability
- Temperature unit selection
- Responsive loading and error states

### Dashboard Component Best Practices

1. **Use TypeScript** for type safety and better IDE support
2. **Externalize React** to avoid bundle size bloat and version conflicts
3. **Handle loading/error states** gracefully
4. **Support auto-refresh** with configurable intervals
5. **Use Tailwind CSS** for styling (matches NeoMind UI)
6. **Keep components small** - split complex UI into multiple components
7. **Test in both Tauri and web** environments

---

## Further Reading

- NeoMind Core: `~/NeoMind/crates/neomind-core/src/extension/system.rs`
- Extension Tests: `~/NeoMind/crates/neomind-core/tests/extension_test.rs`
- Example Extensions: `~/NeoMind-Extension/extensions/`
