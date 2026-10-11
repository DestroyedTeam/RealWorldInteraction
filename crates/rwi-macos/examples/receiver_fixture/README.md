# RWI macOS 接收验收靶场

这是独立 **Rust / AppKit 接收应用 example**，不是 backend、输入桥或对其他软件的操作脚本。
默认不申请权限、不截屏、不发鼠标/键盘或他应用输入，不装事件 tap / 全局 monitor；
只有下文显式分配的双靶场 Command+Tab actor 可以发送一次有界系统快捷键，
不使用私有 SPI。只在自身关闭时向本 NSApplication 排队 ApplicationDefined wake，
用来解除自身 run loop 等待；它不计为输入或目标成功 receipt。
没有 `--open-window` 的构建和纯模型测试不会启动窗口。真实运行以对应验收 run 的
PID、binary/source hashes 和接收记录为准；构建成功不是 native delivery 证明。

## 构建与离线测试（不会创建窗口）

```sh
rtk cargo build -p rwi-macos --locked --offline --example receiver_fixture
rtk cargo test -p rwi-macos --locked --offline --example receiver_fixture
```

没有参数或 `--help` 只打印帮助。创建窗口必须显式提供 `--open-window`、run ID、
非零 u32 `--display-id` 和一个
**绝对路径的新目录**。目录父级必须已存在；目录若存在则拒绝，避免覆盖旧 run。
own-window lifetime 默认 **120000 ms（120 秒）**；可选 `--lifetime-ms` 严格接受
**1..600000 ms**，拒绝 0、负数、越界、缺值和重复；不允许无限生存。
无参数/`--help` 只打印帮助，既不创建窗口，也不启动 timer。

## 仅在 root 分配新靶场后运行

```sh
rtk cargo run -p rwi-macos --locked --offline --example receiver_fixture -- \
  --open-window --run-id receiver-acceptance-001 \
  --state-dir '/ABSOLUTE/EXISTING/PARENT/receiver-acceptance-001' \
  --display-id DISPLAY_ID --lifetime-ms 120000
```

上面的路径是有意保留的 typed slot，不能原样执行。窗口标题是：

`RWI 验收 · Receiver · receiver-acceptance-001 · PID <本次 PID>`

stdout 和 `state.json` 都给出 **本次 PID / 精确标题 / native window number**。
调用者先用 loaded RWI discover 精确 PID/title，再 bind 那一个窗口；不得绑定游戏、
飞书、Chrome 或用户文档。没有猜测 display ID、切换别的应用或自动激活。
DISPLAY_ID 必须来自当前独立确认的虚拟屏，不能原样执行。fixture 通过 public
`NSScreen.deviceDescription["NSScreenNumber"]` 精确匹配后，**创建窗口前**依该
screen 的 AppKit frame 定位；缺失、完整 window frame 不容纳或初始 readback 不匹配
则退出，不回落主屏。窗口 content size 为 `660×520` points，标准 titlebar frame
由 public NSWindow conversion 计算；JSON 有 requested/actual display ID 和 frame。
不要把这些 UI 常量当作截图像素或验收点击坐标；后续一律重新 observe 当前窗口。

生命周期在主线程，采用 NSApplication `finishLaunching` / `run`。
关闭 **这个 fixture** 的标准窗口关闭按钮会记录 `window_closed` / `status=closed`，
取消未完成路径、停止自身 run loop 并释放 retained UI；不退出其他应用。
当显示/共享 chrome 隐去该按钮时，不猜测其他按钮或借键鼠强关：本应用定位验收
成功后，在 main run loop 的 public `NSRunLoopCommonModes` 安装一次性 `NSTimer`，
到期只对所 retain 的 **自身窗口** 调用 public `performClose:`；借真实
`windowWillClose:` 走同一 `closed` model 和退出路径。JSON 单独记录
`lifetime_ms/lifetime_close_requests`，请求不等于关闭。若共享 chrome 禁用
performClose 的按钮动作，只对同一个 retain 的自有 window 调 public `close`，
并向 stderr 明确记录该分支，不碰系统共享控件；若仍无真实关闭通知或 run loop
无原因返回，则记录 error，不伪造 `closed`。
timer 在 own UI/run lifetime retain；关闭/cleanup 在同一主线程 invalidate/release，
没有全局 hook。AppKit timers 并非硬实时；busy/suspended main thread 可迟到，
不能把默认 120 秒解释为操作系统强制实时 kill。SIGINT 仍是进程退出，未观察真实
windowWillClose 时不声称 `state.closed`。

