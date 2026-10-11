# GameHub 全屏游戏 Live 跨屏放置与采样节奏修复（2026-09-28）

接续[游戏只读观察](2026-09-28-gamehub-readonly-live.md)。用户明确要求 GameHub/游戏留在内建主屏供本人使用；RWI Live 只应把预览面板放在 BetterDisplay 虚拟屏。本记录的原生窗口读回、采集帧数与前台 PID 均来自真实 Host RWI/WindowServer；没有向游戏发送键鼠、移动游戏窗口或重启游戏。

## 发现 → 清理 → 修复 → 复验

1. **问题复现。**第一次在游戏 `CreatureCurios`（PID 51946、CGWindow 11351，内建屏 `[0,0,1512,982]`）作为 Live 源、虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41` 作为预览目标时，task-owned Codex 浏览器面板显示后 Live 实际开始采集。`requested_preview_display.bounds=[1512,0,1920,1080]`，但原生 `presenter_readback.bounds=[0,0,1040,720]`、`presenter_visible=true`、已采集25帧。旧 `presentation_matches` 只看可见性和图像是否存在，错误报告 `settled=true`，没有检查屏幕归属。发现后立即 RWI `hide → stop`，读回 `on_screen=false`、worker退出；游戏始终仍是前台 PID 51946。
2. **原因分析与代码。**旧 Presenter 设 `NSWindowCollectionBehaviorCanJoinAllSpaces | Stationary | FullScreenAuxiliary`，其中 Apple 将 `FullScreenAuxiliary` 定义为显示在全屏窗口的同一 Space：[Apple NSWindowCollectionBehavior](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct?language=objc)。**推断**它与正在主屏全屏运行的游戏共同导致预览偏离目标屏；不能仅凭这次观察断言所有系统版本都是同一原因。移除这组三个特殊行为；面板 `orderFrontRegardless` 后重新设目标 frame，并在展示任何图像前用精确 CGWindowID/PID 及当前显示拓扑检查该窗口完全属于请求的显示器。位置不对或无从确认则清空/停止；运行中每250ms复核。`presenter_readback` 新增 `on_requested_display`，`settled` 仅在位置和可见性均符合时为 true。
3. **首次签名部署。**全仓离线 524 passed/10 ignored、fmt/Clippy `-D warnings`/diff check 通过；`rwi-agent` release 构建后在 `work/` 隔离副本以既有 `RWI Local Development` leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8` 签名并严格验证 designated requirement 不变。仅备份/替换 RWI Agent.app、由既有 LaunchAgent 重启；Agent PID 51276、四项权限仍 true。真实游戏源不移动。30秒/1024px/1000ms Live 再次可见时，预览面板原生范围变为 **`[2880,644,528,388]`**，完全在虚拟屏内，`on_requested_display=true/settled=true`；源仍是游戏 CGWindow 11351，游戏前台未失焦。到期后 `on_screen=false/worker_done=true`。
4. **节奏缺陷。**上述30秒会话采集59帧，虽请求 `interval_ms=1000`，实际接近每秒两帧。代码复核发现 Live worker 的 `Condvar.wait_timeout` 被浏览器可见性心跳提前唤醒后直接开启下一次采集；间隔只约束一次 wait，而非从上次采集起算的 deadline。改为按单调时间的下一帧 deadline 循环等待，心跳仅能促使隐藏/停止/重绑等状态响应，不能触发额外截图；加入多次提前 wake 的离线回归。
5. **节奏部署和真实复验。**全仓离线 **525 passed/10 ignored**、fmt/Clippy `-D warnings`/diff check 与 macOS Skill 验证通过，release 构建/同一 leaf 稳定签名/严格验证后仅替换 Agent。实例 `b0fb714b-77f6-4045-abdb-5adcbdfb6f4a` / PID 57837 四权限依旧 true；当前 Host 正常 `snapshot` 仍有精确游戏图像，已部署的 `background_input` 机器可读字段可见。该实例下最终 **30秒请求约30帧**，期间原生面板仍 `[2880,644,528,388]`、`on_requested_display=true`、`last_error=null`；RWI显式 Stop 后 `on_screen=false`、`worker_done=true`。该单次实测支持节奏修复，**不外推为普遍帧率或对游戏性能无影响的保证**。
6. **隐藏后再显示的边界补齐。**代码复核发现首次出现窗口的250ms确认期限不能沿用到一次已验证面板的 Hide/Resume 重新排序。新增每次重排的独立期限，缺失窗口时撤销旧 placement 确认；避免因旧创建时间造成假失败，也避免连续尝试无限延长期限。全仓再次 **525 passed/10 ignored**、fmt/Clippy/diff和release构建通过，同一 leaf 签名严格验证后部署至当前实例 `327f3549-1cb4-43fb-a2e8-9cd6fed58ddb` / PID 68567，四权限仍 true。当前 Host 再次只读确认游戏精确窗口1024×665截图（3024×1964原源，`frame_created=false`），又重新绑定虚拟屏 Paint 主窗，2048×942/63控件/正常像素引用均未回归。**这一最后的小修复尚未再次执行真实 Hide→Resume 面板回归**；第5步跨屏/节奏实测对应上一实例。

## 交付边界

- GameHub/Steam 启动器 PID 自身无 on-screen 窗，真实可观测窗口属于游戏子进程。游戏提供可匹配 AX 窗口根但零子控件；独立 `visual_only=true` 回退仍需另一靶场。没有实际发游戏输入，所以**简单操作兼容性尚未验证**。
- Live 保持任务面板可见性绑定：当前任务切走时清图/隐藏；只在用户短暂回到本任务后采集。把预览目标放到虚拟屏与把游戏源留主屏是两个独立选择。本轮只验收源/目标跨屏、可见性/位置、采样节奏和清理，不宣称游戏内操作成功。
- Plugin macOS Skill 补零 AX 子控件与跨屏 `on_requested_display` 核验，Codex 官方 cachebuster/reinstall 更新为 `0.2.0+codex.20260928140038`；当前已打开任务的旧 Skill 文本不会原地重载，新任务可加载新版。未修改其他插件、Host 配置或配对密钥；未提交/推送。
