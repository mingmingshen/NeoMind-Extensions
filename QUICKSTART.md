# NeoMind 扩展仓库 - 快速开始指南

## .nep 包格式

### 目录结构

```
extension-name-version.nep  (ZIP archive)
├── manifest.json           # 扩展元数据
├── binaries/               # 二进制文件
│   └── wasm/              # 或 darwin_aarch64/, linux_amd64/, windows_amd64/
│       └── extension.wasm # 或 extension.dylib, extension.so, extension.dll
└── frontend/              # 可选：前端组件
    └── dist/
        └── component.js   # React 组件
```

### manifest.json 字段

```json
{
  "format": "neomind-extension-package",
  "format_version": "1.0",
  "id": "neomind.example.extension",
  "name": "扩展名称",
  "version": "1.0.0",
  "description": "扩展描述",
  "author": "作者",
  "license": "MIT",
  "homepage": "https://github.com/...",
  "type": "wasm",  // 或 "native"
  "neomind": {
    "min_version": "0.5.0"
  },
  "binaries": {
    "wasm": "binaries/wasm/extension.wasm"
  },
  "permissions": ["http://api.example.com"],
  "config_parameters": [...],
  "metrics": [...],
  "commands": [...],
  "dashboard_components": [...]
}
```

## 构建命令

### 构建单个扩展
```bash
make package-weather-forecast-wasm
# 或
bash scripts/package.sh -d extensions/weather-forecast-wasm
```

### 构建所有扩展
```bash
make package-all
# 或
bash scripts/build-all.sh
```

### 其他 Make 命令
```bash
make list              # 列出所有扩展
make clean             # 清理构建产物
make test PACKAGE=... # 测试包
```

## 安装方式

### 1. Web UI 上传
1. 下载 `.nep` 文件
2. NeoMind Web UI → 扩展 → 添加扩展 → 文件模式
3. 拖放文件并上传

### 2. API 上传
```bash
curl -X POST http://localhost:9375/api/extensions/upload/file \
  -H "Content-Type: application/octet-stream" \
  --data-binary @extension-name.nep
```

## 文件过滤

以下文件已从 Git 中排除（`.gitignore`）：
- `dist/` - 构建的 .nep 包
- `*.dylib`, `*.so`, `*.dll`, `*.wasm` - 二进制文件
- `target/` - Rust 构建目录
- `node_modules/` - Node.js 依赖
- `models/` - 模型文件（太大）

## CI/CD

GitHub Actions 自动构建：
- 推送到 `main` 分支触发构建
- 手动触发可构建指定扩展
- 成功后创建 Release 并上传 .nep 包