## 三类接收 oracle

|区域|实际接收后改变|不能推导的结论|
|---|---|---|
|Safe count NSButton|`safe_button_actions`；标准按钮 title 和下方 AX 文本更新|target/action 也可由 AXPress 触发，不证明 NSEvent 一定是输入源|
|Hover zone|`hover.enters/moves/exits/inside/last_point`；进入后绿色反馈；`tooltip_requests/last_tooltip_point` 独立记录 AppKit 取字符串回调|`tooltip_registered=true`、`tooltip_requests>0` 都不是 tooltip 已经显示；`mouseMoved` 和 tooltip request 不伪造 `mouseEntered`|
|Drag canvas|原始 down/drag/up receipts，蓝色路径、起终点、move 数、`drag.completed`|孤立 dragged/up、纯 down/up 点击、未完成路径都不计完整 drag|

纯 model 单测不调用 AppKit、不发事件；不是真实接收验收。
AppKit view 用 public `acceptsFirstMouse:` 和 tracking `ActiveAlways`，允许测试 inactive
接收；这并不保证任意 producer 的 process-directed moved 会生成 tracking/tooltip。
tooltip 使用 public `addToolTipRect:owner:userData:`，hover view 本身是 weak owner，
由 own window/UI 维持 lifetime；public `NSViewToolTipOwner` 的
`view:stringForToolTip:point:userData:` 只登记 **AppKit 请求文本**。回调原始 point
独立保留为 NSView-local AppKit points，不与 mouseMoved 混用，不设置 visible 标志。
实际 tooltip 显示仍需另一个限制在本 fixture/owned tooltip 的 GUI 视觉/AX oracle；
字符串回调、登记和截图中的显示是三个不同证据层级。
JSON 每条 receipt 分开记录本应用 AppKit active 和 OS frontmost PID（不读取标题/
文本），mouse receipt 另记录本窗口 key 状态。producer 的 application-activated
通知可能改变 isActive 而 OS frontmost 未变，所以后台验收必须同时检查
系统前台、目标 AppKit active/key、窗口顺序和实际 goal；仅
`receiving_app_frontmost=false` 不证明用户输入连续性或遮挡状态保持。
这些是顺序采样，不声称与输入原子一致；没有全局 hook。

`state.json` 是独立可读 oracle，原子同目录 rename，目录 `0700`、文件 `0600`。
它记录本程序自己的数据，不含其他应用文本；最近事件和路径各保留 128 项，有显式
`dropped_records/dropped_points`，计数不截断。发生写入/回调错误不得接受目标成功，
检查 `output_error`、`status` 和 stderr。

标准 AX 文本 `RWI receiver state` 给出当前 counters/status，按钮 AX identifier 为
`rwi-fixture-safe-button`；hover/canvas identifiers 为 `rwi-fixture-hover` /
`rwi-fixture-canvas`。GUI 可观察这些标准 NSButton/NSTextField 状态；JSON 更完整。

## 显式 first-responder 悬停校准（默认关闭）

`--hover-first-responder` 仅用于明确分配的自有 Receiver：创建 hover view 的专用
子类，让它接受 first responder，并在设置 own content view 后调用一次 public
`NSWindow.makeFirstResponder:`。不激活其他应用，不发事件，不改 backend；原有
`mouseMoved/Entered/Exited` 回调仍由正常 AppKit 路径调用，不伪造 tracking、tooltip
或接收成功。无该参数保留原类和原 first-responder 设置，重复／未分配参数拒绝。

