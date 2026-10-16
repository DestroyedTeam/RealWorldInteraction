# UUID 移窗验收与 RWI 会话预览实现（2026-09-24）

> 同日后续更新：用户再次重载后，两个工具已加载并完成真实 Host 主路径验收，见[Live实机记录](2026-09-24-live-host-acceptance.md)。以下保留本阶段部署时的历史状态，不能把“尚未加载”当作当前事实。

## 当前结论

用户重启 Codex 后，当前任务实际加载了带 `display.uuid` 的工具 schema。UUID 移窗已由真实 Plugin 完成并独立读回。随后新增了 `computer.live` 和 `computer.place_window`，Rust 实现、离线门禁、签名部署及插件重装已完成；**这两个新入口尚未进入当前任务工具表，Live 浮层的实际显示、按钮和跨窗口切换仍待重载后的 Host 验收**。

## 真实 UUID 移窗

- 设备为当前 mac-host；原生会话保持 uid501/security100003，四项 TCC true。新加载的 `computer.move_window` schema 已包含 `display.uuid`。
- 第一次分步 observe/resolve/claim/move 因引用超过既有 10 秒窗口而拒绝，返回 `stale_reference`、`native_dispatch_entered=false`、no move sent；租约已释放。没有重放该 request_id。
- 新鲜观察后以独立请求 `paint-v14-20260924-uuid-placement-02`，仅指定 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`：Paint S 主窗口 PID14404 / `cgwindow:2189` 从 `[1517,7,1434,660]` 移到同一虚拟屏内的 `[1536,24,1434,660]`。返回 `placement_confirmed=true`，2 次稳定读回、57 ms；独立 discover 确认仍在 display7 / 该 UUID。未激活应用或发送键鼠输入。
- [实际移窗后窗口截图](../../outputs/validation/2026-09-24-live-context/01-uuid-placement.png) 为 Plugin `capture_visual` 原始响应导出，1024×471，SHA256 `cefe05927475e2ab88a12ab5da13e7ebfc1794c845677bc48119c4b5d7627f7b`；同目录 metadata.json 保存移动回执与视觉元数据。

## 新增能力

### computer.place_window

直接接收 target、稳定 request_id、display（可用 UUID），在一个工具调用内完成 fresh observe → window resolve → claim → 单次 move → release。复用原有原生尺寸预检、位置稳定回读和持久去重，保留原引用时限。这样模型无需通过临时 JS 拼接四次调用，也不必在推理间隔中维持一个快过期的引用。没有自动选择/激活别的窗口，也不重放移动。

### computer.live

- `start/pause/resume/stop/status`；start 明确指定预览所在显示器，来源为已绑定的精确应用窗口。默认 768px、500ms 采集间隔、2 分钟期限，可配置 250–2000ms、64–1024px、1 秒–10 分钟。
- 由现有 RWI Agent 的主线程创建 borderless nonactivating NSPanel，带标题、暂停/继续、关闭按钮；不增加独立常驻进程。其所有权是**当前 MCP device context**，不是经过证明的 Codex 内部任务 UI 订阅。正常 context close 销毁会话；硬期限覆盖未正常关闭的连接。
- 成功显式 bind 新窗口后切换来源，允许已选中的跨 PID/跨显示器面板；用户暂停状态不会被绑定覆盖。RWI Agent 自己的窗口被排除，避免预览捕获自身。窗口缺失、权限/身份/拓扑变化时清空画面并暂停，等待明确处理。
- 内存中仅保留一个待显示帧和一个当前图像；来源 generation 变化、暂停、停止或到期后丢弃迟到帧。截图回调取消/超时后仍保留原生 operation ownership；未释放的会话继续占用最多 4 个采集槽位之一，不能因停止/重绑不断积累回调。
- `status` 分别报告采集、已应用图像、原生回调是否仍在途、实际 presenter windowID/visible 标志及错误。开始的回执不等于浮层画面已实测。
- 暂停会停止新采集并清空预览，不承诺阻止第三方截图。Apple 明确将 [NSWindowSharingNone](https://developer.apple.com/documentation/appkit/nswindow/sharingtype-swift.enum/none?language=objc)列为不再使用的旧常量，并要求不要借它隐藏录屏内容；实现未依赖该属性制造隐私保证。

系统面板的自动归属识别尚未实现；这次的跟随依据是 caller 已完成的新鲜显式绑定。固定 BetterDisplay PIP、RWI 原生 context 预览和 Codex 自带 Live 的承载与生命周期仍需分别说明。

## 验证与安装状态

- fmt、workspace locked offline 测试 **481 passed / 10 ignored（38 suites）**；all-targets/all-features Clippy `-D warnings`、diff check 通过，release Agent/MCP 构建通过。
- 新回归覆盖：暂停后旧帧拒绝、恢复后 generation 隔离、显式重绑不覆盖人工暂停、停止不可被迟到绑定复活、期限/上下文关闭清理、取消后原生 callback ownership，以及一次移窗错误后的单次派发与租约释放。默认测试未读取屏幕或生成 UI。
- 已签名安装的 Agent SHA256 `06a4afffd1226db21d7001fbd8542f5fd4eea7ae47c71f799a5071db4573e7fc`，PID68622，实例 `07d1e488-5ae2-49f9-8d22-8761199f9078`。同一 `RWI Local Development` 证书，严格签名验证通过，四项 TCC true。
- 已安装 MCP SHA256 `fd2934858062e2dc546a03d26607ef458a934659a2139e03bf4950dc0ad40a3d`；插件版本 `0.2.0+codex.20260924130816`。仅更新 RWI 文件，配对/profile/journal 保留。备份位于 `work/live-context-20260924/`。
- 部署后通过仍已加载的 Plugin 再次 discover/bind/capture_visual 成功：512×235、源 `[1536,24,1434,660]`，frame_created=false。Agent 已报告 live implemented=true；当前任务的 callable tool 表仍无 computer.live/place_window，所以未以 standalone RPC 冒充 Host 浮层验收。
- Paint 图层本轮未编辑；保存快照仍为 446815 bytes，SHA256 `b805184358b19b47e4a293f003a71349a4c044b4c40bf28ddcd84ce7cef53d23`。未提交或推送。

## 重载后的必验项

1. 从 Plugin 新工具启动 Paint 主窗口预览，指定内建屏 UUID，独立核对 NSPanel 的真实位置、浮层外观与持续刷新。
2. 通过真实按钮和工具分别暂停/恢复/关闭，检查清空、帧计数、native_call_pending 与 worker_done；测试短期限自动关闭。
3. 显式 bind 参考窗口/已验证面板，核对预览来源切换及旧帧不闪回；明确记录系统面板跨屏事实。
4. 实测 place_window 的单调用闭环；继续 Paint V14 第三行宽度精调和剩余复刻，保留原生图层文件。
