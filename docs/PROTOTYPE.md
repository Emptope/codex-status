# P0.2 工程与窗口原型记录

验证日期：2026-09-05。本记录只覆盖已实测环境，不替代 P0.2 的三平台验收。

## 当前结果

Rust、Tauri 2、Svelte 5 工程和统一任务入口已经可运行。每个入口清理 `build` 内的页面、单文件发行产物、截图、测试结果和遗留安装包，并保留 `build/cargo/<platform>-<arch>` 与 `build/vite-cache` 增量缓存；Vite 不监听生成目录。`pnpm build` 只输出 `build/release/codex-status-<platform>-<arch>` 可执行文件，不生成安装器或应用包。应用和托盘使用统一生成的 `>_<` 平面图标。构建锁会拒绝并发任务，Windows 入口先加载 Visual Studio 开发环境。格式、类型、Clippy、Rust 测试、Node.js 测试、前端测试及 Playwright 页面测试已纳入同一个验证命令。

Cargo 的可复用 target 缓存保持在项目内的 `build/cargo/<platform>-<arch>`，没有移动到 WSL 文件系统或项目外，也不会在 Windows 与 WSL 间混用。`/mnt/*` 挂载会把 Cargo 主动设置的指纹时间截为整秒，却保留自然写入文件的纳秒，导致生成文件被误判为构建后变化；统一入口会在编译成功、失败或中断后归一化 Cargo 生成目录的时间，不改源码、不删缓存。实测同一测试命令由错误重编译时的 78 秒降至缓存命中时约 9 秒，第三方依赖不再重复编译。需要主动排除缓存影响时才显式执行 `pnpm clean:cache`。

WSL2 / WSLg 从清理目录实际编译并启动了无边框置顶窗口。240px 与桌面宽度的浏览器交互测试通过，长文本、18px 字号、深色主题、折叠态和离线降级没有横向溢出。Playwright 需要允许测试服务器监听 `127.0.0.1:1420`；受限沙箱内会以 `EPERM` 退出。

窗口实现包含拖动、置顶、尺寸自适应、位置保存及屏幕内校正。托盘包含显示/隐藏、刷新、设置和退出；托盘创建失败时关闭按钮正常退出，不会把窗口隐藏成无法恢复的后台进程。Windows 单文件发行程序已实际启动。Windows 子进程解析会保留当前进程 PATH 的优先级，并合并注册表中的最新用户和系统 PATH；即使启动进程继承的 PATH 尚未刷新，也已实测定位并读取原生 CLI 0.153.4。位置校正已有纯逻辑覆盖；托盘点击、实际拖动、位置恢复和显示器变化仍需桌面实测。

Windows 增量构建曾在图标源文件更新后继续复用旧的已编译资源。从新单文件 exe 提取出的仍是旧图标，证明问题不只是资源管理器缓存。桌面构建脚本现显式跟踪整个 `icons` 目录；复用同一 release 缓存重建后，Windows 资源库重新生成，从单文件 exe 提取出的图标已变为新 `>_<` 图标。该跟踪同时覆盖其他平台的图标格式。

Windows API key / 外部提供方环境曾稳定复现为 14 个会话可见，但 `account/rateLimits/read` 返回 `chatgpt authentication required to read rate limits`，随后状态被误记为 Offline。修复后按认证模式选择额度来源：ChatGPT 使用 App Server RPC，API key / 外部提供方从主数据根的本地 `token_count.rate_limits` 增量读取，未登录和不支持模式不请求额度。额度按文件、额度桶标识和事件时间归并，旧事件不能覆盖新快照，其他数据根也不能串入当前账户。

2026-09-05 使用真实 Windows settings 和原生 CLI 0.153.4 持续运行发行采集器 15 秒，最终稳定读取 14 个会话、1 个额度桶的 2 个窗口，`error` 为 `null`，没有再次进入 Offline；采集器正常退出且 stderr 为空。该实测推进 P0.3 的数据接入，不替代 P0.2 尚缺的桌面交互与 macOS 验收。

统一入口现在按进程树停止开发服务器、桌面进程和采集子进程。进程树回归测试覆盖正常停止与异常退出；WSLg 在 Ctrl-C 后未发现应用、开发服务器或查询进程残留。运行时的目录遍历和分块读取检查关闭信号，CLI 调用和回收带超时；正常关闭超过 3 秒时在设置已持久化后执行进程级退出。Windows 真实数据根已复现并修复关闭阻塞，五秒内退出且无原生 CLI 残留。

## 初始资源记录

WSLg 发行构建采样 RSS：桌面主进程 194076 KiB，WebKit 进程 59172 与 317908 KiB，查询进程 49592 与 106980 KiB，合计 727728 KiB，约 710.7 MiB。该结果比 150 MiB 常驻预算高约 560.7 MiB；P1.2 必须按相同进程边界继续定位和优化，不能通过遗漏 WebView 或查询子进程缩小结果。

## 复核

```sh
pnpm verify
pnpm build
pnpm dev
pnpm preview
pnpm clean:cache
```

Windows 使用：

```powershell
./scripts/build/windows.ps1 verify
./scripts/build/windows.ps1 dev
./scripts/build/windows.ps1 build
```

P0.2 仍缺 macOS 最小窗口实测，以及 Windows 与 macOS 的托盘点击、实际拖动、位置恢复、显示器变化和无托盘回退实测，因此保持未完成。
