# R85：原生 AppKit 拖放接收器与实机对照
## 当前结论
已实现 Rust 自有接收器的原生 NSDraggingSource / NSDraggingDestination、独立类型的 NSPasteboardItem、实际 performDragOperation 消费以及有界会话去重。它不是画布蓝线计数器，也不是生产后台的隐藏桥接程序。**目前只验到同一窗口内、同一应用内的真实后台拖放；跨应用输入路线仍未实现/验到，完整 F01–F16 仍未完成。**

## 顺序、结果与问题
1. 原配置基线明确拒绝 --native-dnd-controls，退出1。补齐分配参数的同输入基线仍退出1；新版配置检查退出0并明确 gui_created/input_sent/native_binding_verified=false。
2. 初始170/171启动器未保留持续观察句柄，原生PID/窗口随后不在；原生退出状态及原因未知。未将启动器退出0当 GUI 成功，未发送或重放输入。改用持有原子进程句柄并等待终态的172/173。
3. RWI 对后台173发一条1200ms新拖放：原生开始1、移动19、结束1(copy)，目的地进入1、更新16、准备/实际消费/结尾各1；消费精确类型载荷1次，external=false。应用仍在后台。
4. Computer Use 同坐标对照（550ms调用）：新会话9开始，但未观察到原生结束或第二次载荷消费；普通 canvas completed 增长不能代表 native drop。RWI一次Escape回执返回但没有取消证据，未重放。173寿命关闭后仍挂在原生拖放循环；保留此负例，仅清理该自有进程，真实退出143。
5. 接收器补上原生计数可见摘要和“End own fixture”，与共享窗口标题栏的关闭按钮分离；关闭时若存在未结束的 native session，先记录未确认状态，再仅终止这个自有一次性应用，不伪造 ended/cancelled/drop 回调。
6. 最终174/175两条独立新RWI拖放分别真实消费1次，开始/结束1/1、移动19、copy=1、external=false。两者收/发时采样为后台；不称前台例通过。
7. 新174的CU对照（529ms）重现会话12开始但未结束、未增加消费。第一次RWI关闭保护性拒绝：native_input_dispatched=false、资源释放；确认原请求终态后，以新观察/新请求显式关闭。该请求保留 AXError -25204/uncertain，未重放。独立源码内状态记录 explicit_close_requests=1、active_at_application_termination=12，应用真实退出0。175普通结束按钮关闭后退出0。均非寿命超时。关闭应用不是通用拖放取消验收。
8. 原RWI输入请求与关闭请求均按原durable身份查询，终态、无pending、资源释放；独立窗口已detach。Agent/证书/权限未改变，未操作游戏、飞书、Chrome或用户文档。

## 回归与限制
最终 fmt --check、workspace tests（795通过/0失败/11忽略）、receiver fixture（83通过）、全target/全feature Clippy -D warnings、架构规则及其10个负例、diff --check已通过。错误路径 tools/check_architecture_boundaries.py 的真实退出2保留，复用R84已成功命令数组 tools/check_architecture.py 后通过。架构ratchet通过不等于全部架构债务清零。
RWI动作用1200ms、CU drag为其默认时序；1438/1474ms与529/550ms不能据此做工具性能优劣结论。此对照只说明该接收器上的原生会话/消费差异，不外推所有应用或CU实现原因。
载荷 source_pid 为自报fixture数据，不授予原生身份/输入权限。external=false来自NSDraggingInfo原生getter，不从PID文本推断。未接受跨应用消费、native drag通用取消或真实人手来源。

## 原始截图
均为实际RWI MCP源窗口截图，不是Live浮层或伪造合成。
![初始源](../../outputs/validation/r85-native-dnd-2026-10-12/02-source173-before.png)
![174原生消费](../../outputs/validation/r85-native-dnd-2026-10-12/05-final174-native-drop.png)
![175原生消费](../../outputs/validation/r85-native-dnd-2026-10-12/06-final175-native-drop.png)

## 原始证据
work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/native-dnd-fixture-transaction-r85：
CANDIDATE_TRANSACTION_R85.json、BASELINE_DND_ALLOCATED_CONFIGURATION.command_execution.json、NATIVE_RESULT_R85.json、LOADED_MCP_NATIVE_AND_CU_EVENTS.json、FINAL_LOADED_MCP_EVENTS.json、RWI_FINAL_NATIVE_ORACLE_174/175.json、CU_FINAL_NATIVE_ORACLE_174.json、FINAL_STATE_174_AFTER_EXPLICIT_CLOSE.json、NATIVE_174/175.command_execution.json、FINAL_GATES_WITH_CORRECTION.json、RAW_SCREENSHOTS.json；同四角色保留配置、源码、补丁和独立复制回滚。

## 公共API依据
- [NSDraggingSession](https://developer.apple.com/documentation/appkit/nsdraggingsession?language=objc)：创建会话与下一次run-loop实际开始是不同阶段。
- [beginDraggingSession](https://developer.apple.com/documentation/appkit/nsview/begindraggingsession(with:event:source:))：真实源事件、拖放项和source协议。
- [draggingSource](https://developer.apple.com/documentation/appkit/nsdragginginfo/draggingsource?language=objc)：跨应用时该原生对象为空，不能以自报PID代替。
- [performDragOperation](https://developer.apple.com/documentation/appkit/nsdraggingdestination/performdragoperation(_:)?language=objc)：实际导入载荷，与source/普通鼠标回执分开。
- [NSApplication terminate](https://developer.apple.com/documentation/appkit/nsapplication/terminate(_:)?language=objc)：仅用于这个自有无文档fixture的退出，不成为RWI对任意应用的回退。
