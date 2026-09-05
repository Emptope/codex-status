# P0.1 数据可行性验证

验证日期：2026-09-05。探测版本：Codex CLI **0.153.4**。

## 结论

WSL2 内的双来源只读观察可行：独立查询进程可读取额度，结构化本地记录可观察独立运行的 CLI 会话。该结论不代表三平台桌面验证或正式发布已经完成。

| 项目 | 结果 | 证据与边界 |
| --- | --- | --- |
| ChatGPT 认证环境额度 | 已实测 | `codex login status` 为 ChatGPT；额度查询返回 300 和 10080 分钟窗口及 Unix 重置时间 |
| 生效提供方与认证 | 已实测 | 当前 `codex login status` 与 `account/read` 均识别为 ChatGPT；不会输出账户标识 |
| 独立 CLI 观察 | 已实测 | 10 秒只读观察期间两个会话发生新事件，其中一个写入完成事件；探测器没有发起回合 |
| 运行、完成、用量 | 已实测 | `task_started`、`task_complete`、`token_count.info`；累计快照替换，cached input 不另加到总量 |
| 中断 | 结构已观察 | 历史记录包含 `turn_aborted`；未主动中断用户任务进行实验 |
| 失败、等待批准、等待输入 | 已验证降级 | 脱敏扫描 233 份真实记录后仍无对应明确事件；不得由工具输出、进程退出或时间推断这些状态 |
| API key 语义与额度 | Windows 已实测 | App Server 的额度 RPC 只面向 ChatGPT；API key / 外部提供方的本地 `token_count.rate_limits` 含额度桶、窗口、重置时间和点数信息，与 ChatGPT 订阅额度分开处理 |
| 未登录、离线 | 已实测 | 隔离数据根返回 signedOut；不可达本地代理使额度查询超时，二者均不伪造 0% |
| 原生 Windows 与 WSL 定位 | 已实测 | Windows 原生 CLI 0.153.4 和数据根已由发行采集器读取；另通过 UNC 定位 WSL 数据根的 233 份记录 |
| IDE、桌面应用 | 记录可见，实时未验证 | 原生记录含 8 份 CLI 来源和 2 份 Codex Desktop / VS Code 来源；版本均不在当前支持范围，不推断它们与 CLI 共享实时状态 |

## 只读边界

探测工具仅允许 `initialize`、`initialized`、`account/read`、`account/rateLimits/read`、`thread/list`、`thread/read`。账户查询固定 `refreshToken: false`，会话查询固定 `includeTurns: false`。不恢复会话、不发起回合、不回复任何服务端请求、不写会话文件。

0.153.4 的 `thread/list` 默认可能扫描并修复元数据，因此固定 `useStateDbOnly: true`。实际读取到的 20 个线程均为 `notLoaded`，同时本地记录证明仍有独立 CLI 活动；不能把该值映射为停止或完成。

本工具不打开认证文件，不存储账户标识、原始对话或服务端错误正文；报告仅输出允许的字段和散列标识。实测确认即使只调用只读接口，查询进程初始化也要求数据根内的 SQLite 状态运行时可写；只读沙箱会在初始化前退出。因此发行实现必须允许该进程维护自身状态，并把这种内部写入与会话只读边界分开说明。2026-09-05 再次以 `refreshToken: false` 查询前后，`auth.json` 的大小和修改时间均未变化；当前环境没有系统调用追踪器，因此该结果是元数据审计，不扩大为“进程从未读取认证文件”。工具只管理自己启动的查询进程，结束时等待退出；本轮退出后未发现残留查询进程。

## 原型限制

`scripts/probe` 是无第三方依赖的能力验证工具，不是发行应用的状态后端。它只支持已观察的 0.153.4 记录结构；其他版本明确返回不支持，不按项目、会话名或文件名特判。扫描上限为 10000 文件、8 层目录，只跟踪最近 10 个候选记录，每轮每文件最多读取 32 MiB，达到上限会报告 `limited`。

增量游标处理半行 UTF-8、损坏行、截断、替换、删除及超大行；尚未承担生产级文件监听、完整历史索引、同尺寸原位改写和长期运行保证。

P0.1 验收已通过。真实 ChatGPT 额度、独立 CLI 的运行/完成/中断/用量、未登录与离线降级、Windows 原生与 WSL 数据定位均已实测；API key / 外部提供方的本地额度采用 0.153.4 真实记录和回归测试验证。233 份 WSL 记录不包含可证明失败、等待批准或等待输入的事件，因此这些字段按验收要求明确降级，而不是阻塞后续工程。IDE 与桌面应用仍只证明记录可见，不能据此宣称实时支持或完整首版已交付。

## 复核

要求 Node.js 24 和已安装的 Codex CLI。`--data-root` 显式覆盖当前环境的 `CODEX_HOME`，默认为用户目录下 `.codex`；探测不创建或修改该目录。

```sh
node scripts/probe/main.mjs
node scripts/probe/main.mjs --local-only --seconds 10
node scripts/probe/main.mjs --data-root /absolute/data/root
node scripts/probe/contracts.mjs
node --test --experimental-test-isolation=none scripts/probe/*.test.mjs
```

当前执行沙箱不允许查询进程写入用户数据根内的状态运行时；真实 app-server 探测需在允许其正常维护内部状态的环境运行。显式关闭测试进程隔离后，9 个测试逐项执行通过，真实查询退出已等待。

协议由实际安装的 CLI 生成，选取的只读接口契约与 SHA-256 清单位于 `docs/contracts/0.153.4/`。格式行为仍以该版本实测为准。

官方依据：[Codex App Server](https://learn.chatgpt.com/docs/app-server)，2026-09-05 读取；包括初始化、只读线程读取、认证和额度接口。官方文档可能比已安装版本更新，不能替代版本化契约。
