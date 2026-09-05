# Codex Status

一个小巧的跨平台 Codex 状态悬浮窗，采用极简、平面化界面，集中显示额度、会话活动及需要处理的状态。

当前已有 Rust + Tauri 2 + TypeScript + Svelte 5 原型，可在已配置工具链的开发环境运行。数据可行性和窗口原型仍有跨平台实测缺口，尚未进入正式发布阶段。

- [产品与技术规划](docs/PLAN.md)：范围、技术选型、界面、数据边界、架构、跨平台策略和验收条件。
- [数据可行性](docs/FEASIBILITY.md)：已验证的数据来源、协议版本和只读边界。
- [窗口原型记录](docs/PROTOTYPE.md)：构建入口、桌面实测、资源初测和剩余边界。
- [任务清单](TODO.md)：按重要性与依赖顺序推进。
- [仓库规则](AGENTS.md)：开发与交付约定。

## Release

`pnpm build` 将当前宿主的版本化可执行文件和 SHA-256 写入统一的 `build/artifacts` 目录，并保留该目录中其他平台的已有产物。推送与 `package.json` 版本一致的 `vX.Y.Z` tag 后，GitHub Actions 会验证并构建 Windows x64、Linux x64、macOS x64 和 macOS arm64 发布文件。当前产物未签名；macOS 签名、公证及各平台干净环境实测完成前不视为正式版。
