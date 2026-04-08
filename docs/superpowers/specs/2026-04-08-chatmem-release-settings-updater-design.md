# ChatMem 发布、设置页、多语言与自动更新设计

**Date:** 2026-04-08

## Goal

把 ChatMem 从“本地可构建的桌面工具”升级为“可公开维护、可从 GitHub 下载、具备设置页、语言切换和更新检查能力的正式 Windows 软件”。

本次设计覆盖四个子项目：

1. 仓库整理与公开发布基础
2. 设置页与语言切换
3. 应用内更新检查与启动时自动检查更新
4. GitHub Releases 与自动化发布流程

## Context

当前项目已经具备以下条件：

- 基于 Tauri 1.x + React + TypeScript
- 已能在本地构建 Windows 安装包和便携包
- 已有桌面版产品主线
- 已有基础测试与打包流程

当前项目同时存在以下问题：

- 目录中混有不适合公开仓库的本地产物和历史文件
- 目前没有 `.git` 仓库元数据
- 没有 GitHub 远程仓库
- 没有设置页
- 没有语言切换架构
- 没有 Tauri updater 配置
- 没有 GitHub Actions 自动构建和发布流程

用户明确要求：

- 新建公开仓库 `Rimagination/ChatMem`
- 第一版设置页包含：
  - `检查更新`
  - `启动时自动检查更新`
  - `语言 Language`
- 第一版语言支持：
  - `简体中文`
  - `English`
- 要像常见软件下载项目一样，在 GitHub 上提供 Windows 下载包
- 选择方案 3：同时完成自动化发布能力，而不是只做半手工流程

## External Constraints

### Tauri updater constraint

本次更新能力必须遵循 Tauri v1 官方 updater 机制，而不是自造下载逻辑。

根据官方文档，Windows 下 updater 使用：

- 标准安装包：`NSIS` 或 `MSI`
- 更新包：由 Tauri 生成的 `.zip`
- 签名文件：`.sig`
- 更新元数据：静态 JSON 或动态服务

第一版采用 **静态 JSON + GitHub Releases** 路线。

参考：

