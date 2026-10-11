# Live 已有会话的独立控制通道

## 真实问题（2026-10-11）

Receiver 42 的一次 5 秒后台拖拽确实收到按下 1、移动 79、松开 1；用户却只在结束后看见完整蓝线。该次输入已完成，不重放，也不计作执行中的用户切窗通过。自己的画布绘制代码同时绘制 active 和 last_completed 路径，并非刻意只画最终结果。

离线独立复现了一个产品缺陷：Agent 对所有 Execute 使用同一个 Context.gate 和 Executor mutex。输入占用它们时，computer.live 的状态、已有预览控制和内部 Host-view presence 都返回 context_busy。真实 UI presence 必须持续到达才能维持原生 1500ms 可见期限；延长这个期限会掩盖任务切换，并非修复。该缺陷能解释更新中断，但不能仅凭源码将真实画面的一切延迟都归因于它。

## 分层与约束

- rwi-core 定义 Send + Sync 的 LiveController；只暴露已拥有预览的控制与 presence，不能创建预览、选择窗口、捕获图像或发送输入。默认后台诚实返回 capability_unavailable。
- NativeMacDesktop 输出自己的控制句柄。Session 的控制共享既有 bounded State / worker；拥有 Session 的 backend 被退休时先 stop，保留的控制句柄不能复活它。用户/安全会话/boot/seat 的 presence 校验仍在设备端执行。
- Executor 在正常串行工作通道上发布 exact-target controller catalog；创建、绑定、附加、移除后的目录更新不会推断新设备或授权。
- Agent 保留原来的 Prepare/Execute 身份、principal/target 校验、一次性消费与 cancellation。只有非 start 的 computer.live 进入单槽 live_gate；它不获取输入 lease，不触碰 durable input reservations，不清除活动输入的 entered/fence。
- status/pause/hide/resume/stop 与内部 presence 不再等待长输入。start、重配置、其他观察与输入仍经原工作通道；新能力不是任意后台任务并发。
- CloseContext / idle eviction 同时检查两条通道；原生资源未归还时不能声称停止。control_busy 不是输入失败，更不是重放输入的理由。
- Host component capability、原生 view_id、单调 sequence、1500ms expiry、用户 Hide/Stop 与 hard lifetime 保持原语义。没有假造组件可见、连接即任务活跃、或者原生任务 ID 证明。

## 验证边界

同一离线 status 输入在未占用工作通道时返回真实 unsupported；占用 gate 和 executor 后旧版变成 context_busy，新版保持真实 unsupported。额外测试验证有效 presence、隐藏/旧序列、停止、创建仍串行、主体与目标隔离、取消、容量、关闭竞争及 Session 退休。冻结源环境上的 rollback/reapply 使用同一 probe，临时测试工具在运行后恢复文件哈希，不部署旧 Agent、不重放 GUI 输入。

真实输入期间帧数、原生可见性以及人眼看到渐进蓝线仍需单独验收，不能以这些离线测试代替。本修复不宣称系统面板聚合、多窗口全覆盖、任意应用真后台、性能优越或完整验收已经完成。
