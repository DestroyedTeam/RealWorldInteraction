# RWI 后台原规划恢复、真实接收验收与前台切窗候选 · 2026-10-09

## 结论与范围

没有用这两天的局部 bug 代替原计划：原 F01–F16 已全部落回
[当前机器可读状态](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/product/macos-background-acceptance-status.json)。
这是持续实现中的阶段验收，不是“真后台所有应用／所有窗口状态完成”。

本轮 GUI 操作全部由 **当前 loaded RWI 的 typed tools** 发出，未写临时操作 JS。
仅操作已授权的 own Receiver、BetterDisplay 虚拟屏 UUID
`13E4E9B0-47F2-4043-9CDC-878D5441BD41`，本次 discovery 的 display ID4。
不操作游戏、飞书、Chrome、用户已有文档，不申请／重置 TCC、不改配对和签名材料。
当前生产 Agent 始终 PID73141 / `de7cc66c-8a96-4730-8f4d-a569b711e483`；权限4项仍 true。
当前 MCP仍39 tools；新候选实际进程只读 `tools/list` **40 tools**，包含
`computer.focus_window`，不是已加载 Host 工具或前台 native goal 证明。

本轮窗口状态是 **inactive、on-screen、虚拟屏**。接收 AppKit `active/key` 可为 true
但 OS `frontmost_pid` 不为 receiver；这是两类身份，记录分开。未测 covered/offscreen/
otherSpace/hidden/minimized，未连续观测用户真实键盘归属／恢复，不外推“完全不侵扰”。

## 新实现与选择性整理

- 标准非 secure `NSTextField` + `controlTextDidChange:` 独立 consumed-value oracle。
  不读取 NSEvent characters、密码或他应用文本，最多1024 Unicode scalars、有截断标记。
- 标准 `NSScrollView` 正常消费后读 public clip bounds；收到 delta 与视口变化分开。
  fixture 不用 scrollToPoint/SetValue 自造目标效果。新视口是标准 AppKit 控件，不是 backend。
- 新 **显式前台** `computer.focus_window`：窗口级≤10秒 ref、同 desktop lease、稳定 request ID、
  完整 expected_foreground。AXRaise 一次，必要时 AXFrontmost 一次；每次边界重验 exact target/
  process/session/foreground + HID yield。无 keyboard/warp/unhide/restore/自动重试，≤500ms采样。
  使用 AX 与 listen 权限，不错误地把 CGEvent post 权限绑在 AX 前台动作上。
- 独立 foreground/engine 与 tests 模块，未搬动既有后台实现。持久预约、可观测 side-effect
  counter、uncertain/no-replay、同桌面所有 alias 失效；模拟测试不冒充 native receiver 验收。
