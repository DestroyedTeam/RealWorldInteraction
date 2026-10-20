# R86：两独立应用的原生后台拖放

## 已实现

新增 `Intent::DragBetween`、`paired_pointer_drag` 和 typed `computer.drag_between`。source / destination 使用各自 target、view_id、pixel point；网关独立解析两个视图，再持有同一原生桌面的短租约。核心复核两端身份、原生窗口、引用、映射与落点；macOS 按起始 PID/window 发送一次按下、设备节拍移动和配对松开，由 AppKit 原生拖放管理器交付目标。普通 `computer.drag` 仍要求全程留在自己的窗口内。Windows 明确不实现该路线，不降级为全局输入。

不提供激活、全局光标、剪贴板写入或输入重放 fallback。执行中仍检查两端窗口/进程/上下文、显示布局、模态及 TCC；本次输入的 primer 不再被 combined-session 活动计时器误判为另一名用户。微小浮点舍入允许阈值为 1e-7 desktop point，不允许像素级替代落点。

## 实际验收，按时间顺序

| 步骤 | 实际结果／问题 |
|---|---|
| 两扇专用 AppKit 窗口启动 | source PID82004/window46090；destination PID82005/window46089；同一 virtual4。启动最早 source 曾前台；**操作前独立记录两端 inactive/frontmost=false**，不将最早启动状态冒充后台。 |
| 新 Host 加载工具 | 原任务旧 MCP 未加载新 typed 工具；按已授权接续方式创建同项目新会话，实际 loaded `computer.drag_between`。没有请求人工重启、切窗或展开。 |
| 新鲜发现／绑定／截图 | 初次 attach `window_not_discovered`、native_dispatch_entered=false 保留；重新紧邻 exact-PID discovery 后绑定，无输入重放。01 source-before、02 destination-before 为原图。 |
| 单次1200ms拖放 | 唯一 `r86-native-source-to-destination-1`；source began/ended=1、moved=19、operation=copy；destination prepared/performed/concluded=1、consumed=1。 |
| 目标回读与原图 | `last_source_external=true`，payload.run_id=`r86-native-source`，native source_pid=82004，目标window=46089。03 source-after、04 destination-after。不是 canvas 计数，也不是同应用假装跨应用。 |
| 后台／资源回读 | 前后两端均 inactive/frontmost=false；500ms环境采样未记录动作中的前台变化，但不是连续性／物理人类来源证明。原 durable operation terminal/released、no pending/no release uncertainty；receipt 仍 `goal_verified=false`。 |
| 关闭／detach | 各自一次具名 End own fixture，实际两进程exit0；两个独立target已detach，不关其他程序。 |

原始截图仅保留本地 `outputs/validation/r86-cross-app-dnd-2026-10-13/`，不随源码提交。PNG含可读原生回调及payload；可用于重核验，不能由截图 alone 证明授权。

## 工程与回滚

807 workspace tests /0 failed /11 ignored /42 suites；83 receiver tests；12 paired API/core 专门测试。fmt、locked/offline workspace、all-target/all-feature Clippy -D warnings、architecture检查及10项规则测试均0。Architecture ratchet仍明确architecture_complete=false。首次 workspace101来自新工具未加入catalog期望；修正工具清单及无效参数负例后全套通过，未删负例。

Agent与MCP已构建部署，继续同一本地签名证书／designated requirement，四项权限有效；profile、CA、证书及密钥未替换。native Agent PID80314，instance4715f706-beeb-4199-b150-2226cef5b09e。原子部署前取得原共享租约文件的排他锁并核对当前实例 durable records，无输入pending，不恢复／清除fence。

相同 initialize + tools/list 输入：BASELINE 缺少 paired schema（probe1/server0）；MODIFIED 有 paired schema（probe0/server0）；独立副本运行可执行 ROLLBACK.sh 后再次缺少（probe1/server0），20源码字段hash/缺席及原binary hash全部恢复；REAPPLIED probe0。此处是真实 MCP schema和源／binary副本回滚，不是原生拖放的生产回滚。保留初始RTK单参数含空格路径执行错误；用cwd+相对二进制和原profile参数修正，不能把RTK失败算作MCP进程失败。

## 仍未完成，不扩大结论

本次只证明两专用不同PID AppKit目标的**一次**原生后台drop。原生DnD执行中 cancel、任一端销毁/改布局/TCC变更、目标切前台、file promises/其他 MIME、浏览器/WebView/第三方应用、性能并发边界仍需实机验收。一般Drag已录得的释放／切窗证据不自动等同原生DnD。F01–F16完整目标继续，不以一项阳性停止。
