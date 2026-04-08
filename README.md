# ChatMem

ChatMem 是一个面向 Windows 的桌面应用，用来浏览、搜索、恢复和迁移本地 AI 编程对话记录。当前支持 Claude、Codex 和 Gemini 三类来源。

## 下载

发布页会提供三种 Windows 包：

- `ChatMem_<version>_x64-setup.exe`
  推荐给普通用户的安装包。
- `ChatMem_<version>_x64_en-US.msi`
  适合偏企业化的 Windows 环境。
- `ChatMem-v<version>-portable.zip`
  免安装便携版。

正式下载入口：

- [GitHub Releases](https://github.com/Rimagination/ChatMem/releases)

## 主要功能

- 对话列表浏览与全文搜索
- 对话详情、工具调用和文件变更查看
- 一键复制会话文件位置与恢复命令
- Claude / Codex / Gemini 之间的对话迁移
- 简体中文 / English 切换
- 应用内检查更新

## 本地开发

环境要求：

- Node.js 20+
- Rust stable
- Windows 下可用的 Tauri 构建环境

常用命令：

```powershell
npm ci
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
npm run tauri build
```

## 发布

发布由 GitHub Actions 处理。推送形如 `v0.1.0` 的 tag 后，工作流会自动：

- 构建 NSIS 安装包
- 构建 MSI 安装包
- 生成 updater 所需的 `latest.json` 和签名文件
- 生成便携版 zip
- 上传所有 Windows 资产到 GitHub Release

## 更新机制

应用内更新依赖 Tauri updater，更新源指向：

- `https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json`

第一次接入发布前，需要先在 GitHub 仓库里配置：

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`

细节见 [DEVELOPMENT.md](./DEVELOPMENT.md)。
