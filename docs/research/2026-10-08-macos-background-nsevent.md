# macOS 真后台事件二次调研：NSEvent、hover 与 drag

日期：2026-10-08；GitHub 时间均为 UTC。状态：**研究与契约修正完成，原生候选尚未实现/部署/实机验收**。

后续扩展：用户要求加大覆盖后，已完成[50仓库／150篇技术正文的扩展审查](2026-10-08-macos-background-survey.md)，含维护/只参考分类、补齐难度、复杂度及API范围。本文保留为此前阶段性证据，**当前设计取扩展报告与功能矩阵，不把本篇约7项目的调查称为全面结论**。

## 1. 范围与核心结论

用户指定只读回溯 Strange Beasts General Store 的 **「任务接力/前后台相关」**（`01a10280-b3ea-72c1-9b86-c58ddd9b2cb3`）。本轮读取该会话的相关执行摘要、原项目研究、当前活动候选与既有校准 JSON，另对照 Apple、bgclick、Wine、Hammerspoon、GLFW、SDL、RobotJS 原始资料。没有运行/导入另项目、没有改其代码、没有向该会话发消息；没有启动 GUI、输入、截图、移动游戏或部署服务。它的业务任务仍归原会话。

结论不是“后台做不到”，也不是“换 NSEvent 就全部做到”：

