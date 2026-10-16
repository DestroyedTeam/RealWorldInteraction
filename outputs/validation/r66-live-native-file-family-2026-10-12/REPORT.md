# R66：Live 与真正系统文件面板层叠验收

## 顺序与实际结果

1. 专用 Receiver150（PID38331／根CG44695）位于虚拟屏4。当前已加载 RWI MCP 启动 Live；真实 Host MCP Apps v2 DOM 确认组件可见，没有用 IAB 或伪造 presence 替代。
2. Files AXPress 仅一次，原回执 uncertain／AXError-25204 保留。只读观察随后得到真正 NSOpenPanel sheetCG44713／own parent／虚拟屏4／open_actions1。
3. 初期 owner_graph_read_unavailable 时只捕根窗口，不把同PID的 Colors／Fonts 纳入。之后确证 AXSheet/AXParent，两层分别1320×1104、1760×896，合成1760×1104。
4. 首个 Live 在180秒期限结束；Resume 返回 stopped/duration_expired，不是成功恢复。第二个新观察 live:2 捕获同一仍存活的源，不是输入重放；原生预览CG44726／内建屏1／320×239点／真实640×478 PNG。
5. 精确 sheet Cancel 一次 returned；真实completion response0、visiblefalse、关联解除。首次回读 recovering／清图占位后即关闭预览，**本轮没有等到根窗口恢复，既不判失败也不宣称已验收**。
6. 原生预览 Close 一次后 stopped/requested/worker_done、清图并隐藏；源仍可独立观察。随后单独源 Close，进程exit0／lifetime_close_requests0；3 binding detach，4 durable 请求终态释放，无pending/cleanup uncertainty，禁止重放。

## 原始截图

![根窗口与真实NSOpenPanel层叠](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r66-live-native-file-family-2026-10-12/01-real-native-root-and-system-file-sheet.png)

![取消后清图并进行有界恢复](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r66-live-native-file-family-2026-10-12/02-after-cancel-bounded-recovery-placeholder.png)

## 验证与范围

实际11门全部exit0；workspace774 passed／0 failed／11 ignored，Receiver72、Live UI7。源码与R65已验版本逐字节一致，生产Agent/MCP/证书/配对/权限未改，无需重启。沿用原四角色，源配置检查BASELINE1／MODIFIED0／独立copy ROLLBACK1／REAPPLIED0，不是生产回滚。

同provider根＋真实系统sheet合成已接受；foreign-PID、通用冷态hover、隐藏/最小化/其他Space、原生跨应用DnD与完整压力/性能矩阵仍未全部验收。下一项单独补验取消后恢复根窗口，不能重做本例冒充新进度。
