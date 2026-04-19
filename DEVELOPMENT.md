# ChatMem Development Notes

## Local Commands

```powershell
npm ci
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
npm run tauri build
```

## Updater Key

本地已经生成 updater key：

- 私钥：`C:\Users\93219\.tauri\chatmem.key`
- 公钥：`C:\Users\93219\.tauri\chatmem.key.pub`
- 密码文件：`C:\Users\93219\.tauri\chatmem-updater-password.txt`

仓库中的 `src-tauri/tauri.conf.json` 已写入当前公钥。

## GitHub Secrets

在 `Rimagination/ChatMem` 仓库中添加以下 Secrets：

- `TAURI_PRIVATE_KEY`
  内容为 `C:\Users\93219\.tauri\chatmem.key` 的完整文本
- `TAURI_KEY_PASSWORD`
  内容为 `C:\Users\93219\.tauri\chatmem-updater-password.txt` 中保存的密码

## Local Signed Build

启用 updater 后，本地 `tauri build` 也需要带签名环境变量：

```powershell
$env:TAURI_PRIVATE_KEY = "C:\Users\93219\.tauri\chatmem.key"
$env:TAURI_KEY_PASSWORD = Get-Content -Raw C:\Users\93219\.tauri\chatmem-updater-password.txt
npm run tauri build
```

## Release Workflow

发布流程定义在：

- `.github/workflows/release.yml`

工作流触发条件：

- 推送 tag，格式为 `v*`

工作流产物：

- Windows NSIS 安装包
- Windows MSI 安装包
- updater `latest.json`
- updater 签名文件
- Windows 便携版 `ChatMem-v<version>-portable.zip`
- macOS dmg
- macOS app updater 包

当前 macOS 发布包未做 Apple Developer ID 签名和 notarization。正式分发前如需减少 Gatekeeper 提示，需要在仓库 Secrets 中补充 Apple Developer 证书和 notarization 凭据，并扩展 `.github/workflows/release.yml`。

## Portable Package

本地构建便携版命令：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-portable.ps1 -Version 0.1.2
```

默认读取：

- `src-tauri\target\release\ChatMem.exe`
- `README.md`

输出到：

- `dist-portable\ChatMem-v0.1.2-portable.zip`
