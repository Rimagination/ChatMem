# ChatMem

ChatMem 是一个面向 Windows 和 macOS 的桌面应用，用来浏览、搜索、恢复和迁移本地 AI 编程对话记录。当前支持 Claude、Codex 和 Gemini 三类来源。

## 下载

发布页会提供 Windows 和 macOS 包。

Windows：

- `ChatMem_<version>_x64-setup.exe`
  推荐给普通用户的安装包。
- `ChatMem_<version>_x64_en-US.msi`
  适合偏企业化的 Windows 环境。
- `ChatMem-v<version>-portable.zip`
  免安装便携版。

macOS：

- `ChatMem-v<version>-macOS-Apple-Silicon.dmg`
  推荐给 M1 / M2 / M3 / M4 等 Apple Silicon Mac 用户。
- `ChatMem-v<version>-macOS-Intel.dmg`
  推荐给 Intel Mac 用户。
- `ChatMem_<version>_<arch>.dmg`
  Tauri 自动生成的原始 dmg，内容与上面对应的用户版 dmg 相同。
- `ChatMem_<arch>.app.tar.gz`
  供应用内更新使用的 macOS updater 包。

不知道自己的 Mac 属于哪一种时，点屏幕左上角苹果菜单，选择“关于本机”。如果显示“芯片 Apple M1/M2/M3/M4”，下载 Apple Silicon 版；如果显示“处理器 Intel”，下载 Intel 版。Apple Silicon 电脑也可能通过 Rosetta 运行 Intel 版，但不推荐，优先下载 Apple Silicon 版。

当前 macOS 包暂未做 Apple Developer ID 签名和 notarization。首次打开时，系统可能需要你在“系统设置”中允许打开，或者通过右键菜单打开。

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
- 对应平台可用的 Tauri 构建环境

常用命令：

```powershell
npm ci
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
npm run tauri build
```

## 发布

发布由 GitHub Actions 处理。推送形如 `v0.1.3` 的 tag 后，工作流会自动：

- 构建 Windows NSIS 安装包
- 构建 Windows MSI 安装包
- 生成 updater 所需的 `latest.json` 和签名文件
- 生成 Windows 便携版 zip
- 构建 macOS dmg 和 app updater 包
- 额外上传面向用户下载的 macOS Apple Silicon / Intel dmg 文件名
- 上传所有发布资产到 GitHub Release

## 更新机制

应用内更新依赖 Tauri updater，更新源指向：

- `https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json`

第一次接入发布前，需要先在 GitHub 仓库里配置：

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`

细节见 [DEVELOPMENT.md](./DEVELOPMENT.md)。
