# Codex Status

一个小巧的跨平台 Codex 状态悬浮窗，采用极简、平面化界面，集中显示额度、会话活动及需要处理的状态。

## 使用

从 GitHub Releases 下载对应文件，并使用同名 `.sha256` 校验完整性：

- Windows 11 x64：`codex-status-vX.Y.Z-windows-x64.exe`
- macOS 14+：`codex-status-vX.Y.Z-macos-x64` 或 `codex-status-vX.Y.Z-macos-arm64`
- Linux x64：`codex-status-vX.Y.Z-linux-x64`

macOS 和 Linux 下载后需要添加执行权限：

```sh
chmod +x codex-status-vX.Y.Z-<platform>-<arch>
```

应用默认查找 PATH 中的 `codex` 和用户数据目录，也可在设置中指定可执行文件与数据根。应用直接使用当前 CLI 提供的数据，不固定 CLI 版本。

## 开发

需要 Node.js 24、pnpm 11.25、Rust 1.98，以及目标平台的 Tauri 2 系统依赖。

```sh
pnpm install --frozen-lockfile
pnpm verify
pnpm build
```

`pnpm build` 在 `build/artifacts` 生成当前平台的版本化可执行文件及 SHA-256。Windows 可通过 `./scripts/build/windows.ps1 verify|build|dev` 加载 Visual Studio 构建环境。完整发布闸门见 [docs/RELEASE.md](docs/RELEASE.md)。
