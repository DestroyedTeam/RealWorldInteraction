# M2 0.3.0：本地候选与一小时专用验收

## 本轮边界

本轮只在 Mac 本地准备代码、回归、候选包、SDK 元数据、插件和执行脚本。没有重新占用 Windows，没有新授权、截图、输入、全局安装、Codex 配置修改、commit/push/tag。M1 包及插件保持原 SHA。

M2 的 Windows 输入已实现并交叉构建；**这不等于原生输入或完整任务验收通过**。常见应用的编辑、保存、重新打开、恢复，以及真实错焦点、用户停止，仍须在新的一小时独占窗口执行。

固定候选后才能预约；现场不跑 cargo、不改源码、不临时调整工具审批或写测试程序。若遇到必须改代码的缺陷，停止本轮、保留证据、清理交回，回 Mac 修好再约新一轮，不占着 VM 边修边测。

## 候选行为

- Rust `SendInput` 实现点击、双击、右键、Unicode、有限组合键与滚轮；每个手势/Unicode scalar 保持平衡，逐批检查原生身份、前台、几何、UIA 元素、用户持键、授权和租约。没有 SetForegroundWindow、剪贴板、UIA SetValue/Invoke 或隐蔽输入脚本。
- 输入需显式 `InputGrant`：run、configured VM route、native identity、root HWND/PID/process creation、≤60 分钟到期。只允许 root 和实际 native-owned dialogs；后者不能用同名/同 PID 猜测。现场 grant 提前五分钟停止新批次输入，预留清理。
- 原生 Windows identity 的 `host` 是 guest 机器身份，`guest=null`；物理 Mac→VM route 由外部 owner 分配记录，不能伪称 Win32 自证。
- 每次动作保留 durable no-replay reservation；部分效果、断线、取消不能换 ID 重放。SendInput 返回插入数不证明应用效果；必须另读 UIA/文件。参见 [Microsoft SendInput 契约](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)。
- 取消/lease/grant 是协作式批次边界，不是硬原生抢占。`native_call_pending`/`resources_released` 与 release-only cleanup 单独记录；不能用 cancel ack 冒充已停。
- `discover` 增加 `pid`/`title_contains`、8 条/12 KiB structured JSON 分页、10 秒 cursor。MCP text 还会复制 structured 内容，不能把 12 KiB 说成总传输大小。135 窗口离线用例验证不再默认返回全部记录；真实上下文/性能效果尚未测。
- `computer.bind_window` 的 SDK annotation 改为 read-only：只原生读及连接内绑定，不发送输入。`act/claim/release/cancel` 保留非只读，不能靠谎报只读绕过客户端审批。
- Windows UIA 对跨进程 hosted provider 重新证明 root ancestry；键盘要求确切已聚焦元素，文本要求非密码可写 Edit/Document。TextPattern 读回有长度上限，不能静默 trim/normalize 后过测。

## 本地与原生证据分开

| 层级 | 当前证据 |
|---|---|
| 离线 Rust 回归 | Mac host 执行 73 passed、1 ignored；含 portable Windows input-plan/contract 用例 |
| lint/build | fmt、Mac clippy、Windows ARM64 cross-clippy、两端 release build 通过 |
| packaged SDK | 真正启动 M2 Mac 发布二进制，14 tools、默认 0 targets、版本及审批 metadata 读回 |
| 脚本预演 | 本地 fake stdin ACK/错误 SHA/不读 stdin 超时、授权时段/对象校验、固定 prompt、缺授权阻断 |
| PowerShell/Windows setup | 已准备且静态检查；本机没有 PowerShell parser，不声称 guest 执行过 |
| 实际 Codex 集成 | 父任务已验证进程级 inline TOML approval map；该历史证明不等于 M2 当前工具已在桌面任务热加载 |
| M2 原生完整任务 | **not_run** |

无依赖更新：`Cargo.lock` 仅 workspace 版本 0.2.0→0.3.0；没有以运行时、语言或 fixture 数字推断性能优越。