`responder` 独立采样 own window 的 `first_responder_class/region`、
`accepts_mouse_moved`、hover tracking-area 数量；校准请求及 setter 的实际返回值
单独记录。配置接受不等于 mouseMoved 已到达，更不等于 mouseEntered 或 tooltip
已显示。先以默认模式和显式校准模式接受同一 grounded RWI hover，再分别核对
App/Window dispatch、真实 view receipt、tracking 和像素，不能改应用来冒充通用支持。

依据：Apple [Event Architecture](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/EventArchitecture/EventArchitecture.html)
说明普通 mouseMoved 路由到 first responder；
[Using Tracking-Area Objects](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TrackingAreaObjects/TrackingAreaObjects.html)
说明 tracking owner 回调是另一个证据层级。本模式是接收者配置诊断，不是对任意应用
hover／后台 tooltip 能力的承诺。

## 坐标和输入证据

### 自有 App / Window dispatch 探针

fixture 在首次 `sharedApplication` 前注册自己的 `NSApplication/NSWindow` 子类，
向该 Application 子类请求 shared singleton，并验证实例确含探针；只用自有
Window 子类创建这一个窗口。不 swizzle 既有 App，不安装 local/global monitor。
两个 `sendEvent:` override 对所有事件都 **正常 super 转发一次**，不删、改、重发。
只有 mouse types `1..9 / 25..27` 记录，键盘、flags、scroll、tablet 及自身
ApplicationDefined wake 不记录；不读取 characters/keyCode、全局鼠标或他 App。

`subtype` 只读取普通 mouse down/up/move/drag／other-button 事件；对 tracking
enter/exit（type 8/9）记录 `null`，仍保留原始 type、坐标、时间和正常 super 路由。
本机真实 NSEvent 构造探针显示：对 8/9 调用 subtype 会触发 Objective-C exception，
Rust 的 catch_unwind 不能接住它。不能把“mouse-related”一概解释为所有 accessor
均有效，也不能用 0 冒充未读取的 subtype。此修复是观察者的合法字段读取，不是输入
transport 改变、伪造 tracking 或吞掉原生事件。

`dispatch_probe` 是与 view/tooltip oracle **分离**的诊断字段。application/window
计数指各层观察，不是两个物理事件或目标成功；每条记录包括独立 sequence、stage、
原始 native type/windowNumber/window points/subtype/timestamp、own-window 匹配和
public own-content `hitTest:` 的 region 快照。hitTest 使用 **superview 坐标**；
未匹配 own window 不读取它的 view，也不伪造 region。hit region 不保证该 view
收到事件；`mouseMoved/Entered/Dragged` 及 tooltip request 仍只由各自真实回调记录。

记录只改独立 bounded model，不在 super 前 persist、flush、采样环境或改 UI。
`RefCell` borrow 在 super 前释放，重入借用失败仅记 `borrow_skips`；super 返回后
只原子写 JSON，不刷新按钮/文字/绘制/焦点，保持已有正常接收路径。JSON fsync 和
序列化是明确的观察者额外开销，不能据此做无探针性能结论。
history 最多 128 条，application/window counters 不截断，`dropped_records` 显式。
无参数/help 路径不进入 AppKit，也不创建这些子类、窗口或输入。

