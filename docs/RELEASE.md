# 发布

## 构建矩阵

| 系统        | GitHub runner    | 产物                                  |
| ----------- | ---------------- | ------------------------------------- |
| Windows x64 | `windows-latest` | `codex-status-vX.Y.Z-windows-x64.exe` |
| Linux x64   | `ubuntu-22.04`   | `codex-status-vX.Y.Z-linux-x64`       |
| macOS x64   | `macos-15`       | `codex-status-vX.Y.Z-macos-x64`       |
| macOS arm64 | `macos-15-arm64` | `codex-status-vX.Y.Z-macos-arm64`     |

提交和 PR 由 `CI` 工作流执行四个目标的构建检查。`vX.Y.Z` tag 触发相同构建流程，并创建包含八个产物和 MIT `LICENSE` 的 Draft Release。工作流不公开 Release。

构建会检查：

- runner 的操作系统与架构符合目标；
- `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 版本一致；
- 可执行文件非空，macOS 与 Linux 构建结果具有执行权限；
- 产物命名、数量和 SHA-256 内容准确；
- 格式、类型、Clippy、Rust、Node.js、Vitest、Playwright 与 Actions 安全扫描全部通过。

## 发布闸门

Draft Release 满足以下条件后才能公开。

### 实机

在 Windows 11 x64 和 Ubuntu 22.04 x64 干净环境逐项验证：

- 启动和退出；
- 托盘显示、隐藏和退出；
- 窗口拖动、缩放和位置恢复；
- 状态通知和低额度通知；
- 默认路径、自定义路径和 Codex CLI 数据读取；
- 无数据、CLI 不可用和网络失败状态。

记录系统版本、架构、Codex CLI 版本、桌面环境、验证时间和结果。CI 浏览器测试不能替代实机记录。

### 签名

Windows 使用 Authenticode 签名。`Get-AuthenticodeSignature` 的 `Status` 必须为 `Valid`。

macOS 使用 Developer ID Application 签名并公证。以下检查必须成功：

```sh
codesign --verify --strict --verbose=2 <artifact>
spctl --assess --type execute --verbose=2 <artifact>
```

签名和公证完成后重新生成对应 SHA-256。不得发布构建阶段生成的旧校验文件。

### 许可证

根据 `pnpm-lock.yaml`、`src-tauri/Cargo.lock` 和目标系统动态依赖生成生产依赖清单。逐项确认许可证兼容性、署名和许可证文本要求。复核 Release 文件不包含开发缓存、日志、配置或用户数据。

### 持续运行

Windows 11 x64 和 Ubuntu 22.04 x64 分别验证一小时前台运行和一小时托盘隐藏。覆盖空闲、持续刷新和记录更新。每分钟记录 CPU、内存和子进程数。确认预热后无持续增长，退出后无本次创建的 CLI 或 WebView 子进程。

## 发布步骤

1. 更新三处版本及锁文件，运行 `pnpm verify` 和 `pnpm build`。
2. 等待 GitHub 上四个 `CI` 构建全部通过。
3. 创建与版本一致的签名 tag，例如 `git tag -s v0.1.0`，然后推送 tag。
4. 等待 Draft Release 生成，完成实机、签名、公证、许可证和持续运行闸门。
5. 用签名后的 Windows 和 macOS 文件替换 Draft 中的对应文件，并更新 SHA-256。
6. 核对八个产物和 MIT `LICENSE`。在 Windows 和 Ubuntu 重新校验 SHA-256 与启动行为，并核对 Windows 和 macOS 签名结果。
7. 公开 Draft Release。

Linux 产物在 Ubuntu 22.04 runner 构建，运行环境需要 WebKitGTK 4.1 与 AppIndicator。Wayland 下的置顶、全局位置和托盘能力取决于合成器，应按降级路径实测。