## 固定 60 分钟现场协议

| 分钟 | 专用用途与停止点 |
|---|---|
| 0–10 | 一次 ZIP stdin 传输、SHA 读回、Limited interactive Rust server；owner 分配全新 Notepad baseline/decoy 窗口；只读 native bootstrap；owner 确认 fresh root/decoy tuple；激活固定 grant |
| 10–32 | 实际 Codex→新 Rust MCP→常见 Notepad：中文/emoji/换行编辑，真实菜单/另存为、打开 baseline、再打开 result、完整独立 UIA 验证，最后回 baseline |
| 32–37 | owner 只将焦点移到已分配 decoy；模型不得抢焦点或输入。计数零增量与 decoy 独立未保存缓冲读回分别取证。此例不冒充高频抢焦点竞态 |
| 37–44 | owner 恢复 root 前台；模型经 RWI 清空临时缓冲，发送一次 512 ASCII A；owner 在确实出现输入后按住 Escape 一秒；保留停止计数/operation/实际前缀 |
| 44–50 | 独立 UIA 检查精确停止前缀；显式启动 restore 模型阶段，重新观察后恢复 baseline，不重放旧动作 |
| 50–55 | 实际文件 SHA/字节、baseline/result/decoy；恢复 UIA、资源/操作终态与每例真实结果；没有时间就不追加场景 |
| 55–60 | 不发新任务输入；停止本轮模型，等待 worker；仅清理精确 candidate 路径/任务，读回剩余进程和任务，列出保留文件，截止前交回 |

这里的一小时是整段专用窗口，不是把若干五分钟拼起来。脚本拒绝 5 分钟或超一小时的 handoff，拒绝窗口前/到期后调用，不自动延长。现场只填 run 的事实参数、HWND/PID/epoch/identity 和固定截止，不临时改候选逻辑。

## 完整任务验收与失败分类

结果文件只能由模型经新 RWI 在真实 Notepad 中产生。开发脚本只写预先声明的 baseline/decoy，绝不写 result；独立文件 oracle 只读字节。原生 readonly oracle 可用新 Rust MCP 连接验证精确 UIA 元素，并单独标 scripted oracle，不冒称模型选择工具。

必须同时具备：实际 Codex tools/call 事件、固定 candidate SHA、真正菜单/dialog owner 链、保存/切换文档/重新打开、精确 UIA 验证、result 文件字节、baseline/decoy 未变化和恢复证据。CLI exit 0、动作确认、截图或正确拒绝都不单独构成通过。

停止用例：确实插入了至少一个且少于 512 个 A，确有 owner Escape 干预、operation 终态和 `resources_released=true`、随后无新任务输入，精确前缀稳定读回。计数只能计算候选前缀，不能替代 UIA；完整 512 个 A 或首字符前拒绝记 `not_exercised`。`computer.cancel`、CLI disconnect、GUI Escape 必须分场景；本轮未安排的保留 `not_run`。

真实 UIA 不支持可写 TextPattern、文本/换行表现不一致、Notepad 打开独立非 owner HWND、新进程/epoch、原生焦点不稳、客户端不发现工具、approval 阻断、超时、实际任务失败，逐项保留，不弱化目标凑通过。不要用自制编辑器替代 common-app 的完整任务。

清理不默认关闭 Notepad：避免丢未保存内容。模型负责已授权的可逆恢复；异常时保留原 app 现场，清理候选后告知 owner。若强制终止了候选，日志如实列出，不能声称 graceful cancellation 已通过。候选目录、收据和 state 留存，不自动删除，不在过期后补删。

## 原生尚未覆盖

所有 M2 native input、常见应用成功率、真实高频焦点争抢、DPI/窗口移动竞态、disconnect/reconnect、电脑锁屏/会话改变、多屏、native ROI 输入、长时资源/延迟均不得从本地测试推断通过。首轮固定优先完整任务与两个基础干预；其余继续沿父任务 W01–W12 矩阵单列，不改写为已测。