公开依据：Apple [NSApplication subclass/sharedApplication](https://developer.apple.com/documentation/appkit/nsapplication?language=objc)、
[NSView.hitTest 的 superview 坐标](https://developer.apple.com/documentation/appkit/nsview/hittest%28_%3A%29?language=objc)；
本机 `NSEvent.h:236` subtype 为 short、`:384–386` 允许 macOS mouse event subtype，
`:337` mouse-related location；正常转发使用 objc2 public `msg_send![super(...), sendEvent:]`。

`local_point` 是对应 view 的 **AppKit points、flipped 左上角原点**；`window_point`
仍为 NSEvent 原生 AppKit bottom-left window points。它们不是截图像素或 Quartz
desktop 坐标。自绘 canvas 的 down/drag/up 直接由 NSResponder 回调记录，不使用
本应用给自己 post/send 合成事件的捷径。mouseMoved 不读取 SDK 未保证有效的
`eventNumber`；pressure 只在 down/drag/up 读取。timestamp/flags 是原始事件值。

公开 API 依据：本机构建 SDK 的 `NSResponder.h`、`NSView.h`、`NSEvent.h`、
`NSTrackingArea.h`、`NSApplication.h`；特别是 `NSEvent.h:331–335` 的 accessor
有效类型范围，以及 `NSTrackingArea.h` 的 `ActiveAlways` 语义。后续验收以实际
receipts 和 goal oracle 为准，不以 API 名字、posted ack 或离线构建替代。

## 2026-10-09 标准文本／滚动接收靶场扩展

新增本 fixture 的非 secure、空白 `NSTextField`（`rwi-fixture-input`）与标准
`NSScrollView`（`rwi-fixture-scroll`），不把实现写进 RWI backend、不安装 bridge。
文本字段只有标准 delegate `controlTextDidChange:` 实际通知才增加 `text.changes`；
读取自身 `stringValue`，最多 1024 Unicode scalars，超限标记 `truncated`。
不读取 NSEvent 的 characters、全局按键、其他应用文本或密码。必须同时用独立
AX／真实截图核对消费的文本；可用末尾字符删除验证 key 的效果，而非派发 ACK。

滚动由普通 NSWindow／NSScrollView super 路径消费。仅本窗口 `NSEventTypeScrollWheel`
在 super 返回之后记录 delta 和 public NSClipView `bounds.origin`。fixture 不调用
`scrollToPoint:`、不直接更改 clip origin，不根据请求的 delta 伪造消费量；收到 delta
但 clip 不变属于无目标效果。`scroll.events` 与 `clip_origin` 分开，初始／每次自身
环境采样也读 clip。当前原生拖拽区缩窄至左侧 360×206 points，右侧为滚动列表；
之前真实拖拽证据继续绑定当次旧 binary hash，不能把旧坐标复用于新版。

公开依据：Apple [controlTextDidChange](https://developer.apple.com/documentation/appkit/nscontroltexteditingdelegate/controltextdidchange%28_%3A%29)、
[NSScrollView.documentView](https://developer.apple.com/documentation/appkit/nsscrollview/documentview)、
[NSScrollView.contentView](https://developer.apple.com/documentation/appkit/nsscrollview/contentview)。
实机验收仍只允许明确分配的 own Receiver／虚拟屏，不改其他应用或文档。

## 2026-10-10 covered-control 验收模式

显式追加 `--cover-controls` 才创建第二个 own borderless window；无该参数时保留
单窗口行为。参数不授予新权限，重复或没有 `--open-window` 等完整分配就拒绝。
cover 在同一个指定 NSScreen 上盖住 harmless Safe count 按钮，不 makeKey、不激活
App、不搬到主屏。创建前核对完全容纳，orderWindow 后独立 readback 其 frame、screen、
key=false；不符即关闭 own 窗口并退出，不回退。只保留自身 window 的生命周期。

`cover_window_number/frame/visible/display_id` 来自 public own-window readback。
`cover_above_primary` 通过 public `NSApplication.orderedWindows` 中两个自有对象的
实际索引采样；任一未找到则为 null，不把创建时的 order 请求当作当前顺序。
关闭 primary Receiver 或到期时只关闭 retain 的 own cover，再核对 isVisible；失败
保留 output_error，不把 public close 请求伪造为关闭完成。cover 没有 delegate，
避免重入 primary windowWillClose 或留下窗口；run-loop cleanup 再执行同一路径。

用 loaded RWI 按 exact PID/window 同时发现这两个窗口。分别 snapshot primary 和
cover：前者 SCK 精确窗口图可能包含被 cover 盖住的控件，这是 capture 与 input
visibility 的不同事实。普通 pointer click 的拒绝与独立 explicit AXPress 的目标
效果分开验收，不能把 semantic action 冒充 covered pointer 支持，更不能自动fallback。
此模式不测试 offscreen/otherSpace/hidden/minimized、用户physical keyboard归属或
第三方应用兼容；这些矩阵项继续保留，不因一个 covered case 就全通过。

## 可选：无输入的自身环境定时采样

显式追加 `--environment-sample-ms 100`，范围严格为 **25..2000 ms**。默认不开，
没有 `--open-window` 和完整分配参数不能使用，重复/缺值/越界拒绝。
只在已验证分配的自身窗口进入 running 后，主线程 common modes 启动 bounded
重复 timer。它只刷新既有 public AppKit active/key、own-window order/display/frame、
NSWorkspace frontmost PID 和 own scroll readback；没有截图、输入、全局 monitor、
自动 activation 或其他应用内容读取。

`environment_sampled_unix_ms` / `environment_probe_ticks` 是此 observer 的采样证据，
不是连续状态/物理用户输入证明。未启用时为 null/0。避免把 startup 或早先 callback
写出的 flags 当成当前状态；AXFocusedApplication 与 NSWorkspace frontmost PID 也
应分别核对，不能因其中一个 tuple 改变便推导另一个状态。

close/error cleanup 与 lifetime timer 一起 invalidate/release 此 timer；closed 后不再
增加 tick。nested callback 正在借用自身 UI 时会跳过该采样，不伪造数据。JSON fsync、
UI summary 更新和定时采样有观察者开销，这个模式不用于声称无探针性能优势。

`environment_history` 单独保留初次采样及 active/key/frontmost tuple 的变化，
最多 128 条；相同状态的后续 tick 不重复写入，淘汰数量显式记录。
每条包含原始 Unix 毫秒、采样 tick 和当时的输入 sequence，便于核对变化发生在
拖拽之前、期间还是之后。它不增加输入计数、不激活应用、不监控全局事件；
未知原生值保持 null，`physical_human_origin_attested=false`。
初次采样不是一次实际切窗；采样间隔内或重入时跳过的变化仍可能遗漏。
timer 先读取自身原生状态，再记历史并持久化，不再先记旧值、随后覆盖当前值。

### Explicit regular-app foreground test

`--regular-app` opts this allocated, own fixture into AppKit Regular policy
(Dock/Cmd+Tab eligible). Without it the existing Accessory behavior remains.
The flag still requires the full `--open-window`/run/state/display allocation;
duplicates reject before GUI setup. The setter result and public native policy
are checked. No `activate`, another-app input, global keyboard hook or implicit
main-display placement is added. This is test support, not a product startup
dependency or proof that a real user switched to it.

`application_identity` records the public startup activation policy and own
`NSRunningApplication.bundleIdentifier/localizedName`; an unbundled executable
may have a null bundle identifier. For a human Cmd+Tab acceptance run, package the
frozen fixture binary in a disposable, uniquely named `.app` with an Info.plist
and independently check these fields. A policy setter or bundle name alone is
not proof of the target becoming frontmost. Retain contradictory human/native
observations rather than treating a successful drag as a foreground takeover.
An already-Regular bundled app can return false from a no-op policy setter.
The fixture retains before/request-ack/after readbacks and accepts that no-op
only when both native states equal the requested supported policy; an
unacknowledged change or mismatched/unknown final policy still fails closed.
See Apple's [Regular activation policy](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum/regular)
and [running application identity](https://developer.apple.com/documentation/appkit/nsrunningapplication/bundleidentifier).

## Explicit foreground-transition actor

`--self-foreground-after-ms 100..60000` is off by default and requires an explicitly allocated
`--open-window`, `--regular-app`, run/state/display plus a shorter delay than the own lifetime.
After validated placement it installs one common-mode timer. The callback records a request,
then invokes public `NSApplication.activate` / `NSWindow.makeKeyAndOrderFront` on **only its own**
retained app/window. It does not post keyboard/mouse input, inspect/control another app or act
as an RWI input bridge. There is no implicit fallback and a request is not a transition proof.

Use fresh native foreground and recorded MouseEvent AppKit/key/frontmost states to test an
external lifecycle change during one RWI drag. Label this as a fixture-induced transition,
not physical-user Command+Tab. Close/expiry invalidate the timer; ordinary fixtures never activate.
[Public activation API](https://developer.apple.com/documentation/appkit/nsapplication/activate%28%29).

Native15/16 recorded an explicit modern request but no independently observed system foreground
change. Apple describes `activate` as cooperative. For an explicitly allocated takeover scenario
only, `--legacy-self-foreground` additionally opts into the public but deprecated
`activateIgnoringOtherApps(true)` on this fixture's **own** app. It requires the existing
own timer and Regular allocation, is recorded as a separate method, and never becomes a
production RWI route or an automatic fallback. Read back foreground; do not assume acceptance.
[Deprecated activation API](https://developer.apple.com/documentation/appkit/nsapplication/activate%28ignoringotherapps%3A%29).

## Explicit own document-modal controls

`--modal-controls` is off by default and requires the complete own-window allocation. It adds
Open own sheet / Modal safe count / Close sheet controls. They use public `NSWindow.beginSheet`
and `endSheet` on a retained **own** NSPanel, with a correctly typed nil Block completion handler.
No CGEvent/NSEvent input bridge, OS permission prompt, foreign app access or explicit global activation call is added. Native22–24 observed beginSheet changing own AppKit active/key state; it is not an active/key-preserving background operation.
The native sheet number, visible/key/frame/display and exact `sheetParent` relation are sampled
separately from the NSButton action counters. A request/counter is not a modal/native-input goal.
`isSheet` means ever used as a sheet, not currently visible; check visible and attached-to-parent.
Own lifetime cleanup ends/closes the sheet before the parent and cover. This fixture supports
distinct native identity, parent-blocking, exact popup actions and per-source Live acceptance;
it is not proof of remote system panels, all modal frameworks or automatic owner inference.


### 首次真实 canvas down 后的自主前台 actor

显式 `--self-foreground-on-first-down-after-ms 100..5000` 与 launch timer 互斥，
仍需完整 own-window 分配、Regular policy，且延迟短于 lifetime。默认 null/off。
仅在自身 running 正常窗口接收到第一条有效的 own-window/canvas/left-down
NSResponder 回调时 arm 一次；hover、错窗、未知 policy、非有限坐标、关闭后或
第二条 down 均不能重新 arm。没有事件发送、他应用控制或自动 fallback。

`self_foreground_first_down_sequence/unix_ms/event_timestamp_seconds` 记录实际
接收 cause，不增加派发/消费计数、不伪造前台成功；foreground 请求仍由现有
common-mode timer callback 一次执行，原生前台及 balanced down/move/up 独立回读。
close/expiry 撤销 timer；输入之前等待任意模型时间都不触发。explicit legacy
method 也只作为另一个已选择的 fixture-only opt-in，不在失败后自动切换。
此 actor 避免聊天/tool 排队导致“提前切窗”，不冒充物理人类或 Command+Tab 来源。

辅助 own 第二应用可显式加 `--right-side`：仍只在所分配 NSScreen 内创建，保留原尺寸与
24pt 边距，把 x 放到右侧；不足则拒绝，无 main-display fallback。默认原左侧不变。
这个 fixture-only setup 用来避免前台 actor 覆盖后台目标，并非新增产品窗口移动工具。
实际可见/覆盖关系仍需独立 native discovery 与 hit/readback，不从几何公式推目标成功。

## 显式 AppKit 系统浮动面板（接收测试，不是后台实现）

`--system-panel-controls` 默认关闭，仍需完整 own-window 分配；重复/缺少分配拒绝。
增加 Colors / Fonts 原生按钮，分别通过 public NSColorPanel.sharedColorPanel 与
NSFontManager.orderFrontFontPanel 打开当前测试应用自己的系统面板。
不替换 framework panel delegate、不发送事件/读取其他软件、不伪造 owner graph，
不预先搬动系统面板或把 own main-screen 放置当产品能力。

`system_panels` 将 open request 与原生 windowNumber、visible/key/frame/screen/isSheet/
sheetParent==own-parent 分开记录。未创建保持 null；float 不等于document-modal，也不能
从同PID推父子关系。panel可能按系统行为出现在别屏/失焦隐藏：实际 loaded RWI须fresh
discover/bind/readback，必要时明确move验证。Close/expiry仅结束本process保留的shared panels。
[Color shared API](https://developer.apple.com/documentation/appkit/nscolorpanel/shared?language=objc)；
[Font panel API](https://developer.apple.com/documentation/appkit/nsfontmanager/orderfrontfontpanel%28_%3A%29?language=objc)。

## 明确分配的 AXWindows 慢 provider 对照

--ax-windows-delay-ms 75..150 仅完整分配并使用 --regular-app 的自身fixture可启用，默认0；
只延迟自身NSApplication公开accessibilityWindows getter，仍返回superclass真实窗口数组。
不伪造AXError、不发送事件、不读取或更改其他app，也不进入生产Agent依赖/配置。
ax_provider_probe记录实际getter调用/延迟和耗时，不宣称调用者、人类来源或AX失败；
消息错误及恢复须由loaded RWI真实结果与独立scope/像素/接收oracle证明。
CLI请求本身不是provider override被系统采用的证据；getter_calls必须非零。

## Explicit two-fixture system Command+Tab actor

Ordinary fixtures remain passive receivers. To verify an OS-level shortcut **during** an already-received background drag, opt in to `--command-tab-on-first-down-after-ms 1000`, with exact `--command-tab-peer-pid`, `--command-tab-peer-window`, and `--command-tab-peer-bundle` from a second allocated RWI receiver. A regular app, 25–200ms environment sampling, delay100–3000ms and a longer bounded lifetime are mandatory. Do not combine with either own-activation actor.

Root must allocate both fixtures and positively establish peer foreground and switcher order through the loaded RWI tools. The actor pins the peer's same-UID kernel proc_pidinfo birth plus NSRunningApplication bundle/liveness and exact on-screen CG window ID/bounds/title/observed layer. WindowServer publication can lag the fixture's running record: pin process identity immediately, but positively bind the exact CG surface before any post; missing or changed identity never permits input. Existing post/listen authorization, foreground and recent HID activity are checked before each down. Held-resource preflight runs before the first down, like production `native/hotkey.rs`: HID keys/buttons and momentary flags from either table block; combined-only non-modifier residue is recorded without being misclassified as physical input. Once our Command-down is posted it can appear in the HID table too, so do not repeat the held-resource preflight against our own chord. Per-down recency/foreground/identity guards and paired cleanup remain active; this is not continuous physical-user attribution.

Allocate one private-source Command-down/Tab-down/Tab-up/Command-up chord before posting. Only its own paired ups are attempted on refusal or unwinding. It never requests permission, retries the chord, activates a replacement app, captures other apps or adds a production backend bridge. Native refusals are terminal; a fresh separately allocated case is not permission to replay the refused gesture.

The first actual own-background canvas down arms one common-mode timer. A released gesture, wrong timer, absent peer, changed fingerprint, active/unknown input or missing authorization sends no further down. `state.command_tab` records cause/time, API call counts, private source ID, permissions, guard HID ages and sampled foreground; none are input acknowledgement or physical-user attestation. A returned chord is not proof the OS switched: independently intersect native environment/drag records and screenshots.

`--check-only` requires the same explicit allocation/configuration arguments but returns a JSON configuration report without creating the state directory, loading AppKit, opening a window or posting input. Native binding remains unverified in that report. All default/no-argument behavior is preserved.

Primary APIs: [Apple private source state](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid), [existing post-event preflight](https://developer.apple.com/documentation/coregraphics/cgpreflightposteventaccess()), [CGEventPost](https://developer.apple.com/documentation/coregraphics/cgevent/post(tap:)).
