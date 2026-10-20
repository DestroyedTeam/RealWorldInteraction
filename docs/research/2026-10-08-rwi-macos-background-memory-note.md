# RWI macOS 后台事件：已复核基石与旧概括替代
Date: 2026-10-08
Project: /Users/vincentadamnemessis/AI Projects/Real World Interaction
Scope: this project macOS native backend; not Windows/Linux; no global preference.

用户明确要求继承 Strange Beasts General Store「任务接力/前后台相关」的研究，优先探索真正后台 NSEvent 路线，二次检查 hover/drag；不要把失败摘要当不可能。

- NSEvent 是构造/本进程分发对象；已研究跨进程方案仍 factory → CGEvent → postToPid。不可声称换构造器自动通用真后台。
- 该项目137项校准记录中hover37：13 blocked before post，24 posted但tooltip未确认；一个dragged-hover在PID tap收到但无down/up，不是完整drag失败。业务reason解析还有历史不兼容；不要将其scene/oracle gate移入RWI核心。
- RWI已有PID/window定向输入、同窗口drag；旧“Mac drag未实现/必须OS前台/后台hover必不能”概括被本note与当前研究取代。真实on-screen枚举、屏幕mapping约束仍未解除，不能通过删guard宣称解决。
- Wine普通move可全桌面重新选窗；GLFW hovered getter读全局光标；SDL macOS≥26部分版本用全局位置覆盖event位置。上游不是本机GameHub已确认版本；分层证明接收与客户端消费，不静默activate/warp用户光标。
- 下一步：Rust macOS构造roundtrip probe → 自有AppKit接收fixture → typed Move/Hover与完整source/button drag → window-local/offscreen独立适配 → 显式分配窗口的同状态A/B。inactive、covered、offscreen、otherSpace、hidden、minimized分开测。
- 本轮仅研究/文档修正，无新native实现或部署、无GUI输入/截图。保留历史记录，不改生成memory或已有notes。

Evidence: /Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/research/2026-10-08-macos-background-nsevent.md
Sources: /Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/research/2026-10-08-macos-background-sources.json
Receipts: /Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/research/2026-10-08-macos-background-evidence.json
