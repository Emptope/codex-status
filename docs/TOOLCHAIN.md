# 本机工具链

2026-09-05 已更新并验证。此记录只证明当前机器的开发环境，不替代规划中的发行包和跨平台验收。

| 工具         | Ubuntu 24.04 / WSL2                                                    | Windows 11 x64                                 |
| ------------ | ---------------------------------------------------------------------- | ---------------------------------------------- |
| Rust / Cargo | 1.98.1                                                                 | 1.98.1                                         |
| Node.js      | 24.20.0 LTS，nvm 默认版本                                              | 24.19.0 LTS，winget 安装源版本                 |
| npm          | 11.19.0                                                                | 11.14.1                                        |
| pnpm         | 11.25.0                                                                | 11.25.0                                        |
| 原生编译器   | build-essential                                                        | Visual Studio 2022 17.14.14 / MSVC 14.44       |
| 桌面依赖     | GTK 3.24.41、WebKitGTK 2.52.6、Ayatana AppIndicator、librsvg、patchelf | Windows SDK 10.0.26100，包版本 10.0.26100.7705 |

Rust 两端均通过实际编译、链接和运行测试；临时测试源已删除。WSL Clippy、rustfmt 均可用，数据探测的 9 项回归测试通过。Codex CLI 保持已验证的 0.153.4。

WSL 通过既有 nvm 升级 Node.js，原有全局包已迁移，未删除旧版本。已有终端若仍使用旧 PATH，可重新打开终端或执行 `nvm use default`。

Windows 全局 npm 源仍为原镜像。本次 pnpm 安装临时使用官方 registry 和既有本机代理，未修改全局源或代理配置。Windows 的当前 winget LTS 清单比官网 Node.js LTS 落后一个补丁版本，二者均满足本工程 Node.js 版本要求。

## Windows 构建

普通 PATH 中存在同名 `link.exe`，可能抢先匹配 MSVC 链接器。仓库提供 Visual Studio 开发环境入口，只影响当前构建进程：

```powershell
./scripts/build/windows.ps1 verify
./scripts/build/windows.ps1 dev
./scripts/build/windows.ps1 build
```

Windows 和 WSL 不能复用另一平台生成的 `node_modules`。pnpm 内容仓库固定为 `D:\Projects\.pnpm-store`，两端按锁文件完整性校验包内容；Windows 统一入口会先以非交互模式执行离线的 `pnpm install --frozen-lockfile --trust-lockfile`，缓存不全时再联网执行同一锁定安装。切回 WSL 或其他系统时须在目标系统执行锁定安装。统一任务入口清理 `build` 内的页面、单文件发行产物、截图、测试结果和遗留安装包，并拒绝同时启动另一构建任务；项目内的 `build/cargo/<platform>-<arch>` 与 `build/vite-cache` 会跨任务复用，Cargo 缓存不会跨宿主平台混用。`pnpm build` 只生成一个带平台和架构后缀的可执行文件，不创建安装器或应用包。入口还会归一化 `/mnt/*` 上 Cargo 指纹时间，避免挂载时间精度使缓存被误判失效；需要完整重编译时才显式执行 `pnpm clean:cache`。

Windows 桌面进程可能继承安装 CLI 之前的旧 PATH。应用启动 CLI 时保留进程 PATH 的优先级，同时读取注册表中的最新用户和系统 PATH；无需注销即可在刷新后发现新安装的可执行程序。显式绝对路径仍按原值执行，不重新解释。

## 其他机器

需要 Rust 1.98.1、Node.js 24 LTS 和 pnpm 11.25.0。Linux 原生构建依赖：

```sh
sudo apt-get install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

Windows 需要 Visual Studio C++ 工具和 Windows 11 SDK；macOS 需要 Xcode Command Line Tools。尚未验证 macOS 实机环境。
