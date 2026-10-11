# 非标准 macOS 窗口：只读兼容路径（2026-09-25）

**后续状态**：2026-09-26 已按原签名部署 Agent/MCP/Plugin，并修复 Retina 2048px `capture_visual` 校验；见[部署与真实 Paint 回归](2026-09-26-visual-only-deployment-retina.md)。下文“未部署”描述本记录产生时的阶段事实；真实 Wine/GameHub GUI 仍未验收。

## 范围与实际观察

本次仅使用当前 Host 加载的 RWI `mac-host` 进行只读目标/窗口元数据查询，并用 `ps` 核对候选 PID。未启动、激活、移动、截图或输入任何用户应用；未读取内建屏的窗口内容；未部署/重启 Agent 或 MCP。

1. `computer.targets(device_id=mac-host)` 返回 `platform=macos`、同一原生用户/安全会话、Agent 可用，辅助功能/屏幕录制权限为 true。
2. 精确标题过滤：CrossOver 有 CGWindow 8630/PID 84210 和 CGWindow 7313/PID 925，均位于内建屏；未截图。GameHub、奇兽杂货铺无匹配的 on-screen 窗。`Steam Support` CGWindow 6874 在 BetterDisplay 虚拟屏，但 `ps -p 85012` 证实它属于 Arc 浏览器，**不是** Wine/Steam 进程，不作为兼容验收靶场。
3. Wine `winedevice.exe` PID 1941/65083 可见于进程表；精确 `computer.discover(pid=1941)` 没有 on-screen 窗。当前没有可供验证的 CrossOver/GameHub 游戏 GUI。此结论限于查询时刻和本会话；空列表不证明这些产品不受支持。

## 已有路径与缺口

- `discover` 已以 `CGWindowListCopyWindowInfo` 枚举当前会话的 on-screen CGWindow，用 PID 和进程启动时间生成精确窗口指纹；`bind_window` 重新核对 CGWindow/PID/epoch，不借标题或宿主应用推断窗口归属。Apple 说明该 API 只返回当前用户会话中的窗口元数据，`OnScreenOnly` 只列在屏窗口：[CGWindowListCopyWindowInfo](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo%28_%3A_%3A%29)、[OnScreenOnly](https://developer.apple.com/documentation/coregraphics/cgwindowlistoption/optiononscreenonly)。
- 现有 `computer.capture_visual` 和 Live 来源为精确 CGWindow 的 ScreenCaptureKit `desktopIndependentWindow` 过滤，不走 AX 树；其可捕获性仍须由真实窗口及 SCK 回调验证。[Apple SCContentFilter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter)。不能从 CrossOver 的宿主窗或 Arc 页面推断 Wine 子进程画面。
- `computer.snapshot` 原来调用 `NativeMac::observe → metadata()`，要求能从目标 PID 的 AXWindows/子树精确解析同一窗口。因此 **推断**：若 Wine/GameHub 的窗口只有 CG/SCK 表面而不提供匹配 AX 窗，`snapshot` 会失败；本次没有活跃游戏窗口，不能称为已在该应用复现。Apple 将 AX attribute unsupported `-25205` 与 no value `-25212` 分别定义为不支持和无值：[unsupported](https://developer.apple.com/documentation/applicationservices/axerror/kaxerrorattributeunsupported)、[no value](https://developer.apple.com/documentation/applicationservices/axerror/kaxerrornovalue)。

## 本次未部署的实现

- AX 降级白名单只包括 `AXWindows` 明确 unsupported/no-value，或有诊断证据的完整 AX 搜索无匹配。AX 超时、权限拒绝、搜索截断、AX 身份歧义、窗口/进程/会话变化不降级。
- 降级时重新通过 CG/SCK 获取**同一个**已绑定 CGWindow 的原像素图；SCK 校验 PID/窗口 ID/边界，CG 库存与显示拓扑前后相同才返回。SCK 失败或窗口变化原样报错。
- 返回的 Scene 明确标 `visual-only; AX unavailable; no input authority`，`binding=None`、`pixel_to_desktop=None`、`mutation_ready=false`、`elements=[]`。可通过 `computer.image` 查看原像素局部，但不给 `resolve_view` 可解析的 view ID；`resolve`、`act_checked` 和任何依赖该 view 的 click/drag 均不能得到输入引用。Live 仍沿用原有独立视觉来源，不受降级路径授权。
- 相关代码：`crates/rwi-macos/src/native.rs`、`crates/rwi-macos/src/native/visual.rs`、`crates/rwi-mcp/src/server/mod.rs`、`crates/rwi-mcp/src/gateway/convenience.rs`。离线测试在 `native/visual.rs` 与 `server/tests.rs`。

## 验证结果与剩余步骤

- `cargo fmt --all -- --check` 通过。
- `cargo test --workspace --locked --offline`：520 通过、10 忽略。覆盖 AX 降级分类、visual-only 场景无输入绑定/映射、原像素 tile 可读、无法 `resolve`/`act_checked`/`resolve_view`。
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` 通过；`git diff --check` 通过。
- 尚需由主任务按正常签名/部署流程更新 Agent/MCP；之后在**用户明确分配的虚拟屏**上对实际 CrossOver/GameHub 子进程窗口执行 fresh PID/CGWindow/epoch 发现、精确绑定、`capture_visual`、`snapshot` 与 Live 只读验收。未完成这一步前，不声称非标准应用实机兼容通过。
