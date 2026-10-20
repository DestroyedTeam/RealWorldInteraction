# R84 新会话：真实 Host 与双 PID Live 验收

## 顺序与结果
1. 先只读核验本会话真实 resources/list/read：v3、10460字节、SHA256 420f80f4…，与当前源码逐字节相同；安装MCP为9fe5ae6d…。Agent26360、配对、证书、四权限不变。父四角色标记出现后才启动fixture，未重复应用R84。
2. 复用159控制器，启动全新Receiver168/169（PID12935/12945）。真实CG/epoch重新发现，源均在授权Virtual4；只把168移动到该屏左侧，169保持右侧。以下是实际源窗口截图，**不是Live浮层截图或全主屏截图**：

原始Receiver168截图留在本地验收输出，不提交raw screenshot。
原始Receiver169截图留在本地验收输出，不提交raw screenshot。

3. 正式MCP Apps新卡真实DOM为“组件可见 · 预览连接已确认”，document.visible/hidden=false。独立native回读：session3第59帧含两个精确PID/出生身份/CG窗口，composition完整、无遗漏；原生浮层280×194、内建屏1/on_requested_display=true。该会话最终147帧，240s真实duration_expired；过期卡恢复选择器无匹配保留，未重放。
4. 新建有界session4，不复活旧capability：真实新卡可见、隐藏确认/恢复同会话；恢复后第31帧仍为双PID。隐藏时native image=false/visible=false，恢复后继续呈现。UI点击后第一读可能先于异步响应，已保留literal race记录，不把初始无帧读算成功。
5. 169一次具名AXPress关闭，原生discover0窗、process0/stderr空/非expiry。session4明确omits该精确PID，reason=process_identity_unknown、replacement_selected=false；根168继续，第110帧有原图且on_requested_display=true。不是推断owner关系或自动选邻窗。
6. 168一次具名AXPress关闭，process0/非expiry；初次status尚在最后一帧的异步边界，随后独立回读paused/process_identity_unknown/image=false，保留差异。真实Host卡Close一次→stopped/requested/worker_done/visible=false，终值111帧。
7. 三项原request durable回读terminal/resources_released/no replay，两target detach，桌面target剩1，反馈marker已隐藏。未输入其他app、请求人工、调用UI-only live_panel、伪造presence或跨实例授权。

## 门禁与限制
- 不改Rust/UI源码；精确源hash与已完成R84十二门一致，复用实际795/0/11/42、Receiver75、UI15、packager6及严格fmt/clippy，不虚构重跑。
- 沿用同四角色/原baseline，配置probe BASELINE1→MODIFIED0→独立copy ROLLBACK1（194+扩展路径原hash/absence恢复）→REAPPLIED0。该离线probe不是native GUI的baseline替身。
- 原生浮层AX读到本任务标题和image，但CUA精确App截图timeout；最终卡再观察也timeout/kernel reset。截图错误保留，不伪造浮层像素截图。正式Host接受由真实DOM＋native presenter/source/frame/status共同支持，不宣称用户肉眼见证、密码学Host task-ID、全窗口族或所有F01–F16完成。
- 保留原BG→FG/FG→BG、冲突、CmdTab、sheet/菜单、R49切任务、R73小浮层等既有证据。当前新增接受的是正式双源与生命周期，非游戏/raw、跨app原生DnD、所有hidden/otherSpace或通用cold-hover验收。

## 证据
同目录原PNG未编辑；详细literal工具回复、receiver命令stdout/stderr/exit、native状态、操作终态及错误保存在原R84/fresh-task-runtime-r84/；本文件只补充原四角色，不替换它们。