- Public API 边界见 [AXRaise](https://developer.apple.com/documentation/applicationservices/kaxraiseaction)、
  [AXFrontmost](https://developer.apple.com/documentation/applicationservices/kaxfrontmostattribute)、
  [text change delegate](https://developer.apple.com/documentation/appkit/nscontroltexteditingdelegate/controltextdidchange%28_%3A%29)、
  [scroll documentView](https://developer.apple.com/documentation/appkit/nsscrollview/documentview)。
  API可用／返回不代表目标成功，下面以真实 oracle 为准。

## 顺序实机结果（每张为工具返回的原生 PNG，无修改）

|步骤|结果／问题|证据|
|---|---|---|
|Receiver01初始|PID13567 / cgwindow21126，drag counters0；PNG1320×1104|03、07|
|1600ms直线|down1、dragged25、up1、completed1；无 orphan，所有原始回调 OS frontmost≠13567|05–07|
|1200ms四顶点折线|增量down1、dragged21、up1；总completed2，拐点和终点真实呈现|08–14|
|越界折返|`capture_fringe_not_grounded` / rejected_before_dispatch；sequence50→50、App probe52→52、无新down|15–19|
|hover|App/Window收到type5，window point[260,372]、hit hover_zone；view/enter/exit/tooltip仍0，**未通过**|20–22|
|Receiver01关闭|自身600000ms lifetime timer真实closed+exit0；随后AX close尝试已遇process_identity_unknown、无派发，不能写成AX关闭成功|24–26、RECEIVER|
|Receiver02标准控件|PID25177 / cgwindow21257；空文本、clip[0,0]，新binary独立hash|27–32|
|后台Unicode|`RWI-bg-验收-🙂Z`，AX value、截图和delegate oracle一致；changes1、未截断、OS frontmost≠25177|33–35|
|后台backspace|只去掉Z，值`RWI-bg-验收-🙂`；changes2，OS前台仍其他PID|36–38|
|原生scroll -150|实际clip y0→150、native delta[0,-150]、events1；截图首行00→05|39–41|
|AX分页（问题）|advertised AXScrollUpByPage 返回-25205、dispatch uncertain；随后图像与独立clip采样却150→344（不是向上目标）。保存原操作查询，不重放、无fallback|42–44、47–48|
|独立AXPress|Safe count0→1；目标文本没变。下一自身回调采样clip344，非原子滚动ACK|45–47|
|Receiver02关闭|fresh AX close后真实closed、run exit0；detach后当前context只剩desktop，无 held drag|49–52、RECEIVER02|

派发 receipt均保持 `goal_verified=false`；以上目标只由独立 receiver/AX/截图读回判断。
AX分页失败不能根据 action名字、错误名字或位移就判定适配成功；本轮没有诊断出它一定
属于 RWI transport、AppKit provider 或 fixture flipped-document 原因，后续按对照用例定位。

### 01 初始拖拽区
![01 初始拖拽区](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/01-drag-baseline.png)

### 02 长拖拽完整路径
![02 长拖拽完整路径](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/02-long-drag-complete.png)

### 03 折线路径
![03 折线路径](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/03-bent-drag-complete.png)

### 04 Hover无view效果
![04 Hover无view效果](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/04-hover-no-view-effect.png)

### 05 标准原生控件初始
![05 标准原生控件初始](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/05-standard-widgets-baseline.png)

### 06 Unicode文本实际消费
![06 Unicode文本实际消费](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/06-unicode-background-text.png)

### 07 Backspace结果
![07 Backspace结果](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/07-background-backspace.png)

### 08 原生滚动结果
![08 原生滚动结果](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/08-native-scroll-150.png)

### 09 AX分页错误却有位移
![09 AX分页错误却有位移](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/09-uncertain-AX-scroll-effect.png)

### 10 独立AXPress计数
![10 独立AXPress计数](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-09-background-plan-return/10-safe-AX-count.png)

## 原完整计划：不能遗漏的剩余项

|原编号|当前边界／下一项|
|---|---|
|F01 构造|factory/roundtrip已实现；与transport和goal分开|
|F02 click/key|本轮native标准控件通过；其他framework、全局shortcut及状态矩阵待补|
|F03 SkyLight|显式SPI实验route未实现，不默认替换|
|F04 drag|同window完整路径和边界通过；covered/取消/介入/其他client继续|
|F05 hover|App/Window收到、view/tooltip未通过；继续修路由/接收，不宣判不能做|
|F06 raw/relative|按GameHub/Wine/客户端继续，普通click≠raw完整兼容|
|F07 receiver/恢复|本轮text/key消费和OS前台分离已实证；physical-input连续性／恢复、前台新tool待部署验收|
|F08 capture|本轮inactive窗口真截图；covered/offscreen专门matrix待补|
|F09 window-local|Retina有证据；单display容纳限制仍在，不直接删identity/hit/lease|
|F10 hidden/minimized|泛用路线未实现，paused/stale不能冒充fresh；不自动unhide|
|F11 AX语义|AXPress通过；分页provider错误／反向效果保留，需独立适配验证|
|F12 多窗口/popup/Live|独立targets已有；授权归属图／不同PID系统面板／聚合Live继续|
|F13 native DnD|未实现；与本轮canvas drag完全分开，先双own-app/MIME/destination|
|F14 browser route|显式页面协议尚未整合；native工具栏／OS边界保留|
|F15 cleanup|本轮timer/AX退出0与detach通过；真实中途取消／资源丢失及压力继续|
|F16性能/输出|本轮实际耗时/原图已保留；分层p50/p95、资源压力和跨模型readability继续|

## 可复核耗时与离线回归

这是**单次 adapter wall time**，包含guards/native/probe JSON IO，不是CPU或语言优势benchmark：
Unicode101ms、backspace101ms、scroll291ms，直线1600ms路径用1901ms，hover253ms。
采样/未原子／原图变换信息、按钮及坐标、uncertainty没有从输出删除。

- `cargo fmt --all -- --check`：exit0。
- `cargo test --workspace --locked --offline`：**662passed / 11ignored / 41suites**，exit0；ignored不计native验收。
- all-targets/all-features locked offline Clippy `-D warnings`：exit0。
- `git diff --check`、release Agent+MCP build：exit0。
- fixture example独立26项纯模型测试：exit0；不计入 workspace数量，不能替代真实GUI。
- 新候选真实MCP进程20ms握手/list，40tools、focus必填tuple/ref/lease/request：exit0；没有tools/call。
- 同一 `foreground_focus_catalog` 输入：**BASELINE101（缺tool）→MODIFIED0（1pass）→ROLLBACK101（相同缺tool）→REAPPLIED0**。
  portable rollback在独立 evaluator copy恢复本轮15个before hash和新module缺席，再重新应用；产品root保持modified。
- 中途实际 fmt E0753、JSON macro recursion、widget E0283已保存，修复后重跑；CLI --help返回1是usage，改用明确profile启动后list退出0。

## 同一四角色与交付状态

- MODIFIED_FILE：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- DIFF_FILE：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- VERIFICATION：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- ROLLBACK：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`

当前候选二进制与SHA见 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/background-plan-return/CANDIDATE_ARTIFACTS.json`：保留release输出，**未用本轮候选重启共享生产Agent、未替换MCP、未签名部署**。
前台tool的新schema需协调更新并重载后，通过loaded工具在own窗口再做native acceptance。
不能把40tools只读inventory或662项离线结果当成前台切窗实机成功。

后续顺序：部署/重载前台候选并验收 → F05 hover接收 → F07连续性 + F08/F09窗口状态矩阵 →
F12窗口族/Live → raw、hidden、DnD、browser独立路线 → 跨路线cleanup/performance。
完整规划仍在，并未因本轮完成一个切窗tool／拖拽case就关闭整个任务。
