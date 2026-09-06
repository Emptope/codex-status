# Release Runbook

[简体中文](RELEASE.zh-CN.md)

## Build Matrix

| Target      | GitHub runner    | Artifact                              |
| ----------- | ---------------- | ------------------------------------- |
| Windows x64 | `windows-latest` | `codex-status-vX.Y.Z-windows-x64.exe` |
| Linux x64   | `ubuntu-22.04`   | `codex-status-vX.Y.Z-linux-x64`       |
| macOS x64   | `macos-15-intel` | `codex-status-vX.Y.Z-macos-x64`       |
| macOS arm64 | `macos-15`       | `codex-status-vX.Y.Z-macos-arm64`     |

Every push to `main` and pull request runs all four targets through the reusable `Build target` workflow. Each job validates the runner, project versions, checks, build output, executable permissions, artifact name, and checksum.

A matching `vX.Y.Z` tag rebuilds the matrix and publishes a GitHub Release containing four binaries, four `.sha256` files, and `LICENSE`.

## Release Gates

Complete every gate before pushing the release tag because a successful release workflow publishes immediately.

### Native smoke tests

On clean Windows 11 x64 and Ubuntu 22.04 x64 systems, test:

- startup, shutdown, tray show/hide, and tray exit;
- window dragging, resizing, and position restore;
- status and low-quota notifications;
- default paths, custom paths, and Codex CLI data ingestion;
- no-data, unavailable-CLI, and network-failure states.

Record the OS, architecture, Codex CLI version, desktop environment, timestamp, and result. For Linux, also record X11 or Wayland. Browser tests do not replace native test records.

### Signing

- Automatic releases do not currently have access to code-signing credentials, so their Windows and macOS binaries are unsigned.
- Before distributing signed binaries, configure Authenticode signing for Windows and Developer ID signing and notarization for macOS in the release workflow. Verify that both commands below pass for the generated macOS artifacts.

```sh
codesign --verify --strict --verbose=2 <artifact>
spctl --assess --type execute --verbose=2 <artifact>
```

Generate the SHA-256 files after signing and notarization.

### Licenses and package contents

Build the production dependency inventory from `pnpm-lock.yaml`, `src-tauri/Cargo.lock`, and each target's dynamic libraries. Verify license compatibility, required attribution, and license text. Confirm that release files contain no caches, logs, configuration, or user data.

### Endurance

On both Windows 11 x64 and Ubuntu 22.04 x64, run one hour in the foreground and one hour hidden in the tray. Cover idle, continuous refresh, and record updates; capture CPU, memory, and child-process counts every minute. After warm-up, resources must not grow continuously. After exit, no CLI or WebView child process started by the app may remain.

## Procedure

1. Update the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`; update lockfiles.
2. Run `pnpm verify` and `pnpm build`, then merge only after all four CI jobs pass.
3. Complete the native, license, package-content, and endurance gates. Confirm that unsigned Windows and macOS distribution is acceptable unless CI signing has been configured.
4. Create and push a signed matching tag, for example `git tag -s v0.1.0`. This starts automatic publication.
5. Wait for the public Release, then confirm the four binaries, four checksums, and `LICENSE`. Recheck checksums and startup on Windows and Ubuntu, plus Windows and macOS signatures when CI signing is enabled.

The Linux binary targets Ubuntu 22.04 and requires WebKitGTK 4.1 and AppIndicator at runtime. Under Wayland, always-on-top, global positioning, and tray behavior depend on the compositor and must be tested with the supported fallback behavior.
