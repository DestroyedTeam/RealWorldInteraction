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

随后发现窗口已展开但没有精确键盘控件焦点：没有强行发快捷键，而是根据新截图
显式点击地址栏、重新确认 `Address Bar` 焦点，再打开系统 Run 界面。最终由 GUI
启动新 Paint 进程并打开既有 PNG；原图像素和磁盘 SHA 都与基线一致。

证据：[按顺序看截图](../../outputs/acceptance-20260913/README.md)。本地原始记录在
`work/resume-and-performance-20260913/` 的 `minimized-explorer-*`、
`restored-explorer-*`、`native-run-paint-reopen.json`、`paint-new-process-reopened.json`。

公开门禁：默认393通过／4忽略，全feature480通过／5忽略；fmt、严格clippy、Windows
ARM64构建、架构ratchet通过。独立 reviewer 另跑36项定向回归。
不将这些结果写成“所有焦点边界已解决”或“已与 Computer Use 全面打平”。
