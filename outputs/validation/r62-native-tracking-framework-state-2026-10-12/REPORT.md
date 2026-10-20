# R62 · 后台鼠标事件与AppKit光标／tracking状态对照

当前：RWI / 完成真实RWI—Computer Use和框架位置条件对照 / 泛化后台悬停尚未修复，不标为完成。

## 按真实执行顺序

1. **139基线**：inactive/nonkey；RWI一次默认background_window move真实到达Application/Window（type5、同CG44285、hit_region=hover_zone），但view entered/moved/exited与tooltip均0。完整保留该请求，不replay。
2. **只增加原生诊断**：`DispatchEvent::framework_pointer` 可选快照来自公开NSEvent.mouseLocation、own NSWindow.convertPointFromScreen / mouseLocationOutsideOfEventStream 和own NSView转换。未改变responder、active/key、event sender、tracking/tooltip计数；未读其他应用UI。未知保持null，不把诊断inside变成输入目标已达成。
3. **140目标当时active/key也不够**：事件window_point=[100,382]，实际framework window point=[-1151.03125,130.53125]，hover-local=[-1175.03125,261.46875]，inside=false；view hover0。这反驳“只是目标不在前台”这一单因解释；并未证明全部因果链。
4. **Computer Use实际对照**：该API未公开独立hover方法，所以只对无害hover区域调用一次click([200,340])，不是虚构hover call。它真实收到down/up，随后RWI同点click也真实down/up；两者回调均inactive/nonkey，hover和tooltip仍0。Cua事件subtype3/flags1048576，RWI subtype0/flags0；未采集修饰键来源，不能断言这些位由谁加入。输入比较只限这两个click，不拿click与单move作相同耗时比较，不泛化成某工具优劣。
5. **140明确物理路线正例**：单独focus140，然后请求foreground_system move（不是自动fallback）。framework screen=[1924,495]为AppKit lower-left；换到own window=[100,382]、own hover=[76,10]且inside=true，真实enter1/move1，之后tooltip1。默认move未把framework pointer搬到这里。
6. **141最终源条件对照**：新窗口恰好出现在上述已留在屏幕位置的指针下面，输入前已经真实entered1/tooltip1；不把这些既有计数归功于RWI。一次window-addressed move使view moves0→1，但target仍前台，不能冒充后台验收。随后明确focus不覆盖141的右侧peer142；141 inactive/nonkey/fg76688，指针与既有tracking仍在141hover内，另一次新background move真实moves1→2。证明该fixture在已建立tracking/真实pointer-inside条件下可以后台接收；不等于能在任意后台点建立hover。
7. **私有字段独立校准**：仅对本进程新建CGEvent内存，用动态解析的SLEventSetIntegerValueField写field58=987654321；公开CGEventGetTimestamp真实从123456789变为987654321。post/record符号只检查存在，不调用，不访问认证材料、不创建窗口、不发输入。不能把某参考代码的“click-group ID”注释直接搬进RWI，更不能据此破坏实际event timestamp。
8. **清理**：139明确AXClose/expiry0/exit0；140到期expiry1/exit0。140最后snapshot返回process_identity_unknown后，读取elements的JS TypeError保留，Close请求从未启动，不能称关闭成功。141/142明确Close/expiry0/exit0、所有固定binding退役；11原durable requests terminal/released/no pending/no release uncertainty/no replay。Cua只有一次独立已捕获的click，不由RWI receipt冒充它的记录。

## 原始截图

### 01 原139，未收到view tracking

![01 原139，未收到view tracking](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r62-native-tracking-framework-state-2026-10-12/01-baseline139-no-tracking.png)

### 02 同140后台click比较前

![02 同140后台click比较前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r62-native-tracking-framework-state-2026-10-12/02-140-before-background-click-comparison.png)

### 03 141输入前已是green/entered1，不能冒充新成功

![03 141输入前已是green/entered1，不能冒充新成功](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r62-native-tracking-framework-state-2026-10-12/03-141-native-pointer-already-inside.png)

### 04 141 inactive时新move确实1→2

![04 141 inactive时新move确实1→2](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r62-native-tracking-framework-state-2026-10-12/04-141-inactive-background-move-received.png)

## 资料与结论边界

- Apple说明tracking依赖cursor进入/移动，并区分activeAlways；这是配置机制，不是对任意合成type5都能触发tracking的保证。[Apple Tracking Areas](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TrackingAreaObjects/TrackingAreaObjects.html)
- 本轮重新获取并固定trycua/cua source tree，Rust mouse.rs的move_cursor_desktop明确warp真实pointer并走HID，还发送两次move；不能把这个实现包装成不干扰用户的真后台hover。SkyLight函数的可解析性不是应用成功保证，当前实验尚未做SLEventPostToPid输入。[Primary mouse.rs](https://github.com/trycua/cua/blob/main/libs/cua-driver/rust/crates/platform-macos/src/input/mouse.rs)，[Primary skylight.rs](https://github.com/trycua/cua/blob/main/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs)
- 参考mouse.rs field58所谓gesture group通过skylight.set_integer_field写入；本机当前runtime实际timestamp被改，故不能盲目移植。仅对固定源码和本机实验下结论，不等于全部仓库版本都有同一行为。
- 最终完整11门exit0：workspace769/0/11/42，Receiver70/UI7，fmt check/全targets-features Clippy-Dwarnings及架构门通过。先前objc msg_send缺comma warning保留，已修正后重跑；Native141为最终无警告构建，140为诊断初版的真实历史，不伪装为同hash。
- **本轮没有更换生产Agent/MCP、证书或权限**。只补测试观察器和无输入的事件字段校准，后台通用hover/tooltip仍未实现。全目标继续保留，不以这份条件性通过、Cua同例失败或私有symbol存在代替完整验收。原R47/48/56/61结果保留。
