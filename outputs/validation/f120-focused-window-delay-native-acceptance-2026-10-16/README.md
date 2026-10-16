# F120：真实前台读取错误恢复与 Live 控制复验

## 原生错误恢复（不是模拟成功）

1. 两个专用 AppKit 测试窗口均在已分配的 BetterDisplay 虚拟屏；生产 Agent 未替换。仅延迟窗口显式启用默认关闭的 `--ax-focused-window-once-delay-ms 60`。公开 `accessibilityFocusedWindow` getter 在真正后台读取后武装、首次激活读取时仅延迟一次，始终返回 NSApplication 父类的真实窗口；不伪造 AXError、不发输入。
2. 当前会话加载的 RWI 首次显式切到同伴窗口，独立前台完整身份相符。延迟窗口自身记录为后台，已收到4次公开焦点窗口读取，尚未消耗延迟。
3. 对延迟窗口只发一次首次切窗：AXRaise1、AXFrontmost1。Agent 实际记录 `AXUIElementCopyAttributeValue/AXFocusedWindow -25204`，随后同一保留对象仅补读一次，错误后的额外阶段23.102959ms（申请间隔20ms、软预算100ms），前台读取完成。独立工具再次读取的完整前台身份一致。接收程序记录一次延迟、最大getter耗时61087µs；其 `AX_error_attested=false` 保留，真正错误证据来自 Agent。
4. 无重发、键盘/点击替代或私有API；协议仍 `goal_verified=false/input_authority_granted=false`。结果仅证明此公开原生延迟用例中的恢复，不证明Chrome根因，也不覆盖其他属性/所有应用。

## Live 的真实控件与边界

- Computer Use 实际读取本任务的新 MCP Apps 控制卡，并从新卡点击隐藏→恢复→关闭；RWI 原生回读分别为清图暂停（会话仍运行）→同一会话重新出图→worker_done且清图。没有人工配合。
- 原生浮层在虚拟屏4，真实bounds为198×200pt；源图1320×1104px，缩略尺寸不降低原图。Computer Use 按安装绝对路径读取实际浮层，两按钮分别“隐藏预览”“关闭预览”，帮助文本明确不关闭源窗口，未见经典三个窗口按钮。记录原始JPEG，不把它改标成PNG。
- 首个90秒观察在交互前已到期；旧卡按钮禁用，点击未执行。保留此错误，另建独立观察测试控件，未重用过期能力。外观用第三个观察稍后出现presence暂停，原因未独立证明，随后明确停止。
- **控制卡仍只有控制，没有图像节点（img/canvas/video=0）。严格Host任务切换绑定仍未验过；虚拟屏上的预览也不等于用户实际看到。** 这是下一阶段需要实现/验收的缺口。已保存完整关闭卡截图；MCP Apps不支持clip截图，保留错误而未用其他手段改图。

## 清理与回归

两窗口各通过新快照 Close window/AXPress 关闭一次；独立发现消失、绑定分离、两进程退出0。四项持久输入操作全终止且资源释放，无待处理/释放不确定/可重放；三个Live全停止，原前台完整身份恢复。1375项workspace通过、11忽略；独立example新增8项，fmt/Clippy全目标全特性-D warnings/架构检查均退出0。fixture参数同一CLI BASELINE退出1（缺新开关）→MODIFIED0→ROLLBACK1→重应用0，全部484项原源码哈希在独立副本恢复；生产Agent/MCP/Host/权限/证书/配对/密钥不变。

## 时间顺序原图

1. [01-peer-before.png](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f120-focused-window-delay-native-acceptance-2026-10-16/chronological/01-peer-before.png) — 1320×1104，原字节未改。
2. [02-delayed-before.png](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f120-focused-window-delay-native-acceptance-2026-10-16/chronological/02-delayed-before.png) — 1320×1104，原字节未改。
3. [03-delayed-after-recovery.png](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f120-focused-window-delay-native-acceptance-2026-10-16/chronological/03-delayed-after-recovery.png) — 1320×1104，原字节未改。
4. [04-live-card-closed.jpg](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f120-focused-window-delay-native-acceptance-2026-10-16/chronological/04-live-card-closed.jpg) — 816×1079，原字节未改。
5. [05-native-compact-live.jpg](/Volumes/ChatGPT/Real World Interaction/outputs/validation/f120-focused-window-delay-native-acceptance-2026-10-16/chronological/05-native-compact-live.jpg) — 396×400，原字节未改。
