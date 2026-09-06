# 发布流程

[English](RELEASE.md)

## 构建矩阵

| 目标        | GitHub runner    | 产物                                  |
| ----------- | ---------------- | ------------------------------------- |
| Windows x64 | `windows-latest` | `codex-status-vX.Y.Z-windows-x64.exe` |
| Linux x64   | `ubuntu-22.04`   | `codex-status-vX.Y.Z-linux-x64.deb`   |
| macOS x64   | `macos-15-intel` | `codex-status-vX.Y.Z-macos-x64.dmg`   |
| macOS arm64 | `macos-15`       | `codex-status-vX.Y.Z-macos-arm64.dmg` |

每次向 `main` 推送和提交 PR，都会通过可复用的 `Build target` 工作流构建四个目标。每个任务校验 runner、项目版本、代码检查、构建结果、产物格式、名称和校验和。

匹配 `vX.Y.Z` 的 tag 会重新构建矩阵，并自动公开包含一个 Windows 可执行文件、一个 Linux 安装包、两个 macOS 磁盘映像、四个 `.sha256` 文件和 `LICENSE` 的 GitHub Release。

## 发布闸门

成功的发布工作流会立即公开 Release，因此必须在推送发布 tag 前完成全部闸门。

### 实机冒烟测试

在每个支持目标的干净系统中测试：

- 使用发布产物完成安装、升级和卸载；
- 启动、退出、托盘显示/隐藏和托盘退出；
- Windows 和 Linux 无任务栏入口，macOS 无 Dock 入口；
- 窗口拖动、缩放和位置恢复；
- 状态通知和低额度通知，包括两类提示音的配置与试听；
- 默认路径、自定义路径和 Codex CLI 数据读取；
- 无数据、CLI 不可用和网络失败状态。

记录系统、架构、Codex CLI 版本、桌面环境、时间和结果。Linux 还需记录 X11 或 Wayland。浏览器测试不能替代实机记录。

### 签名

- 自动发布目前无法访问代码签名凭据，因此其中的 Windows 可执行文件和 macOS 应用没有签名。
- 分发签名产物前，需在发布工作流中配置 Windows Authenticode 签名，以及 macOS Developer ID 签名和公证；挂载每个磁盘映像后，其中的应用必须通过以下命令。

```sh
codesign --verify --strict --verbose=2 <mounted-app>
spctl --assess --type execute --verbose=2 <mounted-app>
```

签名和公证后生成对应的 SHA-256 文件。

### 许可证和包内容

根据 `pnpm-lock.yaml`、`src-tauri/Cargo.lock` 和各目标的动态库生成生产依赖清单。确认许可证兼容性、署名和许可证文本要求，并确认发布文件不包含缓存、日志、配置或用户数据。

### 持续运行

在 Windows 11 x64 和 Ubuntu 22.04 x64 分别完成一小时前台运行和一小时托盘隐藏。覆盖空闲、持续刷新和记录更新，每分钟记录 CPU、内存和子进程数。预热后资源不得持续增长；退出后不得残留由应用启动的 CLI 或 WebView 子进程。

## 发布步骤

1. 只在 `src-tauri/Cargo.toml` 中更新一次版本，再更新 `src-tauri/Cargo.lock`。包元数据、Tauri、产物命名和发布校验均自动获取该版本。
2. 运行 `pnpm verify` 和 `pnpm build`；四个 CI 任务全部通过后再合并。
3. 完成实机、许可证、包内容和持续运行闸门；如果 CI 尚未配置签名，需确认可以分发未签名的 Windows 和 macOS 文件。
4. 同步 `main`，生成并校验发布 tag，然后创建无签名的附注 tag 并推送：

   ```sh
   git switch main
   git pull --ff-only
   release_tag="v$(node scripts/build/validate.mjs version)"
   node scripts/build/validate.mjs tag "$release_tag"
   git tag -a --no-sign "$release_tag" -m "codex-status $release_tag"
   git push origin "$release_tag"
   ```

   推送 tag 后将自动公开发布。不要移动或覆盖已有发布 tag；应提升项目版本并创建新 tag。tag 签名仍为可选项。

5. 等待公开 Release 生成，核对四个发布产物、四个校验和及 `LICENSE`。在每个目标上复核校验和、安装与启动；配置 CI 签名后，还需复核 Windows 和 macOS 签名。

Linux `.deb` 面向 Ubuntu 22.04 构建，并声明 WebKitGTK 4.1 和 AppIndicator 运行时依赖。Wayland 下的置顶、全局位置和托盘行为取决于合成器，必须按支持的降级行为实测。
