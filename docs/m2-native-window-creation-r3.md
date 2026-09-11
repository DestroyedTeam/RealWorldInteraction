# M2：真实 Notepad 独立窗口创建与认领缺口

## 结论

**本轮没有验证出一个满足所有限制的 Notepad 新窗口启动入口。不能再次申请时段靠猜参数试装。** 当前 Rust 产品只有窗口 discover/bind 与已授权目标输入，没有应用启动/新窗口生命周期能力。r3 离线完成的是安全认领契约及拒绝路径，不是原生创建成功；因此 r3 保持 `ready_for_live=false`。

## 实机与源码证据

- r2 的 `owner-setup-return.json` / `result.json`：既有 HWND 394290、PID 7524 恢复四个旧标签；decoy 是同窗新标签，baseline 从未打开。没有可认领的 root/decoy；父任务没有 bootstrap/model/native input。
- `crates/rcu-mcp/src/main.rs:292-491` 注册 14 个工具，无应用启动或新建窗口工具。`computer.discover` / `computer.bind_window` 明确只读，不抢焦点、不输入。
- `crates/rcu-core/src/lib.rs:213-243` 的 Backend 提供 discover/bind，不提供应用生命周期；`crates/rcu-windows/src/native.rs:309-356` 对已有 catalog 条目与当前窗口 epoch/owner-chain 重验后绑定。
- `crates/rcu-windows/src/contract.rs:122-179` 的 InputGrant 要求已有 HWND/PID/birth。不能先给一个不存在的窗口发输入 grant，也不能把 app 激活返回的 PID 当成新窗口所有权。

这些结论已经按当前冻结 Rust 源码核查，见 evidence/native-capability-audit.json；产品字节未修改。

## 官方资料实际支持什么

Microsoft 的 Notepad 标签公告描述了多个文件共享窗口、拖出标签变成窗口，以及控制默认打开方式的设置。它**没有在该文档中给出可保证独立新窗口且不恢复会话的 CLI 契约**；拖旧窗口内标签或修改默认设置也不满足本轮约束。该资料是 2023 年功能说明，不代表确认了当前 VM 的具体 Notepad 版本。[Microsoft Notepad 标签说明](https://blogs.windows.com/windows-insider/2023/01/19/tabs-in-notepad-begins-rolling-out-to-windows-insiders/)

`CreateProcessW` 保证的是进程/主线程创建；返回时应用可能尚未初始化完毕。由此不能推出 Notepad 已生成一个独立且空白的新 HWND。[Microsoft CreateProcessW](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)

`IApplicationActivationManager::ActivateApplication` 激活当前会话中的应用，并返回满足激活契约的实例 PID；参数是应用自定义字符串。**推断边界：** 仅凭此 API 成功和 PID，不足以证明独立新 HWND、无旧标签恢复或文档隔离。[Microsoft ActivateApplication](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication)

未采用猜测的 Notepad 参数；没有借用 Terminal 的 `-w new`、Notepad++ 参数或 PowerShell 的 console-window 语义。没有改设置/注册表、拖拽旧标签、关闭旧窗口、杀 PID、新建 Windows 用户、复制替代 Notepad 二进制或把专用测试 fixture 冒充真实应用任务。

## 产品 Rust 的最小后续实现切口（尚未实现）

1. **独立启动授权**：在 core 增加应用生命周期授权类型，而不是扩大现有 InputGrant。它绑定 host/logon/session/seat/desktop、精确应用路径或 package/AUMID/version/hash、唯一 launch request、允许的本 run 文件和固定截止时间；记录 durable no-replay reservation 后才允许一次原生启动。普通 `computer.targets` 默认依然无启动权限。
2. **明确能力而非通用命令行**：MCP 暴露经审阅的应用生命周期入口；Windows Rust adapter 原生 FFI 实施。必须先确认该具体应用构建确有“不触碰既有窗口/不恢复用户会话”的 new-window 契约；没有证据就返回 `capability_unavailable`，不能先启动试试看、再发现污染旧标签。不得把 Python/PowerShell 当产品启动桥。
3. **原生证据**：启动前在当前绑定下收齐目标应用的窗口 inventory；明确分页完成。启动后用有界原生创建事件/快照收集候选，同时保持相同 session/desktop。返回启动 receipt 不发输入授权；必须证明唯一新 HWND、PID birth/epoch、与本次 launch 的关联，以及新 root 不包含恢复/未知标签，旧窗口未被修改。不能只靠标题或新 PID；无歧义证据缺失即拒绝认领。
4. **一次性认领**：产品返回候选新窗口证据，协调方确认后才生成现有格式的 InputGrant，保留 bootstrap 后 180 秒首次激活、HWND/PID/birth/native_epoch 及 Rust 实时重验。认领前再次确认 native identity；不自动 focus 未分配窗口。
5. **精确释放**：窗口 A/B 同 PID 可以接受；不能对该 PID 做 terminate/job-kill。只能对已证实属于本次 run 的窗口采取明确授权且可验证的窗口级收尾。若可能显示用户内容/保存提示则不自动关闭。既有 HWND 394290/PID 7524 永远不在本轮清理集合。

实际 new-window 契约验证应先在具有相同应用构建、但不含用户既有会话的专用 Windows 研发环境进行。这是未来需要独立分配的验证环境，不是本轮要求 owner 停工，也不是拿 fixture 代替后续真实 Notepad 全任务验收。

## 本轮离线验证与剩余 blocker

`acceptance/window_creation_contract.py` 是**无 I/O 的设计 oracle**，没有启动、输入或授权能力。其测试接受“有足够假设证据的新 HWND 与旧窗口同 PID”的设计形状，但返回 `native_creation_verified=false` 且禁止 kill/close；同时拒绝同 HWND 新标签、恢复旧标签、旧窗口消失/变化、PID birth 改变、跨绑定/不完整 inventory、多候选、未知启动契约、无 launch 关联、修改设置或向旧窗口输入。

这些测试证明拒绝边界可编码，不证明真实 Notepad 会产生这样的证据。目前完整的“不改默认设置、不恢复旧标签、不影响旧窗口”的原生创建契约及其生产实现仍是 **remaining blocker**。预算自测通过、分发成功、离线 mock 都不能清除此 blocker；不得仅修改 readiness 标志后再申请下一小时。
