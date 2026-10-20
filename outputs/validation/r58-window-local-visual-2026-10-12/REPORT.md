# R58：独立只读截图与输入坐标授权分离

日期：2026-10-12（Asia/Shanghai）。自有Receiver128，PID68206/CG43156；不请求人工配合。

## 同一原生窗口前后对照

窗口Quartz bounds为[3225,504,660,552]，Virtual4右边界3720；右侧165pt跨出屏幕，3/4宽度留在已分配虚拟屏。NSScreen ID仍为4，但catalog.display=null，不能把“主所属屏”当成完整包含。

| 路径 | 旧Agent84129a56 | 新Agent4a4e2cf7 |
|---|---|---|
| capture_visual | visual_display_unknown，无图 | 完整1320×1104，122552 bytes |
| 普通snapshot | 已有完整图、7控件 | 同样有图、7控件 |
| 普通输入映射 | mutation_ready=false；original_to_desktop=null | 保持相同限制 |
| 只读图授权 | 无 | frame_created=false；input_authority_granted=false |

因此**没有声称修好了原本可用的普通snapshot**，只是修正独立只读通道误用完整单屏要求的不一致。`record_for_visual`保留真实display=None；NativeMac::visual_window的会话、UID/process birth、on-screen exact PID/CGID、重复行和pre/post几何/拓扑检查不变。SCK仍独立核对精确SCWindow owner/frame/contentRect/pointPixelScale，不借另一窗口或屏幕像素。

## 实际时间顺序原图

1. 旧Agent的普通snapshot本来就能展示全窗口，但不能产生可执行像素映射。

![旧snapshot](01-old-normal-snapshot-already-readable.png)

2. 新Agent的独立capture_visual现在也能展示全窗口，包括屏幕外右侧的滚动区内容。

![新独立只读图](02-new-independent-window-local-visual.png)

以上保留原1320×1104 PNG字节；未使用放大、拼接、上一帧或合成内容充当结果。

## 输入／清理的真实限制

- fresh Close window/AXPress请求被checked_context_unavailable拒绝，native_dispatch_entered=false；原durable记录终态、resources_released=true、pending=false、replay_allowed=false。没有反复点击。
- 另一次明确的清理placement在snapshot/resolve阶段target_binding_unknown/native_execution=false；这是当次AX只读降级缺binding，不是假称位置已改变或一般placement能力不存在。未产生move dispatch，没有用只读图片代替输入frame。
- 三个binding均detached。fixture之后实际lifetime到期，status=closed、expiry1、进程exit0；**不是成功AXClose**。Safe、drag、hover、text、scroll所有输入计数均0。这个限制继续进入后续window-local输入／语义动作实现，不能以只读通道的成功掩盖。

## 回归、部署与回滚

- strict single-display旧合同1pass/0保留；新窗口局部合同真实基线1pass/1fail/exit101，修正后相同命令2pass/exit0。
- 全部11门exit0；workspace762通过/0失败/11忽略/42套，Receiver66、Live UI7，严格fmt/Clippy-Dwarnings、release和架构门均通过。默认fixture完整单屏分配保持不变；新--boundary-right显式、重复/混合模式拒绝、读回精确frame+screen ID。
- Agent68669 / instance4a4e2cf7-a45c-4996-8f79-5f8f2a40a22a同证书/DR原子部署；原inode排他lease、durable资源门先通过；四权限、MCP/配对/CA/profile不变，无Codex重启或新授权。
- 源配置BASELINE false1→MODIFIED true0→独立copy ROLLBACK false1→REAPPLIED true0；六份pristine源hash恢复一致。它不代表生产回滚或通用跨屏输入完成。同四角色与158累计路径、原97immutable基线继续保留。

## 主源及未完成范围

Apple的[desktopIndependentWindow filter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter/init(desktopindependentwindow:))只选择指定窗口；[SCContentFilter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter)独立提供contentRect和pointPixelScale。这里从公开接口推导只读通道可与显示器输入映射分离，并以上述同源原生对照验证；未外推一般隐藏/最小化或另一个Space。

F01–F16完整计划未完成：后台tracking/tooltip、通用window-local输入、foreign-provider菜单/归属、隐藏/最小化、跨应用DnD等继续。R47真实执行中Cmd+Tab、R48实际Host v2 Live显隐/返回/关闭和R56 sheet隔离已完成结果保留，不要求用户重演。
