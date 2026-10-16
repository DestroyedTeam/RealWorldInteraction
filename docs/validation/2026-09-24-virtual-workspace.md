# BetterDisplay 虚拟工作区与 Paint V14（2026-09-24）

> 同日后续：已实现并实测 MCP context 所属的 RWI Live 浮层；不是 Codex 内置 Live。第三行头像已保存为48×48，详见[最新实机记录](2026-09-24-live-host-acceptance.md)。以下屏幕坐标与未实现项是各阶段历史事实，输入前仍须当前discover。

## 实机状态

- Codex 重启后，当前任务实际加载的 RWI Plugin 已恢复，四项 TCC 权限 true；不是 standalone MCP 探针。
- BetterDisplay 5.0.5，控制台 `get -proAvailable` 返回 on。通过 CUA 操作用户已打开的 Console，先枚举，再仅创建一块 **RWI Workspace**。没有安装、升级、购买或新增授权。
- VirtualScreen tagID 67、Display tagID 68、当次 displayID 7、UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。最终逻辑 1920×1080 HiDPI / 60 Hz，物理 3840×2160，扩展模式，桌面矩形 `[2946,0,1920,1080]`。displayID 后续须重新发现。
- 内建屏仍为主屏，iPhone（display 6）未断开、未改配置。RWI 将 Paint 工作文档和参考原图都移至虚拟屏；最终主文档 `[2951,7,1434,660]`，参考图 `[2970,24,1220,784]`。遗留 Open 面板通过新鲜 AX Cancel 关闭，最终窗口清单不再包含该面板。
- BetterDisplay PIP 已开启：仅查看虚拟屏，主屏右下 454×288、普通窗口层级、目标 15 fps、autoStart off。实际截图看到了头像从导入位置到第二行的变化；15 fps 是配置目标，不是帧率基准。

控制台实际命令：

```text
get -identifiers
get -proAvailable
create -type=VirtualScreen -virtualScreenName="RWI Workspace" -useResolutionList=on -resolutionList=1920x1080,3840x2160 -virtualScreenHiDPI=on -connected=on
set -displayID=7 -resolution=1920x1080 -hiDPI=on
set -displayID=7 -pip=on -targetDisplayID=1 -width=30% -height=30% -originX=68% -originY=5% -fpsTarget=15 -priority=normal -showTitlebar=on -autoStart=off -cursorShown=on
```

