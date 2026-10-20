# R87：原生跨应用拖放执行中取消／端点退出

## 结果

- 两项均用部署的 `computer.drag_between`，各一条新的5000ms请求。服务-issued exact operation_ref用于取消，原始输入从未重放。RWI产品源码／Agent／MCP／证书／权限不变。
- source PID23247/window46257，destination PID23248/window46258，virtual4。两个真实不同进程；操作前后台条件由原生state与截图确认。

| 顺序 | 行为／实际结果 |
|---|---|
| 1：旧R87观察器 | 两次正常drop，但短观察窗口未覆盖输入活动期；cancel、SIGTERM均未发出。明确**未接受**，保留原记录，不重新解释为成功。 |
| 2：独立父控制器武装 | 不从MCP工具提交时间猜测开始；独立上下文等待实际source_began/active_sequence，输入仍只由新Host加载的RWI工具产生。没有新的隐藏input bridge。 |
| 3：执行中取消 | native active16、began1/ended0时查询原request获得服务ref；观察1791824925318，cancel提交1791824925433、返回1791824925449。ack仍executing/drag_wait，尚非停止。 |
| 4：取消终态 | `operation_cancelled`，原trajectory219ms、moves3、release_inserted=true；独立native ended1/cancelled1、operation0、destination consumed0。原receipt uncertain、fenced=true保持历史原样。 |
| 5：只读资源恢复 | writer idle、HID和combined样本pressed keys/buttons均0，recover_input没有native input；解除隔离但old_operations_replayable=false。非手工删除fence／重置权限。 |
| 6：执行中目标退出 | 新请求native active17、began2/ended1。PID、CG窗口、进程出生/完整命令、UID、bundle、run_id、binary hash均核对后，仅SIGTERM分配的destination，命令exit0。 |
| 7：退出后中止 | `process_identity_unknown`，trajectory152ms、moves1、release_inserted=true；原生累计began/end/cancel=2/2/2，operation0，目标consume仍0，未向其他目标发送fallback。 |
| 8：清理 | source具名AXPress一次，实际exit0；destination受控SIGTERM实际exit143，不冒称正常退出。两绑定detach；再次只读恢复后当前fence=false，历史原request仍uncertain／no replay。 |

原图按原时间线保存在本地 `outputs/validation/r87-native-interruption-2026-10-13/`；不随源码提交。旧R87原图/失败记录留在事务目录。

## 调度与耗时边界

旧R87取消实验外层tool到return12466ms，而实际MCP convenience elapsed5353ms（act5331ms，2次resolve合计15ms）；观察器只运行7秒。这说明该测试的观察窗口没有覆盖真实输入，**不能据此证明RWI取消实现失效，也不能把差额直接归为原生CPU耗时或某语言慢**。独立控制器复验获得实际活动期及原生终态，避免同一编排中的观察／action排队猜测。

MCP规范要求引用进行中的请求，说明取消可以迟到、接收者可忽略已完成请求；rmcp RequestContext cancellation token是通知机制，仍需资源/应用回读，不是业务成功保证。[MCP 2025-11-25 cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)。同时复核项目R17控制优先级产物和Rust trajectory配对释放测试。

## 同一四角色与剩余范围

保留R86 `Intent::DragBetween` / `paired_pointer_drag` / `computer.drag_between`。同初始化+tools/list输入的实际MCP副本行为仍为BASELINE缺失(1/server0)、MODIFIED存在(0/server0)、ROLLBACK恢复缺失(1/server0)，原source20字段及binary hash恢复，REAPPLIED0。该副本回滚不是生产原生drag回滚。

R86 exact head ab38366f11fe445b4b07b9b6c2141f168523436c，Actions38278523521的Mac和Windows两job实际success。当前Rust源保持该已验版本；完整F01–F16仍未完：原生DnD中BG→FG、窗口几何变化/更广payload与客户端、冷后台hover、窗口族和browser等继续，不能以两个中断阳性替代全功能验收。
