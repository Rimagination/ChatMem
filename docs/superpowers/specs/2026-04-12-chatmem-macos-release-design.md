# ChatMem macOS Release Design

## Goal

Add a GitHub-hosted macOS build for ChatMem while keeping the existing Windows release flow intact.

## Release Model

- GitHub Actions remains the only release entry point.
- A pushed version tag such as `v0.1.0` creates or updates a GitHub Release.
- Windows continues to publish NSIS, MSI, portable ZIP, updater bundles, signatures, and `latest.json`.
- macOS builds run on GitHub's macOS runner and upload macOS artifacts to the same Release.

## macOS Packaging

- First macOS release is unsigned and not notarized.
- The workflow builds macOS bundles on GitHub macOS runners using Tauri's macOS toolchain.
- The workflow publishes both Apple Silicon (`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`) builds.
- The expected user-facing asset is a `.dmg`.
- The updater artifacts are signed `.app.tar.gz` files plus matching `.sig` files for each macOS architecture.
- The macOS Tauri build explicitly includes the `updater` bundle target; otherwise Tauri will create the app and dmg but skip updater signatures.
- Users may need to bypass Gatekeeper on first launch because there is no Apple Developer ID signing or notarization in this phase.

## Updater

Tauri v1 supports updater artifacts for the macOS app bundle. The existing updater key remains valid across platforms, so the macOS job uses the same `TAURI_PRIVATE_KEY` and `TAURI_KEY_PASSWORD` GitHub secrets.

## Validation

- Add a workflow structure test so the release workflow must include macOS jobs for both architectures.
- Run the frontend test suite and Rust tests locally.
- Push the workflow and force-update `v0.1.0` to regenerate the GitHub Release.
- Verify the GitHub Actions run succeeds and the Release contains Apple Silicon and Intel macOS assets.
