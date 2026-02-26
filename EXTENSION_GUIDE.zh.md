# NeoMind 扩展开发指南 V2

本指南解释如何使用 **V2 扩展 API** 为 NeoMind 边缘 AI 平台开发、构建和安装扩展。

[English Documentation](EXTENSION_GUIDE.md)

---

## 目录

1. [概述](#概述)
2. [V2 扩展架构](#v2-扩展架构)
3. [项目结构](#项目结构)
4. [核心概念](#核心概念)
5. [V2 API 参考](#v2-api-参考)
6. [构建和安装](#构建和安装)
7. [测试](#测试)
8. [最佳实践](#最佳实践)
9. [Dashboard 组件（前端）](#dashboard-组件前端)
10. [从 V1 迁移](#从-v1-迁移)

---

## 概述

NeoMind 扩展是动态库（`.dylib`、`.so`、`.dll`），用于扩展平台功能。扩展可以提供：

| 功能 | 描述 |
|-----------|-------------|
| **指标** | 时间序列数据点（例如：温度、湿度） |
| **命令** | 供 AI 智能体和直接 API 调用的 RPC 风格命令 |

### V2 主要变化

| V1 | V2 |
|----|----|
| `capabilities()` 方法 | 分离的 `metrics()` 和 `commands()` 方法 |
| `execute_tool()` | `execute_command()`（异步） |
| ABI 版本 1 | ABI 版本 2 |
| `neomind-extension-sdk` | `neomind-core::extension::system` |

---

## 扩展类型：原生 vs WASM

NeoMind 支持两种类型的扩展，各有不同优势：

### 原生扩展（.dylib / .so / .dll）

原生扩展是通过 FFI（外部函数接口）加载的平台特定动态库。

**优点：**
- 最高性能（无运行时开销）
- 完整系统访问（文件系统、网络、硬件）
- 通过 C FFI 广泛支持多种语言（Rust、C、C++、Go 等）

**缺点：**
- 必须为每个平台分别编译
- 跨平台分发的构建设置复杂
- 安全问题（完整系统访问）

### WASM 扩展（.wasm）

WASM 扩展是在使用 Wasmtime 的沙箱环境中运行的 WebAssembly 模块。

**优点：**
- **一次编写，到处运行**——单个二进制文件适用于所有平台
- 沙箱执行（安全、可控的资源访问）
- 小文件大小（通常 < 100KB）
- 多语言支持（Rust、AssemblyScript、Go、C/C++）

**缺点：**
- 约 10-30% 的性能开销
- 系统访问受限（仅通过主机 API）
- 需要 WASM 目标（`wasm32-wasi`）

### 如何选择？

| 使用场景 | 推荐类型 |
|----------|------------------|
| 生产分发 | WASM（跨平台） |
| 性能关键型操作 | 原生 |
| 学习/开发 | 原生（更容易调试） |
| 不受信任的扩展 | WASM（沙箱） |
| 硬件/OS 集成 | 原生 |

---

## V2 扩展架构

```
┌─────────────────────────────────────────────────────────┐
│                    NeoMind 服务器                        │
├─────────────────────────────────────────────────────────┤
│  扩展注册表                                              │
│  ├─ 发现（扫描 ~/.neomind/extensions）                  │
│  ├─ 加载（dlopen + 符号解析）                           │
│  └─ 生命周期管理                                         │
├─────────────────────────────────────────────────────────┤
│  扩展安全（V2）                                           │
│  ├─ 熔断器（5 次失败 → 打开）                            │
│  ├─ Panic 隔离（catch_unwind）                           │
│  └─ 健康监控                                             │
├─────────────────────────────────────────────────────────┤
│  您的扩展（动态库）                                       │
│  └─ 实现 Extension Trait（V2）                          │
│      ├─ metadata()                                       │
│      ├─ metrics() → &[MetricDescriptor]                  │
│      ├─ commands() → &[ExtensionCommand]                 │
│      ├─ execute_command()（异步）                        │
│      ├─ produce_metrics()                                │
│      └─ health_check()（异步）                           │
└─────────────────────────────────────────────────────────┘
```

---

## 项目结构

```
my-extension/
├── Cargo.toml          # 包配置
├── build.rs            # 可选的构建脚本
├── src/
│   └── lib.rs          # 扩展实现
└── README.md           # 扩展文档
```

### 最小 Cargo.toml

```toml
[package]
name = "neomind-my-extension"
version = "0.1.0"
edition = "2021"

[lib]
name = "neomind_extension_my_extension"
crate-type = ["cdylib"]   # 重要：生成动态库

[dependencies]
# 使用 NeoMind 项目中的 neomind-core
neomind-core = { path = "../NeoMind/crates/neomind-core" }
neomind-devices = { path = "../NeoMind/crates/neomind-devices" }

serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
once_cell = "1.19"   # 用于静态指标/命令
semver = "1.0"       # 用于版本元数据
tokio = { version = "1", features = ["sync", "rt-multi-thread", "macros"] }
```

### ⚠️ 关键：Panic 设置

**扩展必须使用 `panic = "unwind"` 编译（不能是 "abort"）**

这是**必需**的，以便主服务器能够安全地捕获扩展的 panic。如果您的扩展使用 `panic = "abort"`，任何 panic 都会立即终止整个 NeoMind 服务器进程！

#### 工作区 Cargo.toml 示例

```toml
[workspace]
members = ["extensions/*"]

# 关键：必须与主项目的 panic 设置匹配
[profile.release]
opt-level = 3
lto = "thin"
panic = "unwind"  # 不是 "abort" - 安全性要求！
```

#### 为什么这很重要

| 设置 | 行为 | 安全性 |
|---------|----------|--------|
| `panic = "unwind"` | Panic 展开栈，可以被捕获 | ✅ 服务器继续运行 |
| `panic = "abort"` | Panic 立即终止进程 | ❌ 服务器崩溃 |

NeoMind 服务器使用 `std::panic::catch_unwind()` 包装所有 FFI 调用，以隔离扩展 panic。这只有在扩展使用 `panic = "unwind"` 编译时才有效。

---

## 核心概念

### 1. Extension Trait（V2）

每个扩展必须实现来自 `neomind_core::extension::system` 的 `Extension` trait：

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

// 重要：使用 #[async_trait::async_trait] 宏
#[async_trait::async_trait]
impl Extension for MyExtension {
    // 返回元数据引用（非拥有值）
    fn metadata(&self) -> &ExtensionMetadata {
        &self.metadata
    }

    // 返回指标描述符切片（使用 static 避免生命周期问题）
    fn metrics(&self) -> &[MetricDescriptor] {
        &METRICS
    }

    // 返回命令描述符切片（使用 static 避免生命周期问题）
    fn commands(&self) -> &[ExtensionCommand] {
        &COMMANDS
    }

    // 异步命令执行
    async fn execute_command(&self, command: &str, args: &Value) -> Result<Value> {
        match command {
            "my_command" => {
                // 处理命令
                Ok(json!({ "result": "success" }))
            }
            _ => Err(ExtensionError::CommandNotFound(command.to_string())),
        }
    }

    // 同步指标生成（dylib 兼容性）
    fn produce_metrics(&self) -> Result<Vec<ExtensionMetricValue>> {
        Ok(vec![
            ExtensionMetricValue {
                name: "my_metric".to_string(),
                value: ParamMetricValue::Float(42.0),
                timestamp: current_timestamp(),
            },
        ])
    }

    // 异步健康检查
    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}
```

### 2. 静态指标和命令

**关键**：使用 `once_cell::sync::Lazy` 或 `lazy_static` 来避免生命周期问题：

```rust
use once_cell::sync::Lazy;

/// 静态指标描述符
static METRICS: Lazy<[MetricDescriptor; 1]> = Lazy::new(|| [
    MetricDescriptor {
        name: "temperature_c".to_string(),
        display_name: "温度".to_string(),
        data_type: MetricDataType::Float,
        unit: "°C".to_string(),
        min: Some(-50.0),
        max: Some(60.0),
        required: false,
    },
]);

/// 静态命令描述符
static COMMANDS: Lazy<[ExtensionCommand; 1]> = Lazy::new(|| [
    ExtensionCommand {
        name: "query_weather".to_string(),
        display_name: "查询天气".to_string(),
        payload_template: r#"{"city": "{{city}}"}"#.to_string(),
        parameters: vec![],
        fixed_values: Default::default(),
        samples: vec![],
        llm_hints: "查询任意城市的当前天气。".to_string(),
        parameter_groups: vec![],
    },
]);
```

### 3. 错误处理

```rust
pub enum ExtensionError {
    NotFound(String),           // 资源未找到
    InvalidInput(String),       // 无效参数
    ExecutionFailed(String),    // 运行时错误
    CommandNotFound(String),    // 未知命令（V2）
    IoError(String),            // I/O 错误
    Serialization(String),      // JSON 序列化错误
}
```

---

## V2 API 参考

### 扩展元数据

```rust
pub struct ExtensionMetadata {
    pub id: String,                  // 唯一 ID（例如："my.company.extension"）
    pub name: String,                // 显示名称
    pub version: semver::Version,    // SemVer 版本（不是 String！）
    pub description: Option<String>, // 简短描述
    pub author: Option<String>,      // 作者名称
    pub homepage: Option<String>,    // 项目 URL
    pub license: Option<String>,     // 许可证标识符
    pub file_path: Option<String>,   // 由加载器设置
}
```

### 指标描述符

```rust
pub struct MetricDescriptor {
    pub name: String,           // 指标 ID（例如："temperature_c"）
    pub display_name: String,   // 显示名称
    pub data_type: MetricDataType,  // Float、Integer、String、Boolean
    pub unit: String,           // 单位（例如："°C"、"%"、"km/h"）
    pub min: Option<f64>,       // 最小值
    pub max: Option<f64>,       // 最大值
    pub required: bool,         // 此指标是否必需
}

pub enum MetricDataType {
    Float,
    Integer,
    String,
    Boolean,
}
```

### 扩展命令

```rust
pub struct ExtensionCommand {
    pub name: String,                  // 命令 ID
    pub display_name: String,          // 显示名称
    pub payload_template: String,      // 参数的 JSON 模板
    pub parameters: Vec<Parameter>,    // 参数定义
    pub fixed_values: HashMap<String, Value>,  // 固定参数值
    pub samples: Vec<Value>,           // 示例输入
    pub llm_hints: String,             // AI 智能体提示
    pub parameter_groups: Vec<ParameterGroup>,  // 参数组
}
```

### Extension Trait 方法（V2）

| 方法 | 返回类型 | 异步 | 描述 |
|--------|-------------|-------|-------------|
| `metadata()` | `&ExtensionMetadata` | 否 | 返回扩展元数据引用 |
| `metrics()` | `&[MetricDescriptor]` | 否 | 列出所有提供的指标 |
| `commands()` | `&[ExtensionCommand]` | 否 | 列出所有支持的命令 |
| `execute_command()` | `Result<Value>` | 是 | 执行命令 |
| `produce_metrics()` | `Result<Vec<ExtensionMetricValue>>` | 否 | 返回当前指标值 |
| `health_check()` | `Result<bool>` | 是 | 健康检查 |

---

## FFI 导出（必需）

扩展必须导出这些 C 兼容函数：

```rust
use std::ffi::CString;
use std::sync::RwLock;

/// ABI 版本（V2 必须为 2）
#[no_mangle]
pub extern "C" fn neomind_extension_abi_version() -> u32 {
    ABI_VERSION  // = 2
}

/// C 兼容元数据
#[no_mangle]
pub extern "C" fn neomind_extension_metadata() -> CExtensionMetadata {
    use std::ffi::CStr;

    let id = CString::new("my.extension").unwrap();
    let name = CString::new("我的扩展").unwrap();
    let version = CString::new("0.1.0").unwrap();
    let description = CString::new("做一些有用的事情").unwrap();
    let author = CString::new("您的名字").unwrap();

    CExtensionMetadata {
        abi_version: ABI_VERSION,
        id: id.as_ptr(),
        name: name.as_ptr(),
        version: version.as_ptr(),
        description: description.as_ptr(),
        author: author.as_ptr(),
        metric_count: 1,   // 指标数量
        command_count: 1,  // 命令数量
    }
}

/// 创建扩展实例
#[no_mangle]
pub extern "C" fn neomind_extension_create(
    config_json: *const u8,
    config_len: usize,
) -> *mut RwLock<Box<dyn Extension>> {
    // 解析配置
    let config = if config_json.is_null() || config_len == 0 {
        serde_json::json!({})
    } else {
        unsafe {
            let slice = std::slice::from_raw_parts(config_json, config_len);
            let s = std::str::from_utf8_unchecked(slice);
            serde_json::from_str(s).unwrap_or(serde_json::json!({}))
        }
    };

    // 创建扩展
    match MyExtension::new(&config) {
        Ok(ext) => {
            let boxed: Box<dyn Extension> = Box::new(ext);
            let wrapped = Box::new(RwLock::new(boxed));
            Box::into_raw(wrapped)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// 销毁扩展实例
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

## 构建和安装

### 1. 构建扩展

```bash
cd ~/NeoMind-Extension
cargo build --release
```

输出位置：
- macOS: `target/release/libneomind_extension_my_extension.dylib`
- Linux: `target/release/libneomind_extension_my_extension.so`
- Windows: `target/release/neomind_extension_my_extension.dll`

### 跨平台原生编译

原生扩展是平台特定的。要分发到多个平台，需要为每个平台编译：

#### macOS

```bash
# Apple Silicon (M1/M2/M3)
cargo build --release --target aarch64-apple-darwin

# Intel Mac
cargo build --release --target x86_64-apple-darwin

# 通用二进制（两个架构）
lipo -create \
  target/aarch64-apple-darwin/release/libneomind_extension_my_extension.dylib \
  target/x86_64-apple-darwin/release/libneomind_extension_my_extension.dylib \
  -output target/universal/libneomind_extension_my_extension.dylib
```

#### Linux

```bash
# x86_64（最常见）
cargo build --release --target x86_64-unknown-linux-gnu

# ARM64（树莓派、服务器）
cargo build --release --target aarch64-unknown-linux-gnu

# 从 macOS 交叉编译（需要 cross）
cargo install cross
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target aarch64-unknown-linux-gnu
```

#### Windows

```bash
# 从 Linux/macOS（需要 cross）
cross build --release --target x86_64-pc-windows-gnu

# 从 Windows 直接构建
cargo build --release --target x86_64-pc-windows-msvc
```

#### 使用 `cross` 进行交叉编译

跨平台编译最简单的方法是使用 `cross`：

```bash
# 安装 cross
cargo install cross

# 安装 Docker（cross 需要）
# macOS: brew install --cask docker
# Linux: sudo apt install docker.io

# 为所有平台构建
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target x86_64-pc-windows-gnu
```

#### 交叉编译总结表

| 平台 | 目标三元组 | 交叉编译 | 原生编译 |
|------|-----------|----------|---------|
| macOS ARM | `aarch64-apple-darwin` | ❌ | ✅ |
| macOS Intel | `x86_64-apple-darwin` | ✅ (从 ARM Mac) | ✅ |
| Linux x86 | `x86_64-unknown-linux-gnu` | ✅ (cross) | ✅ |
| Linux ARM | `aarch64-unknown-linux-gnu` | ✅ (cross) | ✅ |
| Windows | `x86_64-pc-windows-gnu` | ✅ (cross) | ✅ |

### 2. 安装扩展

```bash
# 如果不存在则创建扩展目录
mkdir -p ~/.neomind/extensions

# 复制编译的扩展
cp target/release/libneomind_extension_my_extension.* ~/.neomind/extensions/
```

### 3. 验证安装

```bash
# 通过 API 列出已加载的扩展
curl http://localhost:9375/api/extensions

# 检查特定扩展的健康状态
curl http://localhost:9375/api/extensions/my.extension/health
```

---

## 测试

### 单元测试示例

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

### 运行测试

```bash
cargo test
```

---

## 最佳实践

### 1. Panic 安全

**永远不要让 panic 从扩展中逃逸！**

NeoMind 提供多层安全机制来保护服务器免受扩展错误影响：

```
┌─────────────────────────────────────────────────────┐
│ 安全层 1: panic = "unwind" (编译时要求)             │
├─────────────────────────────────────────────────────┤
│ 安全层 2: catch_unwind (FFI 调用保护)              │
├─────────────────────────────────────────────────────┤
│ 安全层 3: Circuit Breaker (5次失败 → 熔断)         │
├─────────────────────────────────────────────────────┤
│ 安全层 4: Panic Tracking (3次panic → 禁用)         │
├─────────────────────────────────────────────────────┤
│ 安全层 5: Timeout (30秒命令超时)                    │
└─────────────────────────────────────────────────────┘
```

**⚠️ 关键要求：扩展必须使用 `panic = "unwind"`（不是 "abort"）**

```rust
async fn execute_command(&self, command: &str, args: &Value) -> Result<Value> {
    // 不好：unwrap() 会 panic
    // let city = args.get("city").unwrap().as_str().unwrap();

    // 好：正确处理错误
    let city = args.get("city")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtensionError::InvalidInput("需要 city 参数".to_string()))?;
    // ...
}
```

### 2. HTTP 客户端选择

**重要**：在原生扩展中进行 HTTP 请求时：

- **使用 `ureq`**（阻塞客户端）用于简单的 HTTP 调用
- **避免使用 `reqwest`**，因为它会带来 Tokio 运行时依赖

```toml
# 推荐：ureq（阻塞、轻量级）
[dependencies]
ureq = { version = "2.9", features = ["json"] }

# 不推荐用于扩展：reqwest（会带来 Tokio 运行时）
# reqwest = { version = "0.11", features = ["json"] }
```

```rust
// 示例：使用 ureq 进行 HTTP 调用
fn fetch_data(&self, url: &str) -> Result<MyData> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| ExtensionError::ExecutionFailed(format!("HTTP 错误: {}", e)))?;

    let data: MyData = response
        .into_json()
        .map_err(|e| ExtensionError::Serialization(format!("JSON 错误: {}", e)))?;

    Ok(data)
}
```

**为什么选择 ureq？**
- 没有 Tokio 运行时依赖（避免与宿主冲突）
- 更简单的 FFI 边界
- 更小的二进制大小
- 在阻塞上下文中工作良好

### 3. 线程安全

扩展可能被并发调用。使用适当的同步：

```rust
use std::sync::{Arc, RwLock};

pub struct MyExtension {
    state: Arc<RwLock<MyState>>,
}
```

### 4. 静态指标/命令

始终使用 `once_cell::sync::Lazy` 来定义指标和命令：

```rust
// 好：使用 Lazy 的静态定义
static METRICS: Lazy<[MetricDescriptor; 1]> = Lazy::new(|| [
    MetricDescriptor { /* ... */ },
]);

fn metrics(&self) -> &[MetricDescriptor] {
    &METRICS
}

// 不好：返回局部数组引用
fn metrics(&self) -> &[MetricDescriptor] {
    &[
        MetricDescriptor { /* ... */ },  // ❌ 生命周期错误！
    ]
}
```

### 4. 资源清理

```rust
impl Drop for MyExtension {
    fn drop(&mut self) {
        // 清理资源（关闭连接等）
    }
}
```

### 5. 幂等操作

尽可能将命令设计为幂等的：

```rust
// 好：多次调用具有相同效果
async fn refresh(&self) -> Result<Value, ExtensionError> {
    // 带缓存检查的刷新逻辑
}
```

---

## 从 V1 迁移

| V1 API | V2 API |
|--------|--------|
| `use neomind_extension_sdk::prelude::*;` | `use neomind_core::extension::system::*;` |
| `fn metadata() -> ExtensionMetadata` | `fn metadata() -> &ExtensionMetadata` |
| `fn capabilities()` | `fn metrics() + fn commands()` |
| `fn execute_tool()` | `fn execute_command()`（异步） |
| `NEO_EXT_ABI_VERSION = 1` | `ABI_VERSION = 2` |
| `neomind_ext_version()` | `neomind_extension_abi_version()` |
| 返回拥有值 | 返回引用（使用 static） |

### 迁移步骤

1. 更新 `Cargo.toml`：将依赖更改为 `neomind-core`
2. 添加 `async-trait` 和 `once_cell` 依赖
3. 将 `capabilities()` 更改为分离的 `metrics()` 和 `commands()`
4. 将 `execute_tool()` 转换为异步 `execute_command()`
5. 使用 `once_cell::sync::Lazy` 定义静态描述符
6. 更新 FFI 导出名称和 ABI 版本
7. 为 impl 块添加 `#[async_trait::async_trait]`

---

## 完整示例

参见 `extensions/weather-forecast/` 目录获取完整的工作示例：

```bash
cat ~/NeoMind-Extension/extensions/weather-forecast/src/lib.rs
```

此扩展展示：
- 使用 `once_cell::sync::Lazy` 的静态指标和命令
- 异步命令执行
- 指标生成
- 健康检查
- 正确的 FFI 导出

---

## WASM 扩展开发

WASM 扩展提供跨平台兼容性，单个构建工件即可。它们在使用 Wasmtime 的沙箱环境中运行。

### WASM 目标系统

NeoMind 支持多种 WASM 目标，适用于不同用例：

| 目标 | 描述 | 用例 |
|------|------|------|
| `wasm32-wasi` | WASI（WebAssembly 系统接口） | **推荐** - 完整系统访问、文件 I/O、网络 |
| `wasm32-unknown-unknown` | 纯 WebAssembly | 仅浏览器，无系统访问 |

> **注意**：NeoMind 主要使用 `wasm32-wasi` 作为扩展，因为它提供了物联网和自动化所需的系统级功能。

### 安装 WASM 目标

```bash
# 安装 WASI 目标（推荐）
rustup target add wasm32-wasi

# 安装纯 WebAssembly 目标（可选）
rustup target add wasm32-unknown-unknown

# 验证安装
rustup target list --installed | grep wasm
```

### 跨平台 WASM 编译

WASM 的主要优势之一是 **一次编写，到处运行**。单个 `.wasm` 文件可在以下平台运行：
- macOS（Apple Silicon 和 Intel）
- Linux（x86_64 和 ARM64）
- Windows（x86_64）
- Web 浏览器

```bash
# 构建 WASM 扩展（适用于所有平台！）
cargo build --release --target wasm32-wasi

# 输出：target/wasm32-wasi/release/my_extension.wasm
```

### WASM 构建配置

#### WASM 的 Cargo.toml

```toml
[package]
name = "my-wasm-extension"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # WASM 必需

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 可选：WASM 特定优化
[profile.release]
opt-level = "z"      # 优化大小
lto = true           # 链接时优化
codegen-units = 1    # 更好的优化
panic = "abort"      # 更小的二进制文件（WASM 不支持展开）
strip = true         # 剥离符号
```

#### WASM 的 .cargo/config.toml

创建 `.cargo/config.toml` 进行 WASM 特定设置：

```toml
[target.wasm32-wasi]
# 如果安装了 WASI SDK（可选，用于更好的优化）
# rustflags = ["-C", "linker=clang"]

[build]
# 此扩展的默认目标
# target = "wasm32-wasi"
```

### WASI 系统接口

WASI（WebAssembly 系统接口）提供对系统资源的访问：

| 功能 | WASI 支持 | 说明 |
|------|-----------|------|
| 文件 I/O | ✅ 完整 | 在沙箱目录中读写文件 |
| 网络 | ✅ 部分 | 通过宿主 API 的 HTTP 客户端 |
| 环境变量 | ✅ 完整 | 读取环境变量 |
| 参数 | ✅ 完整 | 命令行参数 |
| 随机数 | ✅ 完整 | 加密安全随机数生成器 |
| 时钟 | ✅ 完整 | 系统时间访问 |
| 线程 | ⚠️ 有限 | 某些运行时中为实验性功能 |

### WASM 扩展结构

```
my-wasm-extension/
├── Cargo.toml          # 包配置
├── my-extension.json   # 元数据 sidecar 文件
├── README.md           # 扩展文档
└── src/
    └── lib.rs          # WASM 扩展实现
```

### WASM 最小 Cargo.toml

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

### WASM 扩展实现

WASM 扩展使用与原生扩展不同的方法：

```rust
use serde::Serialize;

/// 简单指标值
#[derive(Debug, Clone, Serialize)]
struct MetricValue {
    name: String,
    value: f64,
    unit: String,
}

/// 命令响应
#[derive(Debug, Clone, Serialize)]
struct CommandResponse {
    success: bool,
    message: String,
    data: Option<MetricValue>,
}

/// 获取指标值
#[no_mangle]
pub extern "C" fn get_my_metric() -> f64 {
    42.0
}

/// 处理命令并返回 JSON 响应
#[no_mangle]
pub extern "C" fn neomind_execute(
    command_ptr: *const u8,
    _args_ptr: *const u8,
    result_buf_ptr: *mut u8,
    result_buf_len: usize,
) -> usize {
    // 读取命令字符串（null 终止）
    let command = unsafe {
        let mut len = 0;
        while *command_ptr.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(command_ptr, len)
    };

    let command_str = std::str::from_utf8(command).unwrap_or("unknown");

    // 匹配命令并生成响应
    let response = match command_str {
        "get_metric" => CommandResponse {
            success: true,
            message: "指标已获取".to_string(),
            data: Some(MetricValue {
                name: "my_metric".to_string(),
                value: get_my_metric(),
                unit: "count".to_string(),
            }),
        },
        "hello" => CommandResponse {
            success: true,
            message: "来自 WASM 的问候！".to_string(),
            data: None,
        },
        _ => CommandResponse {
            success: false,
            message: format!("未知命令：{}", command_str),
            data: None,
        },
    };

    // 序列化为 JSON 并写入缓冲区
    let json = serde_json::to_string(&response).unwrap_or_default();
    let json_bytes = json.as_bytes();
    let write_len = std::cmp::min(json_bytes.len(), result_buf_len.saturating_sub(1));

    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), result_buf_ptr, write_len);
        *result_buf_ptr.add(write_len) = 0; // Null 终止
    }

    write_len
}

/// 健康检查
#[no_mangle]
pub extern "C" fn health() -> i32 {
    1 // 1 = 健康
}
```

### 元数据 Sidecar 文件

WASM 扩展使用 JSON sidecar 文件存储元数据：

```json
{
    "id": "my-wasm-extension",
    "name": "我的 WASM 扩展",
    "version": "0.1.0",
    "description": "一个跨平台 WASM 扩展",
    "author": "您的名字",
    "homepage": "https://github.com/your/repo",
    "license": "MIT",
    "metrics": [
        {
            "name": "my_metric",
            "display_name": "我的指标",
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
            "display_name": "获取指标",
            "description": "获取当前指标值",
            "payload_template": "{}",
            "parameters": [],
            "fixed_values": {},
            "samples": [],
            "llm_hints": "返回当前指标值",
            "parameter_groups": []
        },
        {
            "name": "hello",
            "display_name": "问候",
            "description": "从 WASM 打招呼",
            "payload_template": "{}",
            "parameters": [],
            "fixed_values": {},
            "samples": [],
            "llm_hints": "返回问候消息",
            "parameter_groups": []
        }
    ]
}
```

### 构建 WASM 扩展

```bash
# 安装 WASM 目标
rustup target add wasm32-wasi

# 构建 WASM
cargo build --release --target wasm32-wasi

# 输出：target/wasm32-wasi/release/my_wasm_extension.wasm
```

### 安装 WASM 扩展

```bash
# 复制两个文件到扩展目录
mkdir -p ~/.neomind/extensions
cp target/wasm32-wasi/release/my_wasm_extension.wasm ~/.neomind/extensions/
cp my-extension.json ~/.neomind/extensions/

# 重启 NeoMind 或使用扩展发现
```

### WASM vs 原生：关键区别

| 方面 | 原生 | WASM |
|--------|--------|------|
| **元数据** | 通过 FFI 嵌入代码中 | 单独的 JSON 文件 |
| **导出** | `neomind_extension_*` 函数 | `neomind_execute`、`health`、自定义函数 |
| **状态** | 可使用 `Arc<RwLock<T>>` | 仅限于 WASM 内存 |
| **系统访问** | 完整访问 | 仅通过主机 API |
| **分发** | 平台特定二进制文件 | 所有平台的单个 `.wasm` 文件 |

### WASM 最佳实践

1. **保持简单**：WASM 扩展最适合简单操作
2. **避免外部依赖**：许多 crate 不支持 `wasm32-wasi`
3. **使用 JSON 作为响应**：沙箱处理 JSON 序列化
4. **在目标平台上测试**：WASM 行为可能与原生不同
5. **提供良好的元数据**：JSON 文件是主要文档

### WASM 主机 API

WASM 扩展可以通过 NeoMind 沙箱提供的主机函数访问外部资源。当前支持：

| 主机函数 | 描述 | 参数 | 返回值 |
|---------|------|------|--------|
| `host_http_request` | 发起 HTTP 请求 | method, url | response JSON |

#### 从 WASM 发起 HTTP 请求

WASM 扩展可以使用 `host_http_request` 主机函数发起 HTTP 请求：

```rust
// 导入主机函数
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

// 辅助函数：发起 HTTP GET 请求
fn http_get(url: &str) -> Result<String, String> {
    let method = b"GET";
    let url_bytes = url.as_bytes();
    let mut result_buffer = vec![0u8; 65536]; // 64KB 缓冲区

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
        return Err("HTTP 请求失败".to_string());
    }

    // 查找空终止符
    let end = result_buffer.iter().position(|&b| b == 0).unwrap_or(result_len as usize);
    String::from_utf8(result_buffer[..end].to_vec())
        .map_err(|e| format!("无效的 UTF-8 响应: {}", e))
}
```

#### 声明权限

发起 HTTP 请求的扩展必须在其 manifest 中声明允许的 URL：

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

### 示例：天气预报 WASM 扩展

参见 `extensions/weather-forecast-wasm/` 获取完整的 WASM 扩展示例，它：
- 使用 HTTP 主机 API 获取天气数据
- 调用 Open-Meteo 的免费地理编码和天气 API
- 演示 WASM 中的正确错误处理
- 展示如何解析 JSON 响应

```bash
# 构建 WASM 扩展
cd extensions/weather-forecast-wasm
cargo build --target wasm32-unknown-unknown --release

# 输出: weather_forecast_wasm.wasm (~170 KB)
```

### 示例：wasm-hello 扩展（Rust）

参见 `extensions/wasm-hello/` 目录获取完整的工作示例：

```bash
cat ~/NeoMind-Extension/extensions/wasm-hello/src/lib.rs
cat ~/NeoMind-Extension/extensions/wasm-hello/metadata.json
```

---

### AssemblyScript WASM 扩展

AssemblyScript 是一种类似 TypeScript 的语言，可编译为 WebAssembly。它非常适合希望创建编译时间快的 WASM 扩展的 JavaScript/TypeScript 开发者。

#### 为什么选择 AssemblyScript？

| 特性 | AssemblyScript | Rust WASM |
|---------|----------------|-----------|
| **语法** | 类 TypeScript | Rust |
| **学习曲线** | 低（适合 JS/TS 开发者） | 高 |
| **编译时间** | ~1s | ~5s |
| **二进制大小** | ~15 KB | ~50 KB |
| **性能** | 原生性能的 90-95% | 原生性能的 85-90% |

#### 项目结构

```
my-as-extension/
├── package.json          # npm 依赖
├── asconfig.json         # AssemblyScript 编译器配置
├── metadata.json         # 扩展元数据 sidecar
├── README.md             # 扩展文档
└── assembly/
    └── extension.ts      # AssemblyScript 源代码
```

#### 最小 package.json

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

#### AssemblyScript 实现

```typescript
// ABI 版本 - V2 API 必须为 2
const ABI_VERSION: u32 = 2;

// 扩展状态
let counter: i32 = 0;

// 导出 ABI 版本
export function neomind_extension_abi_version(): u32 {
  return ABI_VERSION;
}

// 获取指标值
export function get_counter(): i32 {
  return counter;
}

// 增加计数器
export function increment_counter(): i32 {
  counter = counter + 1;
  return counter;
}

// 执行命令
export function neomind_execute(
  command_ptr: usize,
  args_ptr: usize,
  result_buf_ptr: usize,
  result_buf_len: usize
): usize {
  // 读取命令字符串
  const command = getString(command_ptr);

  // 根据命令生成响应
  let response: string;

  switch (command) {
    case "get_counter":
      response = JSON.stringify({
        success: true,
        message: "计数器已获取",
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
        message: "计数器已增加",
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
        message: "未知命令",
        error: `命令 '${command}' 未找到`
      });
      break;
  }

  // 将响应写入缓冲区
  return writeString(result_buf_ptr, result_buf_len, response);
}

// 健康检查
export function health(): i32 {
  return 1; // 1 = 健康
}

// 辅助函数：从内存读取 null 终止字符串
@inline
function getString(ptr: usize): string {
  if (ptr === 0) return "";

  let len = 0;
  while (load<u8>(ptr + len) !== 0) {
    len++;
  }

  return String.UTF8.decode(ptr, len);
}

// 辅助函数：向内存缓冲区写入字符串
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

#### 构建 AssemblyScript 扩展

```bash
# 安装依赖
cd ~/NeoMind-Extension/extensions/as-hello
npm install

# 构建扩展
npm run build

# 输出：build/as-hello.wasm
```

#### 安装 AssemblyScript 扩展

```bash
# 复制两个文件到扩展目录
mkdir -p ~/.neomind/extensions
cp build/as-hello.wasm ~/.neomind/extensions/
cp metadata.json ~/.neomind/extensions/as-hello.json

# 重启 NeoMind 或使用扩展发现
```

### 示例：as-hello 扩展（AssemblyScript）

参见 `extensions/as-hello/` 目录获取完整的工作示例：

```bash
cat ~/NeoMind-Extension/extensions/as-hello/assembly/extension.ts
cat ~/NeoMind-Extension/extensions/as-hello/metadata.json
```

此扩展展示：
- WASM 的类 TypeScript 语法
- 快速编译时间（~1s）
- 小二进制大小（~15 KB）
- 指标导出和命令执行
- 健康检查实现

---

## 故障排除

| 问题 | 解决方案 |
|-------|----------|
| 扩展未加载 | 检查 ABI 版本返回 2，而不是 1 |
| metrics()/commands() 中的生命周期错误 | 对静态数据使用 `once_cell::sync::Lazy` |
| 加载时 panic | 确保没有 panic 在 `new()` 构造函数中 |
| 命令失败 | 检查熔断器是否因失败而打开 |
| 指标未出现 | 验证 `produce_metrics()` 返回有效数据 |
| 错误的 SDK 引用 | 使用 `neomind-core`，而不是 `neomind-extension-sdk` |

### WASM 特定问题

| 问题 | 解决方案 |
|-------|----------|
| WASM 扩展未加载 | 确保 **两个** 文件 `.wasm` 和 `.json` 都存在 |
| 找不到 `wasm32-wasi` 目标 | 运行 `rustup target add wasm32-wasi` |
| 依赖无法为 WASM 编译 | 某些 crate 不支持 `wasm32-wasi`；检查兼容性 |
| 扩展加载但命令失败 | 检查函数名称完全匹配（区分大小写） |
| JSON 元数据未识别 | 验证 JSON 有效且匹配架构 |
| 指标未显示 | 确保 JSON 中的 `data_type` 有效（`integer`、`float`、`string`、`boolean`） |

---

## Dashboard 组件（前端）

扩展可以提供自定义的 **Dashboard 组件**，在 NeoMind Web UI 中渲染。这些是基于 React 的组件，打包为 IIFE（立即调用函数表达式）模块。

### 概述

```
┌─────────────────────────────────────────────────────────┐
│                    NeoMind Dashboard                     │
├─────────────────────────────────────────────────────────┤
│  组件库                                                  │
│  ├─ 内置组件（仪表盘、图表、状态等）                     │
│  └─ 扩展组件（动态加载）                                 │
├─────────────────────────────────────────────────────────┤
│  扩展组件加载                                            │
│  ├─ manifest.json 定义 dashboard_components             │
│  ├─ 前端包通过脚本注入加载                               │
│  └─ 组件使用配置中的 props 渲染                          │
└─────────────────────────────────────────────────────────┘
```

### 项目结构

```
my-extension/
├── Cargo.toml                    # Rust 后端
├── src/lib.rs                    # 扩展实现
├── manifest.json                 # 扩展元数据 + dashboard 组件
└── frontend/                     # 前端 dashboard 组件
    ├── package.json              # npm 依赖
    ├── vite.config.ts            # Vite 构建配置
    └── src/
        └── index.tsx             # React 组件
```

### 1. manifest.json 配置

在 `manifest.json` 中添加 `dashboard_components` 数组：

```json
{
  "id": "my.extension",
  "name": "我的扩展",
  "version": "0.1.0",
  "main": "libneomind_extension_my_extension.dylib",
  "dashboard_components": [
    {
      "type": "my-component",
      "name": "我的组件",
      "description": "自定义 dashboard 组件",
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
            "description": "自动刷新间隔（毫秒）",
            "default": 30000,
            "minimum": 0,
            "maximum": 3600000
          },
          "showDetails": {
            "type": "boolean",
            "description": "显示详细信息",
            "default": true
          }
        }
      },
      "default_config": {
        "refreshInterval": 30000,
        "showDetails": true
      }
    }
  ]
}
```

### 2. 前端组件实现

在 `frontend/src/index.tsx` 中创建 React 组件：

```tsx
/**
 * 我的扩展 Dashboard 组件
 */
import { forwardRef, useEffect, useState, useCallback } from 'react'

// 数据源接口（由 dashboard 提供）
export interface DataSource {
  type: string
  extensionId?: string
  [key: string]: any
}

// 组件 props 接口
export interface MyComponentProps {
  title?: string
  dataSource?: DataSource
  className?: string
  editMode?: boolean
  // 从 config_schema 来的配置 props
  refreshInterval?: number
  showDetails?: boolean
}

// 扩展 ID 用于 API 调用
const EXTENSION_ID = 'my.extension'

export const MyComponent = forwardRef<HTMLDivElement, MyComponentProps>(
  function MyComponent(props, ref) {
    const {
      title = '我的组件',
      dataSource,
      className = '',
      refreshInterval = 30000,
      showDetails = true
    } = props

    const [data, setData] = useState<any>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    const extensionId = dataSource?.extensionId || EXTENSION_ID

    // 从扩展获取数据
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
          throw new Error(result.error || '获取数据失败')
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : '未知错误')
      } finally {
        setLoading(false)
      }
    }, [extensionId])

    // 初始获取和自动刷新
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
          <div className="text-gray-500">加载中...</div>
        )}

        {error && (
          <div className="text-red-500">错误: {error}</div>
        )}

        {data && (
          <div>
            {/* 在此渲染您的数据 */}
            <pre>{JSON.stringify(data, null, 2)}</pre>
          </div>
        )}
      </div>
    )
  }
)

MyComponent.displayName = 'MyComponent'

// 为 IIFE 兼容性导出默认
export default { MyComponent }
```

### 3. package.json 配置

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

### 4. vite.config.ts 配置

**关键**：组件必须构建为 IIFE 包，并将 React 外部化：

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [
    react({
      // 使用经典 JSX 运行时以避免 jsx-runtime 依赖
      jsxRuntime: 'classic'
    })
  ],
  define: {
    // 替换 process.env.NODE_ENV 以避免 "process is not defined" 错误
    'process.env.NODE_ENV': JSON.stringify('production')
  },
  build: {
    lib: {
      entry: './src/index.tsx',
      name: 'MyDashboardComponents',  // 全局变量名
      fileName: () => `my-dashboard-components.js`,
      formats: ['iife']  // IIFE 分配到全局变量
    },
    rollupOptions: {
      // 外部化 React 以使用宿主应用的 React
      external: ['react', 'react-dom'],
      output: {
        // 使用 'named' 导出以获得命名导出可用性
        exports: 'named',
        // 为外部依赖提供全局变量
        globals: {
          'react': 'React',
          'react-dom': 'ReactDOM'
        }
      }
    }
  }
})
```

### 5. 构建和安装

```bash
# 构建 Rust 后端
cd ~/NeoMind-Extension/extensions/my-extension
cargo build --release

# 构建前端组件
cd frontend
npm install
npm run build

# 安装扩展
mkdir -p ~/.neomind/extensions/my-extension
cp ../target/release/libneomind_extension_my_extension.dylib ~/.neomind/extensions/
cp manifest.json ~/.neomind/extensions/my-extension/
cp -r dist ~/.neomind/extensions/my-extension/frontend/

# 重启 NeoMind
```

### 组件 Props 参考

扩展组件从 dashboard 接收这些 props：

| Prop | 类型 | 描述 |
|------|------|------|
| `title` | `string?` | 从 dashboard 配置来的组件标题 |
| `dataSource` | `DataSource?` | 数据源绑定配置 |
| `className` | `string?` | 额外的 CSS 类 |
| `editMode` | `boolean?` | Dashboard 处于编辑模式时为 true |
| `...config` | `any` | 来自 `config_schema` 属性的 props |

### 全局变量命名约定

前端加载器将组件类型映射到全局变量名：

| 组件类型 | 全局变量名 |
|----------------|---------------------|
| `weather-card` | `WeatherDashboardComponents` |
| `image-analyzer` | `ImageAnalyzerDashboardComponents` |
| `yolo-video-display` | `YoloVideoDashboardComponents` |
| 自定义（`my-component`） | `MyDashboardComponents` |

**约定**：`{PascalCase(component-type)}DashboardComponents`

### 完整示例：天气卡片

参见 `extensions/weather-forecast/` 获取完整的工作示例：

```
weather-forecast/
├── Cargo.toml
├── manifest.json                 # 定义 weather-card 组件
├── src/lib.rs                    # 带有 get_weather 命令的 Rust 后端
└── frontend/
    ├── package.json
    ├── vite.config.ts
    └── src/
        └── index.tsx             # WeatherCard React 组件
```

展示的功能：
- 基于天气条件的美丽渐变背景
- 带可编辑输入的城市搜索
- 自动刷新功能
- 温度单位选择
- 响应式加载和错误状态

### Dashboard 组件最佳实践

1. **使用 TypeScript** 获得类型安全和更好的 IDE 支持
2. **外部化 React** 以避免包大小膨胀和版本冲突
3. **优雅处理加载/错误状态**
4. **支持自动刷新** 具有可配置的间隔
5. **使用 Tailwind CSS** 进行样式设计（匹配 NeoMind UI）
6. **保持组件小** - 将复杂 UI 拆分为多个组件
7. **在 Tauri 和 web 环境中测试**

---

## 延伸阅读

- NeoMind 核心：`~/NeoMind/crates/neomind-core/src/extension/system.rs`
- 扩展测试：`~/NeoMind/crates/neomind-core/tests/extension_test.rs`
- 示例扩展：`~/NeoMind-Extension/extensions/`
