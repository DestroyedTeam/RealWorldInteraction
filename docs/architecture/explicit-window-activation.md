# 显式窗口激活：恢复状态与前台状态分开

2026-09-13 的真实 MCP 测试发现：Windows 对最小化 Explorer 的
`SetForegroundWindow` 返回成功，原生前台确实是目标，但窗口仍在最小化位置。
因此 **foreground 不等于已展开，更不等于获得了可用的键盘控件焦点**。

## 现在的合同

1. 仅在调用方明确 `activate_window` 时工作；targets/discover/bind/observe 不恢复或抢焦点。
2. 新鲜 PID、窗口、进程出生标识、native epoch、原生会话、原授权和 lease 均需匹配。
3. 读取 `before_presentation`：仅对 visible、非 cloaked 的最小化目标请求一次恢复。
4. 恢复本身可改变前台，必须在 durable reservation 和 native-effect 计数之后。
5. 同一 **500 ms / 50 slices** 内观察实际状态；恢复已使目标处于前台，则不再调用
   SetForegroundWindow；否则在新检查后最多调用一次。
6. 每阶段保留取消、用户输入／焦点变化、身份、事件、lease/deadline 检查；先前恢复
   的副作用不能被后续“未派发”覆盖。最终清理后重新读取 presentation。
7. `after_presentation` 必须证明未最小化、可见、非 cloaked、几何可用，才返回兼容的
   `foreground_observed`／`already_foreground`。这些仍不是像素或业务成功证明。

`ShowWindowAsync` 的非零返回只证明请求已启动；已排入系统队列的显示事件不能靠
取消令牌撤回。超时／介入后不追加焦点请求，不自动重放，保留不确定状态。
依据：[ShowWindowAsync](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindowasync)、
[ShowWindow / SW_RESTORE](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow)。

## 跨层边界

- core 负责显式阶段与公共预算；Windows driver 只负责原生状态和单次 API。
- ObservedLifecycle、GuardedLifecycle 都透传状态／恢复，并保留累计 native-effect。
- 不支持的 driver 明确 unavailable；不伪造其它平台的 presentation。
- MCP 请求/schema、wire v2、SDK 没有变化；新增回执字段为 additive/default。
- 未加入应用名、文件名、业务流程、AttachThreadInput 或模拟 Alt 等聚焦规避技巧。

## 实际验证

现有 Host `45ae1911…` 未再重启，远程更新 Windows agent 后即可使用此修复。
同一 Explorer 从 minimized=true 变为 false；恢复约 **105 ms**，没有额外
SetForegroundWindow（`api_accepted=null`），lease 释放，截图证实窗口展开。

随后发现窗口已展开但未获得精确键盘控件焦点：没有强行发快捷键，而是根据新截图
显式点击地址栏、重新确认 `Address Bar` 焦点，再打开系统 Run 界面。最终由 GUI
启动新 Paint 进程并打开既有 PNG；原图像素和磁盘 SHA 都与基线一致。

证据：[按顺序看截图](../../outputs/acceptance-20260913/README.md)。本地原始记录在
`work/resume-and-performance-20260913/` 的 `minimized-explorer-*`、
`restored-explorer-*`、`native-run-paint-reopen.json`、`paint-new-process-reopened.json`。

公开门禁：默认393通过／4忽略，全feature480通过／5忽略；fmt、严格clippy、Windows
ARM64构建、架构ratchet通过。独立 reviewer 另跑36项定向回归。
不将这些结果写成“所有焦点边界已解决”或“已与 Computer Use 全面打平”。


## macOS 显式前台切窗（2026-10-09 源码候选）

新增 `computer.focus_window`，与 Windows 的 lifecycle grant + `activate_window`
分离；后台 click／drag／move 不调用它。用正常 typed calls 执行：

1. 选中明确授权的窗口，fresh observe（≤10秒）并 resolve `kind=window`。
2. 从 fresh `computer.targets` 读取 `current_foreground` 的
   PID/window/process_start/native_epoch；缺失或未知就拒绝，不猜窗口、不给 null grant。
3. claim 同一 desktop lease，指定 stable request ID 与完整 expected_foreground。
4. 原生只对该 AXWindow 请求一次 `AXRaise`，必要时对所属 App 一次 `AXFrontmost=true`。
   每次边界重验原／目标窗口、进程、会话、HID activity；最多 500ms 顺序 readback。
   无快捷键、鼠标、warp、unhide/restore、隐式 replay 或重试。
5. release；重新 observe/targets，核对精确目标前台。原 scene/ref aliases 均失效。

`foreground_observed` 仅为采样到的精确前台状态，不是 app 业务目标，也不授予后续
键盘/安全桌面权限。已在目标窗口的分支没有 native requests。API 返回、AX writable
与动作存在不是效果保证；异常后可能有迟到效果，保留 uncertain/no-replay。
Public API 依据：[AXRaise](https://developer.apple.com/documentation/applicationservices/kaxraiseaction)、
[AXFrontmost](https://developer.apple.com/documentation/applicationservices/kaxfrontmostattribute)。

本次这是新增 tool schema（39→40）的源码候选；当前 loaded adapter 仍 39 tools。
正式 Host-native 验收必须在协调共享 Agent 更新和 MCP 重载后，通过 loaded tool
操作专用 Receiver 完成，不用独立 stdio probe 替代实机 Host 验收。

## 2026-10-10 实机：声明不等于参数路由已接通

重启后Host实际40工具，own Receiver调用focus却unknown capability。共享catalog::validate
漏了FocusWindowArgs分支，Gateway在Prepare前拒绝。已最小补齐并增加每个公布工具都有
validator的ratchet、缺字段/类型检查及真实MCP child+mTLS单次转发测试。native身份/
lease/foreground/HID/no-replay均保留。Agent/MCP已同稳定证书实际部署，新磁盘进程
正确进行required-field校验；当前旧Host仍待重载、前台native goal未通过。
不能仅用tools/list代替实际调用验收。

## 2026-10-10：真实 loaded Host 验收与 Describe 易变字段

40工具与 validator 现均确认。发现 owner catalog 的 foreground/HID age 缓存不能作为
当前状态；Describe 利用已有独立 probe 对响应克隆中的 current_foreground 和
user_input_recent_seconds 按同 desktop identity刷新，未知/null不沿用旧值。
原binding/owner/selection/input_opt_in和native guard不变，Describe不等待executor锁。

Agent-only同稳定签名部署，不需另一次Codex重启。两个own app真正A→B切换，
fresh targets与独立fresh observation均是目标PID/CGWindow；already_foreground
0 native requests和stale tuple0 dispatch也分别观察。只验收explicit native foreground，
不推断后台AppKit key/order、physical-user连续性、通用hover或业务goal。

报告：/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-fresh-foreground-facts-and-native-focus.md
