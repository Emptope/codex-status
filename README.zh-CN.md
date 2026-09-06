<p align="center">
  <img src="assets/icon.png" width="96" height="96" alt="Codex Status 图标">
</p>

<h1 align="center">Codex Status</h1>

<p align="center">一个小巧的跨平台 Codex CLI 状态悬浮窗</p>

<p align="center">
  <a href="README.md">English</a>
</p>

<p align="center">
  <img src="assets/overview-light.png" width="300" alt="Codex Status 浅色主题">
  <img src="assets/overview-dark.png" width="300" alt="Codex Status 深色主题">
</p>

## 概览

Codex Status 显示本地会话、上下文用量、账户额度和状态通知。应用只读监听会话数据，并通过配置的 `codex app-server` 进程查询实时账户数据。

新完成的任务默认播放内置的“叮”提示音，低额度告警可从两种内置音效中选择。可在设置的“Notifications”中选择、试听或关闭提示音。

应用在 Windows 和 Linux 中仅保留系统托盘图标，在 macOS 中仅保留菜单栏图标，不会额外出现任务栏或 Dock 入口。取消勾选托盘中的 **Show / Hide** 可隐藏悬浮窗，且不会停止后台监听或提示音。关闭 Status notifications 并保持 Task completion sound 开启，即可只保留声音提示。

应用默认从 `PATH` 查找 `codex`，并读取 `CODEX_HOME`，未设置时使用 `~/.codex`。可在设置中修改可执行文件和数据目录。

## 安装

从 GitHub Releases 下载对应的发布产物和 `.sha256` 文件：

| 平台           | 产物                                        |
| -------------- | ------------------------------------------- |
| Windows 11 x64 | `codex-status-vX.Y.Z-windows-x64.exe`       |
| macOS 14+      | `codex-status-vX.Y.Z-macos-{x64,arm64}.dmg` |
| Ubuntu x64     | `codex-status-vX.Y.Z-linux-x64.deb`         |

使用前校验 SHA-256。Windows 直接运行 `.exe`；macOS 打开 `.dmg` 并将 Codex Status 移入“应用程序”；Ubuntu 使用以下命令安装：

```sh
sudo apt install ./codex-status-vX.Y.Z-linux-x64.deb
```

## 开发

需要 Node.js 24（`>=24.19 <25`）、pnpm 11.25、Rust 1.98 和 [Tauri 2 平台依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
pnpm install --frozen-lockfile
pnpm verify
pnpm dev
pnpm build
```

`pnpm verify` 执行格式、类型、Lint、单元、集成和浏览器检查。`pnpm build` 在 `build/artifacts` 中生成当前平台的发布产物和 SHA-256。

Windows 使用 `./scripts/build/windows.ps1 verify|build|dev` 初始化 MSVC 环境并执行对应任务。

## 文档

- [发布流程](docs/RELEASE.zh-CN.md)
- [MIT License](LICENSE)
