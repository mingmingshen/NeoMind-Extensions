# SDK API Reference

## Core Traits

### Extension Trait

The main trait that all extensions must implement:

```rust
#[async_trait]
pub trait Extension: Send + Sync {
    /// Return extension metadata (name, version, etc.)
    fn metadata(&self) -> &ExtensionMetadata;

    /// Return list of available metrics
    fn metrics(&self) -> &[MetricDescriptor];

    /// Return list of available commands
    fn commands(&self) -> &[ExtensionCommand];

    /// Execute a command with given arguments
    async fn execute_command(
        &self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Produce current metric values (synchronous!)
    fn produce_metrics(&self) -> Result<Vec<ExtensionMetricValue>>;

    /// Optional: Initialize extension (called after creation)
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Optional: Cleanup resources (called before destruction)
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
```

## Data Structures

### ExtensionMetadata

```rust
pub struct ExtensionMetadata {
    pub id: String,                    // e.g., "weather-forecast-v2"
    pub name: String,                  // Display name
    pub version: Version,              // Semantic version (semver crate)
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub file_path: Option<PathBuf>,
    pub config_parameters: Option<Vec<ConfigParameter>>,
}
```

### MetricDescriptor

```rust
pub struct MetricDescriptor {
    pub name: String,                  // Internal metric name
    pub display_name: String,          // UI display name
    pub data_type: MetricDataType,     // Integer, Float, Boolean, String
    pub unit: String,                  // e.g., "celsius", "percent"
    pub min: Option<f64>,              // Min value (for numeric types)
    pub max: Option<f64>,              // Max value (for numeric types)
    pub required: bool,                // Must always be present
}
```

### ExtensionCommand

```rust
pub struct ExtensionCommand {
    pub name: String,                  // Command name (snake_case)
    pub display_name: String,          // UI display name
    pub payload_template: String,      // JSON template for parameters
    pub parameters: Vec<ParameterDefinition>,
    pub fixed_values: HashMap<String, ParamMetricValue>,
    pub samples: Vec<serde_json::Value>,
    pub llm_hints: String,             // Hints for AI assistants
    pub parameter_groups: Vec<ParameterGroup>,
}
```

### MetricDataType

```rust
pub enum MetricDataType {
    Integer,    // i64
    Float,      // f64
    Boolean,    // bool
    String,     // String
}
```

### ParamMetricValue

```rust
pub enum ParamMetricValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}
```

### ExtensionMetricValue

```rust
pub struct ExtensionMetricValue {
    pub name: String,           // Metric name
    pub value: ParamMetricValue,
    pub timestamp: i64,         // Unix timestamp in milliseconds
}
```

## Error Handling

### ExtensionError

```rust
pub enum ExtensionError {
    CommandNotFound(String),
    InvalidArguments(String),
    ExecutionError(String),
    NetworkError(String),
    IoError(String),
    ConfigError(String),
    Other(String),
}
```

### Result Type

```rust
pub type Result<T> = std::result::Result<T, ExtensionError>;
```

## Macros

### neomind_export!

Automatically generates FFI exports for your extension:

```rust
neomind_extension_sdk::neomind_export!(YourExtension);
```

This generates:
- `neomind_extension_abi_version()`
- `neomind_extension_metadata()`
- `neomind_extension_create()`
- `neomind_extension_destroy()`

## Common Patterns

### Using OnceLock for Static Data

```rust
fn metadata(&self) -> &ExtensionMetadata {
    static META: std::sync::OnceLock<ExtensionMetadata> = std::sync::OnceLock::new();
    META.get_or_init(|| {
        ExtensionMetadata {
            id: "your-extension-v2".to_string(),
            name: "Your Extension".to_string(),
            version: Version::parse("2.0.0").unwrap(),
            // ...
        }
    })
}
```

### Atomic Counters for Metrics

```rust
use std::sync::atomic::{AtomicI64, Ordering};

pub struct MyExtension {
    counter: AtomicI64,
}

impl MyExtension {
    fn increment(&self) {
        self.counter.fetch_add(1, Ordering::SeqCst);
    }

    fn get_count(&self) -> i64 {
        self.counter.load(Ordering::SeqCst)
    }
}
```

### Mutex for Shared State

```rust
use std::sync::Mutex;

pub struct MyExtension {
    cache: Mutex<HashMap<String, Data>>,
}

impl MyExtension {
    fn update_cache(&self, key: String, value: Data) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(key, value);
    }
}
```

### Async Command with HTTP Request

```rust
async fn execute_command(&self, command: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
    match command {
        "fetch_data" => {
            let url = args.get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ExtensionError::InvalidArguments("Missing url".to_string()))?;

            // Use async HTTP client
            let response = reqwest::get(url)
                .await
                .map_err(|e| ExtensionError::NetworkError(e.to_string()))?;

            let data = response.json::<serde_json::Value>()
                .await
                .map_err(|e| ExtensionError::NetworkError(e.to_string()))?;

            Ok(data)
        }
        _ => Err(ExtensionError::CommandNotFound(command.to_string())),
    }
}
```

## Dependencies

### Common Dependencies

```toml
[dependencies]
# Required
neomind-extension-sdk = { path = "path/to/sdk" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
tokio = { version = "1", features = ["rt", "sync"] }
semver = "1"

# Optional but common
chrono = "0.4"                    # Timestamps
reqwest = "0.11"                  # HTTP client (async)
ureq = { version = "2" }          # HTTP client (sync)
```

### Choosing HTTP Client

**Use `ureq` (sync)** for simple requests to avoid Tokio runtime issues:

```rust
let response = ureq::get(url)
    .call()
    .map_err(|e| ExtensionError::NetworkError(e.to_string()))?;
```

**Use `reqwest` (async)** for complex async workflows:

```rust
let response = reqwest::get(url)
    .await
    .map_err(|e| ExtensionError::NetworkError(e.to_string()))?;
```
