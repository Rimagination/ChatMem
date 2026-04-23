# ChatMem Release Checklist

This checklist is for shipping a user-visible ChatMem update. A release is not complete until all three delivery surfaces are updated:

- desktop package
- ChatMem MCP binary
- ChatMem skill copy used by local agents

## 1. Confirm release scope

- Merge the intended branch into `main`.
- Confirm the app version in [package.json](/D:/VSP/agentswap-gui/package.json) matches [src-tauri/tauri.conf.json](/D:/VSP/agentswap-gui/src-tauri/tauri.conf.json).
- Confirm user-facing docs match the shipped behavior, especially:
  - [docs/CHATMEM_MCP_SETUP.md](/D:/VSP/agentswap-gui/docs/CHATMEM_MCP_SETUP.md)
  - [skills/chatmem/SKILL.md](/D:/VSP/agentswap-gui/skills/chatmem/SKILL.md)

## 2. Run validation before tagging

- `npm ci`
- `cargo test --manifest-path .\src-tauri\Cargo.toml`
- `cargo test chatmem_memory:: --manifest-path .\src-tauri\Cargo.toml`
- `npm.cmd run test:run`
- `npm.cmd run build`
- `cargo build --release --bin chatmem-mcp --manifest-path .\src-tauri\Cargo.toml`

If the release changes retrieval, memory UI, or bootstrap flow, also do a quick manual smoke check:

- open a repo with no history index and confirm automatic bootstrap starts
- confirm the memory drawer shows empty-index copy before indexing completes
- confirm the one-time ready cue appears after bootstrap
- confirm the first post-bootstrap `Memory` open lands on the first approved memory card
- ask a recall-style question and confirm history evidence is used when approved memory is missing

## 3. Update the MCP delivery

- Rebuild the release MCP binary:
  - `cargo build --release --bin chatmem-mcp --manifest-path .\src-tauri\Cargo.toml`
- Verify the launcher resolves the newest release binary:
  - [mcp/run-chatmem-mcp.ps1](/D:/VSP/agentswap-gui/mcp/run-chatmem-mcp.ps1)
- Smoke-test the MCP process:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File D:\VSP\agentswap-gui\mcp\run-chatmem-mcp.ps1`
- If Codex App or another client already has ChatMem configured, fully restart that client after replacing the binary.

## 4. Update the skill delivery

- If the agent reads the repo-local skill, no extra copy step is needed after release.
- If the agent uses a copied skill under `%USERPROFILE%\.codex\skills\chatmem`, sync the latest [skills/chatmem/SKILL.md](/D:/VSP/agentswap-gui/skills/chatmem/SKILL.md) into that installed location.
- Restart the client after updating the installed skill so new guidance is actually loaded.

## 5. Build and publish the desktop package

- Ensure updater signing secrets exist in GitHub:
  - `TAURI_PRIVATE_KEY`
  - `TAURI_KEY_PASSWORD`
- Create and push a release tag in `v*` format that matches the app version, for example `v1.0.0`.
- The GitHub Actions workflow at [.github/workflows/release.yml](/D:/VSP/agentswap-gui/.github/workflows/release.yml) should publish:
  - Windows `.exe`
  - Windows `.msi`
  - Windows portable `.zip`
  - macOS `.dmg`
  - updater assets including `latest.json`

Optional local signed build check:

```powershell
$env:TAURI_PRIVATE_KEY = "C:\Users\93219\.tauri\chatmem.key"
$env:TAURI_KEY_PASSWORD = Get-Content -Raw C:\Users\93219\.tauri\chatmem-updater-password.txt
npm run tauri build
```

## 6. Verify the GitHub Release and updater

- Confirm the GitHub Release contains the expected installer assets.
- Confirm `latest.json` is present on the release and points to the new version.
- Confirm the updater endpoint in [src-tauri/tauri.conf.json](/D:/VSP/agentswap-gui/src-tauri/tauri.conf.json) still matches the published release channel:
  - `https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json`
- Install or open an older desktop build and confirm in-app update detection reaches the new version.

## 7. Final post-release check

- Open the shipped desktop app, not just a dev build.
- Verify the new version number is visible in the app.
- Verify the packaged app can still open the memory drawer, load repo context, and trigger bootstrap for an empty repo.
- Verify a real agent session is using the new MCP behavior and the new skill guidance.

If any one of package, MCP, or skill remains old, users can still see stale behavior even after the code is merged.