依据：[BetterDisplay 官方说明](https://github.com/waydabber/BetterDisplay)、[官方 CLI 文档](https://github.com/waydabber/BetterDisplay/wiki/Integration-features%2C-CLI)，以及本机实际 help。虚拟屏可以不依赖实体副屏，但仍是同一 macOS 用户/安全会话；焦点、剪贴板、系统面板和 RWI 桌面租约共享。没有验证无限扩展、多 Agent 独立并行输入或性能优势。

## 反馈光标修复

旧版收到 `[3751,372]`，实际 CG 窗口仍在主屏 `[1,1,158,158]`，复现用户报告。改为显式 `setFrame:display:animate:`（无动画）和事务提交，并回读真实 applied_point/window_id/display_id。窗口服务器存在提交延迟，最多 50 ms 只读稳定等待，不重复位置设置或输入。

最终三个 requested/applied point 完全一致：`3751,372`、`4151,500`、`3502,312`；均 display 7、error null。另用系统 screencapture 仅抓取 RWI 自己的反馈窗口，确认图形渲染。没有捕获或绕过不允许访问的 Codex UI。接口依据：[Apple NSWindow frame](https://developer.apple.com/documentation/appkit/nswindow/setframe%28_%3Adisplay%3A%29)。未隔离证明旧版失败究竟由单个 API 还是事务提交造成。

## 静态边缘与动态选框是两个问题

- 静态 Paint 的两张样本仅底部最后一行 244 像素发生灰度245/246抖动，正文一致。输入指纹新增底部一个逻辑点排除；原始 PNG 不裁切、不改像素。合成指针和整个拖动路径禁止进入该排除边缘，正文单通道变化1仍改变指纹。新增1×/2×和边界回归。
- 部署后 RWI 定位拖动实际生效：1767 ms；后续对齐1483 ms。但选中图层时仍 content_changed；只读比较定位到头像选框 `[530,284,572,326]` 中127个变化像素。**动态选框问题没有修好**，不能把边缘修正当作通用动画容错成功。
- `move_window` 的回读还可能处于系统动画中，返回临时 bounds/display null；独立重新发现才确认到达目标屏。后续需有界稳定读回，不能重放移动操作。

## V14 图层与实际保存

RWI 在参考图框选第二行头像82×82，copy/paste形成独立Image层，并完成定位。CUA辅助文件定位、窗口切换和缩放对照；缩放时出现Paint首次提示，确认后为48×48，再由RWI对齐。工作文档未关闭，原图文件未保存改写。

RWI cmd+s返回但磁盘SHA未变；CUA保存后才更新，因此最终落盘是混合流程。快照：`outputs/paint-v14-source/paint-v14-20260924-avatar-row2.paint`，438740 bytes，SHA256 `f2b2d4e5615b24649355ceaba7cc5f514bae0b65f52d94ebf888e1db1f413f1c`。只读归档确认129对象：6图片、27矩形、75线、11椭圆、7圆角矩形、3文字。工作源文件与快照字节一致，旧版本保留。Undo跨重启持久性未验证。

一次KeyChord去除私有window-routing字段的候选通过离线测试，但实机cmd+o未确认新增Open窗口，已从源码及部署撤回。原键盘路由保留；后台菜单快捷键与保存读回继续排查，不宣称已修复。

## 最终验证/部署

- fmt通过；workspace locked offline测试 **470 passed / 10 ignored（38 suites）**；all-targets/all-features clippy -D warnings通过；release Agent构建通过。
- 最终Agent PID74634、实例`452b5396-22de-4fbf-95b6-a3eeb6dfb30d`；二进制SHA256 `7d9107e4896f6089599977a3c3af3fab75b7ed80f6c85de80820e001da34c806`。
- 同一本地签名证书，最终四项TCC true；profile、配对、持久journal和MCP安装未改。不需要再次重启Codex。备份/诊断在`work/v14-sidecar-20260924/`。
- 没有提交/推送。快照单样本observe约1.1–1.4s、预览0.07–0.10s，不是严格基准。

## 按时间的实际截图

1. [移入虚拟屏，头像待定位](../../outputs/validation/2026-09-24-virtual-workspace/01-virtual-before.png)
2. [RWI移动后，82×82待缩放](../../outputs/validation/2026-09-24-virtual-workspace/02-avatar-moved.png)
3. [第二行48×48头像](../../outputs/validation/2026-09-24-virtual-workspace/03-avatar-final.png)
4. [本地实时预览已更新](../../outputs/validation/2026-09-24-virtual-workspace/04-live-preview.png)

同名metadata.json保留工具映射；这些是原样截图，不是重绘效果图。PIP不等于模型持续接收视频。用户提到的Companion是Computer Use附带观察面板；当前工具不开放Codex UI访问，未改其Live/隐私设置。**本次虚拟屏验收当时** Mac `capture_visual` 尚未实现；同日后续已补单窗轻量通路，见[Live 视觉底座实测](2026-09-24-live-visual-display-uuid.md)。

## 对 Live 观察的补充澄清（用户 2026-09-24 纠正）

用户实际观察到的 Computer Use Live 是**跟随当前 Codex 会话的叠层**，没有普通预览窗的经典三按钮；同一应用调用系统文件、字体或颜色面板时，观察仍能跟进。这里的 UI 形态与跨面板体验是用户观察，尚未从 Codex 内部实现或权限契约独立验证。此前把本地 PIP 说得过于接近该 Live 体验，现明确区分：

| 观察通路 | 所属与生命周期 | 当前已确认的观察范围 | 跨屏/跨进程面板 |
|---|---|---|---|
| Computer Use Live | 用户观察：当前会话的叠层 | 用户观察：与当前任务的应用交互同步 | 用户观察：系统调用的文件/字体/颜色面板也能跟进；内部关联规则待调研 |
| BetterDisplay PIP | BetterDisplay 进程的独立普通窗口 | 已验：连续显示指定虚拟屏，显示 Paint 更新 | 面板若出现在别的显示器，固定虚拟屏 PIP 不会自动变成另一个屏的视图；本轮 Open 面板曾在内建屏出现 |
| RWI `snapshot` | Host-wide MCP + 常驻 Agent，单次绑定一个精确原生窗口 | 已验：返回该窗口单帧及 AX 控件；需要模型下次调用再刷新 | `discover` 可发现授权桌面同一用户的可见窗口；Mac `popups_for_pid` 仍以当前 PID 为范围，owner-chain 弹窗关系切换未实现。跨进程须新鲜发现与精确绑定，不能凭标题/同一 PID 推定归属 |
| RWI 反馈光标 | Agent 独立透明窗口 | 已验：标记一次输入或指定的桌面点，约45秒闲置后隐藏 | 仅指示点位，不是观察视频或会话 Live |

因此，虚拟屏解决了 Paint 画面放在哪里，PIP解决了人在本地如何旁观；二者尚未解决**会话内 Live 如何组织多个相关窗口**。针对 RWI 后续设计，需由会话侧明确分配目标设备与根窗口，基于原生窗口/进程/系统面板关系追加或切换观察对象，记录每次窗口的身份、显示器、出现/消失与焦点证据，并把观察生命周期与当前会话绑定。系统面板可能跨屏或由别的进程提供，不能只枚举根 PID，也不能把桌面上其他无关窗口悄悄纳入。会话标识、叠层承载和隐私模式能否由 Codex 插件直接提供，仍需以官方 Host 能力与实机验证确定。

### 观察边界：不是把 PIP 换一个皮肤

1. **观察对象**：虚拟显示器是桌面空间，PIP 只镜像指定显示器；RWI 的 `observe` 是绑定窗口的离散帧；会话 Live 则应是「当前任务相关的窗口集合」的视图。一个面板即使是用户刚从 Paint 调起，也未必跟随 Paint 的窗口、PID 或显示器。因此「固定屏全录」和「把根 PID 的所有窗口合并」都不是完整且精确的替代品。
2. **身份和切换**：根窗口仍需用户指定、记录 CGWindowID/PID/进程创建时间/native epoch。面板出现时，先拿原生新窗口与焦点证据，再证明它属于本次交互；证据不足时只能列为候选供显式选择，不能把用户其他应用一起纳入。每个面板独立绑定、独立刷新坐标；返回根窗口也重新核对身份。当前 macOS 实现不具备这条 owner-chain 关系证明，Windows 已有跨 PID owner-chain 合约，不能直接照搬其 HWND 机制到 Mac。
3. **生命周期**：Agent 为 TCC 与设备连接保持常驻，不代表每个任务都在 Live 观察。当前 MCP Gateway 仅按设备 route 保有一个 Agent context（`crates/rwi-mcp/src/gateway.rs:30–79,150–197`），MCP 进程退出时尝试关闭；Agent context 还可在闲置 600 秒且资源静止时清理（`crates/rwi-agent/src/runtime.rs:211–226`）。这不是经验证的 Codex 任务级观察订阅。拟议的观察订阅由 Host 的任务/会话创建、暂停、结束和撤销；断开订阅应停止该会话的画面分发，但不杀 Agent、不破坏另一任务的设备连接。MCP streamable transport 或 progress 通知可以输送状态，却不会自动生成 Codex 的叠层 UI，更不能自动判定面板关联。
4. **双通路**：模型所需的是按需/变化后、有身份和坐标映射的压缩帧；用户所需的是连续、可暂停、可明确关闭的观看 UI。两者可以复用 Agent 的原生采集/窗口图，但不能把 15 fps 本地 PIP 视频持续塞进每次 MCP 工具输出，也不能用无帧元数据的视频像素作为输入授权依据。
5. **隐私与并发**：视图仅含被证明属于会话的窗口；窗口跨屏时应显示「面板已转到显示器 X」并切换相应窗口，而不是默默扩大到整屏。多个任务可各自订阅观察，但同一原生输入域仍遵守现有租约、持久去重与不重放；观察共享不等于并行输入安全。

Apple [ScreenCaptureKit 的 `SCContentFilter`](https://developer.apple.com/documentation/screencapturekit/sccontentfilter) 有单窗口、指定窗口集合、按应用/显示器筛选的采集原语；[AXObserver 通知](https://developer.apple.com/documentation/applicationservices/1462089-axobserveraddnotification) 可监听窗口/焦点变化，但可能返回不支持或无法完成，因此只能作候选事件线索，仍需枚举和原生身份复核。OpenAI [Plugin UI 文档](https://developers.openai.com/plugins/build/chatgpt-ui) 说明可选 MCP UI 资源、ChatGPT iframe 与 MCP Apps bridge，并要求无 UI 时工具可工作；[UI 指南](https://developers.openai.com/plugins/concepts/ui-guidelines) 描述 ChatGPT 会话内的 PIP 生命周期。这些说明了可探索方向，**没有证明 Codex 当前允许第三方插件复用内置 Computer Use Live 叠层或持续帧通道**。

### 下一轮验证矩阵

| 场景 | 必须记录 | 通过条件 |
|---|---|---|
| Paint/TextEdit 根窗口留在虚拟屏，打开文件面板 | 根/面板窗口 ID、PID、显示器、前后台、面板与根的原生关系证据 | 无论面板落在哪块屏，都能独立发现并正确重新绑定；不误取其他任务窗口 |
| 打开系统颜色/字体面板 | 同上，外加 AX 类型与出现/消失顺序 | 同 PID 和跨 PID 均按真实身份处理；若关系不明，明确标为待选而非自动输入 |
| 用户切到不相关应用，或两个 RWI 任务共用设备 | 两个任务的观察订阅、设备/窗口身份和输入租约 | 仅当前任务相关窗口进入该任务视图；用户切窗不误触发已绑任务输入 |
| 面板跨屏/关闭、虚拟屏断连、任务结束 | 显示器拓扑、视图来源、资源释放记录 | 停止旧帧/旧坐标，更新或结束该任务视图；Agent 及其他任务不被误停 |

独立任务「BetterDisplay 虚拟屏与低干扰操作方案调研」（`01a0d242-9beb-7541-bf1e-63b17b42a89d`）已收到用户纠正并完成隔离实测，其 GUI 独占时段已结束。

该独立任务的[完整隔离实测记录](</Users/vincentadamnemessis/.codex/worktrees/bf80/Real World Interaction/docs/validation/2026-09-24-betterdisplay-foreground.md>)表明：RWI 将 TextEdit 文档移到 display 7；由原生 AppKit 激活后，TextEdit 确为前台且窗口留在虚拟屏；`⌘O` 的 Open 面板留在虚拟屏，但 Colors 面板跑到内建主屏，RWI 另行 `discover/bind/observe` 看见它。虚拟屏能承载前台应用，却**不能保证相关系统面板永远同屏**；这也不证明 RWI 自身会激活应用，因为当前 `computer.activate_window` 在 macOS 仍标为 `implemented:false`。

后续修正/第三行头像的现场进展另见[2026-09-24-placement-avatar-row3.md](2026-09-24-placement-avatar-row3.md)：移窗已增加稳定回读并部署，第七个可编辑Image经RWI保存；会话Live仍未实现。

## 后续优先级

1. 独立研究BetterDisplay相对无线iPhone副屏的干扰性、稳定性、重连/休眠和性能；复用现有虚拟屏，避免重复创建。
2. 选框动态像素误拒绝、后台菜单快捷键及实际保存读回；继续其余头像/侧栏图标和整体布局，V14未完成。
3. 窗口移动稳定读回、Mac轻量单窗视觉通路已完成并实测，后续为会话级相关窗口观察/输出优化。多虚拟屏不是多安全会话，不绕过原生身份、租约或不重放约束。
