# RWI macOS 真后台：扩展调查、缺口与实施结论

日期：2026-10-08；阶段：**研究完成，未部署新的输入实现，未实机复现被审项目**。

## 1. 先回答“是否全面”

上一版约7个项目、39条来源，只能支持机制级阶段性判断，**不足以宣称已完成生态调查或通用兼容性结论**。用户指出的问题成立。本轮增加维护、真实实现、许可、缺口与实现难度审查，不能只筛“刚好能直接用”的库。

本轮实际审查 **50个去重的公开GitHub仓库**：17个直接相关、33个周边/对照。这里不把客户端框架、窗口管理器、视觉识别器、文档发行库冒充50套后台驱动，更不把公开可读等同于可直接复用的开源实现。每仓库核验metadata、固定默认分支commit、实际README/设计资料及相关源码；bgclick只有研究文档、Qt没有根README、BetterDisplay没有公开app原生实现等例外单列。

**技术正文150篇**，不是150篇完整长论文，也不是搜索命中数：

- 社区/维护讨论101篇：46个Issue、21个PR、34个Stack Overflow问答；未解提问、建议和作者报告分别标强度，不能据它们直接判OS不支持。
- 原作者技术文章9篇。
- 官方指南、会话、设计说明、其他技术文档40篇。
- 另保留49个API参考页核对精确契约，**不计入150篇门槛**。README、源码、metadata、license、重复镜像/版本、空正文、404、只返回JS壳和链接/checklist型弱PR均不拿来凑技术篇数。

目录中保留每项的canonical identity、正文SHA、日期、阅读深度、具体发现与可支持的范围。去重后的全部参考记录数含metadata等，不叫“论文篇数”。计数可由[来源索引](2026-10-08-macos-background-literature.json)复算；仓库详情见[50项矩阵](2026-10-08-macos-background-repositories.json)，能力难度见[功能矩阵](2026-10-08-macos-background-feature-matrix.json)。

这是**有范围声明的系统性工程调查**，仍不是穷尽GitHub/论坛、逐项目完整安全审计或本机所有应用成功证明。增加篇数降低漏掉路线的风险；是否可用最终仍须接收/目标效果/用户输入连续性的原生实验。

## 2. 方法、证据等级与评分

### 2.1 检索与筛选

分成原生输入/AX、自动化与验证、客户端/窗口/capture/remote/display三个仓库组；另检索Apple技术契约、原作者复现文章与社区反例。采用已知项目→实际代码→其Issue/PR→作者文档的链路，兼顾正例、失败、弃用和“只做其中一层”的项目。没有运行安装脚本、被审程序、屏幕采集、输入、权限请求或服务部署。

不以stars选择可行结论，不由`updated_at`猜代码更新，不由旧HEAD猜弃置，不由closed猜merged；表中只给核验过的默认分支HEAD日期，不把它当native文件最后修改日；未逐文件查询history的日期保持unknown。release是仓库自己的release而非整个软件所有发行渠道。GitHub star数单列为当日热度，**不是专业度、RWI成功率或难度星级**。

### 2.2 强度与专业适用性

- **A：官方API/原始源码**，可证明所读版本的契约/实现；源码存在不证明实机接受。
- **B：维护者或原作者的技术实验**，可支持明确环境的观察，不当作普适API承诺。
- **C：社区报告/提案/未解问题**，用来找变量和负例；答案点赞、PR合入都不代替本机验收。
- **弱或不计数**：只有声明、标题、链接、JS壳、营销承诺、镜像重复、未读取依赖内部。保留有价值的symbol/维护线索，不作为通用行为证明。

“是否专业”改成可检查的采用门槛：精确作用域、真实实现与宣称一致、release/取消状态、窗口与focus恢复、ABI/版本保护、readback证据、测试/实际兼容矩阵、许可来源。**缺某项表示RWI需补齐，不能直接给作者贴“不专业”标签**；稳定小库也可能长期无需改代码。

### 2.3 难度与复杂度

每仓库分数针对“将列出的有用部分补成RWI组件”，不是给整个项目打分：

| 补齐难度 | 含义 |
|---|---|
| ★ | 单个局部适配/孤立检查 |
| ★★ | 公开FFI、对象生命周期或小状态机 |
| ★★★ | 跨层事件状态、typed接口、错误/取消和测试 |
| ★★★★ | 多窗口/客户端差异/window-local authority及较大验收矩阵 |
| ★★★★★ | SPI ABI/认证/focus恢复、raw input、DnD、渲染暂停等多重不确定性 |

复杂度另描述状态数量、耦合与版本矩阵。星级是**工程估计**，不是工期/人日承诺；public API高支持也不意味着实施简单，SPI可找到符号也不意味着Apple公开支持。

## 3. 必须单列的项目：已实现、部分实现、参考与维护状态

### 3.1 有价值但不完整的后台driver

**trycua/cua**值得作为优先技术对照，当前Rust有SkyLight、window stamping、no-raise/恢复、AX enablement等实际实现；但窗口后台`drag`仍拒绝，window-scope `move_cursor`只是overlay移动。其4/23博客与现源码不是同一阶段；remote observer SPI在被审Rust中未找到同等实现，现AX用其他enablement策略。**不能把博客“任何app后台”扩成已验证能力**。[固定mouse源码](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/rust/crates/platform-macos/src/input/mouse.rs)、[drag](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/rust/crates/platform-macos/src/tools/drag.rs)、[move_cursor](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/rust/crates/platform-macos/src/tools/move_cursor.rs)。

