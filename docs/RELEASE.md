# 发布

## 构建矩阵

| 系统        | GitHub runner    | 产物                                  |
| ----------- | ---------------- | ------------------------------------- |
| Windows x64 | `windows-latest` | `codex-status-vX.Y.Z-windows-x64.exe` |
| Linux x64   | `ubuntu-22.04`   | `codex-status-vX.Y.Z-linux-x64`       |
| macOS x64   | `macos-15`       | `codex-status-vX.Y.Z-macos-x64`       |
| macOS arm64 | `macos-15-arm64` | `codex-status-vX.Y.Z-macos-arm64`     |

提交和 PR 由 `CI` 工作流执行四个目标的完整验收与发行构建。`vX.Y.Z` tag 触发相同构建流程；四个任务全部成功后，发布任务只接受上述四个可执行文件及其四个 SHA-256 文件。

构建会检查：

- runner 的操作系统与架构符合目标；
- `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 版本一致；
- 可执行文件非空，macOS 与 Linux 构建结果具有执行权限；
- 产物命名、数量和 SHA-256 内容准确；
- 格式、类型、Clippy、Rust、Node.js、Vitest、Playwright 与 Actions 安全扫描全部通过。

## 发布步骤

1. 更新三处版本及锁文件，运行 `pnpm verify` 和 `pnpm build`。
2. 等待 GitHub 上四个 `CI` 构建全部通过。
3. 在 Windows、macOS Intel、macOS Apple Silicon 与 Linux 干净环境验证启动、退出、托盘、拖动、缩放、位置恢复、通知与数据读取。
4. 完成 Windows 签名及 macOS 签名、公证，再核对第三方许可证清单。
5. 创建与版本一致的签名 tag，例如 `git tag -s v0.1.0`，然后推送 tag。
6. 核对 Release 的八个文件并在各目标系统重新校验 SHA-256 与启动行为。

Linux 产物在 Ubuntu 22.04 runner 构建，运行环境需要 WebKitGTK 4.1 与 AppIndicator。Wayland 下的置顶、全局位置和托盘能力取决于合成器，应按降级路径实测。