1. **值得优先尝试窗口关联 NSEvent 工厂**，尤其独立补齐 hover 和完整 drag。前台的连续输入语义是很好的对照基准，不能用单个诊断失败否定整项能力。
2. 这条跨进程链仍是 **NSEvent 构造 → CGEvent 桥接 → CGEventPostToPid**。AppKit 本进程队列不是任意第三方进程的输入接口。构造、投递、窗口接收、客户端使用、目标完成是五层不同结果。[Apple mouseEvent](https://developer.apple.com/documentation/appkit/nsevent/mouseevent(with:location:modifierflags:timestamp:windownumber:context:eventnumber:clickcount:pressure:))、[CGEvent 桥接](https://developer.apple.com/documentation/appkit/nsevent/cgevent)、[postToPid](https://developer.apple.com/documentation/coregraphics/cgevent/posttopid(_:))、[postEvent](https://developer.apple.com/documentation/appkit/nsapplication/postevent(_:atstart:))。
3. **旧 dragged-hover 实验不是完整拖拽验收**；旧 hover 失败主要是没有读到预期 tooltip，不是全部投递 API 报错。当前业务门禁还包含历史 reason 不兼容，不应带入 RWI 平台能力结论。
4. RWI 已有 PID/window 定向输入，并非所有输入都要求系统前台；真正仍有 on-screen discovery/capture、显示器完整映射等约束。**构造器替换和解除显示器依赖必须分开改造**。
5. 目标客户端可能重新读取全局光标/全桌面 z-order，覆盖事件提供的位置。Wine、GLFW、SDL 的原始源码给出了具体可检验机制。这是适配线索，不是已证明的本机 GameHub 根因。

## 2. 原项目的实际基石与证据缺口

原项目根：`/Users/vincentadamnemessis/Documents/ChatGPT/Strange Beasts General Store`。
本次活动候选：`/Users/vincentadamnemessis/Documents/ChatGPT/Strange Beasts General Store/artifacts/verified-transactions/combat-click-focus-continuity-20261007/MODIFIED_FILE`。历史原目录不等于活动候选；未复制其 Python 实现为 RWI backend。

### 2.1 构造和派发

候选 `autoshark/platform_macos.py`（审计时 SHA256 `8f039efb10f4961a9442f5358393f40a5746735d8cd380ab421a113f9c26475c`）：

- 378–393、1335–1376：鼠标 NSEvent 工厂填 target windowNumber、窗口局部 AppKit 点，然后取 CGEvent、设置 screen point。
- 1300–1305：全局 top-down 点转为窗口 base 点；含 Y 翻转。后续必须区分 frame/content/titlebar、backing pixel 与 point，不能直接搬到 RWI 的 SPI 坐标。
- 1157–1173：填目标 PID 和两个 window-under-pointer 字段，再 postToPid；本候选未发现手写数字私有字段或 SkyLight/CGS 调用。
- 708–755、1004–1007：使用全窗口目录核对身份和几何，不激活目标；这部分比 RWI 当前 on-screen 目录更宽，但不保证所有隐藏状态可操作/可截图。

值得继承的是窗口关联、双坐标、非激活定向投递和分层证据。**不继承游戏疲劳/地图 oracle、scene 白名单、业务校准成功才开放所有动作的设计**；生产启动/工具发现不依赖业务任务。

### 2.2 校准 JSON 的原始事实

只读扫描目录 `/Users/vincentadamnemessis/Library/Application Support/CreatureCurios Automation/logs` 中 `background-input-calibration-*/result.json`，快照 **137 项**：hover 37、window click 23、keyboard 48、dragged-hover 1、fatigue reveal 28。不是随机同状态试验，也不是 RWI 成功率。可复核投影及 SHA256 见本研究目录的[证据 JSON](2026-10-08-macos-background-evidence.json)。

| 既有记录（目录名） | 观察到的事实 | 不能推出的结论 |
|---|---|---|
| `20261006-120646-655047` | down/up 地图打开与关闭，visually_confirmed=true，active 前后 false | 不证明所有应用或 offscreen 状态可点击 |
| `20261006-134622-482540-keyboard` | 地图打开后 Escape 关闭，visual=true，未用 fallback close，active 前后 false | 不证明全部键盘/IME/快捷键 |
| `20261006-134631-046939-hover` | posted=true；enter/release 各一次 MouseMoved；samples=[null,null]；无目标 PID 接收证明 | 不等于平台拒绝事件；也不能算 hover 成功 |
| `20261006-235519-563499-dragged-hover` | **只有 LeftMouseDragged，没有 down/up**；PID event tap received=true；光标前后不变；active=false；tooltip 未确认 | tap 收到不等于 NSView/游戏消费；更不等于完整 drag 已失败 |
| `20261008-022043/022127/022142-…-fatigue-reveal` | 三个固定 HUD down/capture/up 记录分别读到 0/1000，visual=true，active=false | 这是点击取值，不是通用 hover；也不覆盖之后其他批次的失败/新取值 |

37 个 hover 中 **13 个输入前 blocked，24 个 posted=true 但未视觉确认**。这两类不能混成“发送失败”。上述集合没有完整 down→dragged→up 的实机校准；此结论仅限已扫描集合，不宣称所有历史从未测试。

### 2.3 要重新设计的实验变量

- 通用 hover（候选 1529–1547）是一条 MouseMoved；普通 helper 等40ms。旧 producer（`logs/transaction-20261006-174453/window-dragged-hover-probe/MODIFIED_FILE/autoshark/background_input_calibration.py:294–378`）在 enter 后隔350ms采图、要求两次相同疲劳OCR。tooltip 瞬态/起效延迟/位置错误/未渲染都可能产生 null，**OCR null 不定位接收环节**。
- 完整 drag 函数存在（候选 1713–1773）：down→80ms→12个 dragged（25ms间隔）→finally up；目前先受 action receipt 门禁限制。不能说“没有写拖动事件”。
- 工厂每事件递增 eventNumber；drag pressure=0；通用 pointer 默认带 Command；未显式配置连续 source/button state、delta。工厂可能补默认值，不能凭源码未填就断言最终值为零。必须对照 native roundtrip 和目标 recorder。
- `background_input_capability.py:468–475` 只识别 `single_valid_tooltip_value_not_observed`，旧 hover 记录是 `two_matching_tooltip_values_not_observed`，因此可能落 ineligible。574–586 不解析 dragged-hover kind。**当前 unknown/blocked 含业务记录解析因素，不是 OS 判定**。
- 名为 `routed_not_accepted` 的状态有时只依据 posted/no-visible-delta，没有 PID receive 证明。RWI 应改用明确的 `dispatch_attempted / receiver_observed / goal_unconfirmed`，但不在本轮修改另项目。

原项目已读研究：`/Users/vincentadamnemessis/Documents/ChatGPT/Strange Beasts General Store/logs/research-20261005-124500/background-input-macos-wine.md`，SHA256 `3fb88ad2a509f5381e6be9160dd6c7592a52a5662c1c694e0095f7a521239e25`；其中明确引用 Wine。下文 bgclick 等是本轮扩展检索，不伪称全部来自原会话。

## 3. RWI 当前实现：哪些是真限制

以下路径以唯一产品根 `/Users/vincentadamnemessis/AI Projects/Real World Interaction` 为基准；下表 `native/…`、`native.rs`、`capture.rs` 省略共同前缀 `crates/rwi-macos/src/`，其余列出完整相对路径。

| 位置 | 本轮源码事实 |
|---|---|
| `crates/rwi-macos/src/background.rs:21–46,62–91` | 已用 NSEvent 构造 app-activated 准备事件；鼠标仍用 CGEvent 构造，带 PID/window/私有 window-local setter，再 postToPid。不是系统光标 warp 路由。准备事件的 AppKit 局部状态副作用应由 fixture 单独记录。 |
| `native/metadata.rs:134–137`、`native/input/anchor_native.rs:89–103,197–199` | 普通观察/输入不要求 target 为 OS foreground；跨 app 遮挡不直接阻塞 addressed input。应用内部 AX focus/hit 与系统前台不是同一判定。 |
| `native.rs:455–461`、`native/semantic.rs:174–194` | 正常窗口目录只列 on-screen；目录外可能 window_missing。 |
| `capture.rs:177–178,263` | SCK filter 虽用 desktop-independent window，枚举仍明确 onScreenWindowsOnly=true。 |
| `native.rs:660–673`、`native/metadata.rs:91–104` | mutation mapping 要求完整处于唯一活跃未旋转显示器；这是 offscreen 路线的单独障碍，不能简单删除 guard。 |
| `crates/rwi-core/src/lib.rs:189–196` | 公共 Intent 没有 Move/Hover；typed hover 尚待补齐。 |
| `native/input/drag.rs:32–40,65–144` | 同窗口有界左键拖拽已实现：primer→down→paced dragged→up/cleanup。无跨窗口 DnD。 |
| `native/keyboard.rs:4–5` | source() 每次新建 Private CGEventSource；现 drag 每事件调用。共享一次 sequence 的 source 是需实验验证的改进候选，不是已证实失效原因。 |

`docs/architecture/bounded-pointer-trajectories.md` 的旧 “Mac drag unsupported” 已修正为 dated superseded；9/20 实机 Paint 单次 drag/Undo 证据仍保留，**不外推全部后台状态**。既有焦点/身份/no-replay 安全契约不因此删除。

## 4. Apple 原始契约：hover 和 drag 并非一刀切

| 要素 | 系统契约与实验含义 |
|---|---|
| 鼠标工厂 | windowNumber 与 window base coordinates 都要正确；timestamp/eventNumber/clickCount/pressure 应在 roundtrip 中读回，不只检查函数调用成功。[mouseEvent](https://developer.apple.com/documentation/appkit/nsevent/mouseevent(with:location:modifierflags:timestamp:windownumber:context:eventnumber:clickcount:pressure:)) |
| 进程内分发 | postEvent 是 receiver 的队列；sendEvent 是该 app 分发；不是给 PID 注入的替代途径，NSWindow.sendEvent 不应被外部直接调用。[NSApplication](https://developer.apple.com/documentation/appkit/nsapplication/sendevent(_:))、[NSWindow](https://developer.apple.com/documentation/appkit/nswindow/sendevent(_:)) |
| 悬停/追踪 | 普通 acceptsMouseMovedEvents 默认 false；tracking 可选择 moved/entered/exited，activeAlways 可在 inactive app 中工作。故不能写“后台 hover 一律不支持”。[acceptsMouseMovedEvents](https://developer.apple.com/documentation/appkit/nswindow/acceptsmousemovedevents)、[NSTrackingArea](https://developer.apple.com/documentation/appkit/nstrackingarea) |
| tooltip/entered | inactive tooltip 默认关闭；entered/exited 有 trackingNumber/userData。未知第三方 tracking 指针不能凭空伪造；hover高亮、tooltip、enter/exit、cursorUpdate 要分别测。[inactive tooltip](https://developer.apple.com/documentation/appkit/nswindow/allowstooltipswhenapplicationisinactive)、[enterExitEvent](https://developer.apple.com/documentation/appkit/nsevent/enterexitevent(with:location:modifierflags:timestamp:windownumber:context:eventnumber:trackingnumber:userdata:)) |
| 拖拽 | down→dragged→up 是连续语义；drag/up 通常回到 down 的 view；按住期间普通 moved 不能代替 dragged。非 key 窗口还可能消费第一下用于变 key。前台事件对照应检查实际类型/捕获者，不能只把一次 click 后 move 当完整拖拽。[Event Architecture](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/EventArchitecture/EventArchitecture.html) |
| 按钮/源状态 | pressedMouseButtons 是瞬时全局组合，不能作为事件序列的唯一状态；Private source 是独立状态方案，需要持有 lifecycle 并读回验证。[pressedMouseButtons](https://developer.apple.com/documentation/appkit/nsevent/pressedmousebuttons)、[CGEventSourceStateID](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid) |
| 系统 DnD | slider/画布 drag 与跨窗口文件拖放不同；后者还涉及 dragging session、pasteboard、destination operation。[beginDraggingSession](https://developer.apple.com/documentation/appkit/nsview/begindraggingsession(with:event:source:))、[draggingUpdated](https://developer.apple.com/documentation/appkit/nsdraggingdestination/draggingupdated(_:)) |
| 不可见/暂停 | 完全遮挡、其他 Space、最小化和 inactive 是不同状态；应用可选择停止渲染/工作。因此输入接收、fresh画面与业务推进须分开验收。[occlusion](https://developer.apple.com/documentation/appkit/nswindow/occlusionstate-swift.property)、[Work When Visible](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/WorkWhenVisible.html) |

## 5. 开源源码、Issues、PRs 与维护时点

仅依赖原始源码/仓库记录；repo updated_at 可受 issue/star 等变化影响，**不当作代码更新时间**。固定 SHA 对照见 sources.json。

### bgclick-rev-skill（扩展检索）

只有一个文档和一个提交：`85dc2c49…`，2026-04-18 03:28:45；pushed_at 同日03:28:48，API updated_at=2026-10-08不表示代码更新。2026-10-08 API 查询 all-state Issues/PRs 均为空；未发现独立 LICENSE 文件，metadata license=null，不直接 vendor 实现。

作者路线是 NSEvent mouse factory→CGEvent，补 window 字段、非公开 subtype=3、私有 CGEventSetWindowLocation，最终 postToPid；Command flag 作为避免激活的技巧，不保证任意 app 保留普通点击语义。其参考目标主要验点击计数，**没有通用 hover/drag、所有不可见状态的验收矩阵**。这是技术线索，不是可直接套用的兼容保证。[固定文档](https://github.com/Lakr233/bgclick-rev-skill/blob/85dc2c49ce1bf3413654430df99d91c15687a51f/bgclick-rev-skill.md)、[提交](https://github.com/Lakr233/bgclick-rev-skill/commit/85dc2c49ce1bf3413654430df99d91c15687a51f)、[Issues](https://github.com/Lakr233/bgclick-rev-skill/issues)、[PRs](https://github.com/Lakr233/bgclick-rev-skill/pulls)。原文将私有符号描述为 documented-private，RWI 仍必须标 SPI/兼容风险，而非 Apple 公共稳定保证。

### Hammerspoon

MIT；repo pushed_at=2026-07-08，`libeventtap_event.m` 最新文件提交 `844aa098…`=2024-11-12（smart magnify，不是后台鼠标修复）。指定 app 时 PID→PSN→CGEventPostToPSN，无 app 才走 session tap。[固定源码](https://github.com/Hammerspoon/hammerspoon/blob/844aa09856b84ca3b419f8b214f0361819b231b0/extensions/eventtap/libeventtap_event.m)。

- [Issue #2883](https://github.com/Hammerspoon/hammerspoon/issues/2883)：Chrome 定向鼠标无效果报告；open，2021-05-25建立，2023-11-29更新。不能把 post(app) 的声明当目标已经消费。
- [Issue #3213](https://github.com/Hammerspoon/hammerspoon/issues/3213)：closed，2022-05-17更新；维护者建议 down、起点 dragged、终点 dragged、up，留处理时间/补 delta；用户报告奏效。例子激活 Finder，**不是全后台成功证据**。
- [PR #2144](https://github.com/Hammerspoon/hammerspoon/pull/2144)：mouseMoved 构造支持，merged=2019-09-22；[PR #2140](https://github.com/Hammerspoon/hammerspoon/pull/2140)：modifier 构造错误修正，merged=2019-08-23。都只修特定 API 构造，不能视作解决 #2883。
- [Discussion #3710](https://github.com/Hammerspoon/hammerspoon/discussions/3710)：2024-11-04建立、11-05更新；DaVinci knob 连续 down/up 被误认 double click。显示事件序列/时间/点击次数会影响实际行为，非 RWI 同状态复验。

### Wine mac driver：窗口关联不保证 move 用该窗口

`cocoa_app.m` 固定 `85ad031e…`，文件最近提交2026-10-07 21:01:46（IME变更，不是 hover 修复）。1328–1353：有 capture 则用 capture window；dragged 用事件 window；普通 MouseMoved 用事件全局点重新查询 windowNumberAtPoint，可能落到另一 app/window。1477–1507 的普通左击还区分 Command 的 brought-forward 行为。**推论**：全覆盖时，正确定向 move 仍可能被客户端重新选窗；完整 down/capture/drag 和孤立 dragged-as-hover 是两种实验。未确认本机 GameHub/CrossOver 的实际构建是否含相同分支，不能直接归因。[固定源码](https://github.com/wine-mirror/wine/blob/85ad031e9c788a466d27fd3a06fa43e7c53c2af9/dlls/winemac.drv/cocoa_app.m#L1328-L1353)。

### SDL：macOS26+ 可用全局位置覆盖事件位置

window 文件最近提交 `bfd81a0…`=2026-09-10；mouse 文件 `6b178ba…`=2026-09-20；main HEAD `e4af6f8…`=2026-10-07。window listener 无 mouse focus 可丢 move，drag handlers 复用 move；**macOS≥26 分支读 SDL_GetGlobalMouseState 减窗口全局原点，覆盖事件位置**。mouse 路径还区分 raw GCMouse/relative input；所以 window-local 正确且真实光标不动，也未必等于客户端使用正确位置。[window 固定源码](https://github.com/libsdl-org/SDL/blob/bfd81a0da4c0a07e59e5622dee9c3b4badfe9bd5/src/video/cocoa/SDL_cocoawindow.m#L1870-L1966)、[mouse 固定源码](https://github.com/libsdl-org/SDL/blob/6b178ba756e65ddf8fc2420726b3e3d7d73b5ae3/src/video/cocoa/SDL_cocoamouse.m#L779-L911)。

- [Issue #15967](https://github.com/libsdl-org/SDL/issues/15967)：macOS26 focus/窗口边缘导致 event location 陈旧；closed/completed，2026-09-14 20:48:03更新/关闭。这是正常鼠标 bug，不是后台注入测试。
- [commit 958c4cb](https://github.com/libsdl-org/SDL/commit/958c4cbf0d77182254101670e01afd3b09207b7e)：2026-08-03 06:19:24，workaround 扩到所有窗口；associated-pulls为空，不编造合并时间。
- [PR #16153](https://github.com/libsdl-org/SDL/pull/16153)：closed，merged_at=null。实际采用的 [PR #16223](https://github.com/libsdl-org/SDL/pull/16223) 于2026-08-31 15:59:00合并，修 popup parent-relative→global 转换，merge SHA `013844b…`。**closed 不等于 merged**。

这些是可复现的客户端机制，不等于“SDL应用都不能后台”；要先确认目标框架版本，再做 event-local/global-poll 对照。RWI 不能为适配而静默 warp 用户光标，也不能默认向游戏注入库/改第三方进程。

### GLFW / RobotJS：可借鉴但不能混同路线

- GLFW master `92dcf4ce…`=2026-08-03，开发线3.6；stable3.5.1于2026-07-31发布。普通 move 读 locationInWindow，disabled cursor 读delta；tracking用key-window/visible-rect；hovered getter另外读全局光标及最前window。**callback与hovered查询应分测**。[固定源码](https://github.com/glfw/glfw/blob/92dcf4ce74f2e2554a98fea09be7c705c17daa5a/src/cocoa_window.m#L397-L560)、[hovered](https://github.com/glfw/glfw/blob/92dcf4ce74f2e2554a98fea09be7c705c17daa5a/src/cocoa_window.m#L1432-L1447)。[Issue #2690](https://github.com/glfw/glfw/issues/2690) open，2025-02-24更新；[#2523](https://github.com/glfw/glfw/issues/2523) open，2025-02-28更新；是相对/disabled cursor边界，不是本次目标复验。
- RobotJS mouse 文件 `9d5bc18…`=2026-07-26（最后改Windows timing）；master2026-08-07。mac路线补delta、正确button/dragged类型，但发session tap、使用全局光标，不是定向后台实现。[固定源码](https://github.com/octalmage/robotjs/blob/9d5bc18d7d684a45cf669d03b3307f449f12737a/src/mouse.c#L100-L229)。[PR #177](https://github.com/octalmage/robotjs/pull/177) merged=2016-04-16；之后仍有失败反馈，不是全游戏保证。借鉴字段/序列，不照搬全局光标依赖。

## 6. 优先实施路线（未执行的下一阶段）

**A. 首个可执行步骤：macOS Rust 原生事件构造/roundtrip 离线 probe。** 实现在 rwi-macos，不引入 AppKit 到平台中性 transport，不把 Python/AppleScript 当实现。比较现 CGEvent 与 NSEvent 工厂：读回 type、windowNumber、locationInWindow/globalLocation、timestamp、eventNumber、clickCount、pressure、button、delta、flags、source state；明确坐标基准。构造成功只算构造验收。

**B. 自有 AppKit 接收 fixture（需分配窗口后才启动）。** recorder 同时记录进程收件、NSWindow/NSView、tracking回调、客户端使用位置、goal；activeAlways/activeInKeyWindow、inactive-tooltip开关作为可控对照，第三方应用不被强改策略。比较一次drag共享Private source与现多source、flags=0与显式Command、正确连续down→dragged→up。除纯诊断负控外不再用缺down的dragged来代表drag。

**C. typed Move/Hover＋有界 dwell。** 分别描述移到目标、路径移动、hover高亮、tooltip。dwell只用于等待预期状态，不无限重复事件。每个action/窗口状态独立能力，不以一个失败把所有background动作禁用；unknown不等于unsupported。技术能力测量不依赖地图/疲劳oracle。

**D. 与构造器并行的 window-local authority。** 显式全目录/SCK non-on-screen枚举；原生 identity/epoch/window-base geometry替代显示器映射是独立设计，不直接删mapping_ready、hit、lease等检查。capture支持与input支持单独报告；最小化/hidden不能被“强行取消最小化”伪装成后台成功。

**E. 经用户分配的实际 AppKit/Electron/Wine A/B。** 保留原事件路由作为显式选项，候选不静默fallback到activate/globalHID/warp。先可逆动作再业务工作流；GameHub/游戏仍留主屏供用户使用。Windows/Linux后端独立开发，不从mac实验外推。

## 7. 验收矩阵与拒绝混淆

状态轴：`active-visible`（对照）、`inactive-visible`、`fully-occluded`（其他app覆盖）、`same-app-sibling-covered`、`offscreen`（显示器范围外）、`other-Space`、`minimized`、`hidden`。虚拟屏visible只是其中一种inactive-visible，不算全部真后台。

动作轴：click/double/right、key/IME、move callback、entered/exited、hover高亮、tooltip、slider drag、canvas drag、跨窗口DnD、relative/raw mouse。后两者不得从同窗口left-drag成功继承能力。

每格记录：构造/dispatch尝试计数→PID接收→窗口/view命中→客户端最终位置/按钮→目标读回；外加foreground identity、系统光标、窗口几何/key/main/active、未知第三方输入和资源释放。真实接收fixture与第三方pixel/AX读回分层；无receiver证据时保留unknown。

通过门槛：目标读回符合预期且前台未被抢、用户光标未被warp、无其他窗口收件、sequence/一次up-cleanup完整；记录延迟分布与失败阶段。先每格5次探索，再对目标支持格做20次同状态可逆重复，不将5/5当普适成功率。人工在其他app输入时不把global modifier/button直接冒认为RWI序列状态；同资源冲突仍需lease/uncertainty处理。

归因负控：错window/错Y/无down dragged/纯dispatch无receiver/正确receiver但global-poll不同/paused rendering；不对真实第三方应用执行故意错window负控，使用自有fixture。失败不重放down；release失败沿用fence。**输入确认不是目标完成确认**。

## 8. 已执行产物、记忆与边界

本轮只修正文档现行契约、添加研究/来源/证据JSON和一个用户明确请求的project-scoped memory note；不删除历史验收，不改生成memory或已有note。新note位于 `/Users/vincentadamnemessis/.codex/memories/extensions/ad_hoc/notes/2026-10-08-rwi-macos-background-nsevent.md`。旧“必须系统前台/后台hover必不可能/Mac drag未实现”的概括被明确取代，不把仍真实的on-screen约束写成已解除。

沿用四角色事务 `work/transactions/2026-10-06-mcp-output-compile-fix`，新增 `research-2026-10-08` 补充：原始hash、同一文档probe的BASELINE/MODIFIED/ROLLBACK、patch重构、回滚hash、重开记录。先前源码编译失败→修复通过→回滚恢复编译失败的实测仍保留，未冒充本轮重新跑Rust回归。**本轮没有实现/部署NSEvent鼠标候选，更没有宣称真后台已验收。**

工具恢复：Python urllib本地CA校验失败→系统curl公共API部分403→认证 `rtk gh api` GET成功；未关闭TLS校验。hover索引首查把kind写错导致空投影，已按literal `background_input_hover_calibration`修正并保留错误记录。原始命令/错误保存在 `work/research/nsevent-background-2026-10-08`，研究JSON为精简投影，不提交完整屏幕或无关session内容。
