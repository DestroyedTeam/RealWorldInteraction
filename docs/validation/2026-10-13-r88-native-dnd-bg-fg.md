# R88 原生拖放执行中切前台验收（2026-10-13）

## 实际结果：发现问题，不记为 drop 成功

- 两个本轮专用窗口：source PID78136 / CG46312，destination PID78138 / CG46311，均在已确认的虚拟屏。起始均后台。
- 唯一加载MCP请求 `r88-native-pair-bg-fg-1`、5000ms。源native began/moved/ended/cancelled=1/31/1/1；目的consume=0。原请求 returned、goal_verified=false、replay=forbidden。
- 源首次真实Down后2016ms，frontmost_pid变78136；2110ms active/key均true，2306ms回89561。确实观察到切前台，但没有证明native session切前台后继续且成功drop。
- native callback只有累计counter，不能精确证明native ended在foreground前/后；取消原因亦未确定。不能将此推广为macOS永远不支持或为RWI伪造成功。
- 原请求terminal、resources_released=true、无native pending／release uncertainty、fence=false，无重放。两应用通过自身End控件关闭，真实wrapper退出0/0。生产Agent仍80314、原四权限有效；本轮未重新部署Agent/MCP。

## 最小修改与同输入可逆证明

`crates/rwi-macos/examples/receiver_fixture/model.rs` 仅放开 native_dnd_controls 联用 self_foreground_on_first_down_after_ms；保留launch-time/global CommandTab、panel/geometry互斥、regular-app、100..5000ms、lifetime与单触发校验。真实NativeDown接入既有一次性own AppKit actor，无新增外部GUI输入桥。

配置BASELINE退出1，stderr `Error: "native DnD fixture cannot combine with a panel, geometry or input actor"`；MODIFIED退出0，stdout为configuration_valid=true、gui_created=false、input_sent=false、native_binding_verified=false、native_dnd_controls=true；独立ROLLBACK副本恢复源与binary原hash，原输入再次退出1；REAPPLIED退出0。此证明仅配置，不替代上方实机原生结果。

回归：workspace807通过、0失败、11ignored；receiver85通过；fmt、clippy所有target/features locked offline、架构ratchet、10规则测试、diff check退出0。ratchet不是架构全部完成。

## 按时间顺序的实际截图

### 拖放前：源窗口

![拖放前：源窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r88-native-bg-fg-2026-10-13/10-source-before-original.png)

### 拖放前：目的窗口

![拖放前：目的窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r88-native-bg-fg-2026-10-13/11-destination-before-original.png)

### 拖放后：源窗口

![拖放后：源窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r88-native-bg-fg-2026-10-13/18-source-after-original.png)

### 拖放后：目的窗口

![拖放后：目的窗口](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r88-native-bg-fg-2026-10-13/19-destination-after-original.png)

## Live 与后续

R73已有280×300点上限、隐藏／恢复／关闭原生按钮和生命周期实证，本轮只复核源码与官方资料，不再次声称实机验过或Computer Use私有像素1:1。用户睡觉后不再设置人工切窗/点击门槛。

下一轮先增加fixture-only有界native callback时间线，独立记phase/time/native session/frontmost/active/key，再执行一次新实验；不盲目改生产后端，不重复r88请求。冷后台hover、键盘、真实foreign-provider层级、browser多窗口、正式offscreen Host及分层性能按完整F01–F16继续。

证据目录：/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/native-dnd-bg-fg-r88

## 官方API边界复核

[Apple WWDC23](https://developer.apple.com/videos/play/wwdc2023/10054/) 与 [NSApplication.activate](https://developer.apple.com/documentation/appkit/nsapplication/activate())说明activation是请求，不保证成功；不使用legacy强制或循环activate来伪造验收。[shouldDelayWindowOrdering](https://developer.apple.com/documentation/appkit/nsview/shoulddelaywindowordering(for:))延迟排序不是永久阻止排序；此源码差异尚非已证明的取消根因。
