# R67：系统文件面板关闭后 Live 自动恢复根窗口

## 顺序与实际结果

1. 从已成功150控制器复用命令模板，启动新的 Receiver151（PID62667／根CG44733／虚拟屏4）。当前已加载RWI MCP开始live:3；实际Host组件tab3显示“组件可见 · 预览连接已确认”，原生预览在内建屏1。
2. Files AXPress一次：uncertain／AXError-25204，耗时600ms为这一适配器调用墙钟，不是窗口出现耗时。35秒只读轮询还没看到sheet；保留超时，不重发。随后同原进程出现真实NSOpenPanel CG44747／open_actions1／own-parent／display4。早期family incomplete/owner_graph_read_unavailable如实保留。
3. 原生Live CG44755／320×239点；真实640×478 PNG包含两层。family.complete=true、AXSheet/AXParent verified，未包揽同PID其他窗口，也未产生输入权。
4. 精确Cancel一次returned，真实completion response0、visiblefalse、parent解除。连续只读500ms间隔采样：frames231/layers2 → recovering、清图、attempt1 → frames232/layers1、running、family complete、图像恢复。约1.5秒采样间隔为本次观测，不是通用延迟上限。之后frames276仍根1层，原生预览重建为CG44763／320×303点，PNG真实640×606。
5. 对新CG44763重新发现/独立attach，fresh snapshot后原生Close一次returned；Live stopped/requested/worker_done、清图/隐藏。源窗口仍可观察，随后单独AXClose源，原进程exit0／lifetime_close_requests0；4绑定detach、4durable全终态释放，无pending/cleanup uncertainty/replay。

## 原始截图（不是重绘/放大修复）

![真实根＋文件sheet](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r67-live-native-file-return-2026-10-12/01-real-native-two-layer-preview.png)

![取消后根窗口画面已恢复](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/r67-live-native-file-return-2026-10-12/02-real-native-root-restored-preview.png)

## Live大小与按钮参考

当前已部署面板宽上限320点，按源比例分别239/303点高；隐藏eye.slash、继续play.fill、关闭xmark为原生SF Symbols，28点可访问按钮与tooltip。截图采集细节和小浮层大小分离，不能把小浮层当细字原图。已实际重读原生大小、图标和AX动作，不称与Codex私有UI像素级一致。

重新互联网复核：[Apple按钮指南](https://developer.apple.com/design/human-interface-guidelines/buttons)要求符号/标签语义清楚、相邻控件一致；页面正文直接抓取仅JS壳，搜索索引摘录保留此限制。[Codex PiP遮挡反馈](https://github.com/openai/codex/issues/32451)及[打开/隐藏语义反馈](https://github.com/openai/codex/issues/33086)是用户issue，不是官方尺寸规范。故不臆造对方窗口精确像素或复制私有资源；保留非激活、紧凑比例、明确隐藏/关闭与清图语义。

## 回归与剩余范围

本轮未改Rust/安装/权限；R66在相同源码逐字节校验后执行完整11门，workspace774／Receiver72／UI7，fmt、locked offline tests、all-target/all-feature strict Clippy、release与架构门全exit0。同四角色扩展报告，源配置BASELINE1／MODIFIED0／ROLLBACK1／REAPPLIED0，原始哈希与原97项不变；不是生产Agent回滚或输入重放。

真实系统sheet取消→清图→根窗口恢复→关闭只结束观察闭环已接受；未人工配合，无用户眼见或物理人类切窗宣称。完整F01–F16仍未完成，冷态后台hover、foreign-provider、多Space/隐藏最小化、跨应用DnD、浏览器独立生命周期及压力/资源矩阵继续保留。现有公共仓库/PR/精确HEAD CI为已完成的R65发布事实，不冒充新报告已推送。