当前[PR4864](https://github.com/trycua/cua/pull/4864)已于2026-10-08合入：作者VM中LibreOffice PID pointer被丢、AX cell press无业务作用却报成功，后续输入进入旧cell。修正为特定背景pixel动作明确拒绝，toolbar advertised AXPress仍保留。这是反对“ACK=成功”的实例，**不是LibreOffice所有后台方式不可能的证明**。[Issue2688](https://github.com/trycua/cua/issues/2688)仍记录x86 macOS14私有认证路径崩溃，显示ABI隔离/探测的产品要求。

**重要区分：开源trycua/cua不是Codex内置Computer Use的源码。**本轮不能据前者的实现推断后者内部算法、训练偏好或同状态性能。

### 3.2 只有方法/历史header，不能当完整SDK

- **bgclick-rev-skill**：一个逆向方法文档和样例思路，不是完整可安装executor；未见LICENSE。公开NSEvent工厂之外含private setter/非标准subtype，未提供通用hover、完整drag及跨版本验收。可继承字段/坐标/独立fixture思路，由RWI自行设计候选。[固定文档](https://github.com/Lakr233/bgclick-rev-skill/blob/85dc2c49ce1bf3413654430df99d91c15687a51f/bgclick-rev-skill.md)
- **CGSInternal**：2016私有header集合、未见LICENSE；用于术语和ABI差分研究，不作为macOS27稳定API或可交付输入器。[仓库](https://github.com/NUIKit/CGSInternal)
- **BetterDisplay**：公开库提供发行/文档/Issues，**未公开app native driver实现**。BetterDisplayCLI有Swift notification客户端，不是虚拟display driver。可以做具名设备的集成，不能因它有开源CLI就称app内部可复用。[BetterDisplay](https://github.com/waydabber/BetterDisplay)、[CLI](https://github.com/waydabber/betterdisplaycli)

### 3.3 维护、归档、继任与公共版本落后

- **input-leap**：GitHub API实际`archived=true`，归档参考单列；不是推荐的持续维护依赖。[仓库](https://github.com/input-leap/input-leap)
- **sikuli/sikuli**：README明确旧X-1.0rc3不再支持；**oculix-org/SikuliX1**明确历史mirror/继任关系。不能把mirror最近commit当老runtime重新获得全面维护。[旧库](https://github.com/sikuli/sikuli)、[历史mirror](https://github.com/oculix-org/SikuliX1)
- **Barrier、AXSwift、DFAXUIElement、OpenACI、pyatom、JNativeHook**等默认分支较久未更新，但没有据此擅称停止维护。保留“长期无HEAD变化/维护未知”，现代OS支持另验。[Barrier](https://github.com/debauchee/barrier)、[AXSwift](https://github.com/tmandry/AXSwift)
- **PyAutoGUI**：虽然HEAD较旧，维护者2026在[Issue870](https://github.com/asweigart/pyautogui/issues/870)明确没有弃置。它的全局HID/坐标边界仍存在，维护活跃与后台适用性是两个维度。
- **nut.js**：公开源码/公共npm与私有成品渠道分开；维护者[文章](https://nutjs.dev/blog/i-give-up)描述distribution策略，不应误读成停止开发。公开可读drag的异常清理缺口须看所读版本，不假定私有包同样行为。

### 3.4 许可/包装器/识别器不是后台能力

Screenpipe当前实际LICENSE为source-available Commercial License；OmniParser LICENSE与README badge不一致，code/model/dependency许可需分别核验。两者本轮作为机制/接口参考，**未直接vendor**。这些是当前仓库材料事实，不是最终商业许可法律意见。[Screenpipe LICENSE](https://github.com/screenpipe/screenpipe/blob/main/LICENSE.md)、[OmniParser LICENSE](https://github.com/microsoft/OmniParser/blob/master/LICENSE)、[许可讨论365](https://github.com/microsoft/OmniParser/issues/365)

OmniParser是图像解析，不提供native actuator；Node权限包装器只能check/request，不授予用户批准；Python/Ruby/Java/Node框架不成为RWI Rust backend。Browser DOM/CDP的无前台page操作不是Mac任意window输入；远控/KVM是seat控制；虚拟screen/workspace不创造另一个AppKit focus universe。

## 4. 本轮让实施判断改变的具体发现

### 4.1 不应押注单一NSEvent或单一SPI

NSEvent工厂是公开、可验证的构造路线，但transport、WindowServer路由、AppKit view分发、客户端poll/raw、业务effect仍是不同层。RWI应具备**可选择、可报告能力的原生route**，不能偷偷fallback到activate/global HID，也不能让一个app的失败禁掉别的窗口/动作。[NSEvent](https://developer.apple.com/documentation/appkit/nsevent)、[Cua候选设计（Proposed）](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/docs/macos-background-input-v1-plan.md)

### 4.2 事件编号映射和drag状态要完整验证

Apple Quartz字段明确匹配down/up应具有相同mouse-button event number；NSEvent文档以counter描述eventNumber，但本轮未做bridge roundtrip，不能据文字差异断言底层字段/计数彼此独立，需实测factory→CGEvent的映射和配对关系。pressure、delta和source-owned独立状态也不是仅type名称就保证一致。Enigo/RobotJS/pynput有可参考的held button/recent movement/dragged/delta实现；当前RWI每事件新建Private source是需比较的变量，**不是仅凭源码就确认的根因**。[mouseEventNumber](https://developer.apple.com/documentation/coregraphics/cgeventfield/mouseeventnumber)、[Enigo固定源码](https://github.com/enigo-rs/enigo/blob/a88d9b7e2cec7043ab5f03e754500a091ea928d1/src/macos/macos_impl.rs)、[RobotJS159](https://github.com/octalmage/robotjs/issues/159)

旧游戏实验中单个无down/up的dragged只尝试触发hover，不是完整drag验收；这个已有证据不会被新数量覆盖。完整pointer drag与原生DnD仍要独立测试。

### 4.3 hover不是单一“移到坐标”效果

Qt有accepted-down、inactive callbacks过滤；Tao/Winit有越界＋全局button判断；GLFW的hover getter与move callback不同；SDL部分版本会重算全局光标。Wine主线主动安装inactive tracking，说明不能把后台move笼统判不可能；但所装GameHub/CrossOver版本还未确认。root/API/overlay正确不等于第三方client最终采用正确位置。[Qt源码](https://github.com/qt/qtbase/blob/8295380702b4902256341e15ee58dd809f65fd5e/src/plugins/platforms/cocoa/qnsview_mouse.mm)、[Wine主线源码](https://github.com/wine-mirror/wine/blob/63f62f7cd6964f544b08ae191d6a3d08fc8e5925/dlls/winemac.drv/cocoa_window.m)、[GLFW](https://github.com/glfw/glfw/blob/92dcf4ce74f2e2554a98fea09be7c705c17daa5a/src/cocoa_window.m)、[SDL](https://github.com/libsdl-org/SDL/blob/e4af6f8e283812aa693eb2c70c91249a9e1ce92c/src/video/cocoa/SDL_cocoawindow.m)、[Tao](https://github.com/tauri-apps/tao/blob/3fcda65f3f5a7c9ad164a622d2b98e4b5e313554/src/platform_impl/macos/view.rs)、[Winit](https://github.com/rust-windowing/winit/blob/01634067402db15d26a31bfd199f5c69a989813d/winit-appkit/src/view.rs)

hover高亮、tooltip、entered/exited、cursorUpdate、relative/raw必须分别有oracle。未知trackingNumber/userData不能伪造。只监听local monitor也可能漏nested control/menu/window tracking；recorder须覆盖真实window/view callback。[local monitor](https://developer.apple.com/documentation/appkit/nsevent/addlocalmonitorforevents(matching:handler:))

### 4.4 “foreground PID没变”不够证明不干扰

no-raise/focus手法可能保持原app在NSWorkspace报告frontmost，却让key输入接收方改变。必须测用户前台测试控件的输入连续性与目标key/main/active/恢复。NSPanel自身nonactivating机制也有CPS key focus和WindowServer tag，不是调用者能给任意第三方window打开的普通属性。[Cua skylight源码](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs)、[原作者NSPanel实验](https://philz.blog/nspanel-nonactivating-style-mask-flag/)

### 4.5 offscreen capture有公开基础，minimized是另一回事

Apple WWDC22明确单window流在被遮挡/离屏/换display时仍包含完整内容，并独立于Space；minimized时暂停，恢复后继续。单window filter**不自动包含child/pop-up**。这使RWI的on-screen枚举和完整display mapping成为应单独补的自身约束，不应泛归“OS没能力”。窗口renderer是否新绘制与capture流fresh status仍分开确认。[官方会话](https://developer.apple.com/videos/play/wwdc2022/10155/)

不同硬件的stride/缩放与capture质量也需实际证据，不能因为一台Mac不复现就否定另一台；固定`width*bpp`不是普适rowstride。[维护者调查](https://nutjs.dev/blog/apple-silicon-screencapture-memory)、[续篇](https://nutjs.dev/blog/apple-silicon-screencapture)

## 5. 我们能否补齐开源库没有实现的部分？

**可以，而且这是RWI应做的工程工作。**库缺少某功能，不代表平台禁止；我们可以借鉴局部机制、重新用Rust实现状态机/transport、设计更强的观察与验收。反过来，前台能做也不保证所有客户端会消费任意背景event——它可能读global/raw状态或暂停渲染。该部分要转化为版本化实验与显式能力，而不是绝对否定或绝对承诺。

采用策略：

1. **直接保留/扩展RWI现有基础**：原生身份、AX、SCK、定向投递、lease、durable no-replay、release-only、结构化receipt。
2. **借鉴后原创补齐**：事件字段roundtrip、同gesture source/button/delta状态、typed Move/Hover、window-local authority、multiwindow/popup capture、goal verifier。
3. **隔离SPI候选**：SkyLight/认证/focus协议，要求OS/arch/ABI探测、null guard、异常隔离、恢复和明确拒绝；不隐藏风险，不导入平台FFI到MCP transport。
4. **显式限定其他通道**：browser route、AX语义、虚拟屏、合作app API各有scope；不会自动替换native。最小化/hidden/raw/DnD是独立复杂项，不从普通click/同window drag成功继承能力。
5. **没有采用的部分**：全局鼠标warp/抢focus、无人批准的权限请求、未知安装脚本、私有包不可见实现、无许可代码直接复制。未知保持unknown而不是假成功。

## 6. 功能难度/API支持矩阵

下面表格由功能JSON生成；“公开”只支持表述范围，“SPI”没有公开稳定保证。星越多越难，不是可用性越高。具体来源ID可回溯索引。

| 功能 | 难度 | 复杂度 | OS/API支持范围 | RWI补齐重点 |
|---|---|---|---|---|
| F01 NSEvent构造/roundtrip | ★★ | medium | 公开构造API；不负责transport | windowNumber/base坐标、uptime、bridge null与字段读回；容易补构造层，不单独交付真后台 |
| F02 PID/window click与key | ★★★ | high | 公开CGEvent PID post＋部分SPI stamping | 接收窗口/firstResponder、flags/timestamp及goal读回；void post不算消费ACK |
| F03 SkyLight候选transport | ★★★★★ | very_high | 私有SPI，无Apple公开稳定ABI契约 | dlsym/null、private auth、no-raise恢复、一次派发及错误隔离；可受控实现，不全量默认替换 |
| F04 完整drag/source状态 | ★★★ | high | 公开event/state/field；目标接受有条件 | down/dragged/up、Quartz配对eventNumber/pressure/delta、source lifetime、release-only；优先补并禁止重发down |
| F05 typed move/hover/tooltip | ★★★★ | high | 公开app-side tracking；非通用跨进程效果保证 | active策略、nested loop、global poll、dwell/readback；应原创实现，不伪造tracking指针/把overlay当hover |
| F06 relative/raw游戏兼容 | ★★★★★ | very_high | 公开client raw API，无通用定向raw保证 | event-local/global-poll、delta/GCMouse/capture；按client补，未知诚实报告，不静默warp |
| F07 key-focus接收与恢复 | ★★★★★ | very_high | 公开状态＋SPI focus，frontmost PID不足 | 记录key recipient、physical-input连续性、restore；CPS panel与normal window不同 |
| F08 covered/offscreen capture | ★★★ | high | 公开SCK明确支持 | non-on-screen枚举、SC status/dirty/scale/stride；RWI枚举与input mapping缺口应补 |
| F09 window-local authority | ★★★★ | high | 公开geometry/capture，input效果有条件 | identity/epoch、pixel-point-base、frame/content/负origin；可改架构，不直接删mapping/hit/lease |
| F10 minimized/hidden目标 | ★★★★★ | very_high | minimized SCK pause已说明，renderer策略app依赖 | paused/stale不可作fresh proof，不能自动unhide伪装后台；semantic-only/合作app另定义 |
| F11 AX语义/hover-like动作 | ★★ | medium | 公开AX，provider支持/效果需验证 | Press/ShowAlternateUI/SetValue不是pointer；可能no-op须goal读回，优先补且不连带禁其他route |
| F12 多window/popup Live | ★★★★ | high | 公开catalog/AX/SCK，归属需证据 | single-window不带child/popup，OS dialog可不同PID；补聚合/重发现，不扩大secure-session授权 |
| F13 跨window/app原生DnD | ★★★★★ | very_high | 公开app-side session/pasteboard，不保证任意注入 | MIME/destination/capture/enter/move/leave/drop与多资源scope；独立长阶段先自有fixture，未提供不等于不能做 |
| F14 browser内容独立route | ★★★ | high | 公开browser协议，不是任意Mac input API | context lifecycle、buttons/dragover/incognito；可显式整合，native route保留，不静默global fallback |
| F15 hook/runloop/取消恢复 | ★★★ | high | 公开tap/runloop，权限和模式仍有约束 | common modes、callback retain、disabled/wake、cancel/up清理；迁移状态机而非global干扰 |
| F16 capture性能/画质/Live | ★★★ | medium | 公开SCK metadata；需独立hardware测量 | bytesPerRow、dirty/contentRect/scale、fresh status/source调度；可做性能工程，不凭语言/上游benchmark推快 |

## 7. 50项仓库矩阵

表中时间为固定HEAD UTC日，不是热度更新时间；许可与release完整字段见JSON。角色/已实现/可借鉴/缺口是静态审查，未宣称本轮部署或实机通过。

| #／仓库 | 角色与现行范围 | 已确认维护状态／HEAD日 | 可借鉴部分／缺口 | 难度／复杂度 | API类别 |
|---|---|---|---|---|---|
| 1. [Lakr233/bgclick-rev-skill](https://github.com/Lakr233/bgclick-rev-skill) | reference：公开mouse NSEvent factory→CGEvent→按PID投递的逆向方法和Swift样例设计；不是可安装执行库。 [原文](https://github.com/Lakr233/bgclick-rev-skill/blob/85dc2c49ce1bf3413654430df99d91c15687a51f/bgclick-rev-skill.md) | 未归档；以commit/owner证据，不作兼容认证；2026-04-18 | 可借：窗口号、全局/局部坐标双载荷、Command/NonCoalesced区别、独立Echo应用验收。<br>缺口：未提供通用hover/完整drag实现和跨版本矩阵；无README/许可证；含CGEventSetWindowLocation SPI与subtype=3。 | ★★★★／high | mixed_public_spi |
| 2. [Hammerspoon/hammerspoon](https://github.com/Hammerspoon/hammerspoon) | implementation：Lua事件对象、CG鼠标事件、可选指定应用的PSN投递；AX对象查询/动作。 [原文](https://github.com/Hammerspoon/hammerspoon/blob/23e387e2805a9890066366e0ac96c71b27f0cfd5/extensions/eventtap/libeventtap_event.m) | 未归档；以commit/owner证据，不作兼容认证；2026-07-08 | 可借：typed event字段、native对象生命周期、应用定向投递接口、AX错误语义明确不把API成功等同目标成功。<br>缺口：没有当前读取范围内的NSEvent目标窗口factory后台hover/drag证明；旧PSN路线；window过滤可能漏system agents。 | ★★★／high | public_with_deprecated_psn |
| 3. [octalmage/robotjs](https://github.com/octalmage/robotjs) | implementation：Node绑定跨平台桌面输入；Mac鼠标用CGSessionEventTap并设置move/drag deltas，键盘显式modifier down/up。 [原文](https://github.com/octalmage/robotjs/blob/4fba61cf7bd5deefe1cc3c8c1a1620ecf93fe0c1/src/mouse.c) | 未归档；以commit/owner证据，不作兼容认证；2026-08-07 | 可借：DeltaX/Y修复游戏move、left/right/otherDragged、modifier释放顺序、明确等待时延。<br>缺口：全局Session/HID路线会影响用户光标；未绑定目标PID/window；没有应用效果回读。 | ★★★／medium | public_cgevent_plus_iohid |
| 4. [nut-tree/nut.js](https://github.com/nut-tree/nut.js) | implementation：TypeScript鼠标动作/路径/provider接口；公开drag是press→move→release。 [原文](https://github.com/nut-tree/nut.js/blob/e413fa1f19a19c4631812e4e1eaf47aa732b5cbe/core/nut.js/lib/mouse.class.ts) | 未归档；公开快照≠私有产品；2024-05-01 | 可借：动作provider分层、路径插值、autoDelay可配置、窗口/screen能力拆分。<br>缺口：公开drag异常catch没有finally释放；公开GitHub源码落后私有包；native实现另repo，不是inactive窗口路由证明。 | ★★★★／high | wrapper_provider_dependent |
| 5. [asweigart/pyautogui](https://github.com/asweigart/pyautogui) | implementation：Python跨平台GUI；Mac CG HID鼠标/键盘、特殊键NSEvent桥。 [原文](https://github.com/asweigart/pyautogui/blob/b4255d0be42c377154c7d92337d7f8515fc63234/pyautogui/_pyautogui_osx.py) | 未归档；作者2026称未弃置；2023-06-07 | 可借：组合动作易用性、Darwin catch-up timing、move/drag类型选择。<br>缺口：全局HID输入与主显示器坐标假设；无PID/window定向；截图多屏限制须与依赖分开。 | ★★★／medium | public_cgevent_appkit |
| 6. [moses-palmer/pynput](https://github.com/moses-palmer/pynput) | implementation：鼠标controller维护_drag_button并用对应Dragged类型；全局CG HID投递；listener用session tap。 [原文](https://github.com/moses-palmer/pynput/blob/afc64577b069a51a91474a235f0efbb0e7e0357d/lib/pynput/mouse/_darwin.py) | 未归档；以commit/owner证据，不作兼容认证；2026-05-12 | 可借：显式pressed-button状态、clickCount、多事件监听、注入事件分类。<br>缺口：窗口关联/后台效果不提供；现枚举只有三按钮，未合并PR才添加4–32；全局坐标副作用。 | ★★★／medium | public_cgevent_appkit |
| 7. [enigo-rs/enigo](https://github.com/enigo-rs/enigo) | implementation：Rust objc2/CG native输入；pressed_buttons32、最近move缓存、DeltaX/Y、click计数；全局HID。 [原文](https://github.com/enigo-rs/enigo/blob/a88d9b7e2cec7043ab5f03e754500a091ea928d1/src/macos/macos_impl.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-08-10 | 可借：Rust ownership/retain、快速连续事件使用last_mouse_move、synthetic held state、按下时选Dragged类型、显式delta。<br>缺口：无本次源码中的PID/windowNSEvent后台通路；当前兼容性测试仍需目标效果，而非post返回。 | ★★★／medium | public_cgevent_appkit |
| 8. [Narsil/rdev](https://github.com/Narsil/rdev) | implementation：Rust全局listen/simulate；macOS simulate使用HIDSystemState→HIDEventTap。 [原文](https://github.com/Narsil/rdev/blob/b7e201bf80be7b9632207366f277d29fe732b9c1/src/macos/simulate.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-05-12 | 可借：跨平台事件类型、listener线程/布局边界；physical-only PR提供绕开TIS主线程工作的候选。<br>缺口：simulate的MouseMove固定MouseMoved，ButtonPress只left/right；没有完整pressed-button拖拽状态；extra buttons PR未合并。 | ★★★★／high | public_cgevent |
| 9. [pqrs-org/Karabiner-Elements](https://github.com/pqrs-org/Karabiner-Elements) | implementation：按事件队列/时间戳将键鼠转换为virtual HID reports；mouse_key带button manager状态与20ms节奏。 [原文](https://github.com/pqrs-org/Karabiner-Elements/blob/ec2fea8940f7254a748c052a890ee9db7113d9b1/src/share/manipulator/manipulators/post_event_to_virtual_devices/queue.hpp) | 未归档；以commit/owner证据，不作兼容认证；2026-10-04 | 可借：绝对时间队列、button-state report、设备重建/释放模式、证书和服务权限稳定性说明。<br>缺口：系统级HID不是PID定向；需DriverKit依赖与用户权限；睡眠/唤醒和绝对设备问题仍有开放报告。 | ★★★★★／very_high | public_driverkit_iohid |
| 10. [pqrs-org/Karabiner-DriverKit-VirtualHIDDevice](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice) | implementation：DriverKit虚拟键盘/鼠标；报告含32buttons和x/y/wheel，host系统按硬件设备识别。 [原文](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/blob/072fa83e824c1b633f508f60cbad87b41aab3047/include/pqrs/karabiner/driverkit/virtual_hid_device_driver/hid_report/pointing_input.hpp) | 未归档；以commit/owner证据，不作兼容认证；2026-10-03 | 可借：标准HID descriptor、button bitmap、report timestamp和reset/全零释放。<br>缺口：客户端要求root、系统扩展安装/审批/签名；全局HID，缺少按目标窗口后台路由；不能把virtual cursor等同独立focus。 | ★★★★★／very_high | public_driverkit |
| 11. [noah-nuebling/mac-mouse-fix](https://github.com/noah-nuebling/mac-mouse-fix) | reference：真实鼠标增强；FakeDrag用down→OtherMouseDragged→up；自定义滚动/手势事件和设备过滤。 [原文](https://github.com/noah-nuebling/mac-mouse-fix/blob/dae242dbdd5a76d93e9e4d4dbed3084224757473/Helper/Core/Drag/FakeDrag/ModifiedDragOutputFakeDrag.m) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：drag完整生命周期、取消释放、事件来源过滤、滚动连续性/terminal velocity/phase经验。<br>缺口：全局session route；部分原始HID/SPI；自定义MMF许可非MIT，复制发布有额外条件；没有inactive窗口成功证明。 | ★★★★／high | mixed_public_private |
| 12. [linearmouse/linearmouse](https://github.com/linearmouse/linearmouse) | implementation：CG event tap生命周期与恢复；滚动momentum ownership；新增hover-focus但使用SkyLight私有focus协议。 [原文](https://github.com/linearmouse/linearmouse/blob/03b0656dc5e4364ee951449e0851a029a25fd5be/LinearMouse/EventTap/EventTap.swift) | 未归档；以commit/owner证据，不作兼容认证；2026-10-05 | 可借：runloop common modes、保留callback上下文、watchdog恢复、50ms AX超时、精确window匹配、synthetic flags处理。<br>缺口：focus without raising不等于保持应用inactive；WindowFocus调用setFrontProcess/SLPSPostEventRecordTo；不是NSEvent后台executor。 | ★★★／high | mixed_public_spi |
| 13. [ther0n/UnnaturalScrollWheels](https://github.com/ther0n/UnnaturalScrollWheels) | implementation：物理滚轮反向/禁加速/滚动步幅设置，CG HID event tap拦截滚轮；已有wake后恢复。 [原文](https://github.com/ther0n/UnnaturalScrollWheels/blob/07a61a0ddc73f37ff2e14a10afe4cec50f69184d/UnnaturalScrollWheels/ScrollInterceptor.swift) | 未归档；以commit/owner证据，不作兼容认证；2026-08-03 | 可借：disabledByTimeout/UserInput恢复、wake notification/延后复查、锁保护tap状态、无效tap重建。<br>缺口：非输入合成或target窗口route；全局拦截可能影响用户；GPL-3.0代码复用许可需评估。 | ★★／medium | public_cgevent_iohid |
| 14. [tmandry/AXSwift](https://github.com/tmandry/AXSwift) | implementation：Swift公共AX client薄包装，typed属性、批量属性、performAction、observer。 [原文](https://github.com/tmandry/AXSwift/blob/e18a18453d135ad45809a384ee5139e05ea52def/Sources/UIElement.swift) | 未归档；维护未知/需新OS验证；2021-11-14 | 可借：AXUnsupported/noValue与致命错误分开、批量查询、超时控制、CF retain和Observer生命周期。<br>缺口：不是像素hover/dragexecutor；AX可用性取决目标暴露；Observer默认mode需考虑tracking/modal；defaultHEAD2021维护状态不能断言停止。 | ★★／medium | public_ax |
| 15. [DevilFinger/DFAXUIElement](https://github.com/DevilFinger/DFAXUIElement) | implementation：Swift AXUIElement/AXObserver包装和PID observer center；包括Press、ShowAlternateUI/DefaultUI语义动作。 [原文](https://github.com/DevilFinger/DFAXUIElement/blob/938003b3171c710e23c6d1c08da7e61b892f0ba9/Classes/AXUIElement%2BDFAction.swift) | 未归档；维护未知/需新OS验证；2021-01-18 | 可借：hover-like AX语义动作、应用/窗口属性/observer注册表、权限状态明示。<br>缺口：只有目标支持相应AX action才有效，非通用mouse hover；旧Swift样例/mainthread/observer teardown需审计；2021head维护未知。 | ★★／medium | public_ax |
| 16. [NUIKit/CGSInternal](https://github.com/NUIKit/CGSInternal) | reference：2016私有CoreGraphics/SkyLight headers/结构定义；包含event-record和mouse-focus/window事件mask符号。 [原文](https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSEvent.h) | 未归档；维护未知/需新OS验证；2016-03-25 | 可借：历史术语/字段定位和版本差分线索，不作为稳定ABI。<br>缺口：无执行器、无许可证、符号/结构版本漂移；无hover/drag验收；不是官方公开API。 | ★★★★★／very_high | spi_only |
| 17. [trycua/cua](https://github.com/trycua/cua) | 原生多路由/CUA driver：Rust SkyLight post、window stamping、AX enablement、capture、per-PID serialization 已有实现；不是完整通用后台hover/drag。 [原文](https://github.com/trycua/cua/blob/49e924c4632882134b204b2e8a0ce8fae44418df/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：事件字段/明确route/状态矩阵/结构化 refusal；仅独立移植研究，不把best-effort fallback直接接入RWI。<br>缺口：typed native hover、后台drag与所有不可见状态的signed receiver证据；安全ABI探测/错误隔离/恢复边界。 | ★★★★★／高：SPI ABI、事件认证、同PID多窗、焦点恢复、结果精确性 | public CG/AX + private SLEventPostToPid、SLPSPostEventRecordTo、CGEventSetWindowLocation、认证record偏移；SPI不稳定。 |
| 18. [simular-ai/Agent-S](https://github.com/simular-ai/Agent-S) | 模型编排与grounding：Mac ACI 和S3生成pyautogui click/move/drag代码；S1读取frontmost AX tree。 [原文](https://github.com/simular-ai/Agent-S/blob/3aa272d23d2994c7bbde1acbbe0ef8e8d06b8693/gui_agents/s1/aci/MacOSACI.py) | 未归档；以commit/owner证据，不作兼容认证；2026-09-05 | 可借：独立语义grounding、模型计划/动作接口和实际目标读回的分离。<br>缺口：exact native窗口身份/lease/no-replay、非激活输入、原生hover/drag收件证据。 | ★★★／中：输出契约和目标grounding；不是平台执行器替换 | Python/PyAutoGUI包装；未在已审ACI中实现NSEvent/SkyLight背景通道。 |
| 19. [simular-ai/OpenACI](https://github.com/simular-ai/OpenACI) | Mac ACI历史实验：AX/OCR节点与pyautogui动作；open_running_app调用activateWithOptions。 [原文](https://github.com/simular-ai/OpenACI/blob/54d2367b52d7d7a5a3cb6895ca1418e36fca3a44/openaci/macos/system.py) | 未归档；维护未知/需新OS验证；2024-11-26 | 可借：节点可操作性与视觉检测分层的反面案例；模型不应把OCR补的AXButton当真实AX authority。<br>缺口：后台路由、严格窗口scope、现代权限/取消/lease和收件fixture。 | ★★★／中：保留AX/OCR证据类型区分，重新设计native actuator | PyObjC/AppKit公共AX + pyautogui；激活与shell路径不符合RWI后台契约。 |
| 20. [screenpipe/screenpipe](https://github.com/screenpipe/screenpipe) | 事件驱动采集与permission诊断：Rust permissions pure checks；TCCAccessPreflight私有读取与public capture probes分层。架构文档描述AX优先/OCR fallback与event-driven capture。 [原文](https://github.com/screenpipe/screenpipe/blob/fed2c61e10dd0c78481c22a6b0a8823de55bd040/crates/screenpipe-core/src/permissions.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：权限真实check/缓存状态/实际采集/config-disabled区分；不能把frame stale全归因权限。<br>缺口：scope受限exact窗口事件输入与后台动作成功验收不是其被审采集模块能力。 | ★★★／中：permission状态机与capture观测；许可约束需独立处理 | public permission/capture + private TCCAccessPreflight(dlsym)，是状态查询不是授权bypass。 |
| 21. [OpenAdaptAI/OpenAdapt](https://github.com/OpenAdaptAI/OpenAdapt) | 示范编译/独立验证契约：现仓是meta-package/CLI，委托openadapt-flow；docs明确VERIFIED/COMPLETED_UNVERIFIED、reconciliation和native权限。 [原文](https://github.com/OpenAdaptAI/OpenAdapt/blob/7ae7ebc02048f41dd820ad826d3e78c4dcce2cd2/openadapt/cli.py) | 未归档；以commit/owner证据，不作兼容认证；2026-09-14 | 可借：不把halt/无效返回叫成功；独立effect verifier与admitted workflow边界。<br>缺口：本仓没有独立native后台鼠标实现；engine/capture在其他canonical仓，旧legacy不是现平台实现。 | ★★★／中：证据和结果模型可参考；当前launcher不承担native执行 | CLI Python + platform extras；不是Rust backend或权限绕过。 |
| 22. [appium/appium-mac2-driver](https://github.com/appium/appium-mac2-driver) | 原生GUI测试/事件序列对照：AMActionCommands→fb_performW3CActions→XCSynthesizedEventRecord→device.synthesizeEvent；pointer move/down/up/drag真实实现。 [原文](https://github.com/appium/appium-mac2-driver/blob/f38257191fa9f273a684f6a9c8c2b16d1272bd09/WebDriverAgentMac/WebDriverAgentLib/Commands/AMActionCommands.m) | 未归档；以commit/owner证据，不作兼容认证；2026-10-02 | 可借：有界时序、button/relative-origin、gesture recipes与独立receiver fixture的对照。<br>缺口：已审实现未证明无前台/不动cursor/隐藏窗口；release-actions endpoint是dummy而非清理保证。 | ★★★★／高：XCTest runner/私有合成记录生命周期，不能当普适无焦点API | XCTest公开框架 + XCPointerEventPath/XCSynthesizedEventRecord等私有测试接口；WebDriver公共wire不使底层私有。 |
| 23. [AXElements/AXElements](https://github.com/AXElements/AXElements) | AX语义与Mouse DSL历史对照：press→element.perform(:press)，click ensure Mouse.click_up；set先set_focus；move/drag依赖Mouse gem。 [原文](https://github.com/AXElements/AXElements/blob/542e94a4f50077d076521ab9d606e8bfe82112ee/lib/accessibility/dsl.rb) | 未归档；维护未知/需新OS验证；2017-09-28 | 可借：semantic press vs coordinate click分开；ensure-up资源生命周期。<br>缺口：后台pointer不是被审DSL中的exact PID/window路由；Ruby/native dependencies不能直接取代Rust。 | ★★／低中：AXPress/ensure-up思路已有，现代ABI需新验证 | AX公共API/CGEvent包装；未审Mouse gem内部，不能臆测它有SkyLight。 |
| 24. [pyatom/pyatom](https://github.com/pyatom/pyatom) | AX/PID-keyboard与全局鼠标的混合历史：AXClasses._addKeyToQueue对非global key用CGEventPostToPid；_queueMouseButton的down/up发session、drag发HID。 [原文](https://github.com/pyatom/pyatom/blob/a170717408d46f5d872585a877ffd8ee09a1a052/atomac/AXClasses.py) | 未归档；维护未知/需新OS验证；2019-02-02 | 可借：click count/modifier/type/dispatch queue与AX元素接口；分清key和mouse投递。<br>缺口：鼠标window关联/非全局route、窗口身份生命周期、现代macOS实证。 | ★★★／中：键盘定向/序列字段可参考；鼠标route需要重写 | PyObjC Quartz公共CGEvent/AX；mouse路径不是native真后台。 |
| 25. [oculix-org/SikuliX1](https://github.com/oculix-org/SikuliX1) | 历史视觉+Java Robot镜像：RobotDesktop extends java.awt.Robot；heldButtons/keys、smooth move、screen capture/press操作。README明确历史mirror并指向Oculix。 [原文](https://github.com/oculix-org/SikuliX1/blob/c6f17990494541974353a7c64987f41c6761e612/API/src/main/java/org/sikuli/support/devices/RobotDesktop.java) | 未归档；历史mirror；2026-05-15 | 可借：visual match不等于input authority；动作序列与释放状态组织。<br>缺口：exact PID/window native background没有由Robot实现；active successor需另审不能合并本仓计数。 | ★★★／中：识别/输入分离与held状态；native背景渠道独立 | Java AWT公共Robot、全局桌面输入，不是NSEvent后台实现。 |
| 26. [sikuli/sikuli](https://github.com/sikuli/sikuli) | 明确停止支持的旧Sikuli：DesktopRobot extends Robot：mouseMove→mousePress→smoothMove→mouseRelease；README明确X-1.0rc3不再支持。 [原文](https://github.com/sikuli/sikuli/blob/4adaab7880d2f3e14702ca7287ae9c9e4f4de9ab/sikuli-script/src/main/java/org/sikuli/script/DesktopRobot.java) | 未归档；旧版明确不支持；2019-10-27 | 可借：down/move/up和wait/velocity基本对照。<br>缺口：旧global screen动作不提供任意原生窗口背景；版本适配/可靠性需重新实现。 | ★★／低：历史序列示例；不建议作为产品依赖 | Java AWT公共Robot，非window-addressed后台API。 |
| 27. [kwhat/jnativehook](https://github.com/kwhat/jnativehook) | 全局事件hook/recording边界：JNI把Java NativeMouseEvent转换uiohook_event，hook_post_event交给libuiohook；README列global listeners和post。 [原文](https://github.com/kwhat/jnativehook/blob/dd4a524a6e6035acc5180230789992c44ac156b4/src/main/jni/com_github_kwhat_jnativehook_GlobalScreen.c) | 未归档；维护未知/需新OS验证；2023-06-23 | 可借：接收阶段证据/事件源生命周期/权限失败记录。<br>缺口：全局hook不是target NSView消费证明；被审JNI无PID/window定向入口。 | ★★★／中：来源归属与hook生命周期；不能从hook收到推目标消费 | JNI/libuiohook桥；具体macOS libuiohook actuator需独立源审，不能推断SPI。 |
| 28. [codebytere/node-mac-permissions](https://github.com/codebytere/node-mac-permissions) | 权限状态与请求UI对照：getAuthStatus用AXIsProcessTrusted/CGPreflight；Screen请求CGRequest，AX请求实际打开Privacy pane；IOHIDRequestAccess申请listen/post。 [原文](https://github.com/codebytere/node-mac-permissions/blob/7bdd039cc5bb0dd9c81540df4340b0c4bdb3be37/permissions.mm) | 未归档；以commit/owner证据，不作兼容认证；2026-04-05 | 可借：status/check/request/prefs导航分离，签名更新与授权状态观测。<br>缺口：函数不授予用户同意、不自动勾选、不绕过TCC；Node wrapper不适合作RWI backend。 | ★★／低：Rust FFI已有对应check；UI提示需显式授权 | public AX/CG/IOHID权限API和settings URL；未经用户批准不可触发请求。 |
| 29. [microsoft/playwright](https://github.com/microsoft/playwright) | 浏览器CDP/WebKit/Firefox交互对照：crInput发送CDP mouseMoved/Pressed/Released及buttons/force/modifiers/clickCount；dragManager单独处理DnD；pages可不bringToFront。 [原文](https://github.com/microsoft/playwright/blob/4357c237cfde9135fb5b7894c22a45468321a973/packages/playwright-core/src/server/chromium/crInput.ts) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：typed actionability、input state、geometry、hover/drag与readback；页面上下文隔离。<br>缺口：DOM/page后台不是任意macOS窗口后台，native dialogs/toolbar/第三方app不由CDP接收。 | ★★★／中：限定browser route可用；与native scope连接复杂度另算 | public browser automation/CDP protocol，不是Mac窗口NSEvent/SkyLight。 |
| 30. [SeleniumHQ/selenium](https://github.com/SeleniumHQ/selenium) | W3C action模型/浏览器scope：PointerInput编码move/down/up/cancel/pause、viewport/element origin；W3C定义per-context input state及serialized queue。 [原文](https://github.com/SeleniumHQ/selenium/blob/3d4a8c1162ceedd38fce9b117aa5fc4531aecd95/py/selenium/webdriver/common/actions/pointer_input.py) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：单source状态、tick同步、release/cancel、window handle与独立可点击性检查。<br>缺口：WebDriver browsing context handle不是CGWindowID；browser API不证明native hidden app输入。 | ★★／低中：复用协议思想，不直接借浏览器remote端做native控制 | 公共W3C WebDriver/BiDi；下层browser实现和native API另审。 |
| 31. [browser-use/browser-use](https://github.com/browser-use/browser-use) | browser agent typed动作和状态修复：actor.Mouse保存last x/y/buttons，成功dispatch后更新；click finally up；move带buttons；scroll存在多个显式方法fallback。 [原文](https://github.com/browser-use/browser-use/blob/c75e8476e26d18b7617643bc2ae082fae8eae431/browser_use/actor/mouse.py) | 未归档；以commit/owner证据，不作兼容认证；2026-10-07 | 可借：last-successful coordinates、button mask、hover可见交集、state更新时点。<br>缺口：browser viewport/CDP route不是Mac窗口route；scroll catch-all fallback不应照搬RWI不可重放路径。 | ★★★／中：状态机可参考；Python/CDP非native Mac执行器 | 公共CDP/DOM和Python agent layer；nativeMac background没有由此实现。 |
| 32. [microsoft/OmniParser](https://github.com/microsoft/OmniParser) | 视觉screen parsing/grounding：Omniparser.parse对图像OCR+detector+caption，返回labeled image/parsed_content；不是鼠标actuator。 [原文](https://github.com/microsoft/OmniParser/blob/354021201345a96178360b28733573e27269f2de/util/omniparser.py) | 未归档；以commit/owner证据，不作兼容认证；2026-07-20 | 可借：小控件结构化视觉标签、归一化bbox与机器可读视觉来源。<br>缺口：window/element native身份、授权、动作与goal verifier都不是parser提供；不得把detector bbox当AXButton。 | ★★★／中：旁路视觉识别可参考；模型许可、算力、尺寸/延迟独立 | PyTorch视觉模型API；与Mac输入API正交；code/model/dependency许可各自核验。 |
| 33. [glfw/glfw](https://github.com/glfw/glfw) | client_windowing：Cocoa普通motion读取event位置；disabled模式读取delta；tracking限定key窗口；hovered getter独立读全局cursor/hit。 [原文](https://github.com/glfw/glfw/blob/92dcf4ce74f2e2554a98fea09be7c705c17daa5a/src/cocoa_window.m) | 未归档；以commit/owner证据，不作兼容认证；2026-08-03 | 可借：事件坐标/delta/button及tracking矩阵<br>缺口：目标自己的global hover查询与key-window策略不会因外部定向event自动改变 | ★★★／medium | public AppKit/Quartz + GLFW内部状态 |
| 34. [libsdl-org/SDL](https://github.com/libsdl-org/SDL) | client_windowing：macOS26+非relative motion按global mouse重新计算位置；drag共用motion；relative/raw输入有另一GCMouse路径。 [原文](https://github.com/libsdl-org/SDL/blob/e4af6f8e283812aa693eb2c70c91249a9e1ce92c/src/video/cocoa/SDL_cocoawindow.m) | 未归档；以commit/owner证据，不作兼容认证；2026-10-07 | 可借：client框架适配矩阵；button reconciliation与坐标域<br>缺口：event-local正确不等于客户端最终位置正确；raw path可忽略NSEvent | ★★★★★／high | public AppKit/Quartz/GameController + SDL内部状态 |
| 35. [wine-mirror/wine](https://github.com/wine-mirror/wine) | compatibility_layer：主线mac驱动为background mouse move安装ActiveAlways tracking；Win32 capture window有显式native桥接。 [原文](https://github.com/wine-mirror/wine/blob/63f62f7cd6964f544b08ae191d6a3d08fc8e5925/dlls/winemac.drv/cocoa_window.m) | 未归档；以commit/owner证据，不作兼容认证；2026-10-07 | 可借：Wine HWND/native窗口映射；capture状态和tracking选择<br>缺口：用户GameHub/CrossOver定制版本未确认；主线不能代表其已安装行为 | ★★★★／high | inspected public AppKit tracking + Wine内部驱动；完整SPI审计未做 |
| 36. [electron/electron](https://github.com/electron/electron) | client_framework：SendInputEvent转成renderer鼠标事件，offscreen有专门分支；官方webContents文档仍要求所属BrowserWindow聚焦。 [原文](https://github.com/electron/electron/blob/18f7d6cc5ca32920efebf38197cb6ca3ff1be121/shell/browser/api/electron_api_web_contents.cc) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：renderer/OS input层区分；pointer-capture fixture<br>缺口：自有Electron API不是跨进程NSEvent入口；其他app无法直接取得其webContents | ★★★★／high | public Electron app API + Chromium内部renderer输入 |
| 37. [chromium/chromium](https://github.com/chromium/chromium) | client_framework：BaseView以事件流维护mouse containment；隐藏view时补exit；注释说明headless合成event与window global location可不一致。 [原文](https://github.com/chromium/chromium/blob/821e18d4add56df35387fe67113b888706dd3853/ui/base/cocoa/base_view.mm) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：事件流pointer状态；hide/unhide enter/exit平衡<br>缺口：本次仅审Cocoa BaseView，未证明任意Chrome外部输入/每种窗口路由 | ★★★★／high | public AppKit + Chromium内部view routing |
| 38. [qt/qtbase](https://github.com/qt/qtbase) | client_framework：Cocoa维护accepted mouse-down按钮状态，drag/up据此派发；ActiveAlways tracking为激活切换workaround，注释说明inactive相关callback需忽略。 [原文](https://github.com/qt/qtbase/blob/8295380702b4902256341e15ee58dd809f65fd5e/src/plugins/platforms/cocoa/qnsview_mouse.mm) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：accepted-down拖拽状态机；事件filter/几何/DPI测试<br>缺口：Qt/应用可过滤事件与改写hover；根README不存在，已读examples/README | ★★★★／high | public AppKit + Qt内部QPA；完整plugin SPI审计未做 |
| 39. [tauri-apps/tao](https://github.com/tauri-apps/tao) | client_windowing：macOS event-window位置转view位置；越界且全局pressedMouseButtons为0时丢motion；drag/move进入相同处理。 [原文](https://github.com/tauri-apps/tao/blob/3fcda65f3f5a7c9ad164a622d2b98e4b5e313554/src/platform_impl/macos/view.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-10-05 | 可借：坐标转换与边界/button-state测试<br>缺口：目标的global按键状态可能与RWI私有source不同；不可假定与winit完全一致 | ★★★／medium | public AppKit + Tao内部Rust routing |
| 40. [rust-windowing/winit](https://github.com/rust-windowing/winit) | client_windowing：当前AppKit backend从event转view/physical位置；越界且全局button为0时丢motion；click先更新motion位置。 [原文](https://github.com/rust-windowing/winit/blob/01634067402db15d26a31bfd199f5c69a989813d/winit-appkit/src/view.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-10-06 | 可借：pointer stream/DPI/button-state标准fixture<br>缺口：旧winit-rs/winit别名API404；实际canonical库存在；raw input提案尚open | ★★★／medium | public objc2 AppKit + Winit内部routing |
| 41. [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | remote_desktop：server input处理绝对/相对mouse与down/up，使用enigo/rdev/VirtualInput，属于远端seat输入。 [原文](https://github.com/rustdesk/rustdesk/blob/6da7977a9451e0f7d480bd93f95f58024eb21579/src/server/input_service.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-10-08 | 可借：会话授权/输入ownership与相对输入界限<br>缺口：全局远端桌面控制不等于同一Mac上的独立后台窗口；MCP PR尚open | ★★★／medium | native abstraction enigo/rdev；此分片未审其所有macOS FFI |
| 42. [H-M-H/Weylus](https://github.com/H-M-H/Weylus) | tablet_streaming：pointer路径调用before_input；macOS display hook为no-op、window hook刷新geometry，随后由autopilot做global-seat input。 [原文](https://github.com/H-M-H/Weylus/blob/38a01a8f8e429500c7e9f67fc1c88ca37a4d1e93/src/input/autopilot_device.rs)、[mac hook](https://github.com/H-M-H/Weylus/blob/38a01a8f8e429500c7e9f67fc1c88ca37a4d1e93/src/capturable/core_graphics.rs) | 未归档；以commit/owner证据，不作兼容认证；2026-02-16 | 可借：capture/input几何关系；tablet流与失败反馈<br>缺口：mac单窗口capture已在源码实现，README相关FAQ滞后；global-seat input不是窗口定向后台路线，未审autopilot完整mac FFI | ★★★／medium | autopilot abstraction；本次未审其完整mac FFI |
| 43. [deskflow/deskflow](https://github.com/deskflow/deskflow) | software_kvm：OSXComputer维护button/click/modifier状态和delta，以HID tap派发全局seat事件。 [原文](https://github.com/deskflow/deskflow/blob/3ab30dc060cbc855a5fe5c6ed93ece326f9e5a90/src/lib/platform/OSXComputer.mm) | 未归档；以commit/owner证据，不作兼容认证；2026-10-07 | 可借：drag clickState/delta/flags；注入标记与cursor状态一致性<br>缺口：不会提供独立窗口后台；尚open cursor修复PR不可当已发布能力 | ★★／medium | public Quartz HID route + platform internals |
| 44. [input-leap/input-leap](https://github.com/input-leap/input-leap) | software_kvm：OSXScreen补drag clickState、modifier flags与delta，并维护全局cursor位置。 [原文](https://github.com/input-leap/input-leap/blob/34a34fb20b93113a6b26052cb5a54f9be2327775/src/lib/platform/OSXScreen.mm) | 归档；以commit/owner证据，不作兼容认证；2025-11-27 | 可借：旧实现中button/drag字段处理<br>缺口：API archived=true；是冻结参考，不当持续维护分支或后台window服务 | ★★／medium | Quartz/旧Carbon/IOKit混合；有未公开scroll常量注释 |
| 45. [debauchee/barrier](https://github.com/debauchee/barrier) | software_kvm：OSXScreen维护全局cursor、drag clickState、flags和delta；KVM接管seat。 [原文](https://github.com/debauchee/barrier/blob/653e4badeb88f61de901581667d4465d7b1e2d52/src/lib/platform/OSXScreen.mm) | 未归档；维护未知/需新OS验证；2022-02-04 | 可借：drag字段和global-cursor状态管理<br>缺口：HEAD2022且release2021，但archived=false；quiet不证明项目停止；不是窗口隔离 | ★★／medium | Quartz/旧Carbon/IOKit混合；有未公开scroll常量注释 |
| 46. [asmvik/yabai](https://github.com/asmvik/yabai) | window_manager：窗口管理用AX属性及SLS/private window-server API；包含鼠标warp、动画proxy和scripting addition。 [原文](https://github.com/asmvik/yabai/blob/dd845723416f5fe92af49fad5ebab00369e07edd/src/window_manager.c) | 未归档；以commit/owner证据，不作兼容认证；2026-06-14 | 可借：窗口catalog/owned-window关系；AX异步耗时与event tap隔离<br>缺口：窗口移动/聚焦/Space操纵不等于将输入送进后台view；SPI不稳定 | ★★★／medium | mixed public AX/Quartz + private SLS/SA |
| 47. [nikitabobko/AeroSpace](https://github.com/nikitabobko/AeroSpace) | window_manager：focus-follow-mouse读取global NSEvent并做AX hit；virtual workspace通过把窗口放在屏角模拟，不创建display。 [原文](https://github.com/nikitabobko/AeroSpace/blob/74a1bf17e82d70e0a21945ba04bb4f590bf19f83/Sources/AppBundle/mouse/mouse.swift) | 未归档；以commit/owner证据，不作兼容认证；2026-10-01 | 可借：窗口类型/跨进程AX组织；monitor布局变化<br>缺口：workspace模拟≠虚拟显示器≠后台输入；全局mouse会改变focus | ★★／medium | public AX/AppKit inspected；完整repo API审计未做 |
| 48. [lwouis/alt-tab-macos](https://github.com/lwouis/alt-tab-macos) | window_capture：SCK窗口发现cache和bounded capture队列；版本选择中有private WindowServer capture fallback；spec记录超时与missing-window策略。 [原文](https://github.com/lwouis/alt-tab-macos/blob/6d8940b5307d7348aaca70259c28147fc5ab00b3/src/events/WindowCaptureEvents.swift) | 未归档；以commit/owner证据，不作兼容认证；2026-10-06 | 可借：capture discovery/cache/优先级/错误分类<br>缺口：截图可用不证明后台输入；规格中的OS缺陷是项目观察，不是Apple保证 | ★★★／medium | mixed public ScreenCaptureKit + private CGSHWCaptureWindowList fallback |
| 49. [waydabber/BetterDisplay](https://github.com/waydabber/BetterDisplay) | virtual_display_release_docs：landing树仅README；发布app、docs/issues与integration协议，未公开app native实现。 [原文](https://github.com/waydabber/BetterDisplay/blob/ba7ab094b7999ed1192946b8c486adb48c4285d4/README.md) | 未归档；app native未公开；2026-10-06 | 可借：明确授权的virtual-display生命周期及CLI/notification协议<br>缺口：没有开源native driver可复用；虚拟屏上frontmost输入仍是seat输入；latest虚拟屏issue未独立复验 | ★★★／medium | app internals unknown; public documented app integration |
| 50. [waydabber/betterdisplaycli](https://github.com/waydabber/betterdisplaycli) | display_integration_client：小Swift程序通过DistributedNotificationCenter发JSON，UUID匹配响应并处理15秒timeout。 [原文](https://github.com/waydabber/betterdisplaycli/blob/117129a222b3e00e6ba00387830f2e9a0befb647/betterdisplaycli.swift) | 未归档；以commit/owner证据，不作兼容认证；2025-04-07 | 可借：typed request/response相关性、timeout及目标display identifier<br>缺口：必须有running BetterDisplay；CLI不是虚拟display driver，也不提供mouse/window后台事件 | ★★／low | public Foundation notification IPC; BetterDisplay内部未知 |

## 8. 下一阶段：先获得可靠证据，再补功能

### P0：协议、状态与离线native探针

- 先维持adapter/Agent/typed接口的字段和能力协商一致；optional未启用的特性不应发给旧Agent。拒绝发生在schema阶段，不归成NSEvent/窗口不接受。收到其他任务关于`red_marker_cluster`未知字段的反馈已单独列为待溯源事项，本轮未独立执行该调用、未代操作游戏。
- Rust原生测试比较现CG构造与NSEvent工厂，读取type/window/local-global、uptime/event counters、button/pressure/delta/flags/source state。**先构造/roundtrip，无input**，不要启动示例app即称验收。
- 比较完整gesture单source与现多source；shared planner/cleanup继续保持有界、取消、release不确定围栏和不重放。

### P1：自有接收fixture及window-local观察

须明确分配窗口后再启动：目标记录PID tap、NSApp/window/view/tracking/client消费、goal；另自有前台测试控件记录是否继续收输入与恢复，不能监控用户敏感文本。正/负控包含无down的dragged、错window/坐标、global-poll不一致、nested loops、callback与getter差异；故意错window负控只在自有fixture。

并行实现non-on-screen枚举、SCK单window捕获和window-local authority；保留bounds/hit/epoch/lease，不能只是删除display guard。popup/dialog/parent surfaces和Live聚合另验。

### P2：typed Hover与多route比较

typed API提供move、bounded dwell和hover readback，不继续依赖临时JS拼低级动作。AX语义、公开定向event、NSEventfactory、SPI候选都显式选择/报告，不跨scope悄悄fallback。

8类状态分别验：active-visible、inactive-visible、fully-occluded、same-app-sibling-covered、offscreen、other-Space、minimized、hidden。动作分click/right/double/key/IME、callback/highlight/tooltip、slider/canvas drag、relative/raw、DnD。输出包括失败在哪一层、真实input计数、资源状态和目标读回。

### P3：用户分配的真实应用和性能回归

确认实际AppKit/SwiftUI/Electron/Qt/Wine/SDL build，不从上游主线猜本机GameHub。游戏/应用留用户指定主屏；不为验收强行移动、激活或unminimize。先5次探索、再支持格20次可逆同状态重复；报告所有拒绝/失败，不用这一小样本证明普适成功率。

分别量capture/convert/encode、native input/guard、transport/MCP/模型交互；只对相同目标、状态、动作、图像分辨率做比较。重启/证书/部署、其他已存在MCP成功率/输出/多窗口计划仍保留，不由本轮研究取代。功能源码变更时再运行规定Rust全workspace门禁。

## 9. 最终研究结论与完成边界

**本轮最终设计判断：RWI不应等一个“全功能完美开源库”，也不应只做NSEvent替换。**采用macOS Rust原生、分层多route、动作/窗口状态能力矩阵，优先补事件状态、真实hover/drag、window-local capture/authority、key接收方恢复和goal readback。NSEvent是优先构造候选，SkyLight是高难度SPI对照候选；可公开稳定实现的部分先做，其他部分有证据地实验/拒绝。

本文完成了用户要求的扩大范围、维护/参考分类、缺口、难度、复杂度与API支持评估；**没有完成原生后台功能实现、全应用兼容认证或线上性能验收**。原7项目研究保留历史结果，现设计以本文和功能矩阵为准。补齐过程遇到库没做、deprecated、未维护或不适配，分别判断可原创实现/需隔离实验/不可直接借用，不简单等同“无法实现”。

四角色沿用原事务，新增survey补充；源代码原生实现未改。BASELINE没有扩展报告/矩阵，MODIFIED有50仓库/150技术正文与16项能力，ROLLBACK恢复此前文档/新增文件缺席且hash一致，随后再应用。具体实际退出码/字节/哈希/错误恢复见VERIFICATION，不把旧Rust测试当本轮新回归。
