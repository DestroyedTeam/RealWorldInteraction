# M2 r3：阶段预算与主任务优先（离线修复）

## 当前状态

r3 的编排修复可独立复核，但 **不可排期、不可进入 Windows**。`readiness.json` 固定为 `ready_for_live=false`，真实入口在发起 guest 调用前拒绝。原因是当前 Rust 产品尚无经过验证的真实 Notepad 独立窗口创建能力；不得把这个阻塞转嫁成“再请 owner 布置两个窗口”。未来应由新的产品候选解决，不能仅翻转此标志或沿用本 run/grant。

r2 实机结论是 **distribution pass / task not_run**：安装与两 EXE/两文件 SHA 读回通过；bootstrap、实际模型和 RCU native input 均未运行。owner 回执说明旧四标签恢复，decoy 仅为旧窗口的新标签，baseline 未打开。没有可认领的新 root/decoy。cleanup 读回 0 个候选进程、0 个候选任务，强制终止为空；这不是中途取消通过。旧 HWND 394290 / PID 7524 不能关闭或杀进程。

## 根因与修复范围

旧 install/bootstrap 的 `gate(3300)` / `gate(3000)` 将一小时误当成“前 5/10 分钟内必须完成 setup”，没有从实际后续阶段计算余量；同时把可选 decoy 布置放到主任务之前。r3 删除这两个阈值，新增纯函数预算策略，保留不可自动延长的一小时外部 allocation 和提前五分钟输入截止。

只修改 Mac 开发验收编排。Rust 31 个 build-source 文件和三个产品二进制保持原 SHA，没有重编或重跑 Rust 回归。PowerShell 沿用 r2 短入口/原始 grant 文件协议，唯一脚本差异是新增只读 `oracle-main` case，避免主任务验证占用 stop/restore 的一次性 oracle 名称；它不增加应用启动或输入能力。另修复只读 MCP 开发客户端缺失 `import time`，用真实本地假 MCP 进程执行 RPC 验证，未连接 VM。

## 预算（秒）

| 阶段 | 本阶段 | 另外保留的必需步骤 |
|---|---:|---|
| install | 180 | bootstrap、180 秒确认、完整模型任务、文件 oracle、主窗口 UIA oracle |
| bootstrap | 180 | 180 秒确认、完整模型任务、文件 oracle、主窗口 UIA oracle |
| full-task | 1335 | 120 秒启动 + 1200 秒完整模型预算 + 15 秒关闭；之后另留 readback 和 oracle-main |
| readback | 90 | 主任务尚未独立复核时另留 oracle-main |
| 每个只读 UIA oracle | 240 | stop 前缀验证后还留 restore、最终 oracle/readback |
| wrong-focus | 315 | 120 秒启动 + 180 秒模型 + 15 秒关闭；另留 decoy oracle |
| stop | 375 | 120 秒启动 + 240 秒模型 + 15 秒关闭；另留前缀 oracle、restore、最终 oracle/readback |
| restore | 375 | 另留最终 oracle/readback |
| cleanup | 90 | 已从所有非清理阶段预留的 300 秒中使用；不足完整 90 秒则不宣称能完成 |

**所有非 cleanup 阶段额外保留 300 秒清理时间。** 这些是工程上限，不是声称已测得 Windows 原生耗时。每阶段写入一次性 `phases/` 记录，同时用 UTC 和 monotonic 时钟限制运行；RPC/分发等待受阶段剩余时间限制。模型启动前必须还容纳完整固定预算，不再用 `min(...)` 静默压缩实际模型任务。输入 grant 还会被当前模型阶段截止时间收紧，且永远不晚于总截止前五分钟。

## 假时钟验收

bootstrap 所需总余量为 2325 秒：180 + 180 + 1335 + 90 + 240 + 300，即 38 分 45 秒。它来自可审计的必需步骤之和，而不是经验式“至少还剩 50 分钟”。

| setup 已耗时 | 剩余时间 | 主任务链准入 |
|---|---|---|
| 12 分钟 | 48 分钟 | 允许 bootstrap，前提是有效 owner return |
| 20 分钟 | 40 分钟 | 允许 bootstrap，前提是有效 owner return |
| 35 分钟 | 25 分钟 | 不启动；预算不足，不缩短模型任务凑通过 |
| 55 分钟 | 5 分钟 | 只允许 cleanup；不启动只读 oracle、bootstrap 或任何输入 |
| 60 分钟 | 0 | 任何 guest 操作都不允许 |

还覆盖：每阶段恰好够预算/差一秒，owner 未返还/连接未关闭，阶段中途耗尽预算，HOLD/旧 run/旧 grant/已关闭 run 拒绝，修改固定 allocation 起止拒绝，外部复核证据变更拒绝可选故障用例。测试只使用历史/合成离线时钟与本地假进程。

## 主任务优先与凭据

1. 先解决真实 Notepad 独立窗口创建 blocker。只需一个新 root 即可开始完整任务，不等待 decoy。
2. owner 明确交回 setup/input 控制后，保存 `owner-setup-return.json`；必须含本 run/manifest、owner 原话、返还时间、输入连接关闭、baseline 就绪、既有标签保留及独立 root 证明。模板为 false，不是授权。
3. bootstrap 才能开始；生成全新 native identity/catalog。首次 input grant 必须在该观察后 180 秒内，拒绝未来时间。原生 root 的 HWND/PID/process_created/native_epoch 校验保留；返还回执 SHA 被 bootstrap 固定。
4. 实际模型完成真实 Notepad 保存、重新打开和恢复，再做文件读回和 `oracle-main`。脚本不创建 result、不代替模型操作。
5. 协调任务实际审阅事件、文件/UIA 证据后才写 `full-task-review.json`；它绑定四份实际证据的 SHA，exit 0 不构成通过。故障 case 在此之前不可准入。
6. 之后按实际剩余预算选择可选故障包。wrong-focus 才需要一个独立 decoy HWND；两个窗口同 PID 合法，不因此获得对整个 PID 的清理权。stop 包必须给前缀验证、恢复和最终读回留足时间。
7. 一旦需要改代码、阶段失败或存在不确定执行，留证进入 HOLD，只做本轮候选精确清理并交回。清理不依赖损坏的 owner/review 元数据，不关闭 Notepad 窗口或标签，不删除测试文件，不杀共享 PID。成功清理写 `closed.json`，该 run 永不再激活。

当前 CLI 会被真实窗口创建 blocker 拒绝；不要把下列步骤解释为已经获准执行。短命令与凭据清单见本候选 acceptance/README.md；窗口能力缺口见 docs/m2-native-window-creation-r3.md。
