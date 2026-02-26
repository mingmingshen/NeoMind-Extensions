# NeoMind 扩展

NeoMind 边缘 AI 平台的官方扩展仓库。

[English Documentation](README.md)

## 项目描述

此仓库包含为 NeoMind 边缘 AI 平台官方维护的扩展集合。

### 扩展分发格式

NeoMind 支持 **三种扩展分发格式**：

| 格式 | 文件扩展名 | 描述 | 推荐用途 |
|------|-------------|-------------|----------|
| **.nep 包** | `.nep` | ZIP 压缩包，包含二进制 + 前端 + 元数据 | **推荐**：完整的扩展分发 |
| **原生二进制** | `.dylib` / `.so` / `.dll` | 平台特定的动态库 | 开发构建、自定义扩展 |
| **WASM 模块** | `.wasm` + `.json` | WebAssembly 模块 | 跨平台但不打包 |

### 为什么选择 .nep 包？

**.nep (NeoMind Extension Package)** 是推荐的分发格式：
- ✅ **单文件** - 包含二进制、前端组件和元数据
- ✅ **多平台** - 一个包包含所有平台的二进制
- ✅ **前端支持** - 可打包仪表板使用的 React 组件
- ✅ **校验和验证** - SHA256 校验确保完整性
- ✅ **简单安装** - NeoMind Web UI 拖放安装

---

## 快速开始：安装扩展

### 方法 1：拖放安装（推荐）

1. 从 [Releases](https://github.com/camthink-ai/NeoMind-Extensions/releases) 下载 `.nep` 包
2. 打开 NeoMind Web UI → 扩展
3. 点击 **添加扩展** → 切换到 **文件模式**
4. 拖放 `.nep` 文件
5. 点击 **上传并安装**

### 方法 2：扩展市场

1. 打开 NeoMind Web UI → 扩展 → 市场
2. 浏览可用扩展
3. 点击任意扩展的 **安装** 按钮
4. 扩展将自动下载并安装

### 方法 3：API 安装

```bash
curl -X POST http://your-neomind:9375/api/extensions/upload/file \
  -H "Content-Type: application/octet-stream" \
  --data-binary @extension-name.nep
```

---

## 扩展开发者指南

### 构建 .nep 包

使用提供的 `package.sh` 脚本构建 .nep 包：

```bash
# 构建单个扩展
bash scripts/package.sh -d extensions/weather-forecast-wasm

# 构建并验证
bash scripts/package.sh -d extensions/weather-forecast-wasm -v

# 仅构建当前平台
bash scripts/package.sh -d extensions/template -p current
```

脚本会：
1. 构建扩展（Rust/Cargo 或使用预构建的 WASM）
2. 创建正确的目录结构
3. 打包二进制、前端组件和元数据
4. 计算 SHA256 校验和
5. 输出：`dist/{扩展id}-{版本}.nep`

### 扩展目录结构

```
extensions/
└── your-extension/
    ├── manifest.json       # 扩展元数据（必需）
    ├── Cargo.toml          # Rust 项目（如果是原生/WASM）
    ├── src/                # 源代码
    │   └── lib.rs
    ├── frontend/           # React 组件（可选）
    │   ├── src/
    │   ├── package.json
    │   └── dist/           # 构建的 JS 文件
    └── README.md           # 文档
```

### manifest.json 格式

```json
{
  "format": "neomind-extension-package",
  "format_version": "1.0",
  "id": "neomind.example.extension",
  "name": "示例扩展",
  "version": "1.0.0",
  "description": "一个示例扩展",
  "author": "您的名字",
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

详细文档请参考 [EXTENSION_GUIDE.zh.md](EXTENSION_GUIDE.zh.md)。

---

## 可用扩展

### 天气预报 (WASM)
- **ID**: `neomind.weather.forecast.wasm`
- **描述**: 使用 Open-Meteo API 的实时天气数据
- **包**: [下载](https://github.com/camthink-ai/NeoMind-Extensions/releases/latest)
- **组件**: 天气卡片仪表板组件

### 更多扩展开发中...

---

## CI/CD

此仓库使用 GitHub Actions 自动构建：

- **推送到 main**：构建已更改的扩展
- **手动触发**：构建特定扩展或全部扩展
- **发布版本**：创建包含 .nep 包的 GitHub Releases

查看 [`.github/workflows/build-nep-packages.yml`](.github/workflows/build-nep-packages.yml)

---

## 贡献

欢迎贡献！请参阅 [CONTRIBUTING.md](CONTRIBUTING.md) 了解指南。

## 许可证

此仓库采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
