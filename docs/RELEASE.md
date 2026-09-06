# Release Runbook

[简体中文](RELEASE.zh-CN.md)

## Build Matrix

| Target      | GitHub runner    | Artifact                              |
| ----------- | ---------------- | ------------------------------------- |
| Windows x64 | `windows-latest` | `codex-status-vX.Y.Z-windows-x64.exe` |
| Linux x64   | `ubuntu-22.04`   | `codex-status-vX.Y.Z-linux-x64.deb`   |
| macOS x64   | `macos-15-intel` | `codex-status-vX.Y.Z-macos-x64.dmg`   |
| macOS arm64 | `macos-15`       | `codex-status-vX.Y.Z-macos-arm64.dmg` |

Every push to `main` and pull request runs all four targets through the reusable `Build target` workflow. Each job validates the runner, project versions, checks, build output, artifact format, name, and checksum.

A matching `vX.Y.Z` tag rebuilds the matrix and publishes a GitHub Release containing one Windows executable, one Linux package, two macOS disk images, four `.sha256` files, and `LICENSE`.

## Release Gates

Complete every gate before pushing the release tag because a successful release workflow publishes immediately.

### Native smoke tests

On clean systems for every supported target, test:

- install, upgrade, and removal using the published artifact;
- startup, shutdown, tray show/hide, and tray exit;
- no taskbar entry on Windows or Linux and no Dock entry on macOS;
- window dragging, resizing, and position restore;
- approval, completion, and low-quota notifications, including all three configurable sound types, previews, and off options;
- default paths, custom paths, and Codex CLI data ingestion;
- no-data, unavailable-CLI, and network-failure states.

Record the OS, architecture, Codex CLI version, desktop environment, timestamp, and result. For Linux, also record X11 or Wayland. Browser tests do not replace native test records.

### Signing

- Automatic releases do not currently have access to code-signing credentials, so their Windows executable and macOS application are unsigned.
- Before distributing signed artifacts, configure Authenticode signing for Windows and Developer ID signing and notarization for macOS in the release workflow. Mount each disk image and verify its application with both commands below.

```sh
codesign --verify --strict --verbose=2 <mounted-app>
spctl --assess --type execute --verbose=2 <mounted-app>
```

Generate the SHA-256 files after signing and notarization.

### Licenses and package contents

Build the production dependency inventory from `pnpm-lock.yaml`, `src-tauri/Cargo.lock`, and each target's dynamic libraries. Verify license compatibility, required attribution, and license text. Confirm that release files contain no caches, logs, configuration, or user data.

### Endurance

On both Windows 11 x64 and Ubuntu 22.04 x64, run one hour in the foreground and one hour hidden in the tray. Cover idle, continuous refresh, and record updates; capture CPU, memory, and child-process counts every minute. After warm-up, resources must not grow continuously. After exit, no CLI or WebView child process started by the app may remain.

## Procedure

1. Update the version once in `src-tauri/Cargo.toml`, then update `src-tauri/Cargo.lock`. Package metadata, Tauri, artifact naming, and release validation derive it automatically.
2. Run `pnpm verify` and `pnpm build`, then merge only after all four CI jobs pass.
3. Complete the native, license, package-content, and endurance gates. Confirm that unsigned Windows and macOS distribution is acceptable unless CI signing has been configured.
4. Synchronize `main`, derive and validate the release tag, then create an unsigned annotated tag and push it:

   ```sh
   git switch main
   git pull --ff-only
   release_tag="v$(node scripts/build/validate.mjs version)"
   node scripts/build/validate.mjs tag "$release_tag"
   git tag -a --no-sign "$release_tag" -m "codex-status $release_tag"
   git push origin "$release_tag"
   ```

   Pushing the tag starts automatic publication. Do not move or overwrite an existing release tag; increment the project version and create a new tag instead. Tag signing remains optional.

5. Wait for the public Release, then confirm the four release artifacts, four checksums, and `LICENSE`. Recheck checksums, installation, and startup on every target, plus Windows and macOS signatures when CI signing is enabled.

The Linux `.deb` targets Ubuntu 22.04 and declares its WebKitGTK 4.1 and AppIndicator runtime dependencies. Under Wayland, always-on-top, global positioning, and tray behavior depend on the compositor and must be tested with the supported fallback behavior.