- [Tauri v1 Updater Guide](https://v1.tauri.app/v1/guides/distribution/updater/)
- [Tauri v1 JS Updater API](https://v1.tauri.app/v1/api/js/updater/)

## Explored Approaches

### Approach 1: 仅做公开仓库和下载页

只整理仓库并发布 Windows 包，不做设置页和应用内更新。

优点：

- 最快
- 风险最低

缺点：

- 不能满足“像别的软件一样在设置里检查更新”
- 语言切换也无法同时落地

### Approach 2: 正式第一版

做公开仓库、设置页、语言切换、应用内检查更新、手工 Release 流程。

优点：

- 已经形成完整用户体验
- 技术风险可控

缺点：

- 发布流程仍有手工步骤

### Approach 3: 完整第一版发布体系

在 Approach 2 基础上，再补齐 GitHub Actions 自动构建、自动生成更新元数据、自动创建 Release 和上传资产。

优点：

- 仓库结构、产品功能和发布流程一次成型
- 后续发布成本最低
- 最符合用户“全部做”的要求

缺点：

- 需要一次引入 updater 签名、Secrets 管理、GitHub 发布流程
- 初始实现复杂度最高

## Selected Product Decision

本次采用 **Approach 3: 完整第一版发布体系**。

这意味着本次不是只做设置页，也不是只做仓库整理，而是建立一个从本地代码到 GitHub 下载页再到应用内检查更新的完整闭环。

## Product Boundary

### In scope

- 初始化并整理本地 git 仓库
- 清理不应进入公开仓库的文件
- 创建公开 GitHub 仓库 `Rimagination/ChatMem`
- 推送初始代码
- 增加设置页入口和设置面板
- 实现 `简体中文 / English` 语言切换
- 增加应用内“检查更新”
- 增加“启动时自动检查更新”持久化开关
- 配置 Tauri updater
- 增加 GitHub Actions 自动构建与发布
- 生成 GitHub Releases 下载资产
- 提供 Windows：
  - `NSIS 安装包`
  - `MSI 安装包`
  - `便携 zip`

### Out of scope

- 不做多主题系统
- 不做三种以上语言
- 不做 Linux / macOS 发布
- 不做应用内账号体系
- 不做自建更新后端服务
- 不做增量补丁更新

## Repository And Packaging Architecture

### Repository role

`Rimagination/ChatMem` 是正式产品仓库，不是实验目录镜像。

仓库必须只保留：

- 正式桌面产品实现
- 必要文档
- 必要构建配置
- 必要发布自动化文件

### Files to keep

- `src/`
- `src-tauri/`
- `public/`
- `docs/` 中仍有价值的设计与计划文档
- `package.json`
- `package-lock.json`
- `tsconfig.json`
- `tsconfig.node.json`
- `vite.config.ts`
- `vitest.config.ts`
- `README.md`
- `DEVELOPMENT.md`
- `.gitignore`
- Windows 用户说明文件（仅在确认对最终用户有价值时保留）

### Files to remove or ignore

以下内容不得进入公开仓库：

- `node_modules/`
- `dist/`
- `dist-portable/`
- `src-tauri/target/`
- `__pycache__/`
- `backend.log`
- 本地测试报告、构建缓存、临时日志
- 历史试验文件，如：
  - `app_fixed.py`
  - 其他不再是正式产品路径的 Python 辅助版本

### Product line decision

仓库主线明确为：

**Tauri 桌面应用是唯一正式产品实现。**

如果 Python 入口文件仍有保留价值，只能作为开发辅助工具保留；如果只是历史尝试，就从正式公开仓库中移除。

## Settings Experience

### Entry

设置入口使用齿轮图标，放在主界面稳定位置。

首选位置：

- 侧栏底部

备选位置：

- 主界面右上角

要求：

- 在中英文两种语言下都不挤压主布局
- 点击后打开轻量设置面板，而不是跳转复杂二级页面

### Settings contents

第一版设置页仅包含三项：

1. `语言 Language`
2. `检查更新`
3. `启动时自动检查更新`

这是刻意的范围控制。第一版不加入与当前目标无关的设置项。

## Internationalization Design

### Language scope

第一版支持：

- `zh-CN`
- `en`

默认语言：

- `zh-CN`

### Technical approach

不引入重型 i18n 平台。第一版采用最小可维护方案：

- 单独的语言上下文或轻量状态容器
- 语言字典文件
- 统一的 `t(key)` 读取方式

建议目录：

- `src/i18n/strings.ts`
- `src/i18n/I18nProvider.tsx`
- `src/i18n/types.ts`

### Persistence

语言设置持久化到本地。

第一版可用：

- `localStorage`

要求：

- 重启应用后语言保持不变
- 默认值在无记录时回退到中文

### Translation coverage

第一版至少覆盖：

- 主界面导航与标题
- 搜索框 placeholder
- 设置页全部文案
- 更新检查相关提示
- 顶部元信息与主要操作按钮
- 空状态文案

允许第一版存在极少量遗漏，但架构必须保证后续补文案不需要重写机制。

## Update Check Design

### User-facing behavior

设置页中的 `检查更新` 按钮行为：

- 点击后显示检查中状态
- 请求 updater 源
- 若当前已是最新版本，显示明确反馈
- 若发现新版本，显示：
  - 最新版本号
  - 发布时间
  - 更新说明
  - 可执行的更新动作

### Auto-check behavior

`启动时自动检查更新` 是布尔开关。

行为要求：

- 默认开启
- 启动后延迟几秒检查，避免与首屏加载抢资源
- 仅在发现新版本时提示
- 若关闭该选项，应用启动时不发起更新请求

### Update prompt behavior

发现新版本后，不采用强制升级。

第一版交互：

- 弹出轻量更新提示
- 显示版本与说明
- 给出：
  - `立即更新`
  - `稍后`

第一版不做静默强制安装。

## Updater Technical Design

### Tauri configuration

需要在 Tauri 配置中补充 updater 相关配置：

- updater public key
- updater endpoint
- Windows 对应更新格式

### Update source strategy

采用 **静态 JSON**。

建议文件：

- `latest.json`

内容包含：

- `version`
- `notes`
- `pub_date`
- `platforms.windows-x86_64.url`
- `platforms.windows-x86_64.signature`

### Hosting strategy

第一版更新元数据优先放在 GitHub 可稳定访问的位置：

优先方案：

- GitHub Pages

备选方案：

- 仓库中的静态文件配合原始文件地址

要求：

- 地址稳定
- 便于 GitHub Actions 自动更新

### Signature management

Updater 私钥绝不进入仓库。

Secrets 设计：

- GitHub Actions Secret 存放 updater 私钥
- 应用内仅保留 updater 公钥

这是强约束，不允许为了省事把私钥写进代码或仓库文件。

## GitHub Release Design

### Repository

仓库名称：

- `Rimagination/ChatMem`

可见性：

- public

默认分支：

- `main`

### Release assets

每个版本 Release 必须上传：

- `ChatMem_<version>_x64-setup.exe`
- `ChatMem_<version>_x64_en-US.msi`
- `ChatMem-v<version>-portable.zip`
- updater zip
- updater `.sig`
- `latest.json` 对应版本内容（如果采用发布资产同步方案）

### Download surface

GitHub Releases 是 Windows 用户下载入口。

README 需要直接提供：

- 最新版本下载说明
- 三种包的用途说明

例如：

- `安装版（推荐）`
- `MSI（企业环境）`
- `便携版（免安装）`

## GitHub Actions Automation

### Trigger strategy

发布流程以 tag 为准。

例如：

- 推送 tag `v0.1.1`

触发后自动：

1. 安装 Node / Rust 依赖
2. 构建前端与 Tauri Windows 包
3. 生成 updater bundle 与签名
4. 生成或更新 `latest.json`
5. 创建 GitHub Release
6. 上传全部 Windows 资产

### Workflow files

建议新增：

- `.github/workflows/release.yml`
- 生成更新元数据的脚本，如：
  - `scripts/generate_updater_manifest.(js|ts|ps1)`

### Secrets

至少需要：

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`（如果使用口令）

如 GitHub Actions 发布需要额外 token，则按最小权限增加。

## Windows Packaging Decision

### Standard installer

`NSIS setup.exe` 作为普通用户主下载包。

### Enterprise-compatible installer

`MSI` 保留，服务于更严格的 Windows 环境。

### Portable package

便携版继续提供：

- `ChatMem-v<version>-portable.zip`

要求：

- 与 Release 版本号保持一致
- 由自动化流程生成或由正式发布步骤稳定产出

## Data Flow

### Language flow

用户选择语言后：

1. 更新本地语言状态
2. 写入本地持久化存储
3. 全局 UI 重新读取文案字典

### Manual update flow

用户点击“检查更新”后：

1. 前端调用 Tauri updater API
2. updater 请求 `latest.json`
3. 比较本地版本与远端版本
4. 返回“已最新”或“发现更新”
5. 若用户确认更新，则执行安装流程

### Release flow

开发者推送版本 tag 后：

1. GitHub Actions 自动构建产物
2. 生成 updater 签名与清单
3. 创建 Release
4. 上传 Windows 资产
5. 用户端设置页可发现新版本

## Error Handling

### Update errors

若更新检查失败：

- 显示非技术化错误提示
- 给出重试动作
- 不阻塞应用正常使用

例如：

- `无法连接更新服务，请稍后重试`

### Missing update configuration

如果 updater 未正确配置：

- 设置页中检查更新能力必须有明确降级表现
- 不能静默失败

### Translation fallback

若某个 key 缺失：

- 回退到默认中文
- 不显示空白
- 不显示内部 key 名称给终端用户

### Release workflow failures

CI 失败时：

- Release 不应产生半成品说明
- 失败日志必须足够明确，方便定位是构建失败、签名失败还是上传失败

## Security Constraints

- updater 私钥只存在于本地安全环境和 GitHub Secrets
- 不在仓库中存储任何敏感 token
- 公开仓库中的配置只能包含公钥和公开下载地址

## File Impact

### Expected frontend files

- `src/App.tsx`
- `src/styles.css`
- `src/main.tsx`
- 新增设置相关组件文件
- 新增 i18n 目录与字典文件
- 相关测试文件

### Expected Tauri files

- `src-tauri/tauri.conf.json`
- 必要时新增 updater / app config 辅助代码

### Expected repository files

- `.gitignore`
- `README.md`
- `LICENSE`
- `.github/workflows/release.yml`
- `scripts/*`

## Testing Strategy

Implementation must follow TDD for new behavior.

至少覆盖：

- 设置页能打开并显示三项核心设置
- 语言切换后主要 UI 文案会变化
- 语言选择会持久化
- 手动检查更新在“最新 / 有更新 / 检查失败”三种状态下都能正确反馈
- 自动检查更新开关会持久化并影响启动行为
- updater 配置文件和更新清单生成逻辑可通过脚本验证
- GitHub Actions 工作流至少通过本地结构校验与 dry-run 级别检查

## Acceptance Criteria

- 仓库已整理为公开可维护状态
- 已创建并推送到 `Rimagination/ChatMem`
- 应用中存在设置入口
- 设置页支持 `简体中文 / English`
- 设置页支持“检查更新”
- 设置页支持“启动时自动检查更新”
- GitHub Releases 可作为正式 Windows 下载页
- Release 至少提供：
  - 安装版 exe
  - MSI
  - 便携版 zip
- GitHub Actions 能自动构建并发布 Windows 版本
- Tauri updater 所需签名、公钥和更新清单路径已打通
