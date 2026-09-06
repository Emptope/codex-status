<p align="center">
  <img src="assets/icon.png" width="96" height="96" alt="Codex Status 图标">
</p>

<h1 align="center">Codex Status</h1>

<p align="center">一个小巧的跨平台 Codex 状态悬浮窗</p>

<p align="center">
  <img src="assets/overview-light.png" width="300" alt="Codex Status Light 配色">
  <img src="assets/overview-dark.png" width="300" alt="Codex Status Dark 配色">
</p>

## 使用

从 GitHub Releases 下载对应文件，并使用同名 `.sha256` 校验完整性：

- Windows 11 x64：`codex-status-vX.Y.Z-windows-x64.exe`
- macOS 14+：`codex-status-vX.Y.Z-macos-x64` 或 `codex-status-vX.Y.Z-macos-arm64`
- Linux x64：`codex-status-vX.Y.Z-linux-x64`

macOS 和 Linux 下载后需要添加执行权限：

```sh
chmod +x codex-status-vX.Y.Z-<platform>-<arch>
```

应用默认查找 PATH 中的 `codex` 和用户数据目录。可在设置中指定可执行文件和数据目录。

## 开发

需要 Node.js 24、pnpm 11.25、Rust 1.98，以及目标平台的 Tauri 2 系统依赖。

```sh
pnpm install --frozen-lockfile
pnpm verify
pnpm build
```

`pnpm build` 在 `build/artifacts` 生成版本化可执行文件和 SHA-256。Windows 使用 `./scripts/build/windows.ps1 verify|build|dev`。

## 文档

[发布](docs/RELEASE.md)

[MIT License](LICENSE)
