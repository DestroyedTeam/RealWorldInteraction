## 2026-10-11 · R45：执行中系统切窗的自主测试程序（尚未验完）

上一轮head f175181的实际CI38119325985已回读：Mac与Windows均success，不把这个结果冒用于下述新源码。

- 添加默认关闭的两自有fixture固定Command+Tab actor：只由实际后台canvas down武装一次common-mode timer；绑定peer PID、同UID内核出生时间、bundle与精确CG窗口bounds/title/观测layer；使用现有post/listen授权，未知HID/持键/前台变化时不继续down，异常时只尝试自身配对up。无权限请求、激活fallback、输入重放或新后端bridge。`--check-only`只验证配置、不创建目录/GUI/输入。
- 首编译E0624/101保留，finite仅对同fixture模块开放后通过。初8项actor测试0、后新增kernel-birth用例9项0；原53项receiver保留。首次实机发现直接启动.app的NSRunningApplication.launchDate为nil，改用与RWI绑定同类proc_pidinfo原生birth/UID严格检查，而非PID-only。
- 第二次实机发现旧layer0筛选错过显式peer窗口，改为精确CGID/PID选择并固定实际观测layer；不猜layer数值。曾经“setLevel”搜索无匹配exit1使&&后的首patch未执行，已更换命令纠正，不能把该次记录算已修改。
- 最终90/91原生启动，actor已有post/listen/screen三项权限、绑定初始化error=null。但loaded RWI准备focus90时user_active/HIDrecent拒绝于dispatch之前；未发Command+Tab、未跑拖拽，不能计为系统切窗成功，不用别的通道绕过，不要求用户配合。
- 86–91每个实际process终止0：87具名Close一次、其他受限/超时者由显式寿命timer关闭；无快捷键或down输入，actor post_calls0。90/91已detach，其他应用未发送输入。
- 同输入配置检查BASELINE1（旧版不认识check-only）→MODIFIED0（JSON明确no GUI/input/native binding）→独立copy executable ROLLBACK1并恢复原binary hash→REAPPLIED0。四源码hash与新增module缺席状态回滚恢复，仅独立copy，生产不回滚。此结果不是快捷键/Host验收。

最终11项工程命令全部0：fmt apply/check、workspace736/0/11/42、all-target/all-featureClippy、receiver build/62tests、UI7、MCP/Agent release、architecture0/原12debt、10正负例。9项actor含在62 receiver中，不加进默认workspace736。Agent/MCP安装、配对与证书不变；该fixture候选尚未因本轮对话而提交，不把配置probe/绿色源码测试当实机完成。正式Live强任务绑定、真实人类来源、完整窗口族和F01–F16缺口保持原范围。

## 2026-10-11 · R44：无人配合验收与 Live 预加载声明

- 延续原生320点浮层和隐藏/恢复/关闭证据；不改变capture2048，不宣称逐像素复制官方Computer Use。官方插件reference允许资源在加载前声明 `openai/ui.availableDisplayModes`；本轮为控制组件声明fullscreen-only（不是把原生视频浮层放大），initialize同样声明，资源cache key升级v2，v1保持可读。closed-network CSP、实际DOM可见性门禁、单次不重放关闭、presence TTL均保留。参考：https://developers.openai.com/plugins/reference 。
- 首工作区回归实际101：集成test仍期望v1 catalog；修正为v2并新增v1内容/安全metadata相同断言，未删除权限负例。之后八门禁均0：fmt、workspace736/0/11/42、全target/featureClippy、receiver build/53tests、UI7、MCP/Agent release。错误的`--lib`调用101亦保留，并改为真实bin test。
- 实际MCP进程同输入initialize+read v1：BASELINE探针1（未声明模式，server0）→MODIFIED0（资源及组件均fullscreen，server0）→独立copy可执行ROLLBACK1（恢复原binary SHA，server0）→REAPPLIED0；四源码hash亦回滚恢复，生产没有回滚。
- 新MCP二进制5cf842…已原子部署；配对/profile/cert/key等7文件hash不变，Agent25946未重启，也未杀其他会话的六个adapter。安装路径启动的真实新进程协议验收0。现有Host资源列表仍v1，**不冒称本任务已刷新、正式组件挂载或强任务绑定成功**；不请休息中的用户重启/展开。
- 专用84/85仅在virtual4：RWI显式focus85→84，系统前台元组独立回读；Computer Use在84发一次super+Tab后只把应用内焦点Colors→Fonts，原生前台仍84。该调用不等于系统Command+Tab，不能充当执行中切窗/真实人类来源的验收。
- RWI对后台85一次5秒拖拽返回，独立receiver收到1down/77move/1up、completed1、orphan0；当时系统前台为另一个应用，未因这次拖拽切到85。实际截图前后空白→蓝线，不把receipt当应用效果或把77强称79。没有操作游戏/已有文档输入。
- 85具名Close window一次AXPress，process0/closed/lifetime expiry0。84此后系统Window sharing控件替代交通灯；RWI与CUA都观察不到Close，缺失不等于RWI漏名。清理用途的显式focus84遇到recent/unknown HID `user_active`，dispatch前拒绝；不禁用yield、不重放、不用其他通道绕过。84随后由600s硬寿命关闭，实际process0/lifetime_close_requests1；这不计为RWI关闭控件成功。两个自有PID均已独立discover0窗并detach，其他应用不动。没有让用户配合，也没有伪造物理人类证据。

完整后台/前台矩阵、formal Host强任务绑定、foreign-PID窗口族、hidden/minimized/offscreen及系统跨应用DnD仍按原计划partial/unsupported保留。用户现在无需配合；源码/UI协议通过不等同整计划封版。

## 2026-10-11 · 当前：macOS／Windows 真实 CI 全部通过

已确认源码head `e0afa0f2dbfb4f0b5685d8bc68be77dcc6cc08ab` 的 [真实CI](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38117287848)：macOS与Windows两项job均success，Format、默认tests、all-feature compile/lint、architecture、architecture负例全部完成0。此前7次实际失败、监听unexpected EOF及每次最小修正都保留，未移除测试/改SHA/放宽规则。当前本机Rust736/0/11/42、独立selector4、architecture10；各层范围分开，不把CI等同native GUI。下面旧“pending”段落为历史步骤，已由这次实际结果替代。

本记录只补充文档，不修改已验源码/fixture/profile/安装组件。320点宽原生Live、隐藏/恢复/关闭与具名系统窗口按钮的已观察结果保留；formal Host渲染／强任务绑定、真实人工来源、其他native窗口状态和future矩阵仍有未验收项，不作整计划封版。用户不需要现在切窗/展开/授权/重启。

## 2026-10-11 · R43：Windows 工程检查显式 UTF‑8

第七次真实远程 run [38116812011](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38116812011)：Mac全部成功；Windows默认tests及all-feature compile/lint均成功，随后architecture检查按系统CP1252默认编码读取UTF‑8 Rust源码，在0x9d处UnicodeDecodeError。该阶段不是Rust编译/GUI失败。

只对workspace/package Cargo.toml、Rust源码、rules JSON四处read_text显式 `encoding="utf-8"`。本机以CP1252默认decoder shim跑真实完整architecture CLI，baseline同codec/0x9d错误，modified0；独立copy可执行rollback原2hash→1→reapply0。原rules/阈值/12debt不变；不忽略、不替换非法字节。新增中文/emoji源码与manifest跨ANSI默认正例、非法UTF‑8仍拒绝负例，architecture10tests0（原8保留），真实audit0/12debt。

Rust源码、固定fixture原字节及本机运行Agent/MCP未改，R42已完成fmt/workspace736/all-target-all-featureClippy三门禁按相同源码保留，不冒称这次重跑。没有把UTF‑8 shim当Windows原生job；新head仍需真实CI。仍不把工程检查通过当Live Host强任务绑定/完整native矩阵通过，用户无需配合。

## 2026-10-11 · R42：旧 Windows 诊断 CLI 接上严格进程出生校验

第六次真实远程 run [38116056021](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38116056021)：Mac成功，Windows默认unit tests已通过（R41证据换行问题消失）；all-feature compile实际暴露旧rwi-native-agent引用不存在的 `rwi_windows::process_inventory::identity`。

没有新造一个宽泛PID查询/后端bridge或退回当前进程；使用已经实现的 `agent_binding::inspect_process(pid, expected_created)`，要求 `--inspect-process PID CREATION_FILETIME`，正数/整数/原生宽度严格解析，PID-only与错误模式不fallback。现有API保留handle并检查birth、alive、image、token/privilege前后相同；只读，无launch/input/权限变更。源码搜索没有旧命令消费者，usage已一并补齐。

真实Windows rustc对当前源码call expression、真实rwi-windows metadata执行同输入BASELINE1→MODIFIED0→独立copy可执行ROLLBACK1/hash恢复→REAPPLIED0。首probe少了host proc-macro依赖目录的E0463保留；校正-L后真正复现remote E0433，而非把任意失败当baseline。此probe不等于完整Windows native-link build/API执行或GUI验收。新增selector4tests0；fresh fmt/workspace test/all-feature all-targetClippy三强制门禁0、workspace736/0/11/42；不把额外4tests冒加到默认workspace总数。其他未改源码的R40证据保留，新head必须真实CI。Mac Agent/MCP、CA/profile/授权不动，无人工配合或旧输入重放。

## 2026-10-11 · R41：固定证据原始字节跨平台 checkout

第五次真实远程 run [38115460661](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38115460661) 的 Mac job 成功；Windows进入unit tests（此前编译错误已消失），rwi-windows91passed/7failed。7项均在固定r5证据PREFLIGHT原字节SHA处失败，实际eaf690…而expected76c36b…。

本机用隔离Git index/worktree、真实 `core.autocrlf=true checkout-index` 精确复现同一eaf690…，而仓库blob/source的SHA仍为76c36b…。新增 `.gitattributes` 只为 `r5-preflight.json` 与 `r5-launch-grant.json` 设 `-text`，保证checkout原始字节。两份JSON、原SHA断言、7项安全负例都未改；不做运行时/测试内换行归一化、不接受另一SHA、不跳过测试。

同命令Git检查BASELINE1→MODIFIED0→独立copy可执行ROLLBACK1（恢复.gitattributes原本不存在）→REAPPLIED0；两fixture原hash始终一致。新的fmt/workspace test/all-target all-featureClippy三强制门禁0、workspace736/0/11/42；R40另外五项源码未变的已完成门禁保留，不虚构重跑。此前Windows package Clippy0仍仅compile/lint范围，不冒称Windows GUI或整任务完成。原生Agent/MCP、CA/profile/权限均不动；新head仍需真实remote CI，用户无需配合。

## 2026-10-11 · R40：实际 Windows 编译分支补齐

第四次真实远程 run [38114544302](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38114544302) 的 Mac job 成功，Windows实际报4处 `crate::input_trajectory` 找不到。watch 曾因 GitHub API unexpected EOF 返回1；之后 run readback确认failure、failed-log取回0，区分网络监听错误与真实Windows编译错误。

本机Windows target同命令复现101；四处引用改为已有 `rwi_core::input_trajectory`。再次编译实际暴露UIA Reader漏填共享Element的role/enabled/actions（E0063），原错误保留。现只复用CachedControlType/CachedIsEnabled读回；role保留literal UIAControlType:id，actions为空表示没有已知pattern，不能从control type猜授权或能力。没有新增UIA调用、改变native gesture执行/lease/no-replay，也没有增加平台依赖。

实际Windows package all-target检查同输入BASELINE101→MODIFIED0→独立copy可执行ROLLBACK101→REAPPLIED0，三原文件hash恢复。本机Windows package all-target/all-feature Clippy严格0；本机完整八门禁0、workspace736/0/11/42、receiver53/UI7、architecture0/既有12debt、负例8tests0。以上不等于Windows GUI、整workspace Windows原生执行或Live Host渲染已通过；仍要新head真实CI。Mac安装Agent/MCP、CA/profile/权限不动，无人工配合或旧GUI重放。

## 2026-10-11 · R39：默认 MCP 构建排除旧 native pipe 诊断工具

第三次真实远程 run [38113659051](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38113659051)：Mac成功，Windows默认测试实际在 `rwi-pipe-client.rs:16` 因未启用的 `rwi_windows` 报 E0433。该CLI属于旧 local-worker诊断，现明确 `required-features=["legacy-local"]`；Windows/MSVC build script只在同一feature开启时给它发linker指令，默认MCP仍保留asInvoker/uiAccess=false。没有把Windows/macOS后端依赖加回默认网关，也没有修改native pipe执行体、丢弃测试或权限校验。

实际Cargo metadata＋编译真实build.rs（Windows/MSVC环境输入）同命令：BASELINE1、MODIFIED0、独立copy可执行ROLLBACK1、REAPPLIED0；两个原hash与新测试路径原本不存在的状态恢复。此项是构建配置/指令验收，不假称Windows链接或GUI。新增Rust回归已通过；修改后八门禁全部0、workspace736/0/11/42、receiver53、UI7、architecture0/既有12debt、负例8tests0。

新源码与安装版的实际独立MCP进程initialize／41工具完整schema、description、resources逐项相等；profile及安装二进制未变。源ad0ce…与安装4a29…不同hash不等于工具语义不同，更不等于本会话Host已刷新。实际协议说明限定“schema支持时才传include_timings”；当前Host缓存的旧泛化说明不是增加参数的依据。仅构建目标修正不部署/重启Agent或Codex，不请用户配合。先前成功的提交保留；本次新head还需真实Mac/Windows CI，不将本机通过代替远程通过。Live正式Host挂载/强任务绑定和其余native矩阵仍如实保留未完成项。

## 2026-10-11 · R38 第二轮 CI 修正：平台例程与 Unix 私有目录

远程 run [38112670187](https://github.com/DestroyedTeam/RealWorldInteraction/actions/runs/38112670187) 的 macOS job 成功，但 Windows 默认 workspace 测试实际又报4个 `coords.rs` 编译错误，未以第一轮 lib.rs 修正冒称全 CI 已通过。现对 coords 的 Apple API main 补 macOS gate，非 macOS 明确返回 unsupported；receiver Store::create 仅 Unix 可变 DirBuilder，保留0700与新建目录语义，不用 lint allow 掩盖 Windows unused_mut。

同一 Windows all-target/all-feature Clippy：BASELINE101、MODIFIED0、独立副本可执行 ROLLBACK101、REAPPLIED0，两文件 baseline hash 精确恢复。修改后本机八门禁全部0、workspace735/0/11/42、receiver53/UI7、architecture0（既有12debt未清除）、负例8tests0。追加的全 workspace Windows cross-Clippy 在本机因 ring C 构建缺少 Windows `assert.h` 实际101；此结果保留，不排除依赖或据此宣称 Windows 原生通过，等待新 head 的真实远程 Windows job。

Mac Agent 不受这两处例程平台编译修正影响，未重启/部署、不请求人工。320点宽 Live 及其已观察隐藏/恢复/关闭证据保留；正式 Host 挂载/强任务绑定仍未验收。重新打开的 [official Computer Use documentation](https://learn.chatgpt.com/docs/computer-use) 未给出浮层精确尺寸/按钮视觉规范，不宣称逐像素复刻。

### R38 本机修正门禁完成，等待新 head 的 Windows CI

三项macOS cfg修正后的完整八门禁均0，workspace735/0/11/42、receiver53/UI7；architecture0/既有12debt、负例8tests0。Windows target cross-check同输入101→0→rollback101→reapply0，原文件hash恢复，仅作用于独立验证归档。没有以本机cross-check代替Windows job执行，也没有部署/重启不受此修改影响的Mac Agent。

## 2026-10-11 · R38：正式 Live 未挂载与 Windows CI 实际错误修正

- 延续而非重做：R36/R37实机与10条已推送commit、PUBLIC仓库/标准PR1均保留。
- 原82启动确实创建了fixture prepared，但之后两个精确PID已不存在（ps exit1、discovery0窗），不虚构原生exit/cause。新83经tracked exec运行；首命令相对state-dir的实际exit1保留，改绝对路径后真实窗口在virtual4后台出现。
- loaded `computer.show_live` 真协商MCP Apps resource_uri、无browser URL；本轮全文结果也已回读。MCP Apps sidepanel列表仍为空、native frames0/hidden，最终duration_expired。仅接受协议/失去presence后的清图，不接受Host渲染或强任务绑定；不绕过Codex原生访问限制、不伪造UI-only事件、不要求人工，也不以兼容浏览器替代这项。
- 83首次只读snapshot AXWindows/-25204（inventory retry也失败）保留；独立新read成功后只发一次具名Close window，originalterminal/resources_released、detach、pointerhidden、process exit0/nonexpiry。没有重放输入。
- 真实GitHub run38111301651（head72bcf2c…）macOS成功、Windows默认workspace测试失败，26个编译错误来自rwi-macos的background、overlay、NativeMacDesktop reexport没有macOS cfg，而FFI与依赖已有平台gate；并非继续把失败归因私有账单。
- 最小产品修正仅在上述三项补 `#[cfg(target_os = "macos")]`。不移除测试、不增加Windows的Apple依赖、不改变Mac输入/权限/lease行为。已用现有Windows target与cached dependencies本机重现baseline101；独立Git验证归档同命令baseline101→modified0→可执行独立copy rollback101→reapply0，原lib.rs hash恢复。本机cross-check不是Windows执行/桌面验收，最终远程Windows job仍需新head实跑。
- 官方UI参考只作为协议事实： https://developers.openai.com/plugins/deploy/troubleshooting 、 https://developers.openai.com/plugins/reference 。文档中的session correlation不是当前任务可见性的证明，不把同MCP连接当active session。

## 2026-10-11 · 标准 PR 已推送，仓库已公开

- 9条合理拆分的commit已正常推送，源码HEAD `8103793c5f4563ecc6b62a95bb5ac92dbc73912a` 与GitHub PR1回读一致；PR为OPEN/MERGEABLE，未merge、未force-push、未tag/release。
- 公开仓库：https://github.com/DestroyedTeam/RealWorldInteraction ；标准PR：https://github.com/DestroyedTeam/RealWorldInteraction/pull/1 。旧截图/历史沿用用户授权；本批只提交269个明确源码/文档路径，不提交raw PNG、运行时数据、本机设置或凭据。
- 当前/README敏感扫描0；包含9条新commit的完整可达历史扫描0。8个独立源码树check均0；当前完整回归735/0/11/42、receiver53/UI7、fmt/Clippy/release门禁0。两处既有文档EOF空行导致的diff-check2及归档遗漏SDK的编译101均保留，修正后通过，不重放先前成功提交或GUI输入。
- R36/R37稳定签名部署与两组实机结果已验证：320×303点浮层、隐藏/恢复/关闭；Close/Minimize/Zoom明确名称及具名关闭；完整signed.app独立copy rollback恢复原hash。用户不需要今晚配合。
- **未封版的剩余项**：实际MCP Apps Host渲染/强任务ID绑定、物理人类来源切窗、foreign-PID窗口族、hidden/minimized/offscreen及系统DnD等继续按partial/unsupported标记；既有12项architecture debt未消失。公开与PR交付不等于所有能力均已验收。

### PR 与开源准备进展（本轮实际回读）

GitHub 已回读 `DestroyedTeam/RealWorldInteraction` 为 PUBLIC；旧截图与历史公开范围沿用用户确认。8个不可变源码Git树（功能基础、MCP两次拆分、Core拆分、macOS拆分、精确窗口/Live窗口族、320点浮层、窗口控件命名）分别离线check编译0。首次验证归档遗漏JavaScript SDK的真实101已保留；修正归档输入后全部通过，未改产品源码或重建Git树。标准PR1仍使用既有分支；提交/推送结果以之后GitHub回读为准。

## 2026-10-11 · R36/R37：自主缩小 Live、补齐窗口控件名

- 用户休息期间不再要求切窗、展开、密码、授权或重启。只操作虚拟屏自有 Receiver80/81 与 RWI 预览。
- `panel_size` 宽度上限360→320点；同输入 Rust 探针360×335.709→320×302.255→可执行独立copy回滚360×335.709→重新应用320×302.255，状态1/0/1/0，原hash精确恢复。capture max_edge2048不变；真实浮层WindowServer读回320×303点、源1320×1104像素。
- 原生隐藏按钮一次AXPress后独立回读hidden/清图；API恢复重新可见；关闭按钮一次AXPress后stopped/requested、worker_done、清图、CG窗口消失。两项self-AX原receipt仍uncertain/-25200，保留且不重放。源窗口未被Live关闭；另行显式关闭Fixture80，exit0/expiry0。兼容页真实挂载，不冒称MCP Apps强任务绑定。
- 发现普通窗口三个交通灯控件name为空。`window_control_name` 仅对空标题/说明且精确AXButton+标准AXSubrole补Close/Minimize/Zoom/Full-screen名称；不覆盖提供者名称，不猜未知role/subrole，不新增AX调用或改变输入权限。Fixture81实际三名称均返回，通过具名Close window单次AXPress，独立closed、exit0/expiry0。
- 两轮八门禁各0；当前workspace735/0/11/42、receiver53/UI7。architecture门禁0，但12项既有debt仍在，未改阈值。R37完整签名.app独立copy baseline/modified/rollback/reapplied同DR均0、全baseline文件map恢复；生产未回滚。
- 两次Agent-only部署复用稳定证书/DR；当前PID25946/instance2c6f4d9d…，TCC四项true/requests_permission=false。CA/profile/config/MCP4a29不变，不需要人工重新授权或Codex重启。
- Apple Panels指导保留轻量浮层与明确动作、不把隐藏混同最小化。公开Computer Use文档没有给出精确像素尺寸/完整按钮视觉规范，不声称逐像素复制。参考：https://developer.apple.com/design/human-interface-guidelines/panels 。
- R35当前五目录/README/完整历史实际敏感扫描均0，无allowlist；标准PR拆分正在以不可变Git树离线验证。源码根未回退、没有新开发副本、没有Raw PNG/runtime/local设置加入提交。完整Host任务绑定、foreign-PID Live族、隐藏/最小化及系统DnD等既有缺口继续明确保留，不作全计划封版。

本轮原始工具PNG（未编辑、不入Git）：
1. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-compact-live/native-320-point-preview.png`
2. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-compact-live/02-named-window-controls.png`

证据：`work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/compact-live-r36/NATIVE_ACCEPTANCE_RESULT_R36.json`、`window-control-labels-r37/NATIVE_ACCEPTANCE_RESULT_R37.json`（本机验证产物，不承诺公共仓库可访问）。

# 完整实施计划：当前状态

## 2026-10-11 · R34：无正别名的慢AX窗口列表只读恢复与共享面板

- 新真实baseline77在后台两次snapshot均AXWindows/-25204，三个别名路径无精确候选；没有发送颜色点击或任何input，独立1320×1104 SCK图无输入授权。fixture正常expiry退出0、sequence2，不能冒称显式close0；错误handler误取elements/错断言sequence0已保留并更正。
- `resolve_window_element_traced` 仅对精确AXWindows只读messaging失败且aliases0，新建独立applicationRef做一次250ms窗口list读取，候选读50ms/遍历budget500ms，必须native type/PID/CGID/role/rect、CFEqual去重、单候选。原错误保留、不放宽geometry、不选第一条、不激活、不重放input。其他拒绝/写操作/正或歧义aliases不走该支路。SDK原文确认timeout只作用该AX对象，不影响相等其他对象；官方web原文需JS/markdown404保留。
- 完整8门禁0、workspace733/0/11/42、receiver53/UI7，architecture0但12debt未消失。稳定签名部署，CA/profile/config/MCP4a29未变、TCC4true/requestsfalse。
- 新78父snapshot用原AXMainWindow路径，**不冒称初始77条件已原样复现**；真实Colors、Fonts两个窗口snapshot确实走新增bounded_exact_AXWindows_read_after_alias_miss，返回各自精确CGID与控件。两者各open_count1，独立虚拟屏；ColorSliders AXPress后原PNG真变为灰阶滑块。Fonts→Typography弹出不同CG窗口落到内建屏；只操作自有窗口，place_window无activation移回virtual4，[1824,24,340,409]与新CGcatalog一致。它们不是AXSheet/没有父文档attached关系，不能据samePID自动合并Live或借图授权另窗。
- 78正常expiry0/1；晚close3次发现process_identity_unknown，未发input，不能计通过。随后新79相同当前candidate明确open两panel、分别AXclose，catalog只剩parent，parent再显式关闭0/expiry0。78/79合计10originals全terminal/released、targets退休、pointer隐藏；不重放旧请求。
- 正式loaded `computer.show_live` 实际协商MCP Apps resource_uri，不依赖Browser页；当前组件未展开（apps.tabs空）、frame0/悬浮层不可见，bounded session最终duration_expired。这里只接受协议和fail-hidden，不接受真实Host渲染/任务切换；不要求人工、不伪造UI-onlypresence、不绕过Codex原生访问限制。
- 同输入source1/0/1/reapply0（代码契约，不是GUI）、完整独立signed.app同DR0/0/0/reapply0；原hash精确恢复，生产不回滚。延展同四角色141/初始97不变，整体goal仍未完、未commit/push/public。

原始PNG按顺序保存（无编辑、不入Git）：
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-shared-system-panels/01-baseline77-visual-no-input-authority.png`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-shared-system-panels/02-colors-wheel.png`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-shared-system-panels/03-fonts-panel.png`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-shared-system-panels/04-colors-sliders-actual-change.png`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-shared-system-panels/05-typography-grounded-before-move.png`

证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/shared-panel-family-r34/NATIVE_ACCEPTANCE_RESULT_R34.json`。


## 2026-10-11 · R32/R33：所属 Sheet 双层 Live 与慢 AX 降压实机闭环

- 不再要求人工切窗/展开/重启。本轮输入只来自已加载 RWI，只操作虚拟屏上的独立 Receiver73/74/75/76和 RWI 自己的预览；未操作游戏、飞书、Chrome已有页面或用户文档。
- `NativeMac::live_visual_capture` 只在 Live 合成经过原生type/PID/CGID/role/geometry及AXParent校验的同提供者所属Sheet；普通capture_visual仍为独立窗口、不给合成画面输入frame。真实PIP图像已同时包含文档和Sheet，Sheet关闭后回到文档；不是仅凭layers元数据。未知/外部PID和其他同PID窗口不自动纳入，partial图不证明不存在。
- R32原生71显式关闭，但输出盘满，真实exit1/ENOSPC保留；只清理本项目再生incremental并在Cargo锁下等清理完成。原清理/编译竞态exit1/101、首轮Clippy101均保留。72模板误追加重复参数，真实exit1；73移除重复参数的新运行exit0。所有旧mutation保留，不重放。
- R32慢AX下Live运行时新动作读取失败；停Live后独立新动作成功。R33用一个worker-local retained exact root，逐帧前后重新校验全部原生事实和所属图，失败inventory恢复有1秒节流，不缓存输入权限/不去除身份或模态检查。完整八门禁0，workspace732pass/0fail/11ignored/42suites；receiver53、UI7。
- R33真实74前台Live运行期间safe0→1，父safe0且模态路径predispatch拒绝；76原生safe1独立回读AppFrontmost=false、parent/sheetWindowKey=false，frontPID69309(own75)，双层Live同时更新。这里不宣称每次回调中有WindowKey记录或连续物理人类切窗。75focus读回already_foreground/native_requests0，并非实际完成机器切换；最初加错include_timings的schema错误已保留。
- 74共663帧、初始root relay1次，复验本作用域下不再逐帧调用应用窗口inventory；176ms、377ms是实测两个capture样本，不外推通用性能/因果或跨应用正确率。双层普通源仍1320×1104，浮层原生360×336/位于内建屏；原420×386→360×336缩小证据沿用R18。
- 原生隐藏/恢复/关闭均实读：隐藏清图；resume重新可见；Close停止requested、worker_done、无pending、清图且WindowServer不存在；不关闭源。Hide/Close的selfAX原receipt仍uncertain/-25200，各仅一次，独立goal成立不改写ACK。74/75/76随后分别显式关闭exit0/expiry0，13originals全terminal/released，8targets退休、两次pointer隐藏。
- R33已部署signedad99426c…/PID61073/bb385d4f…，沿用相同证书DR；CA/profile/config/MCP4a29未换，四项权限true/requests_permission=false。无需人工重新授权或Codex重启。
- 规范调研：Apple浮层参考 https://developer.apple.com/design/human-interface-guidelines/panels （轻量、简短标题、不把隐藏当最小化）；本轮web正文需JS，独立浏览器两次超时，公开doc JSON的证书验证失败亦保留，未禁用TLS校验。OpenAI官方 https://learn.chatgpt.com/docs/computer-use 已实际打开，未给出Live具体像素尺寸/按钮样式，所以不虚构逐像素对齐。只保留本机真正见到的两个AX按钮：隐藏预览/关闭预览。
- 当前F12仍partial：不同PID系统窗口族、多个源层级、MCP Apps真实渲染及强Host任务ID绑定尚未整体接受；public own-task navigation只返回navigated/queued，不当实际渲染证明，也不绕过Codex原生访问限制。F13真实DnD、F14独立浏览器生命周期及完整A01–A07/F01–F16继续保留，未commit/push/public。

时间顺序真实原PNG（未编辑/不入Git）：
1. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/01-parent-baseline.png`：独立父窗口。
2. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/02-owned-sheet-safe0.png`、`/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/03-exact-parent-excludes-sheet.png`：所属Sheet及单窗截图边界。
3. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/04-owned-sheet-safe1.png`：原生Sheet计数1。
4. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/05-native-live-parent-and-sheet.png`：R33真实PIP双层。
5. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/06-native-live-sheet-removed.png`：关闭Sheet后PIP只有文档。
6. `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-owned-sheet-family/07-true-background-live-and-sheet.png`：真正后台Sheet计数1、两个原生预览按钮。

所有脚本、原stdout/stderr、exit、hash及同输入rollback进入原四角色（延展至141条，初始97 baseline值不变），不新建产品开发副本。原证据索引：/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-owner-pressure-r33/NATIVE_ACCEPTANCE_RESULT_R33.json


## 2026-10-11 · 当前 R27/R28/R29：核心与原生职责拆分、稳定签名部署及后台/模态实机复验

- 当前：同一RWI根 / Core197、macOS159 focused通过；当前Rust全workspace722/0/11/42、fmt、all-target/all-featureClippy、receiver51、UI7、MCP/Agent release八项0 / 继续AXWindows偶发失败、前台干扰/Live窗口族及完整A01–A07/F01–F16，不作整计划封版。
- R27按原Body拆Engine lifecycle/observation/mutation、普通与红点视觉锚点/测试；R28按原Body拆desktop/native Backend、Live worker/panel、AX窗口/模态/纯几何/测试。10个责任超限消除，未调整600或既有ceiling；只修模块移动后的private访问/附着doc/test import，实际101/1错误及改正命令全部保留。两轮大小/责任同输入1→0→可执行独立copy rollback原5hash/新10或7路径消失→1→reapply0，不当GUI效果证明。
- 真实cargo tree默认MCP没有macOS/Windows/Agent/pointer平台包；pointer生产仅serde、imaging仅dev。旧baseline漏了已经存在的纯呈现pointer包，先保留3项失败再登记精确Mac/Windows→pointer及成员，增加外部/optional/target/build/alias/本包build-script拒绝门禁；8个工程正/负例0。真实ratchet19→18→13→8→3→0，但仍architecture_complete=false/既有12项debt。其他边与每项文件阈值不变，不是放宽生产输入边界；见ADR0004。
- Agent-only部署沿用已成功R18控制器、同本地证书/DR与CA/profile，cooperative lease排他、全部当前durable资源终态核对、精确旧PID退出、原子交换与回滚保护。实际新PID74849/instance6290541c…、signed02daaea5…；loaded typed RWI已看到新实例，TCC四项仍true/requests_permission=false。安装MCP4a29未换/Host未重启；新compiledMCPad0ce与4a29真实进程41工具initialize/完整schema/说明/resources一致，不冒称新Host formatter。
- Own Receiver66/67只在虚拟4：66后台AXSafe0→1、App inactive/keyfalse；随后原5sdrag在AXWindows/-25204读取阶段拒绝，原生down/move/up全0、原IDquery终态保留/no replay。独立同窗SCK1320×1104成功，无frame/inputauthority，不能以图像授权输入。错误不归咎本次pure move，R19已有同类证据。
- 67新的独立1s蓝线任务实收down1/move16/up1，18个回调AppActive=false/key=false/systemFrontmost=false，完整蓝线与callback位置/配对事件号确认，无orphan/无重放；不等于66的错误已修、所有framework皆支持后台。
- 67真实document sheet：open1、父旧Safe proposal因blocked/context变化0dispatch拒绝；独立CG37253 child初始0→AXSafe1→close1、visible/attached false，父Safe仍0。两own primary使用fresh observed AX close显式关闭/exit0/非timer expiry，3detach、pointerhidden，11原request全部terminal/released/no pending/replayfalse。
- Foreground新实测未通过：一项oldref stale前置拒绝；一次全新snapshot/ref/tuple仍因近期HID输入user_active拒绝。只证明退让/0dispatch，不证明人类来源、不能删除yield或借ComputerUse绕过。R14/R19/R21原成功状态与系统shortcut证据保留；本轮没有Live start或physical human/执行中CmdTab正例。
- 按实际时间顺序保留6张原PNG：baseline、66Safe1、66AX失效后的只读SCK、67后台蓝线、sheet0、sheet1；未把appkit底左frame113与CG顶左frame504当窗口移动，输入采用snapshot显式0.5仿射映射。源截图均own fixture，无用户应用或主屏截图。
- 原97 baseline逐项保留，新增17生成模块及工程规则/ADR扩展同四角色137条目。累计default rollback/patch/reapply/hash/parse/reopen继续核验；R30只准备精确AXFocusedWindow/MainWindow读取候选（严格type/PID/CGID/role/rect），未产品写入/未native验证、不作已修复陈述。用户无需配合。Repo仍无新commit/push/public，PR/发布目标不缩水。


## 2026-10-11 · 当前 R25/R26：MCP 职责拆分与真实协议等价通过，架构剩余13项

- 当前：同一产品根 / R25/R26 纯模块提取已写入，实际41工具的 initialize、完整说明、schema、资源清单与已安装4a29版本逐项一致 / 继续核心与原生架构债务、全部A01–A07/F01–F16；未新部署、提交、推送或开源。
- R25 convenience 2191行 → 188行入口及12个责任文件，全部≤600；原45 focused与2个真正MCP process正/poison测试通过。7门禁0、workspace722/0/11/42、receiver51/UI7。R26把投影测试、整数字符串边界、执行摘要、图像映射和worker测试分开，5个超限文件均≤600；架构检查仍exit1，但实际19→18→13项，未改规则或调高阈值。
- R26全workspace722通过后，Clippy真实因一个重复test-only导入exit101；原记录保留，仅删除重复导入，最终当前MCP110pass/1ignored、all-target/all-feature Clippy、fmt、fixture51、UI7及release均0。保留已完成前两门禁；不宣称在该import删除后又全跑了一次workspace，也不放宽-D warnings。
- 两轮同输入责任/大小契约均BASELINE1→MODIFIED0→可执行独立副本ROLLBACK精确原hash/新增路径消失→1→reapply0；这是大小契约，不是GUI验收。R25真实进程candidate9fcf与安装4a29完全等价；R26最终candidateb44ee同样等价。原输入、租约、窗口/设备身份、诊断、坐标、按钮、分页与no-replay策略保留。
- 原97 baseline逐entry保持、README第98原件和R25新增12不重置；R26首次管理trace原件加5个新增文件，同四角色为116管理路径。累计patch/default rollback/reapply全部hash/absences和原生360尺寸契约继续核验；此前失败的边界/导入/格式命令都保留，不包装成产品原生故障。
- 用户无需再配合。本轮只读loaded RWI仍设备available、仅desktop目标，不发送输入或截图。Live先前R18小浮层360×336/原图1320×1104与隐藏/恢复/关闭证据保留，不把这些纯代码移动说成新的GUI实测。真正Host任务绑定、composite window family和执行中独立系统键盘等未覆盖项仍公开记录。
- 已准备R27核心Engine/像素锚点/测试职责提取runner，只有parse与当前源码hash检查，尚未执行，不计完成。pointer依赖/旧workspace基线须依据ADR0002和真实依赖图评审，不直接修改allowlist骗绿。


## 2026-10-11 · 当前 R23/R24：开源预检有结果，架构门禁真实失败，切窗actor负例保留

- 当前：同一 RWI 根 / Gitleaks全部231提交及当前源码目录0，72初筛逐项排除；SPDX许可证0/2warnings、JS client50pass；README已对齐实际41/42工具、Rust/QuickJS与Native Live / 继续19条架构问题、完整原生验收和PR/多commit，未开源或推送。
- 初筛1741历史blob，128 binary与3 overbudget不做四regex秘密不存在证明。实际Gitleaks默认规则、无baseline/allowlist/忽略注释的全history0；crates/integrations/architecture/tools/docs及README均0。两条只有header的非法PEM测试字符串无密钥正文；70条为Microsoft Learn ask-learn属性名子串，均逐条检查。未读取生产密钥、未调整原GPL-3.0-only；license inventory通过不作法律保证。
- 额外执行真正architecture checker：exit1，19项，不包装成通过。包含convenience2191>600等文件责任边界、旧baseline membership以及pointer依赖规则问题；应评审和重构，不调高阈值骗绿。已准备最大convenience纯模块提取runner，未执行，不计作实现完成。
- R24原本准备“RWI拖拽期间独立系统Cmd+Tab”actor。Computer Use仅选虚拟4自有Receiver64：super+Tab返回但native前台仍64，焦点Colors→Fonts；新Mac literal CMD+TAB报keyNotFound CMD。这些不是全局切窗成功，也不是RWI拖拽故障；未发RWI drag、不试第三种猜测键名、不重放任何RWI请求、不碰Codex native denial/用户应用。
- Computer Use自己的Window sharing菜单实际提供Close Window；只关闭own64，native状态closed/exit0/non-expiry。own65通过RWI fresh observed AX close也closed0；3原RWI请求terminal/released/no pending/replayfalse、2detach/反馈指针hidden。对照PNG按实际调用顺序保存，不把下降采样预览说成native full-original。实际during-input系统keyboard/物理人类覆盖仍未证明；R14/R19原生状态转换与独立全局shortcut结果保留。
- README旧的20/21工具、JS未部署、plugin未安装陈述已更正，明确开发候选、未知范围与全部未完成项；同输入1→0→可执行rollback原READMEhash→1→reapply0。原97 baseline每个entry不变，第一次管理README为新增第98路径，同四角色扩展而不重置原件。


## 2026-10-11 · 已验证阶段 R20/R21：只读截图降级已部署；存活窗口身份、租约边界自主验收

- 当前：同一 RWI 产品根 / workspace **722/0/11/42**、receiver51/UI7、七项门禁0；MCP **4a29eebf…** 已安装 / 继续全 A01–A07/F01–F16、分层延迟与窗口族，非整计划封版。
- 用户休息后不再提人工配合/重启问题。Live 的 R18 原生实测仍有效：同源 **420×386 → 360×336**，面积减少25.39%，捕获1320×1104未缩；隐藏 eye.slash、关闭 xmark、暂停恢复 play，28点命中区域与中文无障碍标签保留。公开资料参照不等于 Codex 私有界面1:1或公开精确尺寸。
- R20 只改 MCP 的 snapshot 错误分支与测试：明确 AXWindows/-25204/read、原读取终态/资源释放/无 pending/无清理不确定/无输入派发时，才单独尝试一次当前绑定窗口的 capture_visual。原 AX 错误和 isError=true 保留；控件数未知，不说不存在；frame/view/mapping 为空，没有点击权限、自动聚焦、重绑或重放。视觉错误/悬挂/错误绑定附带的媒体不会泄漏。
- 实际 MCP 进程 + 认证合成 peer 的两项测试0，正例1张fixture PNG、poison0张；这是协议/输出策略，不是原生 GUI 图像验收。匹配源行为 **101 → 0 → 可执行ROLLBACK原2hash → 101 → reapply0**。最初mock分支漏匹配导致101已修复，原失败保留。七门禁0；新旧/回滚副本/重应用进程启动均0、41工具、只读targets健康。
- R22只读实际新MCP进程验收41工具：原工具说明/输入schema冲突0；initialize已明确“仅schema暴露时加include_timings”。向targets错误加该字段实际拒绝invalid_arguments，进程0/no input/capture。这是已修正新版本的确认，不再针对旧Host注入文案重复改源码；不把旧会话说明当新安装事实。
- Agent仍67302/ad37/5fe8…，同DR/证书/CA/配对/权限；安装未重启当前父MCP，因此本会话仍旧formatter，不能冒称Host已使用R20截图降级，也不再要求用户重启。
- R21 只用loaded typed RWI操作虚拟4的Receiver58/59：两个窗口仍存活时，先58→59实读切窗，再用旧58前台tuple请求cmd+tab，原生派发前拒绝foreground_changed，59保持前台；补上R19“旧窗口已过期”造成的隔离缺口。旧focus引用拒绝no-dispatch保留；新鲜独立观察后另立请求正常切窗，不重放旧请求。
- 两种精确租约边界均实际拒绝：desktop租约不能拖拽child，child租约不能发desktop全局键。第一对中的drag已拒绝，cmd+tab仍真实58→59，**不算拖拽期间切窗**。新目标租约上的5秒drag实际1/79/1、81FG回调、原生时间4986.726ms/完整蓝线/无orphan；其间发起的错域chord无native dispatch，controller等待4583ms。等待层未测明，不能归咎某个私有Host组件，也不删除租约或durable reservation绕过。
- 12个原请求都查询到terminal/released/no pending/replayfalse；两自有源明确AX关闭且进程0（非到期），2解绑，反馈指针隐藏，11张实际交付PNG按序保存。R21未启动Live，不把“存活窗口”误说成Live预览/真实人类输入。未新commit/push/public；完整窗口族、framework/stress、审查与发布仍继续。


## 2026-10-11 · R14 无人工配合的执行中后台→前台与真实系统快捷键已验到

- 当前：同一RWI根 / Native48首次down后约1010.843ms真实转前台、完整1/79/1继续；loaded Cmd+Tab48→47，随后48后台1/16/1 / 继续系统浮动/模态窗口与完整A01–A07/F01–F16，非整计划封版。
- 只给own接收fixture增加`Options.self_foreground_on_first_down_after_ms`、State.arm_foreground_on_first_canvas_down与one-shot native common-mode timer；默认off，正常own窗口/正确原生down/有限坐标/Regular policy/100..5000ms严格校验。Cause字段不伪造input或foreground。右侧own actor setup防覆盖，同分配virtual4、不移动用户窗口，无backend或隐藏输入桥。
- 普通public activate场景46：actor请求1但81回调均后台，原生拖拽1/79/1正常；未计作切窗成功。独立明确legacy-own方法的新48：前16回调后台、1条系统前台但AppKit非key中间态、后64 active/key/foreground；完整蓝线/无orphan/释放/无pending/no replay。不是失败后自动fallback，不是人类来源声明。
- 通过loaded computer.hotkey单独发一次cmd+tab，fresh foreground70515→69718、四个paired HID事件、lease已释放；之后new48b背景1000ms一条完整1/16/1，18回调全后台、actor仍仅请求1。真实快捷键发生于第一拖拽结束后，不声称执行中物理键盘覆盖。
- 46/47/48均实际source close/进程0/解绑；无Live start、无伪造presence、无游戏/文档/主屏截图或权限/Agent/MCP部署。原始PNG与所有unique receipts留存；no-ack仍不等于goal。
- 两轮full6gates0，最新workspace702/0/11/42、receiver49/UI7；最终三source同输入101→0→portable rollback原hash→101→reapply0，sole root保留新版。actorCLI旧真实1/unknownflag与右侧operator签名错误保留。大型ledger超argv创建失败改apply_patch写入并reopen，不伪装process exit。
- 同四角色97管理路径，累计diff/rollback/hash/reapply/reopen继续；未新commit/push/public。完整工具与系统窗口族/观察、并发清理/审查/标准PR仍未结束，不再发人工配合问题。


## 2026-10-11 · R13 实际MCP并发与Host侧排队定位

- 当前：同一RWI根 / 真正零设备MCP进程16个并发只读响应0；loaded Native45前台drag1/79/1及清理0 / 继续自主首次down后切窗验收与完整A01–A07/F01–F16，不等人工、不封版。
- 修正operator探针`command[2]`与`process_cwd`：RTK把带空格绝对exe误拆为Library/Application，真实exit1/Permission denied不是TCC。改在原installed bin目录执行`./rwi-mcp`，同输入1→0→便携ROLLBACK原hash恢复1→reapply0；MCP二进制/权限/CA/服务均未变，不能称产品权限修复。
- 无profile零设备真实stdio，8对targets+unknown-target live/status，16个id独立关联且有乱序响应；进程0、no input/capture。首样本p50 0.123ms/p95 0.272ms只属于这个无设备协议探针，不是macOS GUI/公平性能或通用成功率。
- 专用Receiver45虚拟4，native初始化及81回调均frontmost/active true，完整5秒drag1/79/1/原生蓝线；不是BG→FG验收。并行只读JS状态403ms发起/5407ms返回=5004ms，但adapter2.386ms、nested native1.470ms、worker2ms；约5001.6ms不在该测量的Repl/native handler内部。与上游Host/tool调度等待一致，具体私有组件未定位，不贸然重构已快的Rust核心。
- 未启动Live/伪造presence，所以本样本不证明像素实时性；R11用户43的实时观察仍保留。原动作terminal/released/no pending/no replay；关闭首次缺`at.kind`在派发前拒绝0，fresh snapshot后唯一新AXPress有效关闭，进程0、detach，无fallback，目标及源均保持分配虚拟屏。原PNG保留。
- 本轮未改产品Rust/Agent/MCP，不部署/重启/提新授权，不新commit/push/public；沿用R12 workspace702/receiver45/UI7的已注明日期回归，不冒称重跑。四角色扩展一份operator源为97个管理路径，累计重构/回滚/hash/重开留证。后续第一次down触发自身切前台的明确fixture能力当前未实现，已准备真实baseline runner，不能称验收通过。


## 2026-10-11 · R11/R12 执行中Live修复与自主紧凑浮层验收

- 当前：同一RWI产品根 / 新Agent实际运行、紧凑预览420×386及隐藏→恢复→关闭已自动实测 / 继续实际MCP进程排队诊断及完整A01–A07/F01–F16。用户休息后不再要求人工配合；自动测试不冒充真人操作来源。
- Native42用户报告蓝线只在拖拽结束后出现。原生输入已1/79/1消费，但输入lane阻塞Live presence/control。R11增加独立有界`Context.live_gate`和精确目标`LiveController`，只服务既有预览控制；不放开输入lease/authority/replay、TTL仍1500ms。Native43用户确认“5 秒内逐渐伸长”，问题实际修复；同轮并行模型status仍排队4957ms，未宣称所有延迟解决。
- R12 `panel_size`宽高上限420点、chrome48点；`button_symbol`改原生28点圆形minus/X，保留中文AX名称/tooltip，pause保留继续图标。capture max_edge2048未降低。参照公开PiP报告及Apple符号API，没有Codex精确尺寸公开证据，420是RWI选择，不说1:1复刻。
- Native44自主使用真正兼容浏览器可见页，源仍虚拟4、浮层内建1；原生矩形[1356,735,420,386]、原图840×772、两项AX控件确验。隐藏清图、恢复新帧、Close为requested→worker_done→无pending，源窗口仍运行；随后单独关闭自有源、进程0/解绑，临时页已关闭。两次AXPress返回−25200不确定保留，用独立状态接受效果，不重放/不鼠标降级。
- 两次Agent-only稳定证书部署，最终PID94874/21e75450…、signedc2c116f1…；四权限true、原CA/配置/配对/MCP5ca未变，无需本轮重启或授权。Native43B仍全部后台，无实际执行中切到目标前台证据，保留未验收；不再让用户重复确认。
- 同输入Live lane与尺寸各101→0→ROLLBACK原hash/absence→101→reapply0。最终workspace702/0/11/42，独立receiver45/UI7，fmt/all-feature Clippy/release0；同四角色扩展96源，累计patch/rollback/hash/reopen核验另留证。安装是可逆本地开发版，不是UIOS正式发布；既有Windows草案release-check/report均1，原始债务保留。
- 未新commit/push/public；原历史截图公开授权保留，全计划与PR流程未被这一项替代。[按序原图、命令及未完成项](../validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)。



## 2026-10-11 · R9 后台视图mouseMoved及Live隐藏恢复已实测／MCP说明修复部署

- 当前：同一产品根 / native36校准视图在inactive/keyfalse/frontmostfalse真实moved1，用户可见APIhide→resume49→51／内建位置true / 继续完整A01–A07/F01–F16及active Close、任务切换、tracking/tooltip；不是整体验收完成。
- 默认NSButton对照35（inactive）和37（active）均App/window送达、view0。35异常134不伪装closed；public NSEvent8/9非法subtype独立复现134，lazy guarded getter/null修复后0、真实rollback134、reapply0。36/37新binary正常timerclosed/exit0；晚期115/enter1/exit1未归因，不冒称RWI hover成功。
- 用户确认Live可见后真实API隐藏清图、恢复新帧／正确内建屏；Close回复之前duration_expired已发生，保留确认但不接受active Close因果。所有绑定退役／原请求terminal/released/no replay／feedbackfalse。
- 修复真实include_timings说明与schema矛盾，初始化339→302bytes；真实MCP同输入1/0/1/0、原hash恢复；MCP-only安装5ca2d986，fresh HTML6807/42model工具验证。Agent14d4ace3/4544、签名／证书／CA／配置／权限未改；当前Host仍c9，下一次重载才会使用新UI/说明。
- 最终697/0/11/42、fmt/allfeatureClippy/releaseMCP0；receiver41+UI7独立。未新commit/push/public；同四角色91源延续。[按序原图、错误及完整边界](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)。

## 2026-10-10 · R8 画面可见性、重显预算与控制卡修复

- 当前：同一RWI根 / 重启42工具+1资源，用户确认内建屏画面及正确矩形；696/0/11/42、UI7 / MCP23ce已部署、Host实读旧UI，待加载复验独立展开及连续hide/resume/Close，完整计划不封版。
- Native32在233帧placement_unconfirmed，未证实错屏；hide→reshow一次预算、pending不续期、错误分类/缺失证据补齐。Agent14d4ace3/4544稳定证书部署、四权限true。Native33状态验到但取样suspended，视觉恢复仍未通过。
- 卡片声明inline/fullscreen、说明画面位置/Close终态，停止后不再心跳误导。Root/缓存Skill同步；1/0/1/0及原hash恢复。30–33 closed/exit0/detached，不重放；一次hover App/window收到而view/tracking/tooltip0，F05校准继续。
- 源虚拟屏、用户预览独立内建屏；卡片可见/帧数不是视觉成功。全A01–A07/F01–F16保留，未新commit/push/public；原截图/历史公开授权保留。

## 2026-10-10 · R7 显式AX契约／错误分类修复部署，模态及双Live生命周期实机验收

- 当前：同一RWI产品根 / 最终693pass、Native29语义计数及拒绝理由真实验到、稳定证书Agent-only两次部署 / 继续hover及完整A01–A07/F01–F16；Host Live渲染待用户“稍后重启”，不封版。
- act_checked保留显式native action，backend不支持不降级pointer；macOS声明advertised_element_action，仅精确已公布动作可执行。native_action_request严格marked reference与value一致，移除pointer按value提升。原对象／窗口／provider／epoch／权限／租约／yield／no-replay保留；普通pointer仍需可见。
- resolve_action将不公布／禁用从visibility_unknown拆为unsupported_action／element_disabled。8组合pure test和Native29两项错误各自验证无native dispatch；此前“Native27实际鼠标降级”推断已撤回，MAC当时仍调用AX，mouse0、无menu-visible proof。
- 两项同输入source101→0→portable rollback原5hash恢复后101→reapply0；原root保持新版。首个宏recursion编译101真实修正、Native29自引用launcher1与runner错误预期保留，不伪装产品bug。最终fmt/workspace693/0/11/42、all-featureClippy/releaseAgent+MCP0；example38和UI VM5独立。
- 最终Agent36549/4741ed85、signedf7a28ff6…，MCP9f96321b…保持；原cert/DR/publicCA/config/profile/四权限未变，无私钥导出／TCC／Codex重启／旧输入重放。当前loaded40、resources[]；新Agent可由旧typed工具验收，不把未重载UI入口当已显示。
- Native29 own virtual4：父Safe0→1、打开sheet；旧父proposal因blocked_changed安全拒绝且no dispatch。独立child0→1、close1／visiblefalse／attachedfalse；父恢复、counter仍1，mouse0、7原IDterminal/released/nopending/replayfalse。原族fresh重新附着只继续未执行child，不复放父拒绝。全部detach／feedbackfalse／timerclosed／实际exit0。
- 父子Live独立session/presence，停止子后父running，随后均stop/worker done。frames0只通过生命周期，不通过像素／显示器放置／Host任务渲染或隐藏恢复关闭。Native26/28过期产生的late process-identity错误均按真实closed/exit0区分；Native27父已过期不算运行中隔离。
- 91角色重构、默认rollback/hash/absence/reapply/reopen及新源码审计留证；旧截图/历史公开授权继续，PRIVATE/PR1 OPEN仍核验，不在整计划前commit/push/public。[按序原图及边界](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)。

## 2026-10-10 · R6 标准会话内Live已实现并部署 / Left原生消费验收通过

- 当前：同一RWI产品根 / 690pass、原生abcXd、MCP-only原子部署成功 / 重载Host后验证MCP Apps真实渲染与任务切换；其它A01–A07/F01–F16继续，不封版。
- 新可选UI resource/list/read与`computer.show_live` 的 `_meta.ui.resourceUri`，通用`computer.live`控制/状态不重建组件，严格HTML extension协商；no-UI41，UI43=模型42+app-only1。`computer.live_panel`只认适配器内短期view/page/sequence，不能选target、capture或input；闭网络HTML、标准消息桥、单在途/隐藏优先/teardown、close不重放。缺UI可见信号仍隐藏，浏览器仅兼容路径；不是Host任务ID attestation。
- Actual MCP同输入BASELINE1→MODIFIED0→portable rollback原6hash/absences恢复后1→reapply0；首个E0308/101及RTK路径启动1真实错误保留，改BTreeMap/相对可执行参数后才接受语义结果。UI3 Rust+1真实process测试，5 JS VM独立；FINAL9 fmt/lockedoffline workspace690/0/11/42、allfeatureClippy/releaseMCP0，example38独立。
- 安装MCP 9f96321b…（首版8e0792c7…保留），旧进程未kill；Agent9721/69c18019与signed8d90970f…、原证书/CA/profile/config/权限保持。当前loaded40、实际Host资源[]；不拿fresh process43冒充Codex UI已显示。仓库Skill已更新和quick_validate0，安装cache未改。
- Native25仅自有virtual窗口：abcd→一次Left→X，PNG/AX/接收State实际abcXd/changes2，补齐此前caret未验收；4原IDterminal/released/no pending/no replay，detach、closed/exit0、反馈visiblefalse，原用户前台只读回归48574。文字回调active/key/frontmosttrue，明确不是纯后台键盘；无游戏/飞书/Chrome/已有文档输入。
- 原Native24 sheet parent隔离与14/19/21证据保留。已获旧截图及历史公开授权，但PRIVATE/PR1 OPEN仍实际确认；不在全验收前commit/push/public。按序原图、错误及当前边界见[整体验收报告](../validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)。

## 2026-10-10 · Host重启确认 + fresh foreground facts修复部署 + 真正A→B验收

- 当前：同一RWI macOS产品根 / loaded40校验已生效、显式native A→B独立读回通过 / 继续F07 no-raise隔离和F05tracking，不缩减F01–F16完整计划。
- 切窗首次只是already_foreground，第二请求因cached targets旧tuple被正确拒绝；源头是Describe复用owner catalog的易变字段。只对相同identity刷新current_foreground/user_input_recent_seconds，未知null、不等待executor、不改owner/input授权、不放宽foreground/HID/lease/no-replay。
- Agent-only稳定签名原子部署：PID84167/a72260d6-98b1-4f92-82a2-208a006b446a，四权限true，证书/CA/配对/config/profile/MCP b12fa991…保持；当前loaded无需再重启。真实two-app A88597/CG29516→B89640/CG29532，fresh targets与fresh observation相同，明确非应用业务goal/后台no-raise证明。
- 669pass/11ignored/41suites；fmt/lockedoffline/alltargets-allfeaturesClippy/diff/release0。源码同输入BASELINE101→MODIFIED0→独立portable ROLLBACK原hash恢复后101→REAPPLIED0；两种Cargo feature/cache evaluator错误原流保留、独立target修正，产品根稳定。
- F05对照仍未通过：native前台B与后台A的mouseMoved均到App/Window/hit hover_zone，view/tracking/tooltip0；未把receipt等同goal，不归咎于只能后台失败。targets说明误示include_timings但schema不接受，另列待修。
- 本轮四个own场景全部关闭/exit0/detach；最后只余desktop/no fence。无游戏/飞书/Chrome/已有文档/主屏键鼠。8张按序原始PNG与所有错误边界：[验收报告](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-fresh-foreground-facts-and-native-focus.md)。

## 2026-10-10 · 重启40工具已验 + 实机foreground参数漏接修复部署

- 当前：RWI macOS / 用户重启生效、Host40tools，own Receiver focus实际unknown capability / 修正共享catalog::validate并部署，当前Host旧MCP仍需重载，native切窗未通过。
- 定义/handler已有但validator缺focus分支，Gateway RPC前拒绝，不是TCC/用户重启/模型/屏幕问题。新增公布工具-validator完整性、完整/缺字段校验及真实MCP child+mTLS单次转发3项回归，旧版先红101→修正绿0。
- 最终Agent PID92522/instance57ac7afb…、MCP b12fa991…；同稳定证书DR/四权限true/CA配对profile不变，native lease排他后原子部署；无游戏/飞书/Chrome/已有文档/主屏输入，无其他MCP终止或Codex重启。
- workspace666pass/11ignored/41suites，fmt/lockedoffline/all-featureClippy/diff/release0；example27旧独立证据不并入666。源码同输入101→0→真实rollback101→重施0，原hash恢复；原生复制件40工具不变、validator unknown→missing target→unknown→missing target，签名/字节恢复，不是生产回滚。
- Own PID74864/CG28373 focus拒绝前后未变、计数0，原ID/release/query保留不重放；timer真实closed+coverfalse/output_errornull/exit0/detach。按序原图与边界：[本轮报告](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-post-restart-foreground-validator.md)。
- 当前loaded只读空参数probe仍unknown/0mutation，已通知方便时再重启加载本轮新修正，不是假称上次重启失败。F01–F16全计划不缩水，未把tools/list或offline通过当native前台/hover/no-raise通过。

## 2026-10-10 · 新版已部署 + 真实遮挡对照 + 后台声明纠正

- 当前：RWI macOS / Agent两轮稳定签名部署、MCP安装、遮挡原生对照完成 / 同四角色回滚证据整理；前台切窗待用户稍后重启Codex后验收。
- 最终Agent PID91566/instance6e601838…，四项权限true，原证书/CA/配对/profile不变。安装MCP独立只读process实际40tools，当前Host仍39；不能从旧JS绕过或提前把computer.focus_window标为native通过。
- Own virtual display4：普通pointer Safe count0→1但AppKit active/key false→true、Cover在上方true→false，系统前台不变不能作为真后台全条件证明。独立新场景显式AXPress0→1、前后Cover仍在上方、AppKitactive/key/systemfrontmost均false，通过的是标准NSButton语义goal，不是通用covered pointer。
- 新Receiver --cover-controls严格opt-in、public orderedWindows真实采样及关闭后visible检查。已部署background_input声明明确不attest AppKit状态/窗口顺序；没有声称修好了pointer no-raise，准备通知/down/client策略的单因素定位继续。
- 三fixture真实closed/cover_visible=false/output_error=null/exit0且detach。无游戏/飞书/Chrome/已有文档/主屏输入。6张按序原始PNG与逐项问题见[本轮验收报告](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-deployment-and-covered-background.md)。
- 全workspace663pass/11ignored/41suites，fmt/lockedoffline/allfeaturesClippy/diff/release0；最终example27pass单独计数。源码同输入BASELINE101→MODIFIED0→ROLLBACK101→重新应用0；部署复制件只读catalog39→40→39→40及原字节/DR签名已验证，不是生产Agent回滚。
- F01–F16全计划保持all_features_accepted=false。优先F07 key/order隔离和实际Host重载后前台验收、F05hover，再覆盖offscreen/多窗Live/raw/hidden/DnD/browser/取消恢复/资源性能；不将本輪发现替代完整计划。

## 2026-10-09 · 恢复后台 F01–F16 + 标准 receiver 接收验收 + 显式前台候选

- 当前：RWI macOS / 长直线+折线完整drag、越界0派发、Unicode/backspace/native scroll已实机读回 / 前台候选回归通过，待协调共享Agent更新与MCP重载验收。
- 原完整规划不是单个fix：当前16项matrix在 `docs/product/macos-background-acceptance-status.json`，全量 `all_features_accepted=false`。
- Own Receiver01: down2/move46/up2/completed2、无orphan；全部原始回调 OS frontmost非receiver。hover只到App/Window，view/tracking/tooltip0未通过。
- Own Receiver02: 文本RWI-bg-验收-🙂Z→backspace→RWI-bg-验收-🙂；NSScrollView真实clip0→150；AXPress0→1。AXScrollUpByPage -25205却移动至344，uncertain/no replay，目标未通过。
- 不操作游戏/飞书/Chrome；两fixture closed、exit0且detach。没有physical-user输入连续性、covered/offscreen/hidden/minimized的全状态证明。
- 新 computer.focus_window 是显式前台路由，不是后台fallback：AXRaise/AXFrontmost一次、fresh tuple/window ref/lease、durable no replay和alias失效；独立模块及9项新增离线契约。
- 当前全回归 662passed/11ignored/41suites，fmt/all-feature clippy/diff/release0；fixture模型26pass另计。
- 候选MCP实际process tools/list40、exit0；本任务loaded仍39、Agent实例de7cc66c/PID73141未变；前台候选未部署/未native验收。
- 同输入catalog BASELINE101→MODIFIED0→ROLLBACK101→REAPPLIED0；same四角色继续扩展，不换成果路径。
- 报告 `docs/validation/2026-10-09-background-plan-return-and-foreground.md`，含10张按序真实PNG；剩余按 F05、F07/F08/F09、F12、F06/F10/F13/F14、F15/F16继续。

更新：2026-10-09。以已批准 ADR 0002 和 requirements-and-acceptance.md 为准。
用户临时发现的问题只改变优先级；提交、推送、一个成功截图均不是总完成标准。

## 2026-10-09 当前：新city拒绝只读溯源 / 构造理由精确记录已修复 / Agent候选待协调部署

- 读取navigation-post-update-city-minimap-lower-interior-20261009-1原durable，顶层pixel/content_changed且其他binding差分false、local/marker对象缺失、0dispatch/terminal/released。按部署前代码定位无保留anchor全帧严格比较；原PNG与creation cause未记录，不宣称小地图外动画误拒绝或地图错误。仅检查历史记录，无游戏发现/截图/输入/activation/restart。
- PixelAnchor::from_png_diagnosed/features_diagnosed从原一次构造返回精确failure；Engine private StoredReference同寿命256保留backend/artifact/真实failure，公开Resolved/MCPschema不改。act_inner短拒绝及diagnostic引用固定保存理由，旧未知not_recorded；Native batch层不冒称已加理由。原96/64/luma32与32/384/16/feature算法/native guards/lease/no-replay不变。
- 精确reason断言BASELINE101(not_recorded)→MODIFIED4pass0→主rollback101→reapply4pass0，三原hash恢复；Pixel namespace14→19pass。冻结workspace653pass/11ignored/41suites、fmt/allfeaturesClippy/diff0；releaseAgent build0、unsigned6b188d7d…，未签名/安装/运行。
- 本补丁需Agent同稳定签名部署并刷新所有source binding；仅此修改不需换MCP/重启Codex。导航唯一worker仍有控制权，本轮不热换。新鲜同点/合法稳定texture面可按现有规则解析，不保证通过，不重放旧request、不套red-marker降级。实机场景双PNG/新reason/正负例与goal独立验收仍待部署协调。
- [本轮详细记录](../validation/2026-10-09-city-refusal-anchor-creation-reason.md)与旧四角色补充证据继续保留。完整Hover/Live/activation/多窗/应用矩阵/资源性能/模型输出计划未缩减。

## 2026-10-09 历史：悬停已定位到App/Window之后 / 缺锚点诊断补齐 / 本轮不热换游戏服务

- loaded新Agent真实own PID6576/CG15045：hover type5/subtype0在App/Window两层point[250,372]命中hover_zone，view/tracking/tooltip仍0；不能再笼统说没送到进程，也不称hover已修复。AXPress0→1、drag1down8drag1up/completed1无orphan、RWI close/state.closed/exit0/detach验证透明super正常。只有自有虚拟屏输入，无游戏输入、第二worker或服务restart。
- GREEN原始durable拒绝terminal/0dispatch/contents changed、无局部metrics；新grounding_diagnostic补anchor availability/policy与not_recorded reason，Engine拒绝文案指明无局部anchor且全帧改变；不猜真实创建原因、不放宽阈值或赋authority。本轮源码尚未部署，current73141/de7cc66c/e1f0…保持。
- 最终workspace644pass/11ignored/41suites、fmt/lockedoffline tests/allfeatures Clippy/diff0，receiver example独立24tests/build/clippy0。两项同输入BASELINE101→MODIFIED0→主ROLLBACK101→REAPPLIED0、beforehash恢复；具体字段和literal证据见[本轮报告](../validation/2026-10-09-receiver-dispatch-and-anchor-availability.md)。旧四角色继续扩展，原baseline不覆盖。
- 用户旧页面出现的实际Live363capture/362present结果保留；新probe Live的open首次Host MessagePort error/第二次queued，采集0，stop清理通过，不能称新运行中switch/HideResume通过。
- 已继续实现test-only actual event(P) vs publicNSEvent→CG探针，same namespace5→6→主rollback5→reapply6全0；两路subtype0/delta0，但CGlocation/source_state不同，真实geometry/input效果未证明、Private政策未替换，首次E0369错误保留。下一动作：自有fixture真实frame/Privatesource验证后single-variable派发（兼容subtype/post通道逐项），offline→隔离实机→独立view效果；不伪造回调/activation/warp/globalfallback。完整keyboard/scroll/多窗OS panels/应用状态矩阵/性能并发回收/Live/输出易读计划均保留，不只修本bug后停止。

## 2026-10-09 历史：新版MCP整体实机已验多项 / Agent已部署 / hover与Live切换仍待补齐

- 本会话真实加载39tools；双自有virtual窗口独立attach/place/AX计数各1、typed drag实收1down8drag1up、跨frame拒绝0native、short输出/explicit timings/details通过；真实PNG与按项结果见[功能验收](../validation/2026-10-09-mcp-functional-acceptance.md)，不是只最近的像素问题。
- Live实际363capture/362present、preview在virtualdisplay4；源closed清图/paused及expiry隐藏worker结束通过。运行中任务切换Hide/Resume中间状态尚未采样，不能称全通过。
- standalone hover两次派发但view moved/tracking/tooltip0，保持effect unverified；下一步self App/Window dispatch probe，不warp/按钮/自动fallback，不从view0推平台不可能。
- 原已验642/11ignore候选真实同证书DR/配对/配置部署，native lease锁下56durable记录安全终态；新PID73141/instance de7cc66c…/四权限true。MCP ab74d313…未更换、当前39tools不需要再次重启Codex；旧execution过期/显式durable回执不重放及新fixture真实counter1+RWI close/exit0已验。
- 游戏未输入/无第二worker；动态原点不宣称修复。之前“candidate未部署”的同日条目是历史。后续keyboard/scroll、同app多窗/OS panels、场景矩阵/性能/资源等完整计划继续。

## 2026-10-09 当前：动态护栏诊断/同帧评估已修复 / 签名候选就绪 / 占用期间不部署

- 已溯源真实46格拒绝，不归因用户干预，不放宽32/384/16，不使用红点模式。新两帧只读真实PNG揭示原点33格且hit984超限；同小地图另外两点稳定，不将离线match或换点当原点修复/游戏goal。
- 诊断分区与failed_checks已实现；Core/Native同fresh-scene评估结果复用、scope/identity/readiness/lease/no-replay保留，失败消息明确实际pixel原因。BASELINE2次比较→MODIFIED1次→主rollback2次→重施1次，匹配policy不变，原hash恢复。
- 最终642pass/11ignored/41suites、fmt/all-features Clippy/git diff/release Agent均0；新文件pair ignored test另明确跑1pass。签名候选4c4d0890… strict同DR验证0，未运行/安装候选。
- 最新新relay PID15346/parent59777已只读ps确认；本轮游戏输入0、无signal/stop-file/第二worker、生产Agent/MCP未动。部署与实际小地图目标仍待新的独占交接；hover Host表加载/后续完整计划保留。
- [顺序实际记录](../validation/2026-10-09-dynamic-pixel-anchor-readiness.md)含原图与完整证据入口。现有生产Agent仍PID66480/79b38f98…，不能把源码修改/离线gate当生产修复验收。

## 2026-10-08 当前：上轮Host验收通过 / F05已实现部署 / 新工具Host验收待重载

- 本会话loaded MCP成功timing explicit true322ms/default短回复+details174ms、独立计数0→2及正常关闭已验收；过期关闭proposal0native拒绝保留，fresh帧/新request纠正，未放松TTL。
- 已继续实现typed computer.hover +共享MovePointer +Mac单MouseMoved（Private/0 flags/无button/无activation请求/无warp/无globalfallback、history unknown）、Windows unsupported。tooltip请求不当作显示；fixture公开callback与lifetime完善分开验。
- 最终冻结后631pass/10ignored/41suites、fmt/全features Clippy/git diff check0；此前同product code双release0；Agent同签名/配对真实部署，loaded metadata PID66480/四权限true；MCP39tools/ab74d313…已安装并新stdio验收0，但当前工具表仍旧38，新hover Host-native验收待重载。上轮“需重启”的success timing已真正验完，不再重复该门禁。
- fixture新增独立有界自身生命周期：16→19→主rollback16→reapply19 exit0；新display4 PID89957实际timer请求→真实window_closed/exit0、fresh匹配0，input计数0。总3597ms含startup，不声称硬实时/后台hover；本次examples修改不重复部署。
- 同窗CUA/RWI click试点都达counter目标；CUA精确.app解析成功、foreground有变化，非公平速度对照。sharing chrome隐藏close后只终止own disposable PID并核对windowgone，不伪称model.closed。
- [本轮实际记录](../validation/2026-10-08-mcp-reload-hover-continuation.md)含顺序截图/原始结果与限制；此前各阶段作为历史保留。原16feature/full implementation矩阵不因一项成功缩成仅多窗口或仅hover。

## 2026-10-08 当前：Agent已部署 / 自有接收窗口已验收 / 全计划仍在实现

以下同日较早条目保留为历史阶段；当前结论以本节与实际命令事件为准。

- F04 MouseGesture shared Private source+明确button/eventNumber/pressure/delta已实现并部署；单次非OS-frontmost fixture drag实收1down/11drag/1up、eventNumber2、delta累计150/60、pressure归0、completed1/无orphan。只证明该配置，不称hover/全面遮挡/所有应用已经完成。两个fixture均RWI关闭，独立state.closed/进程exit0。
- Executor true分支/feature广告与实际journal tail修复已包含Agent；当前loaded MCP确认instance efd220c1…/PID52174/四项权限true，同稳定代码签名/配对/CA/config/profile。首次部署startup-smoke race exit1真实原子回滚，修正read-only就绪检查后第二次exit0，无输入重放。
- 原生截图、布局/2×映射、独立oracle、每步问题、原始耗时与阶段边界见 [本轮实际验收](../validation/2026-10-08-agent-gesture-native-acceptance.md)。click877ms/drag943ms/close188ms是单样本，不等同benchmark。
- 已补compact success timings：include_timings=true保留，默认/false输出不增加；组合101→0→主脚本rollback101→reapply0。最终fmt/workspace618pass+10ignored/41suites/all-feature Clippy均0；MCP release已安装SHA `dbbfc8cdfb497a664e8a4e336b01b1812f6e302275dd9c383b536c65bc12e23c`，新进程strict signature/38tools/mac-host只读smoke0。Agent无需再重启；仅此MCP输出修复需Codex重载后验收。
- 第一未完成主功能：typed Move/Hover与tracking/tooltip真实接收；后续仍覆盖occluded/offscreen/other-Space/minimized/hidden、AppKit/Electron/Wine/SDL/GLFW同状态A/B、cold/warm/P50/P95、resource并发/回收、Live/多窗口/模型输出易读。不是仅剩多窗口或“全实现已验收”。

## 2026-10-08 Codex重启后真实MCP验收（无GUI输入）

- 本会话loaded computer.targets确认macos/permissions reported true/desktop未绑定/32char result_ref；不是只跑独立stdio。default和explicitfalse typed View负例均unknown_view，不再unknown red_marker_cluster；stage仅resolve_view，无claim/act。true当前旧Agent明确拒绝，未静默降级。details_ref原始错误/goal false/native flags均可完整读取。
- 发现并修正production Executor resolve_view忽略red_marker_cluster的源码缺口：true走RedMarkerCluster，default/false仍Pixel；unsupported明确拒绝，无Pixel fallback。实际Executor+PNG回归101→0（三项）；fmt/workspace604pass/10ignored/41suites/all-features Clippy通过。Agent候选未部署/重启，MCP binary不变，不要求再次重启Codex。
- [重启后验收记录](../validation/2026-10-08-mcp-post-restart-acceptance.md)与原四角色扩展证据保留；真实GUI、动态guard、业务goal仍待明确分配测试窗口，不放宽防护，不操作游戏。完整后续计划继续。

## 2026-10-08 新实现轮次（协议兼容/并发修复/原生构造探针）

- 主仓库已实现：`view_resolution_args`默认/false省略red_marker_cluster，true明确拒绝不降级；`compact_action_reply`省略缺失key而保留显式null/false/引用/cleanup/point；完整metadata可读取，MCP结构化/text兼容不变。
- 并发可靠性：operation/cancel绕过OpenContext锁，普通等待可被Host EOF/cancel终止；context_unknown只按同token失效，不清新epoch。inspect_device迟到Describe不再失效新context/覆盖新targets，两个barrier负例证明单OpenContext。控制/race共5个离线TLS组件回归通过，无Prepare/Execute。
- 缓存路径集中make_room与单次size/hash编码，用take少一次原树clone；容量/TTL/分页/未知形状保持。资源及生产性能百分比尚未量测。
- test-only NSEvent探针4项通过；type/window fixture/flags等保留，但pressure有损、delta需明确写入、坐标含隐式上下文。不能称已实现后台hover/drag或游戏兼容；无需部署Agent/TCC变更。
- 本轮统一门禁：fmt、workspace601passed/10ignored（41suites）、all-targets/all-features锁定离线Clippy -D warnings、JS50passed均exit0。详细[实现与验证记录](../validation/2026-10-08-background-foundations-and-mcp-reliability.md)。代码行为变化与缓存结构调整分列，无把旧研究/旧测试当新验收。
- 默认feature MCP release已部署（SHA256 `e5c1a8741e04b21f4e38b010c63649e96c7c20d25a22384e26cbf5929ef0d369`）；candidate与安装路径新进程都完成MCP2025-11-25/38tools/mac-host metadata验收，exit0、stderr空、0images/0input。profile未变，Agent/证书/权限未改、不需重启Agent。现有Codex会话不自动换binary，需重启Codex后在线复验。
- 后续继续：专门取消/epoch负例、能力协商、自有receiver、完整gesture/typed hover、window-local观察、真实应用同状态A/B与分层性能。部署和fresh MCP process核验以事务实际事件为准；已运行Host不因磁盘替换自动加载新代码，完整既有计划保留。

## 2026-10-08 扩展后台生态调查（50仓库，研究完成/原生候选未部署）

- 用户要求扩大范围后，实际审查50个canonical公开GitHub仓库（17直接、33周边/对照），150篇去重技术正文：101篇社区Issue/PR/Stack Overflow、9篇原作者文章、40篇官方指南/会话/设计等。另49个API参考页、README/源码/metadata/许可保留用于证据但不凑技术篇数；空body、404、JS壳、弱checklist和重复镜像排除。并非150篇长论文或50个完美开源后台driver。
- [扩展研究](../research/2026-10-08-macos-background-survey.md)、[仓库JSON](../research/2026-10-08-macos-background-repositories.json)、[来源JSON](../research/2026-10-08-macos-background-literature.json)、[16项功能难度/API矩阵](../research/2026-10-08-macos-background-feature-matrix.json)已生成；CSV可按热度/难度/维护状态筛选。星级是补齐RWI对应组件的工程估计，GitHubstars只作独立热度，不以updated_at/closed/老HEAD推代码活跃/merged/弃置。
- bgclick/CGSInternal是方法/历史header，不是现成SDK；BetterDisplay未公开app native实现，CLI只notification client；input-leap实际归档、旧Sikuli明确不支持、历史mirror单列。PyAutoGUI作者2026称未弃置，nut公共snapshot与私有成品区分；Screenpipe/OmniParser当前许可材料需独立处理。未直接vendor或运行安装脚本。
- 新发现要求更强验收：Cua Driver有Rust SPI路径，但background drag拒绝、window move仅overlay；no-raise可能保持frontmost PID但改变key接收方，必须验用户输入连续性/恢复。开源Cua不是Codex内置Computer Use。Apple明确covered/offscreen单窗口capture可用、minimized流暂停且单window不含popup；hover/drag还受client/global/raw/accepted-down/nested tracking影响，不以库缺口判平台不可能。
- 当前结论改为Rust/macOS多route与状态能力矩阵；优先公开构造/字段/source roundtrip、完整drag/typed hover、window-local capture/authority与goal readback，SPI作为显式高难候选比较。先协议/optional字段兼容与离线probe，再明确分配GUI fixture，最后真实应用A/B/性能。收到另一任务两次red_marker_cluster未知字段反馈，仅记录待溯源，未本轮复现/代操作游戏；不是鼠标平台接受结论。
- 本轮无新native源码改动、GUI输入/截图、权限请求、service部署或commit。四角色沿用10/06事务并扩展survey补充，baseline/modified/rollback文档状态与hash已核验；旧Rust回归保留日期，不冒充新测试。先前MCP输出/成功率/多窗口/Live/证书等完整计划继续保留，不由此调研替代。

## 2026-10-08 macOS真后台事件二次调研（文档修正，未部署原生候选）

- 只读回溯用户指定的 Strange Beasts General Store「任务接力/前后台相关」、活动候选、原始研究及137项既有校准JSON；另审计Apple与bgclick/Wine/Hammerspoon/SDL/GLFW/RobotJS源码、Issues/PRs和固定提交。没有操作游戏、截图、发送输入、修改另项目或重启服务。详见[完整调研](../research/2026-10-08-macos-background-nsevent.md)、[来源索引](../research/2026-10-08-macos-background-sources.json)、[证据投影](../research/2026-10-08-macos-background-evidence.json)。
- 后台NSEvent路线仍是工厂→CGEvent→postToPid；原项目实际后台click/键盘已有视觉读回。37个hover中13个输入前blocked、24个posted但tooltip未确认；孤立dragged-hover只有一个无down/up的dragged事件，PID tap收到了但业务未确认，不能据此否定完整后台drag。历史reason解析不兼容还可能产生业务unknown门禁。
- RWI本来已有PID/window定向输入；当前真限制还包括on-screen窗口目录/SCK枚举、完整显示器映射。公共Intent缺Move/Hover，drag已实现但每事件新建Private source，NSEvent鼠标构造/连续source/relative delta为待验证变量。旧bounded-pointer文档“Mac drag unsupported”已标superseded，不删除历史证据，不解除真实guard。
- 客户端机制必须加入对照：Wine普通move会重新全桌面选窗，GLFW hovered getter会读全局光标，SDL macOS≥26部分版本会用全局位置覆盖事件坐标。具体源码分支不等于用户GameHub实际版本；没有把ack当goal，没有宣称真后台或性能已改善。
- 优先下一步：rwi-macos Rust离线NSEvent构造/roundtrip probe→自有AppKit窗口接收fixture→typed Move/Hover与完整drag/source对照→独立window-local/offscreen能力→经分配窗口的AppKit/Electron/Wine同状态可逆A/B。覆盖inactive-visible、fully-occluded、offscreen、other-Space、minimized、hidden；不静默activate/warp/global fallback。Windows/Linux独立适配。既有MCP刷新/输出/效率计划保留，不由本轮研究取代。
- 新增用户明确要求的项目memory note，只取代过时概括，不改生成memory或旧notes。本轮复用10/06四角色事务并扩展研究补充；文档同输入baseline/modified/rollback、重构及哈希核验是本轮结果，10/06 Rust回归是历史结果而非重跑。无新原生实现/部署/提交。

## 2026-10-06 RWI 成功率、耗时与 MCP 输出复核

- 历史证据沿用 [10/02 同会话调用审计](../validation/2026-10-02-macos-computer-use-comparison.md)：Feishu/Chrome 的 JS 返回中位数为 RWI 1075ms、CUA 494ms，文本块字节中位数 3834/1357.5；任务与调用组成不同，只能说明历史接口负担，不能当作公平速度/成功率对照。游戏会话调用数也不等于目标正确率。当前只筛选 RWI 相关会话与既有审计，不读取无关 Codex 历史。
- 10/06 游戏会话交接新增一个动态画布拒绝样本：`stale_reference`、`content_changed=true`、`input=0`，附带 `act=2430ms`。这是交接摘要，不是本任务读取到的原始 receipt；不得归因到具体像素门槛或称为独立复验。对应仅显式 opt-in 的红点簇锚点保留原生窗口/进程/hit-check，默认 Pixel 仍严格；离线负例覆盖 marker 移动、消失、遮挡，未作线上/实机成功率声明。
- 10/03 MCP 输出候选仍有效：typed convenience 是默认工作流；普通成功回执不重复内联 `stage_timings`/`elapsed_ms`，用 `include_timings=true` 或 `details_ref` 取细节；拒绝、错误和不确定派发保留诊断。只压缩完全重复的控件描述及 adapter-local UUID 分隔符，不缩短原生窗口/frame/element ID，不裁掉 X/Close 名称、坐标、映射、焦点/可见/遮挡、coverage、mutation/fence 或 receipt。
- 本次回归发现并修复候选中未补齐的新锚点字段初始化、一个双重 PNG 编码的离线夹具，以及像素测试夹具缺少纹理；另发现 `computer.controls` 紧凑投影漏掉 `point` 坐标，已保留坐标并补 Close/X 测试。未放宽阈值或改变派发语义。验证：`cargo fmt --all -- --check` 通过；workspace `585 passed / 10 ignored（41 suites）`；all-targets/all-features locked offline Clippy `-D warnings` 通过；JS client `50 passed`；`git diff --check` 通过。详情和命令记录见[10/06 MCP 效率复核](../validation/2026-10-06-mcp-tool-efficiency-revalidation.md)。
- 用户随后明确授权部署与验收后，已将 Release MCP adapter 安装到现有 Host 插件路径 `/Users/vincentadamnemessis/Library/Application Support/RealWorldInteraction/client-v1/bin/rwi-mcp`（SHA256 `4bd4eedef223a2fd1137668a2b856f30bac150fc3c7fee849ba861bd44e486ef`）；旧二进制 SHA256 `b478b93561a97bc31c100478ace4302ae8652c48ede911c6d93aede9e7cd2f76` 已备份。旧/新均保留原有 adhoc linker signature，strict codesign 检查通过；未动 Agent/LaunchAgent、TCC、Host 配置或其他插件。
- 同 profile、独立 stdio MCP 进程的只读 `computer.targets(device_id=mac-host)` 对照：旧二进制结果 32,110 UTF-8 bytes / result_ref 36字符；已安装二进制 32,106 bytes / result_ref 32字符，两次都列出38 tools、目标 available、Agent PID 976，均 `input_sent=false`、`screen_captured=false`。原始 metadata 字节跨调用变化4 bytes，因此仅记录这一个 transport 样本，不外推 token/耗时/成功率百分比。
- 安装后当前 Codex 已加载工具再次只读查询仍返回36字符 result_ref，说明其已运行 MCP 子进程尚未加载新二进制。当前没有可安全调用的 Codex 整体重启 API；为避免强杀宿主或任意一个可能属于其他会话的 MCP 子进程，未替用户重启。下一步需用户重启 Codex后回到本任务，复验活动 Host 的32字符引用；Controls 坐标的实机完整性仍需一个明确分配的安全窗口，未截图/输入。无提交；工作树还含其他未整理的用户工作，不适合合并成大提交。

## 2026-10-06 CreatureCurios 动态地图锚点复查（只读，未修复/未部署）

- 收到「青芽原野」7 目标地图测试的交接：两个红点点击均报告派发前 `stale_reference`、`content_changed=true`、窗口/焦点/layout/epoch 不变。来源会话的 `read_thread` 本轮未返回 turn、仓库也无该请求的原始 receipt/截图；因此只记为交接证据，不说本任务独立核验过该失败原因。
- 本任务按精确 PID46833 重发现并绑定 `CreatureCurios` `cgwindow:22944`，范围`[0,0,1800,1169]`/内建屏；首个 bind 的10秒 catalog已过期（input0/no fence），重发现后绑定成功。只读snapshot 2572ms，原图3600×2338、预览2048×1330、0 AX controls、非visual-only且input reference可用；截图是HUD/小地图而非全地图目标界面，未发输入。
- 代码已有48×48 hit-region与外围动态格预算，但中心像素差仍严格拒绝。交接缺少中心/局部锚点变化量与失败帧，不能判断是动画门槛、目标移动/遮挡还是错误红点；本轮不降全局阈值、不做“用户授权即绕过”分支、不重放。候选适配需先用同一目标两帧原尺寸crop证明marker局部唯一稳定，并覆盖移位/遮挡/相似红点拒绝；详见[动态地图锚点复查](../validation/2026-10-06-creaturecurios-map-anchor.md)。

## 2026-10-03 MCP效率与输出减负（源码候选，未部署）

- 按历史会话审计与最近 RWI/Paint MCP 调用元数据，本轮继续记录 typed tool latency、顶层状态、dispatch 和实际 readback 的区分；不能把工具 envelope `failed` 直接计作输入失败或目标失败。历史同状态审计仍显示 JS 临时编排与冗长结果是实际接口成本，但非公平 CUA/RWI A/B。
- 新增 MCP server 路由提示：device_id 已知时定向调用 `computer.targets`，避免状态发现等待无关离线endpoint；不改变并发探测/超时实现，也不推断设备身份。
- 成功的 typed convenience 回复默认不内联重复的 `stage_timings`/`elapsed_ms`，可传 `include_timings=true` 查询；错误、不确定和拒绝诊断始终内联，完整计时仍留在 `details_ref`。只去掉完全相同的控件名称/描述重复项；仅 adapter-local 引用改为32位十六进制。原生ID、X/Close控件名称、AX动作、坐标mapping、窗口/frame绑定、覆盖状态和输入围栏均保留。
- 离线通过：rwi-mcp 96 passed/1 ignored，workspace 580 passed/10 ignored，fmt、all-targets/all-features Clippy、JavaScript client tests 通过。代码尚未部署/Host刷新，尚无真实 MCP 输出字节 A/B，也不宣称成功率/线上 token 已改善。细节见[效率与输出验证](../validation/2026-10-03-mcp-output-and-efficiency.md)。

## 2026-10-03 Codex重启后RWI Host验收（部分完成）

- 在初次只读枚举之后，已继续绑定用户长期授权的 Paint S 窗口并完成只读截图/字号探针；该后续实测更新了下方“尚需明确分配窗口”的阶段性状态。字号 16 的改进目前只在应用内存，因 Paint S 未聚焦，RWI 的保存键在派发前拒绝；磁盘 `.paint` 仍是 219 对象基线。细节见[重启后 RWI 与 V14 字号探针](../validation/2026-10-03-post-restart-rwi-and-v14-font-probe.md)。
- 新会话工具目录已加载 **38 个 RWI MCP tools**，无 Jev MCP tool；RWI 插件启用。活动 `~/.codex/config.toml` 不含 `[mcp_servers.jev]`，专属 helper 不存在。前次记录中的“当前Host仍为旧工具表”已由本次重启验收更新。
- `computer.targets` 选中 `mac-host`（`platform=macos`）：Agent PID **976**、服务 `healthy/available`，Accessibility、Listen Event、Post Event、Screen Capture 四项权限均为 true，`requests_permission=false`，`input_sent=false`。Windows VM `win11-vm` 返回 `device_connect_timeout`，本轮不将其重试为 GUI 路径。
- 当前 capability 已实报：独立窗口 attach/detach、discover、snapshot、typed scroll、Live 已实现；lifecycle、launch、activate 未在此 macOS Agent 实现。签名 strict verification 通过，Agent/MCP 安装 SHA256 与本节前的部署记录相同；LaunchAgent 仍为 `running`。
- 只读 `computer.discover` 枚举到22个当前窗口，首屏返回8项及两块显示器UUID；`activated=false`、`native_dispatch_entered=false`、`input_sent=false`、资源已释放。未截图、未绑定窗口或输入；该目录读取验证了新 Host 的只读通路。后续分页游标超过10秒缓存期限后返回 `catalog_expired`，应重新 discover，不能将此解释为窗口缺失。完整 UI/多窗/Live验收仍需在明确分配窗口后按原计划继续。

- 2026-10-03 完成用户请求的Jev清理与RWI部署。活动Codex `config.toml`中的`[mcp_servers.jev]`已删除，Jev header-helper及Keychain `codex-mcp-jev`/`JEV_API_KEY`凭据已删除；`uninstall_plugin(Jev)`返回`not_installed`，当前磁盘上也没有独立Jev skill目录/已安装插件记录，活动替代配置中无Jev MCP注册。当前运行中的Codex工具表仍为启动时缓存，需重启Codex后才可验证Jev Skill/MCP不再出现以及新RWI工具目录生效。用户要求沿用的`RWI Local Development` (leaf `24B346A95EC8F450F7D8433A2E1876568DBCB2F8`)已设为**用户login钥匙串、Code Signing policy only**；`security find-identity -v -p codesigning`现为1个有效identity，未动System/Admin域、TLS信任或私钥。Agent候选以该identity签名，strict验证有效，原designated requirement保持为相同identifier+leaf；安装Agent executable SHA256 `660730276b012336b2c66291a345fcab61c273c6fcdc71e6f8f01c83dcddff96`，LaunchAgent重启后PID976 running。MCP原路径已换为候选SHA256 `b478b93561a97bc31c100478ace4302ae8652c48ede911c6d93aede9e7cd2f76`。回滚备份在`~/Library/Application Support/RealWorldInteraction/rollback/20261002-rwi-deploy-01/`。Codex GUI尚未重启：Computer Use明确拒绝控制Codex应用，且无专用restart API；旧MCP进程仍需随Codex重启刷新。无TCC重置；签名requirement未变，但新Host下的权限/工具目录实机复验待Codex重启。Release候选此前workspace门禁576 passed/10 ignored、fmt/Clippy/release build通过；本轮无产品源码变更。 详见[部署与Jev清理记录](../validation/2026-10-03-jev-removal-and-rwi-deployment.md)。

- 本回合继续多主线推进，不以多窗口单项收尾：同一 BetterDisplay Chrome fixture 上只读重查发现 A/B 重叠，RWI snapshot 为398ms，A的窗口/内容控件多标为`visible=false, occluded=true`；目标页面仍为Count5、原中文字段未改，本回合未发输入。这与此前深层AXPress无效果现象相容，但重叠/遮挡是否为根因仍属待验证推断。构建了单独的`computer.reveal`候选，只在exact target宣告`AXScrollToVisible`时才claim/派发，并要求精确frame/element；Agent/MCP Release候选已编译，未部署、未执行。当前Host仍公开旧34工具，没有reveal/scroll。进一步根据游戏现场记录，pixel-anchor从96×96粗粒度全patch累加调整为48×48近命中区硬阈值+外围动态格容许，保留全局变化格数、中心格和外围低变化漂移拒绝；示例21格/raw681已离线复现可通过，2px主体移动和宽域细微漂移仍拒绝。尚未部署或在CreatureCurios验收。全仓门禁：576 passed/10 ignored，fmt、Clippy、`git diff --check`及Agent/MCP Release构建通过。详见[AX入口增量](../validation/2026-10-02-full-plan-live-acceptance.md#9-模型可执行的-ax-scroll-to-visible-入口与当前host差异)与[动态窗口锚点增量](../validation/2026-10-02-full-plan-live-acceptance.md#10-动态窗口pixel-anchor拒绝与局部动画预算)。
- 来自游戏会话的拒绝记录显示：PID70878/`cgwindow:5398` `CreatureCurios`的小地图点击在派发前因点击点周边96×96 patch 的原始`total_delta=681>384`而拒绝；`changed_cells=21`、布局/焦点不变、中心区域稳定。不是整屏差异；严格拒绝也不是整体没用，但动画中的相邻网格确实可使地图入口不可操作。本候选把采样拆成48×48 hit-region硬预算与外围context预算：中心稳定外，外围最多容忍32个高变化格，再对剩余稳定格维持聚合差值预算；全局变化格数上限、中心阈值、native window/process/session/hit-test和no-replay不放宽。离线复现raw681/21格可通过；2px主体移动仍在跨层负例中被拒，广泛微差漂移仍因stable-context预算被拒。变更尚未部署，因此真实游戏入口效果未验收；地图已被用户手动打开，本回合不再发送输入。详见[动态窗口pixel-anchor增量](../validation/2026-10-02-full-plan-live-acceptance.md#10-动态窗口pixel-anchor拒绝与局部动画预算)。

## 2026-10-02 全计划首轮实机进展：可靠执行、动态观察、耗时与契约（已部署，整体未完成）

- 用户纠正原计划不只是多窗；本轮同步推进真实失败链、目标级AX、dynamic/coverage、字段/滚动、性能、工具输出、人机隔离及Live，不以一个候选/成功截图收尾。详见[完整首轮验收及未完成表](../validation/2026-10-02-full-plan-live-acceptance.md)。
- 实机定位 Chrome显式AXPress误拒：仅pointer hit hints变化却污染语义revision。修复保留原对象/窗口/provider/祖先/权限/模态/几何/其他状态检查；普通pointer仍拒绝。实际中文字段读回、deep branch浅层按钮、75ms无关动态区截图/按钮、同app B覆盖A时语义按钮及Incognito Count0→1均验证；最后A Count5/字段保留、timer停止、私窗已关闭。CUA只作setup/只读对照，旧frame跨bind负例零派发，业务飞书/Chrome窗口和游戏不操作。
- 性能删除只读capture前被丢弃的控件全树，保留轻量window/context/modal前置guard及capture后完整有界树，所有input照常严格重验证。Count3相同fixture采样中位568→440ms（各3次、非随机，约22.5%观察降幅）；最终版本Count4烟测中位464ms。前置metadata143–183→4–20ms；不拿拒绝耗时当成功、不与CUA无图语义read冒充公平A/B，不宣称整体优势。coverage明确depth/node/queue/time/read缺口；deep button初始覆盖仍缺，semantic比较未收集为null而非false。
- 新typed scroll/title/window/consistency/coverage契约已打包，actual MCP stdio catalog37；此任务仍加载旧34工具及Skill，attach/detach/scroll新入口未在Host实机验收。底层已加载RWI滚动独立读回0→600，不混作新typed工具通过。源码typed-first、partial-dispatch解释、细证据保持；输出层次/focus/settable/diff等仍需继续。
- 活动gate仅explicit native action按已知focused窗口排除精确sibling；synthetic共享app receiver仍保守同PIDyield，未知仍yield，阈值未放宽。纠正combined-session年龄并非真实人类证明，拒绝诊断区分combined/HID表但source仍unknown；GameIO来源及真人冲突实机未完成。Live panel本轮queued、实测0frames/hidden，stop清理完成；没有伪造任务可见性，不当作overlay/切任务验收。
- 最新 AX 深层覆盖增量在上述版本后重部署：控件walk最大深度40，512节点/1024队列/3秒限制不变。实际 fixture 对深层按钮发现从RWI缺失变为可经`computer.controls`按名字找到（depth33）；快照370ms、90/92节点描述/遍历，仍报child/descriptor读取缺口。该按钮显式AXPress返回后独立读取仍是Count5，业务效果未通过，未重发。最新Agent PID83412 / instance `8a502d62-7b73-4292-91ed-db704c50dd38` / SHA256 `d21536f2580101f7dc79301caa5b4a40e507166e9f1ee477b9f6f87f029609c2`；原本地leaf签名与四权限均通过/保留。较早PID58619及SHA `5a2857960fcb52460343f007811ab32fa607d081a0427311ef1a1045eef94f88`只是上一版本，不是当前运行态。未提交推送/Actions，Paint219源不动。
- 剩余全计划：新Host恢复/工作集/系统panel、深层初始定向观察、native属性设置与明确焦点/activation、全应用性能A/B、输出便利性、事件来源精确隔离、Live实际切任务/Hide/Close/双源/物理屏，之后再恢复Paint。尚未达到全计划完成。

## 2026-10-02 用户授权部署与全计划实机验收（进行中，不缩减为多窗口）

用户本轮明确授权部署，并可用游戏、飞书、Arc/Chrome普通/隐私/多窗验证；不会据此触发发布、消息发送、购买或修改账号权限。完整计划依然来自真实会话审计第10节，单项成功不是结束条件。

| 原计划维度 | 当前事实 | 本轮下一验收/修复 |
|---|---|---|
| P0 contract/错误恢复/模型使用成本 | Agent/MCP binary/Plugin源已部署；此任务MCP仍旧连接，未加载新工具/Skill | 重载后验证typed短路径、title/window/partial-dispatch解释、细证据与恢复，不以改路由宣传代替成功 |
| P0 retained AX对象/准确执行 | Chrome本地fixture已经找到Increment按钮；AXPress零input拒绝，旧诊断绑定项全false | 补控件级diagnostic并定位原生实机缺口，修复后复验；不绕过拒绝 |
| P0多窗/面板/隐私窗 | 独立target候选离线通过，Agent已支持；旧Adapter不支持新tool | 普通/隐私/弹窗逐一验引用，不只是枚举窗口；不将同PID当owner |
| P1动态observe/coverage | 静态Chrome已实际原生截屏；另会话报告Qt空闲窗semantic_content_changed、74/74节点、轻量capture成功 | 定点读取该控件metadata差异、图像/语义证据分级、覆盖截断；不臆断为用户操作 |
| P1后台字段/滚动/显式焦点 | 本地fixture已放到既有BetterDisplay UUID虚拟屏，不动原Chrome业务页 | 分别验证AXPress、输入/滚动及前后台效果；不偷抢焦点或默默换路径 |
| P1性能/单步耗时 | 已取得discover16ms、placement阶段timings、snapshot1310/603ms、拒绝click47ms真实样本 | 保留native/adapter/端到端、读取/输入/goal readback各自成本与分位数，样本少不作优势结论 |
| P1输出/progress/工具便利性 | 旧Adapter尚输出完整控制诊断；源码typed-first候选已安装 | 新连接验证首屏/分页/错误输出负担、常用单步不需临时JS；成功receipt不是goal |
| P1人机冲突/Live生命周期 | 原global HID＋前台PID判断仍未替换；独立Host viewer回收离线通过 | 同app不同窗/其他app操作隔离、任务隐藏恢复、关闭、虚拟/物理屏位置真实验收 |
| P2同状态CUA对照 | CUA初次getState timeout，getApp/新window/local URL可用；仅用于setup/对照 | 同fixture同状态同goal比较tool调用与实际readback，记录失败和不可比部分 |

部署事实：新Agent PID97232/instance `5ec6b858-adb6-4509-ae4b-63beafbd962d`，四TCC权限保持true、围栏false，同leaf严格签名；原APP/MCP/Plugin备份保留，TLS/profile/钥匙串未改。MCP/Plugin源已更新但当前任务仍旧tool表，已询问用户重启或另建验收会话；不将安装二进制说成Host新工具已可用。

实机第一问题不是多窗：fixture A的Increment AXPress被stale_reference拒绝，scope/focus/layout/native_epoch/content均未变且Count仍0。已开始补控件维度的匿名差异诊断，不更改准入防线。原始当次preview/阶段timings/拒绝operation ref见`outputs/validation/2026-10-02-full-plan-live/`；只保存本地fixture，没有复制飞书凭据或游戏内容。游戏会话已报告正在验证Qt，不并行操作游戏；本任务没有向其他会话发消息。未提交推送、Paint219源未改。

## 2026-10-02 第三轮候选：独立窗口target与会话内工作集（未部署）

- Rust Backend/Engine、Agent、MCP加入`computer.attach_window`/`detach_window`：从root最新精确发现selection建立新固定窗口target，不覆盖原绑定/Live，独立AX对象registry及frame/ref/Live；attach不走placement/capture/input分支。最多root＋15 attachments，不嵌套派生；Window fingerprint并非OS birth证明，owned_popup/return关系仍未推断。
- 同一桌面Identity、授权、input domain、capture资源与physical leases保持共享。A租约/帧/element ID不可混到B，Observed effect标记仍传递；detach需释放本target租约，销毁volatile binding/frame/ref/view而非关闭应用，durable回执仍可查询。Context/principal/instance crossing与cancel拒绝，丢回包通过Describe/targets找所属句柄，不重试输入。
- JS可选`desktop.attach(selection)`返回独立wrapper、`window.detach()`销毁；新增工具提供明确title，确认detach时立即回收仅该target的Host presence页，其他viewer保留；typed工具可直接使用，不需要临时脚本。两Skill/多窗参考已按新候选与旧Agent兼容边界更新，生产Plugin cache未改。Windows能力表明确attachment未实现，transport无native FFI依赖。
- 离线新跨层负例、实际MCP子进程＋mTLS synthetic endpoint通过；JS50通过，两Skill结构验证通过。最终workspace563通过/10忽略（41 suites），fmt/Clippy/diff/release/同leaf严格signature通过，细节见本轮manifest；尚无原生GUI/Live/耗时验收，不把候选当生产已修复。共享服务部署/独立Chrome测试分配问题仍等用户答复；游戏/飞书/219对象Paint源未触碰，无提交推送。见[第三轮验证记录](../validation/2026-10-02-independent-window-targets.md)。

## 2026-10-02 第二轮候选：保留真实AX对象、目标级重验证与游戏像素诊断（未部署）

- 针对Chrome已找到按钮却被无关深分支阻塞，Mac backend新增有界、类型检查的retained AX对象registry。public id加入对象serial，CFEqual区分同属性不同对象；换window对象/geometry/淘汰不会恢复旧id。语义revalidate只读取目标及原window/session/process/layout/modal/目标状态，native动作不用全树唯一性搜索；标准AXWindow关系或有限AXParent链证明所属，未知/循环/超预算/取消/替换拒绝，无猜测匹配、无扩大delegate、无修改lease/no replay/围栏。初次完整观察16层/512节点预算与capture settling仍保留。
- 原生title使用已读数据进入binding，document_state未知，不作业务oracle；typed snapshot/JS summary显示观察到的window/PID/title。并非多窗独立handle，single-target当前选窗限制与多窗计划仍在。
- 游戏会话补充锻造关闭X附近的局部像素拒绝：一次core前拒绝、一次prepare-only/input0最终guard失败；本任务未独立读取原帧回执、未点击游戏、未重放。新候选加入同一pixel比较器的维度/中心/周围变化聚合诊断，保持原阈值；中心静态但周边变化超预算的fixture仍拒绝，不把游戏样本说成已修好。
- 最终workspace **553 passed / 10 ignored（41 suites）**、JS **47 passed**、对象专项8、像素专项6；fmt/Clippy/diff/release通过，Agent候选同leaf严格签名，生产binary未更换。新增跨层fixture覆盖两个同属性控件、无关动态兄弟、目标/根替换、detachment、disabled、撤回action，以及durable重复请求/重建Engine后不读对象不派发。无原生GUI验收、无性能优越性宣称。
- 已询问共享Agent部署时机及BetterDisplay独立Chrome测试窗口；不重启正在被游戏会话使用的服务。MCP/Skill新输出需相关进程/插件重载，Agent与adapter可分批。候选细节、来源依据和剩余P0/P1见[第二轮验证记录](../validation/2026-10-02-retained-ax-target-revalidation.md)；未提交推送，Paint219源不变。

## 2026-10-02 当前优先级：macOS真实会话审计、多窗口与模型工具契约（Paint暂停优先级）

- 按用户新要求只读回溯「接入飞书Webhook通知模块」与「观察并自动化冰牙鲨副本」，定点读取其原始MCP调用/返回，建立脱敏时序表。飞书/控制台样本RWI75调用/12顶层failed、CUA87/17；这些跨应用、复合调用与浏览器连接数据不能当准确率/公平性能比较。独立历史原图证明一条RWI act returned后observe失败的cell已实际打开创建表单，不能说此前点击未执行。
- 发现并定位：Mac owned_popup转换未实现与skill指引冲突、同target多wrapper仍只有单个当前窗口、partial AX观察与要求全树完整的native动作解析落差、整体动态AX变化使capture失效、分页/参数错误清空JSstate、全局HID年龄+前台PID不是真正窗口级活动定位。Chrome深树拒绝后CUA原生节点编号/AX diff持续完成应用创建路径，有可溯源实际证据；不猜GPT-6训练或内部路由权重。
- 源码首批：Rust JS `execution_summary`分开cell失败/此前returned dispatch/拒绝/未知/cleanup，保留原receipt/ref、禁止cell重放；JS targets兼容schema形状及只读window.target；shared skill改typed工具日常首选、明确Mac显式initial选窗/多wrapper语义；native capture诊断增加真实变化维度/节点数量/settle次数，零新native读取、零阈值放宽。候选未部署，当前加载插件并未更新；不请求重启、不触碰飞书/Chrome/游戏、不提交推送。
- 首批候选离线门禁：workspace **542 passed / 10 ignored（41 suites）**、JS **46 passed**、fmt、all-targets/all-features locked offline Clippy `-D warnings`、diff检查与两个skill验证通过；无原生GUI验收。
- 深层AX target精准重验证与多窗工作集仍待实现，不把contract修复说成Mac适配已完成。后续按P0/P1顺序推进、离线负例先行，再分配明确目标实机验收。完整根因、原日志定位、调用成本和验收计划见[真实会话审计](../validation/2026-10-02-macos-computer-use-comparison.md)。Paint219对象源保持未改。

## 2026-10-02 最新增量：继续V14前两行原生可编辑标题（219对象）

- 修复部署后继续完整复刻计划。Paint仅开着最近文档、旧PID已失效；Computer Use只读确认，Host `open -g`后台恢复已有源，RWI精确发现新PID724/CG4228并独立确认文档及提示面板至BetterDisplay虚拟屏，GameHub/游戏未操作或移动。绘图/取色/字号/新Text/位置/保存来自加载RWI，不把Host启动说成Mac lifecycle已实现。
- 实际保存[219对象源](../../outputs/paint-v14-source/paint-v14-20261002-rows1-2-titles-editable.paint)，503829 bytes、SHA256 `2fc9d4cd8b2c0deccd758a0efce9f113425843e8f6fdfdd9b1245af29f685e94`，与工作源同字节；新增2局部Rectangle＋2个17pt/`#010101`原生Text。第一行标题基线下移4px，固定ROI误差降37.87%/59.93%，仍宽9px/4px，非1:1，后续优先typeface/字号、两行preview/date、头像灰底、旧轮廓及右侧布局。AppKit重存14个旧Text的系统字体alias/AutoResize缓存bounds发生正规化，配准列表排除两个标题ROI变化像素0，其他201对象语义同；所有12 Image独立保留，Undo跨重启持久性未知。
- RWI实机发现Rectangle绘完自动换Select and Move，必须逐次重选；picker的CG title可为空、取字形边缘误得`#929292`，未设色，实心笔画实读`#010101`才应用，面板显式移至虚拟屏空位。加载/动画期间AX-CG差1–2pt、过期frame或纯色区动态选框均保持派发前拒绝、不围栏/不重放；未放宽像素或身份防线。当前Paint `[1824,24,1220,527]`、Rectangle、无编辑覆盖层/模态；新坐标模式见[接续](paint-v14-resume.md)和[时序原生截图/审计](../validation/2026-10-02-v14-first-two-titles.md)。本GUI阶段未再改Rust/部署，无提交/推送。

## 2026-10-02 最新增量：最终检查拒绝后的准备事件/输入围栏分离（已部署，失败路径待实机复验）

- 游戏会话反馈已部署的empty-AXWindow路径真实打开“奇兽”、休息后读回“骑乘”、关闭/重开管理页，之前的角色误拒绝解除；其后新关闭请求在最终native检查报`stale_reference/uncertain`，却文案“no input sent”且input fence true。本任务独立durable query确认原回执及已释放资源，没有重放/输入或代替游戏会话解围。代码路径先发AppKit准备事件再最后检查，旧单一effect counter/文案未区分准备与请求鼠标输入；旧回执缺少具体失败维度，不能补算是哪一项变化。
- Rust候选保留准备event的effect counter与uncertain/no-replay；仅在计数肯定证明准备event1/请求输入0、无持有资源且清理完成的完整typed native evidence下免于新增围栏。未知/饱和/非零请求输入/cleanup/panic/落盘失败仍围栏，已有围栏不自动解除。最终batch的stale诊断补齐身份/focus/layout/epoch/content/grounding/modal/readiness，无额外capture、不改任何像素或AX准入阈值。Synthetic服务跨层测试覆盖不重放、重启保留分类、新独立请求可执行及非零输入负例。
- 离线全仓 **537 passed/10 ignored**、macOS **98 passed/6 ignored**、fixture专项2 passed、fmt/Clippy/release/diff check通过。同leaf严格签名SHA256 **`2045fc748f334afd3cf3ad6bb2e04491d85301e39d2ec9b0765fef58108d7ab4`** 在游戏会话转为不发RWI输入的只读时机、按用户修正后部署的明确要求完成备份/替换，实际安装SHA一致。北京时间10月2日00:03，新Agent **PID92469/instance `08ac04cf-a444-403d-9839-8933259b245d`** healthy、四权限保留、围栏false，当前MCP无需Codex重启；新target重新发现/绑定游戏PID61792/CG1671，仍在主屏，实际1512×981截图/3600×2338原图、0controls、visual_only=false、mutation_ready=true，2476ms。原失败durable receipt原样保留，不重放；**新最终失败/准备事件only免误围栏仍待游戏会话在新授权步骤复验，本任务无游戏输入/移窗**。详见[最后检查、部署与证据边界](../validation/2026-10-01-game-final-guard-preparation-fence.md)。Paint215对象源、复刻/其余计划均保留，无Git提交/推送。

## 2026-10-01 最新增量：零子控件自绘窗口动画点击（已部署，游戏效果待验）

- 游戏会话反馈三个fresh view点击“奇兽/骑乘”都在派发前被`local_pixel_surface_changed`拒绝。只读PID61792/CG1671诊断确认AX根存在、children明确为空、application hit等于同一窗口；旧macOS后续仅允许`AXScrollArea`的角色限制漏掉此类renderer。新增仅接受正向application hit＋CF同对象＋相同PID/CG号＋明确空child数组的`AXWindow` canvas路径，保留局部像素/身份/几何/modal/焦点/租约/不重放，visual-only继续只读。capabilities和错误证明同步。
- 默认离线全仓533 passed/10 ignored、macOS96 passed/6 ignored、fmt/Clippy/release/diff check通过；跨层fixture证明背景动画可准入稳定按钮、移动按钮/未知children仍零派发、旧request不重放。用户明确同意“现在部署，随后刷新游戏绑定”，另一游戏会话输入worker/在途RWI调用已空闲，暂停后完成备份及同leaf签名替换；实际安装SHA256 `b59d96a375adc0d64a0b193da80b94438ae286b49955b38e0e7c0cd733a5ed0a`。新Agent **PID64545/instance `d738b7f6-8623-4359-937f-ca17da6da636`** healthy，四项TCC均保留，输入域未围栏，已加载MCP无需Codex重启。新target下PID61792/CG1671重新精确发现/绑定，游戏仍在主屏；实际1512×981 preview/3600×2338原图、0controls、visual_only=false、mutation_ready=true，只读耗时2454ms。**本任务未发送游戏输入，真实按钮点击效果仍待游戏会话用新绑定验收**，旧frame/view全部停用。详见[证据、准入边界及部署状态](../validation/2026-10-01-animated-empty-ax-window-canvas.md)。Paint215对象源未改；原复刻和其余计划继续保留，不以本修复为总完成。无提交/推送。

## 2026-10-01 最新增量：V14第3–4行＋弹窗避开自身文档

- 已保存[215对象可编辑源](../../outputs/paint-v14-source/paint-v14-20260930-rows3-4-editable.paint)，与工作源同字节，503094 bytes、SHA256 `b0fd831fc630ba33b2f363b992c1c429b810f3f978f52116f54474673b8d1995`。新增6个原生Text＋6个局部Rectangle，分类12 Image/55 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/17 Text；临时取色线已撤销，误缩放已原像素恢复。两行标题ROI降98.83%/94.22%，预览降50.68%/47.02%，日期降76.65%/86.07%。字体抗锯齿、头像背景块、其他行/图形和右侧布局仍待修，整页未1:1。[时序截图、指标和负例](../validation/2026-10-01-v14-rows3-4-and-panel-placement.md)。
- 发现旧放置工具将modal面板移至文档同一原点，造成同应用遮挡；`blocked=true`本身正确。Rust改为可视dialog优先选同PID窗口旁空位，不抬窗/激活、不改变semantic-only或普通窗策略，仍独立验hit/modal和位置。全仓离线529 passed/10 ignored、macOS 92 passed/6 ignored、fmt/Clippy/release/diff check通过，同leaf签名部署Agent PID86740、四权限保留。真机取色面板`[2994,24,341,367]`变为可操作，设色/关闭成功；MCP自动恢复，不需Codex重启。一次AX超时查原请求并在native writer idle/零键鼠证明后显式解除围栏；字号Caret导致的纯色区严格校验保持，按实际稳定帧继续。游戏未被移动/输入；无Git提交/推送。

## 2026-09-30 最新增量：V14第7行、两处时间与后台输入修正

- RWI真实Paint GUI在虚拟屏完成第7行标题/预览及第6–7行时间的独立Rectangle局部遮盖＋原生DrawText；四处ROI RGB MSE相对195对象阶段分别降63.6%、59.3%、81.0%、65.3%。[203对象可编辑阶段源](../../outputs/paint-v14-source/paint-v14-20260930-row7-and-times-editable.paint)与工作源同字节，499250 bytes、SHA256 `eac46c1e78e86083bb5e7bb62b4e497f83b54029abed540f8e0cac257287245b`，12 Image/49 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/11 Text。Bold试验使标题误差变大，已恢复Regular且未保存试验；全页仍非1:1。[顺序截图/指标/限制](../validation/2026-09-30-v14-time-and-row7.md)。
- 删除可能点到Paint工具栏的无请求后台合成primer click，沿用本地签名证书部署Agent PID98605 / instance `b47ba216-8365-4598-a4b0-0a5efe47b01d`，四权限保留。部署前全仓离线528 passed/10 ignored、macOS crate 91 passed/6 ignored、fmt/Clippy/release/diff check通过；真实遮盖一次成功，但不推断旧问题唯一原因或所有后台拖拽必然成功。游戏留主屏、未向其输入。无Git提交/推送。

## 2026-09-30 最新增量：Paint V14 第5–6行文字＋弹窗focus-owner候选路径

- 第5行预览、第6行标题/预览已用3个独立原生Text＋3个局部Rectangle替换烙底字；字号框实测支持14.5/16.5。固定ROI误差降59.9%/68.7%/62.0%。[195对象源](../../outputs/paint-v14-source/paint-v14-20260930-row6-text-editable.paint)与工作源同字节，495923 bytes、SHA256 `1c83bef6cc429724a9c23dff501c1a6de795c486ce7f15af0273447f9d9b1f49`，12 Image/45 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/7 Text。保留191/189等旧阶段与失败试验。其他行/时间/头像仍待修，全页非1:1，游戏未移动或输入。
- Computer Use对照发现取色dialog存在于AX但未列入当时on-screen CG发现；补足精确聚焦控件自身AXWindow的dialog候选发现，保留独立ID/PID/进程/焦点/CF对象/几何证明与语义-only输入边界。已原证书签名部署Agent PID81531，四权限不变；macOS90 passed/6 ignored、workspace **527 passed/10 ignored**、fmt/Clippy/release通过。用户切回游戏后，RWI在虚拟屏取色面板设置同色并关闭，游戏仍前台、源SHA未变；特定仅AX可见的新分支尚未再次实机触发，不能宣称全修复。[顺序截图与验收边界](../validation/2026-09-30-v14-rows5-6-and-dialog-focus-owner.md)。无提交/推送，不需重启Codex。

## 2026-09-30 最新增量：RWI AXComboBox 文字输入修复部署＋Paint V14 原生标题

- RWI macOS `TextClass` 增加已知非安全 `AXComboBox` 文字分类，后续仍强制精确焦点native对象、AXValue可写及非安全subrole。fmt、macOS crate 88 passed/6 ignored、全workspace **525 passed/10 ignored**、全特性Clippy `-D warnings` 与diff check通过；沿用本地签名叶证书 `24B346A95EC8F450F7D8433A2E1876568DBCB2F8` 仅替换/重启RWI Agent.app。真实Agent PID19786、四项TCC true、MCP健康；真实Paint字号AXComboBox输入16/17并AX回读。直接文字色板仍visual_only，已以RWI Eyedropper→Picked Color→`Set as Text Color` 建立非截屏的取色替代路径。[全程与局限](../validation/2026-09-30-v14-row5-editable-title-and-combo.md)。
- Paint第5行烙底标题以独立局部 Rectangle 遮旧字、独立原生 DrawText 复刻，字号17/取色`#050505`，固定ROI RGB MSE **5358.86→2833.41（降47.1%）**；[189对象可编辑源](../../outputs/paint-v14-source/paint-v14-20260930-row5-title-editable.paint) 492495 bytes、SHA‑256 `12d28a4b39e3ff2c06d82f4940d3c4685e9009cc62310496df1ec638f8f295e7`，12 Image/42 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/4 Text。两次文字字号/位置试验已Undo且截图保留。右沿/下沿仍偏1–2px，其他列表字、头像、底图仍未1:1；未Git提交/推送。

## 2026-09-29 最新增量：Paint V14 选中行边缘与Text路径现场验收

- RWI在BetterDisplay虚拟屏真实Paint文档用2个独立 `#F2F2F2` Rectangle修正第5行左侧上下平坦边缘，原生原像素只改变1190px且未触头像/右侧文字；顶部70×2 RGB MSE **631.68→0.46**，底部70×15 **609.42→0.37**。[顺序截图/比对/工具负向结果](../validation/2026-09-29-v14-selected-row-edge-and-text-probe.md)。保存工作源与[187对象可编辑源](../../outputs/paint-v14-source/paint-v14-20260929-selected-row-edge.paint)同为491377 bytes、SHA‑256 `1066a6a78d7c69716ff199e67776a0f47cf2e6439391b316af1b7ee04088cc57`；12 Image/41 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text。
- 原生Text工具短暂试验汉字输入、Escape、Undo成功且源文件保持不变；但字号AXComboBox被精确焦点门拒绝、文字色板本次只读visual_only、Edit菜单跨显示器无法观察。一次本地像素引用不确定操作已查原请求、未重放，在零按键/零鼠标键及writer idle的原生证明下显式解除输入域围栏。这些是继续可编辑文字重建前需修复/复验的RWI缺口，不能据此宣称整个插件不可用或画稿文字已经完成。本轮无Rust产品代码改动、部署、提交/推送；不冒称cargo门禁重跑。

## 2026-09-28 最新增量：Paint V14 选中行旧酒红环局部修复

- 当前任务RWI在BetterDisplay虚拟屏以9个可编辑局部Rectangle处理第5行头像外的烙底酒红双环，未对主屏游戏输入。固定 x60–145/y405–485 RGB均方误差 **4881.3→2682.4（降45.0%）**；限定阈值下头像框外酒红像素569→0，头像顶边17px残线也逐次验证清除，右侧文字区域像素0改动。颜色对话框默认落主屏时精确绑定并立即移到虚拟屏；AXPress不确定请求查原操作、未重放。头像内部的合法紫色与旧底图残留难以直接区分，未声称全消除。
- RWI `cmd+s` 后工作源与[185对象可编辑阶段源](../../outputs/paint-v14-source/paint-v14-20260928-selected-row-ring-masked-final.paint)同为490905 bytes、SHA‑256 `85f1f52473a4e307822784c3e6fa336af3287a8d026fbdfac308e8304648a52c`，12 Image/39 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text。[按时间截图/4倍对照/验证边界](../validation/2026-09-28-v14-selected-row-ring-mask.md)。左端灰底圆角及列表文字仍明显偏差，整页未1:1；下一步结构性局部遮盖＋可编辑文字重建。本轮无Rust产品代码改动、部署、提交或推送，不冒称cargo门禁重新跑过。

## 2026-09-28 最新增量：Paint V14 搜索加号校准

- 当前任务真实 RWI 在 BetterDisplay 虚拟屏上用2个局部遮盖 Rectangle＋1个小 RoundRectangle＋2条独立细灰 Line 修正搜索栏“＋”；两次试笔Undo后都重新观察，颜色弹窗落内建屏时精确绑定并立即移回虚拟屏，没有碰主屏游戏。固定局部RGB均方误差 **1300.559→15.833（降98.78%）**，更宽邻域1069.479→108.551（降89.85%）；仍有跨区域旧酒红线及文字/选中行大偏差。
- RWI `cmd+s` 后工作源与[176对象阶段可编辑源](../../outputs/paint-v14-source/paint-v14-20260928-plus-button-calibrated.paint)同为489278 bytes、SHA‑256 `485c6eed0a0a40cc3381a1d7516071f9d4defd59600ddddd4e7b430abc17a143`，分类12 Image/30 Rectangle/99 Line/23 Ellipse/9 RoundRectangle/3 Text；[顺序截图、原像素对照及负向结果](../validation/2026-09-28-v14-plus-button-calibration.md)。本轮没有 Rust 产品代码改动、部署、提交或推送；不冒称重跑cargo门禁。下一步继续选中行旧底图与列表文字的局部重建，不能用整幅贴图冒充可编辑复刻。

## 2026-09-28 最新增量：Paint V14 斜顶图标灰阶与笔画校准

- RWI在BetterDisplay虚拟屏真实 Paint GUI 中继续绘制，主屏 GameHub 游戏未被移动或输入。先发现细 Line 被下方侧栏背景 Rectangle 抢选，改用每笔拖拽后 AX状态与原像素读回，不把选择控制柄/`dispatch=returned` 当作品。宽2两条屋顶试笔过重、上传箭头 Smooth 叠线使误差变大、加粗左壁也变差，均经RWI Undo并全幅像素确认移除。最终保留5条宽1平滑灰阶叠线＋3条宽2内横线/底边/右壁，均为独立可编辑Line；RWI实际 `cmd+s` 后正式源与[171对象阶段快照](../../outputs/paint-v14-source/paint-v14-20260928-sidebar-bag-stroke-calibrated.paint)同为487871 bytes、SHA‑256 `ad33cd5357ffec92ad18bbc8358eb57391e22814739a7c9cffd21146d0b45339`，12 Image/28 Rectangle/97 Line/23 Ellipse/8 RoundRectangle/3 Text。
- 对同尺寸参考，图标固定26×31区域RGB均方误差 **5706.7→4094.7**（仅局部降约28.2%），深色mask面积84→120（参考122），bbox对齐但IoU仅 **0.373→0.458**，参考圆弧/灰阶仍未1:1；整页文字、酒红占位轮廓等高影响差异仍在。详见[时序、真实截图和负向试验](../validation/2026-09-28-v14-bag-stroke-calibration.md)。本轮未改 Rust 代码/Agent部署，未提交/推送。

## 2026-09-28 最新增量：GameHub 游戏 Live 跨屏与耗时修复已部署

- 用户短暂切回任务后，Live 对主屏全屏 Wine 游戏实际开始采集，但旧面板虽请求BetterDisplay虚拟屏，窗口服务器读回却是**主屏 `[0,0,1040,720]`**；旧状态错误报 `settled=true`。已立即 `hide→stop`，未触碰游戏。修复 Presenter 的全屏 Space 行为、order 后目标 frame、展示前和运行中精确窗口/显示器校验，错误屏幕一律清空停止；`presenter_readback` 新增 `on_requested_display` 且参与 settled。部署后真实游戏源留主屏，面板稳定在虚拟屏 **`[2880,644,528,388]`**，`on_requested_display=true`，游戏始终前台。
- 随后发现 Live 请求1秒采样间隔，旧 `Condvar` 会被任务可见性心跳提前唤醒，30秒采集59帧。已改为单调时间 deadline，不让心跳触发额外截图；真机30秒约30帧，Stop后 `on_screen=false/worker_done=true`。最后补齐已验证面板 Hide/Resume 的新放置确认期限（此细项尚待再次实机回归）。全仓离线 **525 passed/10 ignored**、fmt/Clippy `-D warnings`/diff check、Skill校验和release构建通过。以既有本地证书同leaf签名、严格校验并替换 RWI Agent，当前PID68567 / instance `327f3549-1cb4-43fb-a2e8-9cd6fed58ddb`、四权限仍 true。Plugin Skill 由官方 cachebuster/reinstall 更新至 `0.2.0+codex.20260928140038`，当前任务无需为Agent运行时更新重启；新Skill文本需新任务加载。完整时序与边界见[跨屏/节奏修复](../validation/2026-09-28-gamehub-live-cross-display-fix.md)。**尚未向游戏发送键鼠；AX-less分支仍未实机验收。**无 Git 提交/推送。

## 2026-09-28 最新增量：GameHub 游戏保留主屏、RWI 只读实机观察

- 用户确认 GameHub 和其启动的游戏**留在内建主屏供本人使用**，RWI 可以观察/Live及不明显干扰的简单操作。本轮未对游戏发送键鼠、移动或激活。RWI 精确绑定 Wine 游戏 PID51946 / `CreatureCurios` CGWindow11351；ScreenCaptureKit 精确窗口 3024×1964 原像素→2048×1330预览成功，原生650×360局部清晰，游戏保持前台。`snapshot` 有AX窗口绑定、零子控件、`visual_only=false/input_reference_available=true`；这是非标准应用的另一种形态，不是 AX-less visual-only 分支的验收。详见[时序与截图](../validation/2026-09-28-gamehub-readonly-live.md)。
- 该阶段首次 Live 指向 BetterDisplay 虚拟屏时，`open_in_codex` 返回 `queued`，整个90秒会话安全悬停、0帧采集/展示并自动过期；这是**首次尝试的历史结果**。用户随后短暂切回，实际 Live 出现并暴露跨屏放置与采样节奏缺陷，已在上方新阶段修复部署。Plugin macOS Skill 的“Wine有AX根但零控件”边界也已随新版安装。

## 2026-09-28 最新增量：主屏 GameHub 边界、后台输入真实性与 V14 小图标

- GameHub PID 46002、Wine Steam PID 50278 当时无 on-screen 窗；实际游戏子进程 PID 51946 的 `CreatureCurios` CGWindow 11351 位于内建主屏。本阶段仅做精确 PID/窗口/前台元数据查询，**当时没有捕捉或操作 GameHub/游戏**；用户随后明确游戏应留主屏，新一阶段已完成只读截图。此前“无真实 Wine GUI”结论仅适用于更早时点。
- Paint S 仍在已分配 BetterDisplay 虚拟屏，Wine 游戏保持前台。一次 RWI 后台拖拽 `dispatch=returned` 但原生图完全不变；另一次后台工具合成点击却使 Ellipse AX 选中值真实变化，说明不能把回执当目标或把某次无效推广成全部失败。新图标采用每笔原像素/工具状态读回，错误 y318 底线经 Undo 清除，最终新增8条独立 Line 并经 RWI `cmd+s` 保存：**163 可编辑对象、486795 bytes、SHA‑256 `254161a76dd7fc8759186f9cbe78bd18bc23f78b9747b460e7d745ef492356a4`**。与参考深色像素 IoU 仅0.373，笔画明显偏细，仍非1:1。详见[按时序验收与实际截图](../validation/2026-09-28-gamehub-background-and-v14-outline.md)。
- 为避免对模型过度承诺，macOS capability 改为机器可读“后台 CGEvent 仅尝试、无原生确认、需独立读回”，Plugin macOS Skill 同步真实边界。该阶段全仓离线 **524 passed/10 ignored**、fmt/Clippy `-D warnings`/diff check 与 Skill 校验通过；**当时尚未部署**，后续 GameHub Live 修复时已同证书部署并经 Host 读回 `background_input`。无提交/推送。

## 2026-09-26 重启后最新增量：生产 image 引用防线与 V14 上箭头图标

- 用户重启后，本任务新 Plugin Skill/工具描述已加载；当前任务真实 RWI Host 的正常 Paint AX路径可 `snapshot`，但初次 `computer.image` 没有 `input_reference_available`，揭示先前只在本地 Server 隔离 visual-only view，而生产 Agent `Executor::image` 仍无条件登记。已补生产 Agent 仅有Binding+像素映射才签发 view_id、Gateway 对旧Agent误发ID做二次裁决；离线生产路径/兼容负例通过。最终全仓 **524 passed/10 ignored**、fmt/Clippy/diff与release构建通过。新签名Agent PID52564 / instance `8d23148e-c118-4ab3-a0ce-27951f8ca6b1`、四项TCC不变，MCP与Plugin `0.2.0+codex.20260925172749` 已更新。重启后当前Host正常路径 `input_reference_available=true`，2048×942 `capture_visual` 仍通过；真实 Wine/GameHub GUI 未出现，visual-only 的 Host实机分支仍待验收。详见[生产引用补漏](../validation/2026-09-26-production-view-guard.md)。
- Paint V14 的大方框网格确认是 Paint 编辑覆盖层；RWI原生 `Edit→End Editing` 清除后发现误画圆角圆形，已Undo。第二次过粗试笔也被5次有界Undo并原像素确认消失。最后绘成1px黑色圆角方框+5条独立黑色箭头线，RWI真实 `cmd+s`，正式源与[155对象阶段快照](../../outputs/paint-v14-source/paint-v14-20260926-sidebar-upload-icon.paint)均485659 bytes、SHA‑256 `a38ceac4c7e0306d378354f71ad34fd55044382a92c88c02aa1b1ee89f11af7b`。参考/实际图标暗区bbox相同、暗像素127/125，但IoU0.370且边缘形状不同，不是1:1。真实截图/过程见[上箭头方框阶段](../validation/2026-09-26-v14-upload-icon.md)。此前149对象快照保留。未提交/推送。

## 2026-09-26 最新增量：非标准 Mac 窗口只读回退与 V14 九宫格

- 子任务在同一仓库实现了**仅在 AX 明确无窗/无匹配时**的 CGWindow+ScreenCaptureKit 精确窗口 visual-only 快照；AX 超时、权限/身份/epoch 不确定、搜索截断或歧义不降级。返回原像素图和 `computer.image` tile，但无 AX 绑定、无可解析 `view_id` 或输入授权；`resolve`/`act_checked`/`resolve_view` 负向测试通过。**2026-09-26 已部署新 Agent/MCP/Plugin**。部署回归发现并修复 2048px `capture_visual` 把 Retina 逻辑点误当原生像素的校验问题；当前 Host Paint 实测 2048×942 成功、原生来源2868×1320，既有 AX snapshot 仍正常。最终全仓离线 **522 passed/10 ignored**、fmt/Clippy `-D warnings`/diff check 和 release 构建通过，四项 TCC 与原代码签名不变。**尚无实际 Wine/GameHub GUI 做新回退的实机验收**；虚拟屏 Steam Support 实为 Arc PID。详见[实现边界](../validation/2026-09-25-nonstandard-macos-visual-only.md)与[部署/Retina回归](../validation/2026-09-26-visual-only-deployment-retina.md)。
- 继续已分配虚拟屏的 Paint S V14：九枚独立可编辑黑色 Ellipse 组成侧栏3×3九宫格，RWI原像素截图验证各行位置。正式源及[阶段快照](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-grid-nine-dots.paint)一致：**149对象、484566 bytes、SHA256 `8fc26d801ca6940a723e6693406d136ceeb5959a9f2c3cea53b9e8e76f3d3943`**。与参考比，中列点右沿多1px，纯黑面积偏大；文字重叠、酒红轮廓与其余图标仍待修，不是1:1。详见[按时序验收](../validation/2026-09-26-paint-v14-sidebar-grid.md)。无 Git 提交/推送。
- 后续尝试侧栏圆角方框/上箭头时，RWI 完成黑色描边设置与一次拖拽，但实际画面仍出现大于参考的方框和网格/控制柄样线；背景 `Escape`、空白单击回执未证明视觉状态改变。此**应用内试笔未保存**，149对象正式源哈希复核不变；用户会话键鼠活跃时先暂停输入，之后再安全确认/清理。见[未保存试笔证据](../validation/2026-09-26-upload-icon-trial-pending.md)。

## 2026-09-25 当前最新 V14 增量：侧栏渲染像素校准落盘

### Picked Color 跨屏闭环与消息图标两白点阶段（最新）

Paint S 的 **Picked Color** 面板由虚拟屏上的取色动作触发，却首次原生发现于内建屏；RWI精确绑定读回蓝 `#1A83FE` / 白 `#FFFFFF`，分别用 `computer.place_window` 按BetterDisplay UUID移至虚拟屏，AX设置描边/填充并关闭。没有臆断系统默认定位原因或全局自动移窗。原参考仅**两颗**白点，已纠正先前“三白点”记载。RWI GUI新增一条蓝色尾线和两枚2×2白色Ellipse，过大的首个试笔已删除并在原像素中确认消失；正式源/新阶段快照 **140对象、482325 bytes、SHA256 `8be9b19056418ff466790036291afdb2129c98e8bf35088902f6566c119295c9`**。参考蓝区 bbox `(29,66)..(50,87)`，当前 `(29,68)..(49,87)`，白点纯白面积偏大；仍非1:1。详见[逐步验收](../validation/2026-09-25-picked-color-and-message-icon.md)。本轮无Rust代码改动/部署、未提交推送。

### 色板弹窗严格绑定修复与消息图标底形（最新）

Paint 色板的独立 CG 窗口先前虽能发现/绑定，AX 观察却 `window_missing`。现将 AX 查找扩到有界后代，并只对确切绑定且 hit-test 祖先链匹配的 `AXPopover` 解开未知 AXModal；真实 Plugin 已读到其526×692原生画面、`blocked=false/mutation_ready=true`，再经色板 `Input Color Manually` 实际打开颜色对话框。AXPress 的 `-25204` 仍按 uncertain 查询原请求、不重放；输入/确认和跨屏移窗仍由 RWI 完成。Agent 同证书签名部署到 PID48339，四权限有效；最终全仓 **517 passed/10 ignored**、fmt/Clippy/diff 全过。当时保存的**历史阶段**源/快照是**137对象/12 Image/12 Ellipse、478263 bytes、SHA256 `de9a6f9e352cb4fd0e3f89fc9448f9f0781bc17f83edc9b310fb3b8a301693ca`**。新增独立蓝椭圆已与原参考主体色 `#1A83FE` 对齐，但当时 bbox、气泡尾部和两颗白点未完成；先前的 `#1697FF` 取色结论已更正。见[色板与蓝色底形实测](../validation/2026-09-25-popup-binding-and-blue-base.md)。未提交/推送。

### Codex 重启后的 Host 验收补记

新任务的工具表已实际加载 `computer.live hide`；同一 mac-host Agent PID40775、四项 macOS 权限和虚拟屏 Paint 主窗绑定均正常。直接 `hide → resume → stop` 回执、原生窗口读回和 worker 清理通过；`computer.snapshot(max_edge=2048)` 实际交付 2048×942 预览（原图2868×1320），正式 `.paint` SHA 未变。**限制**：本次 `open_in_codex` 两次均返回 `queued`，Host viewer 没有实际可见，Live 采集始终安全悬停；不把状态机通过说成可见浮层/任务切换已复验。需真实面板可见时补测。详见[重启后逐步验收](../validation/2026-09-25-live-post-restart-check.md)。

- 从同尺寸参考/RWI 原生局部确认侧栏背景原为 `#E0E0E1`，目标 `#F5F5F5`。RWI 实际 Paint GUI 新增无边框、可编辑64×627 Rectangle；Computer Use 只因 RWI 暂不能绑定色板 popover，辅助选择 `Input Color Manually` 并使用 Paint 原生 Object 菜单把新底色放至栅格底图之上、现存图标层之下。真实 Input Color 对话框每次弹到内建屏时由 RWI `place_window` 移回虚拟屏并确认，再做色值检查/确认。旧酒红侧栏占位圈被遮盖，头像/列表图层未合并。
- 特别核对出**输入 `#F5F5F5` 不等于原生截图渲染 `#F5F5F5`**：前者实际出 `#F7F7F7`，根据既有色块实验把新背景及联系人/星/#三个局部底块输入校准为 `#F2F2F2`。RWI最终原生420×680截图里五个平坦侧栏采样点与参考均为 `#F5F5F5`，不扩大为全图1:1。正式源与新快照 `outputs/paint-v14-source/paint-v14-20260925-sidebar-render-calibrated.paint` 同为**136对象/12 Image/28 Rectangle、477879 bytes、SHA256 `c8b0e73295e8e928c98174e245efe11f59be311c864fd05ec76f6cf2a978f55c`**；第8行/初版底色/错误色值快照均保留。下一优先是蓝色消息气泡 `#1697FF`、缺失图标、选中行旧酒红椭圆和文字重叠，仍非1:1。详见[逐步渲染色验收](../validation/2026-09-25-v14-sidebar-render-color.md)。
- 过程中对一次 RWI 聚焦颜色输入框的 `stale_reference` 查询到 `native_input_dispatched=false`、未围栏；未重放该请求，改用 Computer Use 对已识别输入框设值、RWI新鲜观察后确认。色板 popover 的 CG窗口虽在目录，却在RWI精确绑定后 `snapshot=window_missing`，是待修的原生瞬态身份缺口，不以 CUA 成功掩盖。无新增产品 Rust 代码/重新部署、无 Git 提交/推送；画稿源已保存。

## 2026-09-25 当前最新增量：Live 黑边、隐藏/关闭与 V14 精确核对

- 复现旧 Live 2048×942源在1040×577pt浮层里因固定2:1预览槽/近黑背景形成的大面积磨砂黑边。改为按源宽高比定尺寸（同屏实机新浮层1040×526pt，减少51pt）、系统浅色 `NSVisualEffectView` 材质、去窗口阴影；当前 RWI 原生浮层截图显示黑边消失。新增原生**隐藏**（清帧、移出屏幕、会话可 `resume`）与**关闭**（Stop），保留旧 `pause` API。真实 UI 点击隐藏虽在 Computer Use 报超时，RWI 独立状态/原生窗口发现证实已隐藏，没有重放；`resume` 后同 bounds恢复；关闭后 `worker_done=true`、无在途 native call。
- 新 Agent 已同证书签名部署，Mac 四权限保持 true，MCP 二进制和 Codex Plugin `0.2.0+codex.20260925131605` 已更新。当前任务旧工具 schema 不能直接调用新增 `hide` 枚举，**原生按钮与旧 `resume` 已在当前任务实测**；下次 Host 重新加载才可验收新 MCP 参数。全仓 **517 passed / 10 ignored**，fmt/clippy/diff/release通过；未提交/推送。虚拟屏上实测，用户内建物理屏未占用，故不能冒称物理屏视觉验收。对照与边界见[逐步验收](../validation/2026-09-25-live-panel-polish-and-v14-audit.md)。
- V14 `.paint` 未改，仍135对象/12 Image、SHA256 `3f4c26dc9517edc512c832df417965ef2124825a2246bd0269dbc76d092d6423`。RWI原生侧栏局部与参考等尺寸视觉配准：参考侧栏平坦背景 `#F5F5F5`，当前 `#E0E0E1`；参考首枚消息图标蓝 `#1697FF`，当前为空白灰底；选中行背景也有小差异、酒红占位椭圆/横线与文字重叠仍在。后续优先以可编辑遮盖/形状逐项修复，逐次保存对比，不把整图贴入底层。

## 2026-09-25 当前最新 V14 增量：第 8 行仅参考可见头像上部

- 原参考第8行头像在 x162/y1103 的82×82范围内，仅顶部56px实际可见，黑色截断从 y56 开始。确定性裁切可见82×56、按比例缩放并取48×32成品，**没有**导入黑色带或臆造被截断的下半部。RWI Plugin GUI 真实 Add Image/Open/拖动至画布底缘、Escape、`cmd+s`，新增第12个独立 Image；AXOpen `-25205/uncertain` 后未重放，以面板关闭、图层、落盘归档和原像素截图核验。
- 目前正式源与 `outputs/paint-v14-source/paint-v14-20260925-avatar-row8-visible.paint` 同为**135对象/12 Image、477415 bytes、SHA256 `3f4c26dc9517edc512c832df417965ef2124825a2246bd0269dbc76d092d6423`**，新层 archive bottom-left `[478,0,48,32]`、top-left `[478,647,48,32]`，刚好贴画布下缘。第7行及对齐阶段源均保留。仍有酒红细边/选中行外椭圆、文字、形状、布局差异，不是1:1。无产品代码改动、部署、提交或推送；详见[第8行实机验收](../validation/2026-09-25-v14-avatar-row8-visible.md)。

## 2026-09-25 最新 V14 增量：底图占位环诊断、头像对齐与第 7 行

- 只读解包正式 `.paint` 的底层 PNG 证实酒红色头像占位环**已栅格化在底图**，并非可单独删掉的 Ellipse。先前第3/4/6行外露的大块弧线主要来自新增 Image 相对底图错位；本轮在实际 Paint GUI 用 RWI 逐像素微调第3行下2/右2、第4行下8/右2、第6行上7，第5行保持。对齐阶段仍133对象/10 Image，保存独立 `outputs/paint-v14-source/paint-v14-20260925-avatar-aligned.paint`（466241 bytes，SHA256 `d127d91ab7991a0dd96e31ddf0060176ab5b4bf4197e0f53b9701f6111602114`）。真实原像素截图确认大块弧线减少，约1px细边及选中行外椭圆仍留。
- 第7行蓝色游戏手柄头像从参考精确裁82×82、缩48×48，RWI Plugin GUI 的 Add Image/Open 导入为新独立层、拖至目标、再下移1px对齐，Escape 与 `cmd+s` 实际落盘。AXOpen 返回 `-25205/uncertain`，但面板独立发现已关闭，新增图层/保存归档和截图确认效果；原请求未重放。当前正式源与 `outputs/paint-v14-source/paint-v14-20260925-avatar-row7.paint` 同为**134对象/11 Image、472829 bytes、SHA256 `7332d4c2033f73ac4fad4a53116bbc0ae6b785b91ce63a690eba2510856e4730`**；新增层 bottom-left `[478,59,48,48]`。旧阶段快照均保留。
- 第8行头像在原参考中仅可见上部，下部被原始截图截断；不导入包含黑色遮挡的82×82素材。V14 仍未达1:1，文字、选中行外椭圆、细轮廓与其余图形需继续复核。Undo操作历史跨重启持久性未知。本轮只做已部署 RWI 的真实 GUI 验收和阶段文档，**无产品Rust代码变更、无部署、无提交/推送**。逐步证据：[占位环诊断/对齐/第7行](../validation/2026-09-25-v14-avatar-alignment-and-row7.md)。

## 2026-09-25 最新 V14 增量：第 4–6 行头像独立图层落盘

- 原始参考图第4–6行头像各精确裁82×82、缩至48×48；当前任务 RWI MCP 在 BetterDisplay 虚拟屏的 Paint S GUI 中分别经 Add Image/Open、画布拖放、Escape 与 cmd+s 导入并保存为**三个独立 Image**。每步不确定 AX 回执均先独立发现与快照，从未重放。正式 `.paint` 从130对象/7 Image递增到**133对象/10 Image**；当前工作源与新快照 `outputs/paint-v14-source/paint-v14-20260925-avatar-row6.paint` 均466241 bytes、SHA256 `b804a35ad576296a170e07fb29eb72bc24345f6a51f20f5a74f73b8f9449c652`。旧快照保留。
- 第4/5/6行新增图层只读归档 bounds（Paint 内部自下而上坐标）分别 `[476,292,48,48]`、`[478,208,48,48]`、`[478,127,48,48]`；真实 RWI 原像素局部截图按时间保存在 `outputs/validation/2026-09-25-v14-row{4,5,6}/`。**仍非1:1**：旧底图酒红色占位圆弧在头像周围露出，位置需再微调；第7/8行头像、其他图形/文字也未完成。详情：[第4–6行逐步验收](../validation/2026-09-25-paint-v14-avatar-rows4-6.md)。
- 本轮还复现参考窗有动态选框时背景点击可能在实际鼠标派发前报 `stale_reference` 却围栏输入域；已查询原请求并通过原生键鼠/writer-idle检查解围，不重放。RWI AXOpen 常返回非零但实际导入，继续按独立画面/文件验证。这两类诊断列为后续性能与回执优化。未改产品 Rust 代码、未部署、未提交/推送；Undo 历史跨重启持久性仍未证明。

## 2026-09-25 最新增量：模态弹窗与放置回执可信度

- 隔离 Paint 副本实测普通 `Resize` 弹窗落在内建屏，但 RWI 以原生 `AXPosition` 将其移至 BetterDisplay 虚拟屏并独立发现确认。Paint S 仍报告主文档为 `AXFocusedWindow`，旧版把实际可见的 `AXModal` 弹窗误标为 blocked/occluded；现仅在弹窗自身 AXWindow/PID/CG ID 精确命中时放行。部署后快照变为可操作，RWI `AXPress Cancel` 后独立确认弹窗关闭。`Resize` 首次触发返回 AXError/uncertain，但实际弹出，未重放该输入。
- 窗口级无像素引用现在可进入**放置**，仍不能进入键鼠坐标输入；语义弹窗须重验自身聚焦与几何。实机 Save Sheet 的 AXPosition 虽报可写/调用返回，位置实际上没动；此前仅按“仍在目标屏”报 `placement_confirmed=true` 属误报。已修为必须到达**请求起点**且连续两次稳定。新版 Host 返回 `placement_confirmed=false`（15 次/975ms），独立发现逻辑/可视 Sheet 均未移动。测试副本由 RWI `Don’t Save` 关闭，正式分层画稿未改。先前的跨屏提示框本轮未再次出现，不能声称已解决其放置。
- 最终 516 passed / 10 ignored、fmt/clippy/diff/release 全绿；签名 Agent 已部署，TCC 四项仍 true，MCP/profile/config 未变。macOS Skill 同步新约束，Codex Plugin `0.2.0+codex.20260925071534` 已安装，当前任务的已加载工具无需重启即可继续，后续重载可读新 Skill。无提交/推送。下一步优先继续面板跨屏策略与 V14 正式复刻，并单独测 Live 长输入争用；详见 [本轮逐步验收](../validation/2026-09-25-popup-placement-and-truthful-readback.md)。

## 当前增量：Live 实机任务切换与 Retina 清晰截图

- Codex 重启后，新可续签配对的生产 MCP ↔ Agent 连接健康；在 BetterDisplay 虚拟屏的 Paint S 上真实验证任务切走即隐藏并清屏、返回即恢复原生 Live，而不只依赖可见性探针。
- 用户指出小字发糊后，修正 ScreenCaptureKit 逻辑点/Retina 像素混用；Live 和新 MCP 快照默认上限改为 2048px。实机 Live 源 2048×942、面板 1040×577pt，原始 RWI 全窗截图已保存为本地验收证据；原始像素裁切仍可取细节。本任务旧 gateway 须显式传 `max_edge=2048`。
- 全仓 507 passed / 10 ignored、fmt/clippy/diff 和 Agent release 构建通过；新版 Agent/MCP/Plugin 已部署，稳定代码签名、TCC、TLS profile 均未失效。未提交/推送。可编辑 `.paint` 磁盘源校验未变；V14 **未完成**。
- Paint 普通 Resize 弹窗已通过发现/快照/取消验收，但逻辑 `semantic_only` AXDialog 尚缺真实闭环；Live 长操作争用、离线 Windows 导致全设备 targets 5s、macOS capability 文案及 V14 仍待推进。详见 [2026-09-25 验收](../validation/2026-09-25-live-clarity.md)。

## 后续增量：定向 targets 避免离线端点拖慢 Mac 工作流

- `computer.targets({device_id:"mac-host"})` 已实现并部署，准确只检查已知配对端点；全设备调用保持旧行为。独立生产 profile 新 MCP 进程实测定向 **16/47ms**（release/安装版单次）、全设备 **5003/5002ms**（离线 Windows 超时），不把这组数字称为 Mac GUI 操作耗时。当前任务旧 MCP schema 需 Host 重载才接受新参数。
- Mac capability 元数据纠正 pointer feedback、focused semantic AXDialog 和模态策略；真实 Agent 重启后四项 TCC 仍 true，Paint 的 2048×942 Live 已恢复。JS SDK 预览默认2048；Plugin `0.2.0+codex.20260925031512` 已安装。
- 全仓 **508 passed / 10 ignored**、JS SDK 44 passed、fmt/clippy/diff/release 通过；可编辑 Paint 磁盘源 SHA 不变，未提交/推送。下一优先：实机 `semantic_only` 对话框点击闭环、Live 长操作争用与 V14 继续复刻。详见 [定向发现/元数据验收](../validation/2026-09-25-selected-targets-and-capabilities.md)。

## 重启后最新增量：动画画布输入与逻辑 Sheet 实机闭环

- 当前 Host 已加载 `computer.targets({device_id:"mac-host"})` 新 schema，实测约 25ms、四项 TCC true。隔离的 Paint S 临时副本在 BetterDisplay 上完成实际像素操作；仅 135 个选框动画像素变化曾让旧整窗哈希门槛拒绝，耐久回执确认旧请求没有输入。新版 Mac 局部视觉锚只在严格原生 `AXScrollArea` 画布校验下容忍该类微小动画；拖拽后真实图层从 **48×48 变为 51×58**。正式源未改。
- 真实提示框以不同的逻辑 AXDialog/可视 CG ID 呈现：绑定逻辑 ID、无像素快照、原生 AXPress `OK`、发现关闭均通过。进一步修复 `AXRole=AXSheet` 但无子角色、hit-test 错落主文档的 Paint 保存面板；严格使用其自身 CG ID/PID/几何及聚焦身份，只允许 AX 原生动作，`Don’t Save` 已安全关闭**测试副本**。
- 最终全仓 **514 passed / 10 ignored**，fmt/clippy/diff/release 通过；本地签名/四项 TCC/配对配置不变。Paint 提示框仍可能落在内建屏；长输入时的 Live 争用和 V14 精细复刻仍待推进。详见 [动画画布与语义 Sheet 验收](../validation/2026-09-25-animated-canvas-and-semantic-sheets.md)。

## 当前优先补充：任务可见性 / AX 逻辑弹窗 / 可续签配对

- 用户已实际切换任务并返回，纯可见性探针确认任务内浏览器面板能发布 hidden/visible；新版 Live 使用该信号而不是把 MCP 连接当作前台任务。关闭/遮盖该面板也会隐藏，不声称原生 Host task-ID 绑定。
- Native 短期可见性租约、清屏/隐藏/恢复、旧页面/乱序隔离，以及 Paint 逻辑 AX dialog 的独立 semantic-only 路径已实现并回归，真实 overlay 切换和该类弹窗操作仍待部署后验证。
- 用户明确批准可续签 TLS 配对。已部署本机私有 CA 保留、同端点 key 的 SPKI 配对、到期前续签、新握手热读 leaf；明确的 `journal_identity` 映射保留旧去重身份，不替换 2036 年到期的 macOS 代码签名证书，不改 Windows 配对。
- 截至 **2026-09-24** 全仓 505 passed / 10 ignored、Agent fixture 44 passed，fmt/clippy/diff/release 通过；当时新 MCP 生产传输 healthy、四项 TCC true、账本与 Paint 源校验不变。Plugin `0.2.0+codex.20260924154944` 已安装；当时当前任务旧 MCP 仍旧 CA、GUI 验收待重启。**2026-09-25 已完成该重启与上述 Live 实机验收**，以本页当前增量及新验收记录为准。
- 未提交/推送或触发远端 Actions。详见 [分层验收记录](../validation/2026-09-24-task-visible-live-and-renewal.md)。接续顺序：新 Host Live/弹窗验收 → 延迟/输出及 capability 文案修正 → Paint V14 可编辑源文件持续推进。

## 正在执行的顺序

| 顺序 | 交付项 | 当前状态 / 下一步 |
|---|---|---|
| 1 | 焦点变化、窗口切换、重新接续 | 已补单次显式激活、异步完成/用户退让/最终事件收集、独立前台事实、旧观察失效；再补最小化恢复与前台分步，真实Host约105ms恢复Explorer。窗口展开仍未获得精确控件焦点时，显式点击地址栏后才发键。整Paint进程关闭、服务仍健康、GUI启动新Paint并重开原图均已验证。真实人工介入与异常窗口重用仍单列验收 |
| 2 | 语义动作性能 | 精确 runtime ID/祖先重验已落地、公开负向回归及 Paint/Explorer/文本实测通过；旧版仅一个有效基线，不声称完整 A/B。公共 observe 仍有 UIA 遍历延迟，继续第 3 项 |
| 3 | 可视化与操作通道分离 | capture_visual 已由重启后的真实 Host 加载并实测，不产生input frame；新版面积平均1024px预览保留本次细线，原生512px切片SHA升级前后相同。ROI source_region重验已实机画出第三个闭合菱形。公共语义 observe 仍需缩减全树遍历开销 |
| 4 | 连贯多步操作 | withInput 已补同target多wrapper独占、单调调度预算、重复ID/dedup停止、在途读/输入与异常留证；真实三步工具按钮批次约6.54s，另已用一个Host原生MCP JS单元完成两笔640ms轨迹，整批约5.33s，原生512px切片确认两个闭合菱形。缩略细线已修且实测；菜单focus/pointer身份表示继续修正，不称效率完全达标 |
| 5 | 完整真实验证与简洁交付 | 重启后真实Host已加载45ae版；agent重启后的首次查询自动单次重连成功，原操作记录未变。1200ms截止后的原生停止/一次租约清理成功，显式恢复仍保留不确定历史。最小化修订独立393默认/480全feature门禁通过并实测，无需再重启Host。原Paint PID2024已退出，新PID7372由GUI重开原图；文件与512px切片SHA均不变。另按新发现的窗口尺寸重绑，3次观察约712/591/616ms，只读截图约140ms，不算跨版本A/B。截图36步，历史失败全部保留 |
| 6 | Mac 受控端 | 已在真实 Codex Plugin 与 Paint S/BetterDisplay 虚拟屏上验证 CG 窗口发现/绑定、后台观察与绘图输入、显式移窗稳定回读、反馈光标和无输入帧的 `capture_visual`；active display UUID 已进入发现目录。Agent 四项 TCC true。Mac 显式激活、可靠跨进程面板关联、会话级 Live 叠层和 Paint V14 全量复刻仍未完成；见 2026-09-24 验证记录 |
| 7 | 跨端和架构收尾 | agent_binding从1592行纯拆分至根222行/子模块≤310行，76函数体和12类型保持；继续剩余模块债务、生产feature、多端故障隔离与资源上界。已有离线通过不能替代未测真实平台 |

## Mac 绘图应用实测（2026-09-14，进行中）

- 目标：内建屏上用 Freeform / Paint S 临摹窗口示意图并对比 MCP 体验；证据与对比表
  见 work/macos-mcp-draw-comparison-20260914/（work/ 为中间产物，不提交）。
- Freeform：后台观察/元素点击/拖拽派发可用；但"Draw with Pen"位于 Insert Shape 瞬态
  面板（AX visibility=unknown/occluded），element/pixel/checked 三路径分别以
  `visibility_unknown`/`stale_reference` 拒绝，无法进入画笔模式；选择模式下拖拽只框选。
  仿图未完成，画板仅留此前插入的少量对象（freeform-board-20260914.png）。
- Paint S（沙盒）：窗口/画布可用；形状工具可创建对象（图层与尺寸可读），但默认
  Border:None 且颜色/样式面板为独立瞬态 UI 不可达；新建文档模态尺寸面板不可完成
  （`capability_unavailable` owner-chain 未实现、sheet AX `window_missing`），只能以
  打开已有图片绕过。仿图未完成（paint-s-window-region.png）。
- 对照 Paintbrush（非沙盒）同一路径已完成 29 笔仿图与形状版仿图 → 差别主要来自
  应用 UI 结构（AX 暴露度、瞬态面板、沙盒、模态窗口）。
- 已修复并部署（本机 agent abd74948，离线门禁 fmt/clippy/test 全绿，提交 84bacf9/
  2fb57b5/1aa6d74）：
  ① 身份改用 boot 秒级（修复 kern.boottime 微秒漂移引发的全量 session_mismatch）；
  ② 窗口解析支持 AX 子级 sheet/对话框（修复模态面板 window_missing，使其可观察可操作）；
  ③ 窗口发现/绑定携带所属显示器（id/built_in/bounds）。
- 授权显示器**放置**（提交随本轮，实测通过）：策略开启时，绑定落在其他屏的窗口会先经原生
  AXPosition 放到授权屏（无合成输入、无激活），原生复查通过才绑定并在响应里报告
  `placed`；放不动则拒绝。实测：TextEdit 打开时在扩展屏（built_in:false）→ 绑定后被放到
  内建屏（bounds 306,1190 / display id:1）并绑定成功；已在内建屏的 Paint S 窗口
  `placed=null` 不多余移动。说明：应用启动本身仍由 Host/macOS 决定落点（macOS
  launch_application 未实现），首个绑定即完成归位。
- 授权显示器门槛（已实现+实测，提交随本轮）：macOS 平台策略新增可选
  `platform.authorized_display={built_in,id?}`；绑定前校验窗口所属显示器，不匹配或无
  唯一归属即拒绝 `unauthorized_display`（不绑定、不输入）。实测：扩展屏（built_in:false）
  的 TextEdit 被拒；内建屏 Paint S 正常绑定。已部署（agent a7d6ee11，本机 agent.json 已开
  启该策略）。窗口**移动/放置**原语仍需新的 MCP 工具面（工具清单为固定 19 项且带历史录
  制校验），另列。
- 显示器归属实测：Paint S 当前唯一文档窗口 display={id:1,built_in:true}（内建屏）。
  落点机制：macOS 主显示器=外接 IG27Q，未定位的新窗口由系统/应用放在主显示器
  （首启 Start 面板曾出现在扩展屏），随后应用会恢复其保存位置（Prefs 中
  StartOpenPanel 保存的是内建屏坐标）。macOS 尚无启动/放置原语
  （launch_application 未实现），Host `open -a` 只能接受系统落点。
- macOS 后台启动·实现方案（本轮调研+读码结论，下一步实施；不改 MCP 工具面）：工具已存在
  （computer.launch_application/lifecycle_preflight/lifecycle_receipt，19 项不变）；rwi-core
  的 lifecycle 为通用编排（Engine::launch_application → backend.lifecycle_grant/deadline/
  driver + 共享 lifecycle::launch/preflight/receipt，含持久化预留与不重放），macOS 侧只需：
  ① LifecycleDriver：`LSOpenApplication` + `kLSLaunchDontSwitch|kLSLaunchAndHide`（可选
  `kLSLaunchNewInstance`）后台启动 + 原生 inventory（CGWindowList/进程）前后对比；
  ② 启动后 50ms 轮询发现新窗口 → 复用已实测的 AXPosition 放置（授权屏）+ 原生复查；
  ③ 不生效降级链标记：后台 → 独立屏/屏外 → 前台（disruptive 明示，体感输入化）。
  出处：Apple LSOpenApplication/LSLaunchFlags 文档、NSWorkspace.OpenConfiguration
  (activates=false/hides=true, silent.swift gist)、open -g/-j 对 Electron 类不生效的实测贴。
  **实测修正**：弃用 Carbon 路线 `LSOpenApplication` 在本机 SIGSEGV（已回退并留档）；已改用
  objc2（依赖树已有）调 AppKit `NSWorkspace.openApplicationAtURL:configuration:`——
  实测①（hides=true）：Calculator 后台启动、`lsappinfo front` 未变、状态 `(hidden)` ✓；
  实测②（hides=false + 快归位流水线）：后台启动 → 50ms watcher 捕获首个新窗口 →
  AXPosition 归位到内建屏 → 原生复查 bounds=(306,1190,230,408)、前台仍未变、全程 2.32s ✓。
  **已知约束**：hidden 启动的窗口不在 on-screen 发现范围（watcher 找不到），故流水线以
  "可见但不抢焦点"启动、隐藏作为后续独立步骤。
- 待实现（用户要求，高优先级）：受控的窗口移动/放置原语（将目标窗口强制放到授权
  显示器），并把"仅授权显示器"作为默认可操作门槛；③ 的识别已就绪，可作为策略输入。
- 弹窗/对话框实测（2026-09-14 续，提交 98a37fd）：Paint S 的 "Open" 面板（模态 sheet，
  内建屏）修复前绑定后观察 `window_missing`；经 AX 实机诊断确认它既不在 AXWindows 也
  不在窗口子级、只暴露为应用 AXFocusedWindow，修复后绑定+观察成功（Cancel/Open/Where/
  sidebar 等控件全部可达）。同时发现面板 AXModal 不可用会因"未知模态即拒绝"被误伤，
  已改为：未知模态仅当该窗口正是应用聚焦窗口（即交互面本身）才放行，否则保持 fail-closed。
  Cancel 点击的端到端复核未完成（面板在复核前已被关闭，重开采样窗口内未再出现）——下一轮
  首要项。
- 续测（提交 4015873，agent 3ca5e720）：面板输入前置条件已全部放行——观察
  `mutation_ready=true / blocked=false`，且 Cancel 点击两次均 `dispatch: returned`
  （native_dispatch_entered=true，修复前是 window_missing/visibility_unknown/not ready）。
  但点击后的**面板未被关闭**（延迟复查仍在，且期间 CGWindowID 13550→13725 变化一次）：
  即"能派发、未生效"。已知事实：面板是应用 AXFocusedWindow、元素点/窗口局部坐标正确、
  CGEventPostToPid 无 OS 回执。下一轮方向：确认应用是否处于 active/key 状态（必要时补
  key-window 前置）、并评估用原生 AXPress 作为后台对话框按钮的回退动作。
- 续测 2（同日）：`double_click` 派发成功但面板仍未关闭；用 Host `open -a` 真实激活
  Paint S 后再点一次 Cancel，依然"派发成功、无效果"——排除"应用非 active"假设。
  同一交付路径在**普通窗口**上是有效的（此前 Paint S 主窗口的工具切换/形状绘制均生效），
  差异集中在 **sheet/panel 窗口的事件路由**：候选修复方向 =（a）把事件按父窗口号投递并用
  父窗口局部坐标（sheet 可能在父窗口事件路径内转发），（b）为对话框控件提供原生 AXPress
  动作回退（VoiceOver 同源，不依赖合成事件路由）。
- 判定实验（提交 22bbc67）：在实机诊断中对同一面板的 "Cancel" 按钮执行原生
  `AXUIElementPerformAction(AXPress)` → 返回 0（kAXErrorSuccess），**面板随即关闭、只剩
  文档窗口**。结论：sheet/panel 控件的**原生 AX 动作路径可用**，合成点击路径对该类窗口
  无效。实现（提交 2e05a56，不改动 MCP 工具 schema）：macOS 后端在 `click` 时先判定"该
  窗口是否不在应用 AXWindows 中"（即合成事件到不了的 sheet/panel），是则对同一接地元素
  执行原生 `AXPress`，保留元素身份/可见性/启用/模态/租约/不重放校验，且不发送合成事件与
  primer。**端到端验证通过**：经 MCP 常规 `computer.act`(click) 关闭 Paint S "Open" 面板，
  复查只剩文档窗口。模态对话框链条（发现→绑定→观察→校验→派发→生效）全线打通。
- 本轮未完成：sheet 修复的端到端模态验证、独立验证子会话、Mac 视觉反馈（AI 光标层）、
  正式验收文档与总提交。

## PUA Loop 验收（2026-09-15，Oracle 通过）

- 验收命令（冻结于 .claude/pua-loop-state.md）：`bash work/pua-loop/verify-mac-apps.sh`
  → **ORACLE_OK / exit 0**；证据：`work/pua-loop/evidence-*.json` 与 history.jsonl（8 轮）。
- 覆盖（其他应用不纳入）：Paint S → 后台启动/落授权屏/绑定/观察(77 元素)/点击 Add Image →
  面板出现并可观察(Cancel/Open/Where…) → Cancel 关闭（原生动作）；Freeform → 板窗口绑定/
  观察(Insert Shape 等) → 点击 Insert Shape → 弹层打开(Draw with Pen) → 画布点击关闭。
- 过程中修复的产品缺陷（提交 b30827e）：
  ① 恢复死锁：combined-session 陈旧键(118)不再阻断恢复（硬件源/按钮/修饰键仍 fail-closed）；
  ② 非主窗口点击一律走原生 AXPress（合成点击对 sheet/panel "能派发不生效"）；
  ③ 输入域 fence 跨会话/重启持久的问题随之解开（配合 recover_input）。
- 明确未纳入：Freeform 画笔（Draw with Pen）点击与自由绘制仍受瞬态弹层可见性限制；
  Paint S 画笔/样式可见性；均记录为后续项，不冒充已完成。

## 弹窗一等交付实测（2026-09-15，#3）

- 场景：用户在 Paint S 里打开了系统文件选择面板（Finder 选文件弹窗）。
- 实测：该弹窗是**独立进程窗口**（CG 层可发现，owner pid ≠ 宿主应用）；绑定它时触发
  `unauthorized_display: Native placement did not move the window onto the authorized
  display; not bound.` —— 面板落在非授权屏且 **AXPosition 对系统面板不生效（不可搬移）**，
  已实测的授权屏策略因此 fail-closed 拒绝绑定（不绑定、不输入）。
- 结论与待决：① 授权屏策略按预期保护了"只在 MacBook 屏操作"的要求；② 暴露真实限制：
  系统面板可能不可搬移 → 需要设计"系统面板例外/显式豁免"策略（配置项或一次性授权），
  或由用户在正确屏幕上重开；③ 已在授权屏上的面板路径（bind+observe+AXPress）此前已实测可用。
- 伴随：`role`/`actions`/`enabled`/`subrole` 四项交付已上线（67/67、37/57、34/40、AXStandardWindow
  实测），动作白名单与派发路径均有活体回执。

## 事故与修复项：授权屏放置会作用于被选中的任意窗口（2026-09-15）

- 经过：调用方用一个宽松的标题匹配（"open"）误选中用户自己的 OpenCode 窗口；`bind_window`
  在授权屏策略下**先执行了 AX 归位移动**，随后因窗口(2048 宽)无法完全落入内建屏而判定
  `unauthorized_display` 并拒绝绑定。窗口被移动且不可逆还原（未记录原始坐标），已向用户披露。
- 缺陷 1（安全）：放置属于**有副作用的动作**，却由"绑定尝试"隐式触发；应对齐"无激活/无输入"
  的只读语义——**放置必须显式选择**（例如 selection 中显式放置请求或独立原语），不能因策略
  在绑定路径里自动执行。
- 缺陷 2（健壮性）：窗口大于目标屏时应**在移动前**判定不可放置（预检尺寸），而不是移动后
  再以"未归位"拒绝——避免"动了一半"的中间态。
- 缺陷 3（可用性）：缺少显式的"把窗口移到指定屏 X"原语；用户/agent 无法主动纠正窗口位置。
- 修复顺序：缺陷 2（预检，小）→ 缺陷 1（显式化放置，中）→ 缺陷 3（显式移动原语，中）。

## #3a 显式窗口移动原语 · 可行性判定（2026-09-15）

- 关键结论：**新增 MCP 工具不会破坏历史录制校验**——`startup_diagnostic.rs` 与
  `orchestration/mcp.rs` 的 `tools.len()!=19`/`schema_sha()` 校验读的是**历史 fixture 自身**
  （自洽校验），rwi-mcp 网关侧未发现对实时工具目录的数量/哈希断言。
- **施工清单（按序照单执行，每步一编译）**：
  1) `crates/rwi-core/src/lib.rs`：新增引擎方法 `move_window(target, lease, reference, display)` ——
     参数借 `Resolved`（元素/像素接地引用）+ 目标显示器（`built_in` 或 `id`）；语义=原生移动，
     含尺寸预检、失败 fail-closed、持久台账（复用 act 的 journal 路径与 request_id 去重）。
  2) `crates/rwi-macos/src/native/metadata.rs`：已具 `place_window_on`（含 `placement_impossible`
     预检）→ 仅需暴露一个"按显示器 id/内置判定"的包装与 `Display` 查找。
  3) `crates/rwi-windows/src/...`：同名后端方法返回显式不支持（fail-closed，不静默）。
  4) `crates/rwi-agent/src/runtime`：在工具目录（`catalog::executable/validate`）注册
     `computer.move_window`（write 工具，需 input 授权与租约），并加入 `capabilities.rs`
     的工具清单；执行分支调用引擎方法。
  5) `crates/rwi-mcp/src/server/mod.rs`：注册 `#[tool] computer.move_window`（args：
     target/lease/request_id + reference + display 选择），走既有 Prepare/Execute。
  6) 验收：`work/pua-loop/live-acceptance.sh` 增用例——把测试窗口在授权屏↔非授权屏往返一次，
     证据 JSON 落盘；Windows 侧仅断言"显式不支持"。
- **改动面（已勘定，比预想小）**：设备线协议是 **tool 泛型**（`Call::Prepare{tool,arguments}` /
  `Call::Execute{operation_ref}` 按工具名路由，无 per-operation 枚举），所以不需要动
  device.rs 的 Call/Reply；实现集中在：① rwi-mcp `#[tool]` 注册（args 结构 + 描述）；
  ② agent 侧按工具名注册执行器（runtime/execution + operation Registry）；③ 引擎方法
  （rwi-core）+ macOS 后端（复用 `place_window_on`，含尺寸预检与 fail-closed）；
  ④ Windows 明确不支持；⑤ 验收脚本用例（跨屏往返 + 证据 JSON）。
- 实现路径（跨 crate，一步一验）：① rwi-mcp 注册 `computer.move_window`
  （target/lease/request_id + 目标窗口引用 + 目标显示器 id 或 built_in + 可选 y/x 锚点）；
  ② 网关路由 → 设备线 Call 变体 → agent 分发；③ 引擎方法 + macOS 后端实现
  （复用已实测的 AX `AXPosition` 放置：`place_window_on` + 尺寸预检，失败 fail-closed、
  台账留证、不重放）；④ Windows 后端 fail-closed 明示不支持；⑤ 验收脚本加用例
  （把测试窗口在授权屏与非授权屏之间往返一次，证据 JSON 落盘）。
- 过渡方案（已可用，operator 级显式）：`authorized_display.place_on_bind=true` 仅在对
  "绑定即归位"有明确意图时开启；默认关闭（事故修复 3cc47e7）。

## 已有基础，不重复从零做

- 独立 headless Rust Windows agent、设备 mTLS、全局 MCP；当前通用 Skill 为源码/项目镜像，旧 Plugin 包仅属历史验收，不冒充全局 Plugin/Skill 安装。
- 原生 screenshot/UIA/鼠标/键盘、菱形常驻 Pointer、单笔多点拖拽。
- 同资源租约、持久化不重放、连接复用、compact 元数据与原图按引用取回。
- 原生 Paint 图标绘制；后续 PNG 副本保存/重开哈希一致；Explorer 原生操作。
- Git 主线根 workspace、功能分支、可回滚小提交；正式 UIOS 验收未宣称完成。

## 完成口径

- 每项必须有实现、回归及适用的真实入口证据，才移为完成。
- Linux/移动端/IoT 遵循 ADR 的平台分级，不提前创建空实现或承诺不可用 OS API；
  尚无已分配实机的能力不填“已验证”。它们不阻止已分配 Windows/Mac 的持续实测。
- 不请求虚构的五分钟/一小时总槽位；当前 VM 授权无理论时限。实际单次操作、
  租约和取消仍有界；用户正在输入/切焦点时退让，不持有输入租约做离线开发。
- 最终主文档以“步骤—截图—结果—问题”顺序展示；技术日志、hash 和长表放附录。
  截图不提交 Git，不将旧截图标为本轮新验证。

**整体仍在实施中。** 本文件是持续状态表，不是缩小范围后的完成声明。

## #3a 完成记录：显式窗口移动原语（2026-09-15）

- 交付：`computer.move_window`（lease + request_id + 持久台账；幂等放置；不按键、不夺焦、
  不重放）→ `Engine::move_window`（先写 uncertain 预留、指纹冲突 `idempotency_conflict`）
  → `Backend::move_window`（默认 fail-closed，Windows/fixture 显式不支持）
  → macOS `NativeMacDesktop`/`NativeMac` 经 `place_window_on` 预检 + 原生回读。
- 新增窗口级定位器：`computer.resolve {kind:"window"}` → `placement_only` 引用（无需可见/未遮挡），
  输入路径拒绝之（`invalid_reference_class`）——防止窗口中心被误派发点击。
- 行为变更（已披露）：`place_on_bind=false` 时 `bind_window` 不再拒绝非授权屏窗口，也不再隐式移动；
  选择本身无原生副作用。此变更修掉活体发现的**单向陷阱**：窗口被显式移到外接屏后，原先
  无法再绑定/观察/移回（遮挡还会让元素引用永久不可解析）。
- 活体验收：`docs/validation/move-window-acceptance.py` → 16/16 PASS（跨屏往返 + discover 回读 +
  `dispatched_again=false` 重放证明 + 失败码负例），证据 `docs/validation/evidence-move-window-20260915.json`。
- 提交：`3baeb67`（初版接线）、`4ca5fb9`（台账 + 窗口定位器 + 绑定语义）、`bedaa28`（验收归档）。
- 运维注记：macOS 覆盖安装二进制需 `rm`+`cp`（原地 `cp` 会导致"同 sha 但静默不可执行"）。

## #3b 完成记录：弹窗一等交付（2026-09-15）

- 交付：`computer.observe` 新增 `popups` 清单（window/title/role/subrole/kind/bounds/display），
  来源与绑定同一张 AX 面（AXWindows + 子 sheet/dialog + focused/main 接力），
  覆盖非前台弹窗；用途不猜（kind=unknown、subrole=null 原样上报），AX 缺标题/几何时用
  window server 的事实值补齐。
- 结构：rwi-core `Popup` + `popup_kind`（纯函数 + 单测）、`Observation.popups`、
  `Backend::popups` 默认空（Windows/fixture 显式不实现）；rwi-macos 盘点 + display 富化；
  `Observed` 包装层转发。
- 系统面板策略：#3a 的 bind 变更已解开"跨屏面板被拒"的死结（选择无副作用）；面板不可搬移时
  move 回执如实报告实际 bounds/display（不以请求为准）。此点为文档级结论，未新增 fail-closed 检查，
  避免过度严格重现陷阱。
- 活体验收：`docs/validation/popup-acceptance.py` → 13/13 PASS（清单基线 → Add Image 开出面板 →
  下一次 observe 即交付 `cgwindow:N` kind=unknown + 事实标题 "Open" → owned_popup 绑定 → Cancel 关闭），
  证据 `docs/validation/evidence-popup-1789462163.json`。
- 提交：`3b83a8b`（代码）、`50fa1c2`（验收）。

## #4 完成记录：AI 光标视觉反馈层（2026-09-15）

- 交付：`computer.pointer_feedback {target, visible, point?}` + 每次成功 act 自动在动作点显示标记。
  实现为无边框、点击穿透、永不夺焦的 AppKit 窗口（accessory 策略，level 1000，标题 `RWI AI Cursor`），
  纯观感：不派发输入、不动真实光标、不改变前台。空闲约 45s 自动隐藏；回执同步
  （created/visible/point/last_error 为应用后状态）。
- 结构：rwi-macos `overlay`（命令通道 + 主线程泵 + 有界等待 ack；AppKit 只在进程主线程操作）
  → agent 主线程改为运行 bounded CFRunLoop 泵、服务器移至独立线程（观察者随服务器退出）
  → `Backend::feedback` 默认 fail-closed（Windows/fixture 不实现）→ 目录/执行/能力/MCP 全线注册。
- 活体验收：`docs/validation/pointer-feedback-acceptance.py` → 7/7 PASS（同步回执、discover 精确回读
  [387,1287,26,26] 内建屏、`screencapture -l` 像素证据 6395B、隐藏回读、残留模态清理、
  自动跟随动作点 [1575,1511]），证据 `docs/validation/evidence-pointer-feedback-1789463676.json`。
- 附带知识（已入 Skill）：模态弹窗开启时应用工具栏按钮 `enabled=false`，act 以
  `input_boundary_changed (enabled_disabled)` 正确 fail-closed —— 先关模态再操作。
- 提交：`ea6a819`（代码）、`75ad4d5`（验收）。

## #4 追加：跨平台同源材质（2026-09-15 晚）

- 用户要求"结合 win 版光标，不得平台割裂"→ 把 Win 版 `pointer_feedback` 的
  `material.rs`/`motion.rs` 抽为共享 crate **`rwi-pointer`**（93 个原测试随迁全绿，
  Win 侧原位重导出、呈现端零改动）；mac 端 overlay 改用同一材质渲染为 CALayer
  contents，DPI 由窗体的 `backingScaleFactor` 决定（Retina=192，1:1 锐利）。
- 活体验收仍 7/7 PASS（窗口 [320,1220,160,160]、像素证据 55KB、act 自动跟随）；
  演示图存档 `docs/validation/demo-a1-cursor.png`（sprite）与 `demo-a1-context.png`（环境照）。
- 提交：`acca9fd`（架构移动：抽 crate）、随后一条（mac 呈现 + 验收）。

## #5 完成记录：接管/占用 UX（2026-09-15 晚）

- 用户介入让位：macOS 输入派发前检查"真实点击/按键"的间隔（CGEventSource 组合会话态，
  左/右/其它鼠标按下 + KeyDown；纯移动不设门），1.2s 内有人操作即 `user_active` fail-closed，
  不排队、不重放；实机验证（用户正打字时 Add Image 被拒并留痕）。
- 操作日志可达：每次 act 追加一行到按 principal 的 `operations.jsonl`，`computer.targets` →
  `devices[].state.recent_operations` 交付有界尾部（尝试事实，非效果断言）。
- 能力对齐：capabilities 增加 `pointer_feedback`（含 material/dpi/visible/point）与
  `user_input_recent_seconds`（与 Win 版 `pointer_feedback.status()` 对齐）。
- 设计取舍（如实记录）：不新增"RWI 正在接管"横幅窗——本产品是背景优先，存在感由 A1 透镜
  （持久/自动跟随）+ 能力项 + 操作日志承担；gated 期间连"关面板"也会等用户停手（实测）。
- 验收：`docs/validation/takeover-acceptance.py` → 7/7 PASS（证据 evidence-takeover-1789464892.json）。
- 提交：行为提交 + 验收提交（见 git log）。

## 坐标系主线（2026-09-15 收尾状态，最高优先级）

**已确认（可依赖）**
- 采用**微信截图坐标系**（目标显示器左上原点、y 朝下、pt）；用户实测屏幕左上角=(0,0)。
- 画布局部(0,0) ↔ 全局 pt (729.31,1610.95)（用户读 Paint S 状态栏 + RWI 光标探针一次配对，
  与 PIL 独立推导 (729,1609) 吻合）；换算 `global = (729.31,1610.95) + canvas_px×0.5`。
- 窗口图像 scale=2.000；`pixel_to_desktop` 由当次"窗口 rect + 图像尺寸"推导（无硬编码）。
- 控件点击坐标正确：用场景元素 `point`（全局 pt）直接点，实测命中各自按钮
  （926→画出直线、963→图章形状组 等；用户的"按图标验证"方法有效）。
- A1 透镜每次 act 自动显示在动作点 → 用户可实时目视判定"点在哪"（人工坐标 oracle）。

**待办（实现优先）**
1. act/drag 回执插桩 `press_point_pt` / `anchor_element` → 定位 ≈(−100,−47)pt 按下点偏移。
2. `computer.ink_delta`（规范见 docs/architecture/macos-coordinate-model.md §13）→ 标定/落笔自检自动化。
3. §6 捕获区域贯通（消除非等比带失真）；拓扑（显示器集合）入 native_epoch 并热插拔即失效。
4. JS 车道放行 #3a/#4/#5 新工具。
5. 工具身份全表（矩形/圆角/椭圆/直线/橡皮…）——每单元 ≤3 个工具（工具级超时 ≈15s）。
6. 清理试验残迹 → 按 98 图元清单落笔 → ink self-check → 用户微信坐标复核（≤1pt）。

## 复刻子代理复核 + 用户新要求（2026-09-16）

**子代理有效产出**（采纳）
- 复刻成品与对比：`docs/validation/replica-final-20260916-164008.png`（1220×728 干净裁剪）、
  `replica-comparison-20260916-164008.png`（并排）、`replica-overlay-20260916-164008.png`（50% 叠加）。
- 映射复核：画布原点图像 px (894,838)（与我实测 (894,842) 差 4px）、scale=2.000、两点探针 0 误差 ✓
  → 说明坐标模型与画布映射**已可用** ✓。
- 98 图元全部派发、**78 个验证在原位** ✓；20 个被"随后移动"。
- 根因（有价值）：**Paint S 在失焦/回焦时把当前工具重置为 Arrow** → 后续 drag 变成框选，
  再下一次 drag 就"搬动选区" ✗（与我们此前踩到的选中/蚂蚁线问题同源）。
- 另一发现：**`Picked Color` 面板（同 pid 的第二个 AX 窗口）会静默阻塞主窗口输入** ✓
  （即先前"应用突然不接受输入"的谜团）。
**子代理越界/误判**（不采纳）
- 其"后台输入未被投递、只有 act_checked 可用"的结论与我们已验证事实矛盾 ✗
  （吸管采样与深红落笔都在后台成功改变像素 ✓）；其失败实为"工具被重置/框选"所致 ✓。
- 它不应点评/设计 RWI 实现 ✗ —— 子代理职责 = **按工具面与工具描述操作**，报告现象而非改架构。
  → 后续子代理任务模板加入硬规则：禁止分析或提议实现改动，只操作 + 报告观察。

**用户新要求（待实现）**
- **颜色选择器（第一个按钮）弹窗总是落在主屏**，应"及时移回 App 主窗口所在显示器"。
  设计：面板类窗口（owned_popup）的归属策略——当面板所在显示器 ≠ 其 owner 主窗口显示器时，
  由 RWI 在**显式**流程内将其移回（等价 `computer.move_window`，但由实现检测而非依赖 agent 察觉）；
  与"隐式放置默认关闭"的既有原则一致：仅对**由 RWI 操作引发的面板**生效，并在回执中留痕。

## 复刻 v2（量测版）结果：低保真 + 根因全部指向 RWI 实现缺陷（2026-09-16）

- 产出（**诊断证据，不是验收交付**）：`docs/validation/replica-v2-{final,comparison,overlay}-20260916-194418.png`；
  118 图元全部派发（ack=returned），但**仅 18/118** 落在计划位置 3px 内 ✗（mean coverage 0.339）。
- 五个主导根因（均为**产品级缺陷**，非操作失误）：
  1. **选中态无法可靠清除** ✗：`act_checked` 点画布元素、Arrow/空点、Escape 全部失败
     （Escape 报 `keyboard_target_not_focused` —— 键盘路径要求"精确聚焦元素"，对窗口级动作不适用）。
  2. **工具态不可见** ✗：Paint S 失焦会把工具重置为 Arrow → 下一次 drag 变成"移动选区"，
     已画好的形状整组被搬走（实测位移 ≈(+163,+200) canvas px）；RWI 侧没有任何"当前工具"事实可查。
  3. **选项行漂移** ✗：启用/禁用项会让行内容重排（~150–250 image px），按旧场景点会打到邻居控件。
  4. **同 pid 的 `Picked Color` 面板静默吞掉主窗口输入** ✗（窗口不重绘、pixel act 全 stale）。
  5. `native_press_rejected`（对已激活的 radio / 已勾选的 checkbox 再 AXPress）→ 输入域被 fence ✗。
- **修复方向（实现层，高杠杆且改动小）**：
  - **场景元素增补 `selected` / `value`（AXSelected / AXValue）**：让"当前工具是哪个"与
    "现在是否有选区"成为可读事实 → 模型可在每次落笔前校验工具、在落笔前后确认选区状态 ✓✓。
  - **窗口级键盘路径**：允许把 Escape/方向键等投递给"绑定窗口"而非要求精确聚焦元素 ✓。
  - **面板跟随归属**（已在列）：RWI 引发的面板落到别的显示器 → 显式移回 owner 所在显示器 ✓。
  - 事件读侧：布局变化后元素的 AX 位置可能滞后 → 以"回读裁剪"为准（操作规范已记录 ✓）。

## 事故：MCP 连不上（2026-09-17/18）——配对证书过期

**现象**：`computer.targets` 返回 `device_connection_failed: certificate expired
(verification time 1789694878, not valid after 1789618537)`；随后（重签后）变为 `BadSignature`。
win11-vm 侧为 `device_connect_timeout`（VM 未启动，属预期）。

**根因**：9/15 配对的证书有效期仅约 43h（3 天窗口），到期后适配器→agent 的 TLS 校验失败。
**处置（已完成）**：
1. `rwi-link-provision --profile-dir <AP>/agent-mac-v1/pairing-20260917` 重新签发（新 CA/三身份 ✓，
   有效期至 1789954110 ≈ 3 天）。
2. `agent.json`：`identity.*` → `pairing-20260917/agent/*`；`principals[0]`(input) = 新 **model** 证书 pin
   `e3397cc2…`、`principals[1]`(observe) = 新 **controller** 证书 pin `caf7998f…`（旧配置已备份 `.bak-<ts>`）。
3. `client-v1/profile.json`：`devices[mac_host].identity.*` → `pairing-20260917/model/*`；
   `agent_pin` = 新 agent 证书 pin `efd82456…`。
4. 重启 rwi-agent ✓（实例 189417ab，监听正常）。
**剩余一步（需用户操作）**：**重启 omp/opencode** —— 正在运行的适配器进程仍持有旧 profile（内存），
必须重启后才会用新证书，否则一直 `BadSignature`。

**运维改进项**：配对有效期偏短（~3 天）→ 需“到期提醒/续签脚本/更长有效期”其一，
避免每次静默失效；另可加“证书将过期”能力项提示。

## MCP 配置事故：项目级"缺 type 覆盖"让 proxyman 一直 connecting（2026-09-18）

- 症状：omp/opencode 里 `proxyman`、`codegraph` 长时间 "still connecting"，只有 rcu 正常。
- 根因（proxyman）：此前为省上下文在**项目级** `opencode.json` 写了
  `{"mcp":{"proxyman":{"enabled":false}}}`（**没有 `type`**）。更新后的 omp 会把这类条目
  当成一个不完整/无法启动的服务器 → 一直 connecting ✗。
- 处置：删除项目级 `opencode.json`；改为在**全局**配置的 proxyman 条目上设 `"enabled": false`
  （保留完整 `type`/`command`，schema 合法）→ 期望显示为"已禁用"而非"连接中" ✓。
- 附带：给 codegraph 补 `"timeout": 30000`（rcu 本来就有 `timeout: 15000`，这也是三者差异之一 ✓）；
  `codegraph serve --mcp` 能正常启动（stdio 静默等待握手 ✓）。
- 备份：`~/.config/opencode/opencode.json.bak-<ts>`；如需恢复 proxyman，把它改回 `true` 并重启 omp。

## 设计原则：按钮字典是"缓存"，不是"使用前置条件"（2026-09-18）

- 用户提问：是否每个应用都要先做按钮/功能字典才能使用？
- **答复：不需要 ✗。** AX 本身就是通用字典：`computer.observe` 应给出
  role / name / actions / enabled / selected / value / point，让 agent 零前置知识即可操作任意应用 ✓。
  字典仅作**带来源与版本的缓存**（"实测学习所得"），绝不作为门槛 ✓。
- 本次之所以需要实证：Paint S 的**工具按钮匿名**（无 AXTitle ✗）——属该应用的无障碍缺口，
  不是通用情形 ✓。**产品修复（本日已实现）**：对**无名字**的元素补读 AXDescription/AXHelp
  （图标按钮通常在此提供功能名 ✓），且只对匿名元素读取以避免吃掉有界遍历预算 ✓；
  Element 新增 `description` 字段 ✓。
- 用户判断修正：我提出的"工具栏样式会切换"**很可能不成立** ✗；
  更可能是**文档/窗口状态陈旧**（检查器 Width 从 2144 变 1820 ✓）→ 便宜验证：关闭重开该文档后再 observe ✓。

## 观察到的应用级现象：同一应用的 AX 暴露面随"窗口/工具栏呈现形态"变化（2026-09-18）

- 事实：Paint S 在**工具行独占一行**的形态下，19 个工具按钮以 `AXRadioButton` 暴露 ✓；
  在**工具栏并入标题栏**的形态下（当前 ✓），`AXToolbar` 与具名组 `Tools` 仍在 ✓，但**子按钮全部不暴露** ✗。
- 排除项（均已实测）：遍历**时间预算** 750/1500→1500/3000ms ✗ 无效；遍历**深度上限** 8→16 ✗ 无效；
  **关闭重开文档** ✗ 无效（工具栏属窗口级 UI，不随文档复位 ✓）。
- 结论：这是**应用自身的呈现差异**（非我们的遍历限制、非 omp/MCP 配置 ✓）。深度上限位于
  `crates/rwi-macos/src/native.rs` 的 observe 遍历（产品实现 ✓），与 omp 配置无关 ✓。
- 产品含义：`computer.observe` 的**覆盖度必须自述**（本次遍历是否截断/元素是否可能缺失 ✓），
  并建议在"树明显不全"时以**截屏回读**为准（SKILL 已含该操作规范 ✓）。

## Paint S 微信窗口复刻（2026-09-19，本轮阶段交付）

**背景**：本机重启（boot/security session 变更）后，原先用于复刻的临时文档 `/private/tmp/white-canvas-2x.png`
被清空且**不可恢复**（无 Time Machine、无 APFS 快照、无 autosave 副本，检索清单见 §21）；仓库内的
计划（118 形状 + 28 形状）与全部素材仍在，故改用**位图恢复 + 许可证素材填充**路线。

**已交付（均在 `docs/validation/`）**
| 产物 | 说明 |
|---|---|
| `recovered-canvas-aligned-20260919.png` | 由重启前最后的整窗截图重建的画布（2000×679）；用**画作外框几何**定标，重裁后外框 bbox = 400..1568 / 0..678（与计划框差 ≤1px）✓ |
| `user-reference-latest-canvas-20260919.png` | 用户提供的"丢失前最新"参考图（归档） |
| `replica-v6-vs-user-reference-{comparison,overlay}-20260919.png` | 与参考图比对：2px 容差下双向吻合 91.3% / 84.4% ✓ |
| `replica-v6-icons-filled-{comparison,overlay}-20260919.png` | 图标填充后 |
| `replica-v7-colour-demo-comparison-20260919.png` | 颜色演示（GUI 设色被证据否定后的替代路线） |
| `canvas-coloured-20260919.png` / `replica-v8-coloured-{comparison,overlay}` | 颜色填充成品：6 区域**精确色匹配率 1.0000**（一处 0.9525 系被选中行覆盖）；**墨迹零改动** ✓ |
| `canvas-text-filled-20260919.png` / `replica-v9-text-{comparison,overlay}` | 文本填充成品：擦占位线 + 回贴源文字带（左栏标题/8 行文字列/右栏标题），与源 mean\|Δ\| = 2.6/5.0/5.3 ✓ |

**素材与许可**：腾讯 TDesign icons（`github.com/Tencent/tdesign-icons`，MIT）与 MDI（`cdn.jsdelivr.net/npm/@mdi/svg`，
Apache-2.0）；头像/卡片内容/文字带均取自**本地截图切图**（非公开素材未外部下载）。

**关键环境与规则（接手必读）**
- 本机 MCP agent 部署在 `~/Library/Application Support/RealComputerUse/agent-mac-v1/`；
  重启方式：`hub start rwi-agent-mac-host --config agent.json`（ready 端口 45129）。今日 `rwi-agent` 曾 **exit=1 两次**（无 stderr）。
- 显示环境会变：外接屏断开 → 内建屏变主屏且分辨率/窗口尺寸改变 → **每次操作前重新发现窗口并重量映射**，
  原点是"文档矩形/画作外框几何"，不是画布元素点（视图滚动时后者失效，见 §21）。
- 颜色 GUI 路线已被证据否定：色板（NSColorPanel）bind 成功但 observe `window_missing`（不在进程 AX 窗口表）✗；
  仅 **Pen/画笔类工具**激活时颜色井 enabled ✓。GUI 精确设色若要可行，需另找入口（Paint S 设置里有 "Autosave documents" 选项，未验证）。
- 绘制/导入细节、失败码、工具索引随窗口宽度变化等，见 `docs/architecture/macos-coordinate-model.md` §15–§28。

**最终画布（`canvas-final-20260919.png` / 对比见 `replica-v10-final-{comparison,overlay}-20260919.png`）**
- 第 1–6 行头像：第 1 行为截图切图，第 2–6 行由源图同一圆心（步距 115.6 源 px）切圆后按同一比例（0.6063）并入 ✓；
  第 7–8 行**源图本身无头像**（该处为列表底色 242,242,242），故画布同样留空 ✓（已核对源图逐行 patch）。
- 应用水印：两处文字水印区域已按"中性浅灰且非墨迹"规则置白 ✓（区域内 >250 亮度占比 0.896 / 0.731，残差为穿过该区的画作描边）；
- 底栏占位圆环：按 10 个槽位圆心的环带（r∈[13,19]）置白，移除 43 px 环带墨迹 ✓（槽位内图标保留）。

**尚未完成（后续接手项）**
1. 文本目前是"源图文字带回贴"（位图），非可编辑文本；如需可编辑需另设计（Text 工具 + 可写非敏感 AX 元素，本机未验证）；
2. 若需 GUI 内继续绘制（而非离线合成），需先解决"色板面板不可 AX 寻址"问题（见架构文档 §26；Paint S 设置含 "Autosave documents" 选项，未验证能否解锁画布背景色井）；
3. 底栏之外，`pill_chevron` 处存在"贴图 + 既有描边"重复（轻微），未处理。

### 复刻"取巧"自查与纠正（2026-09-19 晚，用户指出后）

用户指出最终图"像是把源截图的左右内容贴上去"，经自查确认**文本那一步确实取巧**：早期实现是把源截图的
**文字带像素**贴进画布（`replica-v9-text-*`），这不是复刻 ✗。已纠正为**渲染真实字形**（Hiragino Sans GB，
按源图行距/位置排版），并在文档里保留旧的取巧产物以便对照。

**逐项审计（当前 v11 = `canvas-final-v2-rendered-text-20260919.png`）**
| 内容 | 做法 | 是否合规 |
|---|---|---|
| 骨架/线条 | 重启前**我们自己画**的画布位图（`docs/validation/recovered-canvas-aligned-20260919.png`） | ✓ 自己的绘图 |
| 图标 | 下载腾讯 TDesign（MIT）/MDI（Apache-2.0）；phone/videocam 由截图切图 | ✓ 许可素材/按用户指示切图 |
| 头像（行 1–6） | 源截图**切圆**（非公开图片，用户明确要求只能切图） | ✓ 按指示 |
| 卡片内容（图片+文字混合） | 源截图裁切贴入（非公开图片） | ⚠️ 混合内容含文字，如需 100% 合规可把文字部分改为渲染 |
| 颜色 | 按源图采样色做区域平涂（6 区域精确匹配 1.0000，墨迹零改动） | ✓ 复刻配色 |
| 文本 | **渲染真实字形**（v11）；旧版为贴图像素 ✗ 已废弃 | ✓ 已纠正 |

**产物**：`docs/validation/replica-v11-rendered-text-{comparison,overlay}-20260919.png`、
`canvas-final-v2-rendered-text-20260919.png`；画布文件已写回并 Revert 刷新（应用内为 v11）。
**未完**：卡片内文字是否改为渲染、短横线占位与文字轻微重叠、`pill_chevron` 重复。

### 去取巧 v2：卡片改为"照片切图 + 渲染字形"（2026-09-19）

用户要求把卡片内容也改为纯复刻。已执行：
- **发现并修正一处几何错误**：早前把卡片内容贴图放在了 `canvas y 315..620`（中心 465），而计划几何里卡片在
  `(828.7,107.3)-(998.4,413.5)`（中心 260）——**贴低了约 205 px**，这也是"看着像贴图"的直接原因 ✗；
- 卡片整卡重铺：**照片只取源图照片区** `(746,216)-(1020,404)` 缩放为 166×114 贴入 (830,109) ✓（非公开图片，按指示切图）；
- 正文区（canvas 232..300）改为**渲染字形**（可辨认的两句 + 显式披露"其余文字不可辨认、不伪造"）✓；
- 暗带 `(830,306)-(998,413)` 平涂 33,33,32，并以字形渲染 `图文` 徽章与 `#被遗弃的玩具熊 展开` ✓；
- 文字带残线（保留行分隔线，阈值 ≥150px）与 `pill_chevron` 重复贴图已清理，chevron 改为矢量绘制 ✓。

**产物**：`docs/validation/canvas-final-v6-no-shortcuts-20260919.png`、
`replica-v12-clean-{comparison,overlay}-20260919.png`（左源图 / 右最终画布）。

### 用户澄清的贴图策略（2026-09-19，需长期遵守）

> "能复刻的部分尽量以**绘制图形**为主（用各种工具复刻出来）；完全不能复刻的部分可以**贴图**，
> 但要**保留那个图层**。"

据此调整：
1. **可复刻内容**（形状/线条/颜色/图标/文本）→ 必须绘制或**渲染字形**，不得贴源截图像素 ✗；
2. **不可复刻内容**（照片、原始插画、无法辨认的正文）→ **允许贴图** ✓，但必须作为**独立图像图层**存在
   （不要烤进底图）；
3. 已按此准备：卡片"照片+正文"从源图裁为独立图层素材 `assets/card-content-layer-166x191.png`（166×191），
   底图 `canvas-base-layered-20260919.png` 中该区域**已留白**（不烤入），目标落点中心 = canvas (913.5, 204.6)；
   导入方式沿用已验证的 Add Image 单文件流程 + `act_checked` 拖拽（`offset = pt_per_canvas_px × (target − 导入落点)`，
   导入落点 = 画布 AXScrollArea 元素点）。

**本轮受阻（如实）**：执行导入时 RWI 适配器链路退化（`computer.targets` 先返回空数组，随后 `devices: []`）——
本机 `rwi-agent` 进程仍在监听 45129，但适配器已无法描述设备；**这不是"导入成功"**。
下一步：链路恢复后重试导入（预期：文档中新出现名为 `Image` 的图层，且卡片照片/正文出现在 (830,109)-(996,300)）。

## Real World Interaction 更名（2026-09-19）

- 产品显示名：Real World Interaction；MCP 注册键：`real_world_interaction`；Skill/Plugin：`real-world-interaction`。
- GitHub 私有仓库已更名为 `DestroyedTeam/RealWorldInteraction`，origin 已更新。
- 本机源码根改为 `/Users/vincentadamnemessis/AI Projects/Real World Interaction`；运行目录改为 `~/Library/Application Support/RealWorldInteraction/`。旧目录为兼容符号链接，指向同一份源码/配对/防重放状态。
- Codex/OMP/OpenCode 注册改用新键与 `client-v1/bin/rwi-mcp`。新进程 initialize 自报 `real-world-interaction`，发现 26 个工具；只读 `computer.targets` 确认 mac-host available。
- 当前项目三个 Codex 任务标题已加 Real World Interaction 前缀；桌面项目显示名/根关联仍待处理（GUI 工具拒绝访问 Codex；CLI 项目库查无桌面项目 ID）。
- 第一批名称变更：fmt、workspace locked offline test（433 passed，10 ignored）、全特性 clippy 均通过。原始会话、迁移档案及历史验收结论保持原样；本轮未重新验收 Paint S v13。
- 下一批：Rust 包/二进制/JS 帮助器简称统一为 RWI；持久协议身份、证书 DNS 与历史状态路径保留兼容。

### 更名收尾状态（同日，覆盖上节未完成项）

- 10 个 Cargo 包/源码目录及二进制已统一为 `rwi-*`；JS 客户端文件及导出类统一为 `rwi-client.mjs` / `RwiClient`，worker 新全局 `rwi` 与兼容 `rcu` 指向同一对象。
- 共享与 Windows 回退字形已从 RCU 改为 RWI，macOS/Windows 窗口显示标题同步更新；尚未重启本机旧 agent，所以当前运行进程不据此宣称已切换视觉。
- **Codex 桌面项目名仍未完成**：本机应用用缓存覆盖了磁盘元数据修改；读取项目清单仍为旧名。任务标题已更名，关联项目 ID 保留。退出应用后可运行 `outputs/rename-20260919/finish-codex-project-rename.py`，再重新打开核验；脚本在应用仍运行时拒绝修改。
- **Windows 部署仍未完成**：只读设备发现返回 `device_connection_failed / device_connect_timeout`；未唤醒 VM，未更改其运行态。
- 旧文件夹兼容链接、既有 TLS DNS/ALPN、schema 字符串、环境变量和持久锁/日志路径有意保留。它们确保旧进程与新进程仍使用同一配对和输入域，不是第二份产品源码。
- 本轮主要推进统一命名与兼容迁移；Paint S v14 头像独立图层任务未执行，v13 历史业务证据未重新验收。

## 正式 Plugin 聚合（2026-09-19）

- 用户确认更名结果 OK；下一步交付为正式 Codex Plugin，然后继续 Paint S v14。
- 已创建并安装 `real-world-interaction@personal`：一个 Rust MCP 网关 + 通用路由/macOS/Windows 三个 Skill。规范源在 `integrations/codex/plugin/real-world-interaction/`。
- 主机打包仅渲染外部已安装二进制/profile 路径，不复制密钥/状态；生产入口不使用旧 task-bound exporter。
- Codex 独立 MCP 注册已关闭，Plugin 启用；OMP/OpenCode 仍独立注册 MCP，并加载同一套规范 Skill。旧项目内自动发现 Skill 已迁到 Plugin 引用资料，避免 Codex 重复发现。
- 新增跨设备输入句柄/显式 device_id 冲突与同平台多设备 artifact 碰撞回归；435 Rust 测试通过，10 ignored；全特性 clippy 通过；打包4项测试及3个Skill/Plugin校验通过。
- 当前任务未暴露安装后的 Plugin MCP 工具；正在核验宿主发现。v14 需要该原生工具入口后才能执行，未以旁路 GUI 或离线合成冒充 Plugin 操作。

### Plugin 宿主发现闭环

- 单独设置旧服务器 `enabled=false` 仍遮住同名 Plugin 服务器；已移除该 Codex 独立服务器表（私有原配置保存在 work/，未提交）。其他 Host 注册保留。
- 不持久化、不开模型回合的 Codex app-server 测试上下文报告 `pluginId=real-world-interaction@personal`、26 工具、`toolsError=null`，证明来自 Plugin 而非独立注册。证据：`docs/validation/2026-09-19-plugin-acceptance.json`。
- 当前任务刷新后已看到3个Plugin Skill，但本轮 callable tools 仍无其 MCP；需新的任务工具上下文才能继续 GUI。Skill 要求通过 Host 的插件原生入口进行验收，因此未改走 shell MCP 或其他 GUI 通道。
- v14 接续清单在 `docs/product/paint-v14-resume.md`；待 fresh discovery 验证当前文档，再处理第1–6行独立头像层。

### 接续任务 MCP 启动失败根因（2026-09-19）

- 新任务 `01a0b8fc-631c-7cc0-bbeb-c8681babfabf` 的所有外部 MCP 同时报 `No such file or directory`；默认 shell 不指定 workdir 也启动失败。
- 实查旧任务 cwd `/Users/vincentadamnemessis/AI Projects/Real Computer Use` 不存在；已恢复它到 `Real World Interaction` 的兼容符号链接。新任务并不能修复继承的不存在 cwd。
- 本机旧 `rcu-agent` PID24303 正常监听127.0.0.1:45129，由OMP broker托管。新 `rwi-mcp` 通过当前profile实际握手并得到 `mac-host available`、`input_domain_fenced=false`；旧名称不是当前通信失败原因，未无依据重启Agent。
- 重应用Plugin安装成功，但当前任务仍持有启动失败的工具清单；需要桌面Host刷新MCP连接或重新启动后复核，不再重复创建任务。
- 本輪是进程/传输诊断，尚未执行Paint v14 GUI。

### 本机 Agent 运行态更名完成，原生权限待用户确认

2026-09-19：旧rcu-agent已正常停止；新RWI Agent.app内rwi-agent由用户launchd运行，配置/日志命名同步。127.0.0.1:45129监听与原配对TLS握手通过，配置/身份/状态连续。独立进程尚无辅助功能、屏幕录制、输入监控权限；需要用户在系统设置授权RWI Agent.app，授权后再验证GUI能力，不能以mac-host available替代。详见 docs/validation/2026-09-19-rwi-agent-cutover.md。

### RWI Agent 权限与本任务原生工具恢复（2026-09-19）

用户已开辅助功能，并明确请求通过电脑界面开启另外两项权限。本轮在系统设置实际添加RWI Agent.app，确认输入监控与屏幕录制开关开启；随后SIGINT正常停止并用launchctl重新启动同一服务。新进程报告 accessibility/listen_event/post_event/screen_capture/screenshot_api 全部true。当前任务的26个Plugin MCP工具已出现，并通过其 computer.js → rwi.targets / computer.discover 实际调用验证。下一步按当前发现的Paint S窗口继续v14，不再要求另开会话。

### Paint v14 实景复核与捕获候选修正（2026-09-19）

- 经当前Plugin实际observe确认，Paint S `canvas-text-filled-20260919.png` 有两个Image层，但底图仍含一份卡片内容，与独立卡片重复；第1–6行头像未独立分层。不得继承v13历史报告作为现状。
- 背景控件受visibility_unknown阻挡，宿主Finder打开Paint S激活后，RWI重新观察确认Add Image可见。原生AXPress返回native_press_rejected，但发现已出现Open面板；未重放，随后通过computer.recover_input验证空闲并解除fence。
- Open面板需要fresh initial selection（当前macOS未实现owned_popup关系）；成功observe后发现截图混入主窗口且像素映射不可信，控件仍标记不可见。
- 实现macOS26 SCScreenshotConfiguration候选：显式display-logical sourceRect，includeChildWindows=false，SDR输出；旧系统保留旧接口。依据Apple sourceRect文档及本机SDK SCScreenshotManager.h。
- 436 Rust测试通过、10 ignored；fmt和全特性clippy通过。新候选已部署并保留旧Bundle备份，但**实时捕获修正尚未验收**。
- ad-hoc签名的DR包含cdhash，更新二进制后TCC旧权限不匹配；通过系统设置重新关联时，系统弹Touch ID/密码确认，等待用户亲自完成。未改TCC数据库或降低权限检查。
- 当前RWI候选服务在线，但辅助功能/屏幕录制/输入监控尚需重授；Paint仍停留于Open面板，未导入新头像、未修改画布。后续先恢复权限并验证候选截图，再解决面板命中；随后修复底图重复内容、完成独立头像层与v14验收。

### 2026-09-19 Paint v14 权限恢复接续

- 当前 Agent 控制/输入权限经 Plugin 原生查询已恢复（accessibility/listen_event/post_event=true）。
- 旧临时签名授权已按 RWI bundle ID 定向重置；屏幕录制还需通过设置上方列表加号重新加入当前应用包，CUA 的两个同名 Add 控件引用歧义，待用户打开该文件面板。
- Paint v14 头像独立层及截图修正现场验收尚未完成，未新增画布修改。详见 `docs/validation/2026-09-19-paint-v14-capture-candidate.md`。

### 2026-09-19 Paint S 后台行为对照

- 用户补齐授权并重启后，四项原生权限全部 true。
- CUA 成功取消 Open、切换 Line；RWI 能后台截图但控件全被观察层的 system-wide hit-test 标记遮挡。已提交应用范围命中测试候选，437测试通过，尚未部署/现场验收。
- 截图顶部裁切/底部空白仍需修复；设备聚合查询约5秒受离线Windows影响。完整时序、证据边界及接续项见 `docs/validation/2026-09-19-paint-background-comparison.md`。

### 2026-09-19 dac087b 已部署，等待系统认证

- release rwi-agent 编译成功，替换当前应用包可执行文件并验证签名，通过用户 LaunchAgent 启动。旧包备份 `work/deploy-dac087b/previous-bundle`；配置、配对、防重放状态未变。
- 原生查询 PID13929 / instance16a5fa6c-1f5d-4735-8e2f-fdd4cd9eb8c6：四项权限 false。当前 ad-hoc 签名 DR 绑定 cdhash，本机 `security find-identity -v -p codesigning` 返回0个有效签名身份。
- 按 bundle ID 定向重置三项旧权限；CUA 重连成功，打开控制权限 Add 后出现 Touch ID / 密码认证，等待用户确认。没有改动其他应用授权。
- 早上 OMP broker 托管旧 rcu-agent 与当前独立 launchd 启动路径不同；历史运行时 TCC attribution chain 尚无证据，不断言为模型差异或确定继承。
- 依据 Apple TN3127：ad-hoc DR绑定特定代码版本，稳定签名身份才是跨更新维持代码身份的正确方向。普通应用不能以自身调用静默批准TCC；可辅助系统设置流程，用户完成系统认证。

### 2026-09-19 统一图标与本地证书签名已安装

Plugin与RWI Agent使用同一生成式空间轨道/指针logo。Plugin重装成功，Agent通过新建RWI Local Development证书签名并部署；版本修改后仍通过旧DR验证。首次证书身份TCC授权及授权后跨更新实测待完成。详见 `docs/product/rwi-brand-and-local-signing.md`。

### 2026-09-19 固定签名授权保留实测通过（包更新范围）

- 四项权限全部恢复；同证书签名包build2→3替换重启后仍全部true，没有重复授权。范围为包元数据/签名更新，非新Rust二进制。
- 后台观察命中修正已现场验证工具栏visible=true。AXPress仍出现非零返回但观察到界面变化，未重放，已查询终态并安全恢复输入域；继续诊断错误分类/实际输入验证。Paint v14仍未完成。

### 2026-09-19 v14 实际导入驱动的输出与回执修正

- 精简JS摘要补齐macOS role、description后备名称、disabled、selected、value、AX actions，保留帧绑定与未知状态；不再输出空名工具并迫使模型反复取原始元数据。当前真实面板输出已观察到新字段。
- Rust AX非零返回保留错误码与effect unknown，不再一概称应用拒绝；无自动重发/CGEvent补发。
- JS对于dispatch=returned且无dispatch_error但system_accepted=null的macOS回执，保留未验证结果，不再伪造operation_failed；后续动作仍必须重新观察。
- fmt、438 Rust tests passed / 10 ignored、全目标全特性clippy通过；43 JS tests通过。
- 新Rust Agent已重编译、固定证书重签并部署；PID21909四项权限仍true，无再次授权。这次覆盖真实二进制更新，不只是应用包版本号变化。
- `v14-import-open-signed-20260919-02`普通click投递1492ms；独立discover证实Open cgwindow:5429出现，未重放。随后绑定13.6ms、观察543.6ms。
- 新Open面板当前仍hidden/occluded，主窗口工具栏已修复；说明应用范围命中不足以覆盖sandboxed panel代理。下一步诊断面板代理身份与窗口局部命中，不直接放宽检查。截图偏移/头像独立层仍待完成，未宣告v14通过。

### 2026-09-19 v14 全窗口截图边界修正

已部署保留native full-frame默认sourceRect的修正。真实Plugin截图确认Open面板顶部/底部完整；旧版重复裁剪导致的白带消失。重新打开面板的异步出现已独立只读复核，不重发输入。文件面板AX命中仍需诊断，头像导入尚未完成。详见2026-09-19-paint-v14-capture-candidate.md。

### 2026-09-19 文件面板命中根因证据

新增最多8条只读AX命中差异诊断，精简摘要默认不展开。真实Plugin观察显示面板壳5573和控件AX窗口5575不同，应用hit只返回AXSheet；窗口对象hit探测也不可用，未用未知结果授权输入。详见2026-09-19-paint-panel-ax-diagnostics.md。下一步应实现独立的精确AX语义动作验证，而非把几何命中兜底等同于真实hit。v14头像仍未导入。

### 2026-09-19 文件面板精确 AX 动作已落地

- 原生语义动作与指针引用分离；普通点击门禁保持。439测试通过，新增后端能力/禁用/状态变化/动作改变/去重负向覆盖。
- 已固定证书签名部署；Plugin AXPress真实取消旧Open并重新打开新Open，两步均独立观察确认，无重放。
- 面板按钮路径已打通。当前继续文件位置导航，尚未导入头像；不将Cancel/Add Image成功等同v14通过。

### 2026-09-19 v14 文件导航推进与XPC归属

- RWI已把文件面板导航到Home，Where值和截图均确认；AXOpen非零返回未被重放。
- AI Projects图标属于Apple openAndSavePanelService PID46633，而外层Paint PID2578，当前同PID限制在原生投递前正确停止。已拆分诊断错误并部署；未删除进程边界。
- 下一步实现绑定面板的受限XPC委托归属证明。画布未新增图像，v14仍未完成。

### 2026-09-19 授权交互流程的跨进程支持

用户澄清：文件面板、取色板、字体面板、权限弹窗和第三方集成可能由其他进程承载；不得把最初应用PID当作完整任务边界。关联托管UI与独立授权窗口分别处理，敏感系统确认仍需用户参与。

- 首个已实现托管provider：来自精确绑定AX子树的Apple openAndSavePanelService对象，验证同UID、真实进程出生，以及Security框架对运行中代码的anchor apple + identifier校验。没有shell签名验证桥、没有全局搜索后放行整个helper。
- 元素身份加入真实owner进程出生，动作前再次校验provider。普通指针路径不扩大，其他provider未验证时保留显式发现/绑定或用户确认流程。
- 固定证书签名部署成功；440测试passed/10ignored，fmt/clippy通过。通过真实Plugin先进入AI Projects再进入Real World Interaction，Where值分别独立确认。AXOpen仍返回-25205但目录已改变，保留uncertain并查原操作/恢复输入域，从未重放。
- 当前停在项目根的Open面板。work目录可在截图中看到，但其控件位于当前128节点观察预算外，需要改善大目录的原生树预算/分页或位置导航；不能借坐标绕过。头像尚未导入，v14未验收。

### 2026-09-19 v14 首个头像独立层实际导入

已通过 RWI Plugin 导入 48×48 头像，截图确认第三个 Image 图层。文件面板滚动使用 CUA 对照辅助（578 ms 单次），不能算纯 RWI 闭环；RWI AXOpen 返回不确定后通过窗口消失与主画布独立观察确认导入，未重放。定位拖动仍被 stale_reference 在投递前拒绝，正在补充具体 grounding 诊断并修复错误封装丢字段。详见 docs/validation/2026-09-19-paint-v14-avatar-import.md。v14 尚未完成。

后续：grounding 诊断与 Agent 透传已部署，441 tests passed / 10 ignored、fmt/clippy通过，四项权限仍true。新 RWI 拖动返回但未观察到位移；CUA 单次对照也未确认位移，继续定位映射/对象交互问题，不宣称已修复。

### 2026-09-19 更新协作节奏与头像定位验证

用户明确：不按每轮对话提交/推送；先功能验证，再按完整节点提交。避免频繁 push 触发私有仓库 Actions；本轮不提交、不推送，不擅改远程 workflow。

CUA 对照检查 Object 菜单，确认此前处于 End Editing 状态；Transform 为原地变换模式，并非独立位置对话框。通过 CUA End Editing 退出该状态后，RWI 重新截图并执行唯一请求 v14-row1-object-mode-placement-20260919-01，900 ms 路径，实际截图确认 48×48 头像由第六行旁移至第一行，第三个 Image 图层保持。不能把 CUA 状态准备算作纯 RWI 闭环；但本次定位拖动确由 RWI 完成且已独立观察。下一步保存并验证后续独立层。

已保存原生 Paint Document：work/replica-verify-20260915/run6/paint-v14-row1-layer-20260919.paint（327011 bytes）。CUA 设置 Save As 名称和格式，RWI AXPress Save 唯一请求 v14-row1-save-paint-document-20260919-01 返回 -25204/uncertain；后续面板消失、主窗口标题改变、文件出现三项独立证据确认保存，没有重发。新截图仍显示三图层与第一行已定位头像。完整重开图层验证仍待执行。

### 2026-09-19 矩形样式读值缺口

继续对照原图处理重复卡片，不再反复微调首行头像。RWI 已切换 Rectangle，CUA 独立确认 Border=1、Fill=0、Rectangle=1；RWI 摘要此前均无value。根因是 native element_description 将 AXValue 仅作为 CFString 读取，CFNumber/CFBoolean 被丢弃。新增受原有 TextClass 门禁约束的标量转换，保留0/1/混合态2及小数，禁止未知/安全文本读值。尚未绘制覆盖层，避免把默认无填充/有边框矩形当修复成果。本轮不提交、不推送。

标量读值修正已固定证书签名部署，442 tests passed / 10 ignored，fmt/clippy通过。真实 Plugin 新观察确认 Border value=1、Fill value=0、Rectangle value=1、Round Rectangle value=0，与CUA对照一致；四项系统权限仍true。未更改远程、未提交。

### 2026-09-19 v14 重复卡片实际清理

通过 RWI 关闭 Border、打开 Fill，并用900ms原生拖动创建265×317聊天区遮盖矩形，独立观察确认新 Rectangle 图层。CUA 辅助两次 Send Backward，使其位于卡片和头像之下、底图之上；再通过原生调色板 Input Color Manually 设置RGB242（尚未完成源图像素级色差测量）。RWI 再创建170×144输入区遮盖矩形。当前共5图层（3 Image + 2 Rectangle），聊天区重复图片和输入区主要黑色尾块已遮盖；工具栏处仍有残留。没有删除原始底图。

另存为 work/replica-verify-20260915/run6/paint-v14-card-cleanup-20260919.paint（330900 bytes），原row1文件保留。CUA设置名称/格式，RWI执行唯一Save请求v14-card-cleanup-save-01；AX返回不确定后未重发，文件出现、Save消失、主窗口新标题与截图确认保存。此为混合通道真实任务进展，不宣称纯RWI闭环。剩余：工具栏残留、卡片完整内容和比例、图标文字样式、完整重开图层检查与最终对照。未提交/未推送。

### 2026-09-19 v14 完整卡片替换

核实已有素材 card-content-from-screenshot-place170x306.png 包含原图手机卡片的底部内容。RWI打开Add Image、精确AXOpen导入；CUA辅助文件面板滚动与End Editing。RWI依据当前预览映射计算桌面偏移（不再使用固定比例），请求v14-full-card-position-01将完整170×306卡片放到聊天区，实际截图确认底部黑色内容/导航栏可见、位于分隔线上方。现有6层，旧166×191截短卡片通过RWI图层眼睛开关隐藏（请求v14-hide-old-card-stable-mode-02）；未删除，截图眼睛变灰且完整卡片保留。第一次在编辑态下的隐藏请求在投递前被stale_reference拦截；先退出编辑再用新帧执行，无不确定输入重放。

CUA辅助保存现有paint-v14-card-cleanup-20260919.paint，文件由330900增至393480 bytes，SHA256=195232143c3b18f31e73cf0b0ec0ddd7eb8264e76af7fb251fb345864d3c0924。仍待：工具栏细条残留与图标复刻、全局字体/边框样式、重新打开验证及整体验收。本轮未提交或推送。

### 2026-09-19 v14 工具栏清理与原生形状重绘

RWI在独立新帧验证Rectangle=1后绘制185×40的工具栏遮盖矩形，旧黑色残留条消失。首次900ms拖动回执returned但截图未出现新层，未将其算为成功；后续重新核实工具状态、重新观察后另一有界绘制才确认新增矩形。

CUA辅助手动设置描边RGB20/20/20；RWI校验色井与无填充状态，绘制文件夹/图片/信封：3个矩形外框+7条线段（信封折线2、图片山峰2、文件夹标签3）。全部是原生可编辑对象，不是整图贴入；每步独立截图确认，当前约17个对象。剪刀、麦克风及其余红色占位图标尚未替换，不宣称工具栏完成。图标几何尚需最终对照微调。

CUA End Editing与保存后，原生截图显示3个黑色图标且原黑条消失；文件395754 bytes，SHA256 a68a4944b700f9ea6d8787265d43a7781bac425bf5fb60b11209624f752fc571。文件位置仍为work/replica-verify-20260915/run6/paint-v14-card-cleanup-20260919.paint。本轮没有代码提交/推送。

### 2026-09-19 v14 剪刀和麦克风可编辑对象

RWI先用43×34浅灰矩形清除剪刀处残余红色占位，再用2个椭圆+2条交叉线重绘剪刀；麦克风为8×17圆角矩形和5条支架线段。全部通过真实Plugin输入并逐步截图检查，未贴入新位图。现有约28个对象（含保留的隐藏旧卡片），工具栏表情和右侧红色占位仍未完成，当前几何是近似复刻而非像素级验收。

CUA只辅助End Editing和Save。保存文件397886 bytes，SHA256=e9f367519cddd95310d63a85553fc8babd381383d9d51d08da12035548bb58a2；RWI保存后截图确认剪刀、麦克风和原三个图标仍可见。尚待：其余图标/字体/边框，重开验证与最终对照。本轮无代码提交/推送。

### 2026-09-19 v14 表情、屏幕共享与时钟

RWI完成三处占位遮盖后，以原生椭圆/矩形/线段重绘表情（圆+双眼+三段嘴部）、时钟（圆+两指针）及屏幕共享（显示器+不透明手机+支架）。使用当前preview仿射映射计算路径，未直接复制控制端像素。保存后独立截图确认八个工具栏图标已呈黑色，仍有语音转文字与机器人两处红色占位，顶部/侧栏/整体字体线色亦未验收。几何仍为近似，不称1:1通过。

绘制后两次出现ax_attribute_unavailable只读失败；先查询原请求并新观察，已派发操作不重放。首次smiley-mask返回但未观察新增形状，后续独立确认状态再创建扩大范围遮盖。JS辅助函数已在调用外捕获读回异常，避免错误导致整个交互状态丢失，未改产品代码。CUA仅用于End Editing与Save。保存文件400543 bytes，SHA256=c8865647ec7aef0b7f105f6a7b0969edb510cff11e08fc0c152867e5582997a5。本轮未提交/推送。

### 2026-09-19 v14 工具栏剩余图标与重开验证

RWI完成语音转文字和机器人：2个遮盖矩形、圆角矩形主体、天线/双眼、麦克风支架及“文”字形线段。聊天输入工具栏10个主图标的红色占位均已替换；仅属近似几何重绘，尚缺精细弧度/小箭头等细节，不等同整体复刻达标。顶部及侧栏仍保留明显旧样式。

CUA辅助End Editing、Save、Close，再由File > Open Recent实际重开同一.paint文件。新窗口cgwindow:6491，bounds=[0,125,1220,527]，缩放重置100%，所有旧视图映射已作废。RWI重新discover/bind/observe，能看到Rectangle/Image等保留层以及重绘工具栏；独立请求v14-reopen-select-full-card-01单独选中完整卡片，截图左侧明确Width:170 Height:306。这确认重开后仍为独立可编辑卡片而不是扁平PNG。

保存文件403048 bytes，SHA256=5a9932a1270c7f50da2d1a22e59b5aa677cb38e43c3695a722033f59e68cfef7。当前选择完整卡片，后续绘制前先退出编辑、重新观察坐标和工具。剩余主项：顶部标题/搜索/按钮、侧栏图标和头像、原有红色边框线与字体样式、最终源图对照与导出；工具栏也仍需像素级微调。本轮未提交/推送。

### 2026-09-20 多显示器弹窗反馈与标题恢复

用户现场反馈：色彩弹窗曾出现在扩展屏，而不是MacBook内建屏；失去焦点后自动消失。此为用户观察，未取得当时弹窗窗口ID/owner/截图，不能写成已修复或据此判断唯一根因。新RWI发现主文档cgwindow:6491位于built_in=true的display1，bounds=[282,1190,1800,1131]；同PID当前只剩主文档，不能排除已消失/跨进程popover。后续调色需在触发后立即检查多屏与托管弹窗，不仅看主窗口截图，不因未见弹窗而重复输入。

中断前文本框唯一操作v14-title-textbox-current-layout-02回执returned，新观察确认Text层459×43已经存在且为空。没有重建文本框。CUA明确读取到焦点text entry area，填入“拉电线了诶嘿 (756)”并Escape；RWI截图确认独立标题文字层、22号黑色粗体。CUA保存完成，文件409694 bytes，SHA256=0f1aac441a56c5783c9dee0c18039649db014e1756bedfc70d780be04eac2795。本轮未提交/推送。

### 2026-09-20 用户Alt+Tab导致的误拦截修正

暂停搜索区绘制，优先修正用户报告：其他应用的全局修饰键不应当作Paint内的人机冲突。发现具体触发为AX效果不确定误设资源锁，恢复再检查全局HID；另有事件源使用HID共享状态的问题。已改Private source、pointer显式空flags，并只对原生AX“未持有按键资源”证据豁免资源恢复锁，保留uncertain/no-replay与真实清理失败锁。修正一个离线测试曾post到任意PID的问题，现为仅内存stamp测试。

443 tests passed / 10 ignored，fmt/clippy通过，固定证书重签部署且四项权限保持。真实Plugin已重新绑定并成功切换工具。用户持Alt+Tab的现场对照尚未完成；真正中断的CGEvent序列恢复仍为保守global fallback，后续需要RWI自身输入资源所有权证据，不能宣称所有后台并行问题已解决。详见docs/validation/2026-09-20-background-input-modifier-isolation.md。

搜索旧占位已被新Rectangle遮盖，灰色圆角搜索框与文字尚未绘制，当前工具Round Rectangle。该搜索遮盖尚未再次保存；标题已保存在此前409694-byte版本。未提交/推送。

### 2026-09-20 搜索区复刻与新输入隔离实测

RWI创建222×42浅灰圆角搜索底、14×14放大镜圆和手柄，保留为独立对象；CUA辅助设置RGB230、圆角10及常规18号字体，并为RWI创建的文本框填入“搜索”。原红色搜索占位已由可恢复矩形覆盖。保存文件411592 bytes，SHA256=77fa539be44255d9f2166e87bdde43530fa57fafd9028ac344aa892850ae5744；保存后RWI截图确认搜索区内容。仍属近似样式，未宣告整图1:1完成。

调色板出现后立即RWI discover，实际popover cgwindow:7732 bounds=[1263,1264,263,331]，归属display1 built_in=true；这次在内建屏，不否定用户上次扩展屏报告。

新部署实际验证：v14-search-glass-fill-confirmed-20260920-02及v14-search-text-tool-20260920-01的AX动作返回不确定，回执input_domain_fenced=false，独立读回确认工具/Fill状态改变，无全局按键恢复调用。此已补上此前仅离线覆盖的非零AX路径；仍未等同用户按住Alt+Tab的完整并行验收。一次JS15秒超时查询后确认Fill仍1、无新圆形，再按最新状态建立新操作；后续改25秒调用预算，不重放已派发动作。无提交/推送。

### 2026-09-20 加号按钮与只读观察修正

移除NativeMac.observe内的activate_background_window调用，避免截图向目标应用注入AppKit激活事件。443测试通过/10ignored，fmt/clippy通过；固定证书重签部署，四项权限保持。新实例实际截图可用，未借此宣称首击/后台兼容全部解决。代码依据及限定见2026-09-20-background-input-modifier-isolation.md。

部署后RWI创建39×43浅灰圆角按钮和两条加号线段，独立截图确认旧红色圆形占位消失。新消息提示仍待替换。开启其调色板时CUA返回windowNotFoundAtPosition；重新发现证实display1从[282,1152,1800,1169]变为[0,1440,1512,982]，主窗口变为[0,1565,1220,527]，旧坐标作废，没有补发同一点击。后续先重新bind与观察，再计算位置。

CUA保存当前进展，文件412182 bytes，SHA256=e9ccbcaa8217c99e39c6687d20e917085a61022d59af57298d437c9b2b225b11。无提交/推送。

### 2026-09-20 新显示器重新定位与新消息提示

用户确认布局变化来自接入新显示器。重新discover/bind：Paint PID2578/cgwindow:6491，主窗口[247,1148,1512,950]，内建display1=[247,1116,1512,982]；重新获取3024×1900原始帧到1024×643预览的仿射映射，未沿用旧屏坐标。

完成白色132×41圆角背景、独立100×30文字“1条新消息”、4个蓝色线段组成的双上箭头；文字/线条RGB87/107/149。背景由CUA创建，文字框和箭头由真实RWI Plugin创建，CUA辅助颜色、文本和保存。这是混合通道进展，不是纯RWI后台闭环。颜色对话框AX顺序不等于标签顺序：当前93是Green、95是Red；通过截图与色井回读发现并纠正，不能按相邻标签猜测。

RWI首次背景拖拽v14-message-pill-20260920-a returned但截图/AX无新增对象；随后显式CUA Raise+独立拖拽对照成功，不能断言Raise本身已使应用前台。后续RWI AXPress多次返回-25204但工具确实切换且fenced=false；4次箭头拖动及文本框创建后立即snapshot出现ax_attribute_unavailable，独立新观察确认各自对象实际存在，没有重放输入。问题仍在后台首次接收与变更后的即时AX读取，不能判定为用户切换窗口干扰。代码追踪发现Cf::attr丢失具体attribute/AXError，后续先补可定位诊断，再决定是否需要有界只读重查；不放宽原生身份和输入前校验。

CUA保存后文件414128 bytes，SHA256=9750f951b4b5617a654ef7edf6614a065845fa13736f6ad9d89bcd53edfcdcf3。RWI独立回读frame=3b5f981f-131d-47c2-a864-cf9bff2eecf5，fenced=false，实际截图保存在work/v14-readonly-observe-20260920/message-pill-rwi-preview.png。整体红色边框、头像/侧栏/顶栏与细节仍未完成，未称1:1验收。未修改产品代码、未新增回归结论、未提交/推送。

### 2026-09-20 AXWindows读回定位、部署与分隔线复刻

先补Cf::attr诊断并部署，真实RWI绘制v14-divider-header-20260920后明确复现：AXUIElementCopyAttributeValue(AXWindows)返回-25204；独立新观察确认808px原生线条已经创建，没有重发绘图。诊断只记录API、属性名、错误码和read类型，不记录属性值。Apple枚举将-25204标为cannotComplete（https://developer.apple.com/documentation/applicationservices/axerror/cannotcomplete）；不单凭该码推断超时或权限失效。

针对该实测路径，observe既有3次上限/150ms间隔的完整读取循环增加精确AXWindows/-25204只读重查。每次重新检查窗口/进程/会话、前后metadata与截图一致性。没有在Cf::attr、输入派发或动作错误上添加重试；其他错误保持失败。新增2项离线测试覆盖诊断字段与严格匹配，445 passed/10 ignored，fmt与all-targets/all-features clippy通过，release构建成功。

固定证书重签部署，生产Agent实例6360fca8-592a-469b-95fd-230852634972；四项系统权限仍true，持久journal未重置。备份在work/v14-ax-diagnostics-20260920/previous-bundle与diagnostic-only-bundle。随后真实Plugin连续完成输入区横分隔、列表纵分隔和侧栏纵分隔：即时绘后observe均成功，约1163/1180/1358ms。没有内部重查计数，不能据此声称每次都触发了新分支；只能确认本轮连续读回不再报原错误。AXPress动作的-25204仍如实保留uncertain，独立读回后继续，fenced=false。

本轮4条分隔线均为RGB220、4px原生可编辑线条，主要旧红线已遮盖；边缘及整体样式仍须最终对照。CUA只辅助调色/参数和保存。文件414852 bytes，SHA256=0d0850dc626e35f316fbf12175b1a27f9353f397a9af66cd21ac367acc7488bb；保存后RWI frame=f7ccf31c-73cb-45e0-8a3d-300bf9641780，预览work/v14-ax-diagnostics-20260920/dividers-rwi-preview.png。后台首次输入接收、原生AX动作不确定和完整复刻仍未解决；未提交/推送。

### 2026-09-20 顶栏右侧图标重绘

复核原始截图后，用真实RWI Plugin创建173×45背景遮盖，以及屏幕分享（矩形/支架/箭头）、四宫格加下拉箭头、添加会话（椭圆/加号/尾线）、更多（三个2×2椭圆）。共新增20个原生可编辑对象，旧底图保留。CUA只辅助RGB242背景、RGB20描边/2px与保存。现有电话/视频图标未重做，四宫格圆角/旋转菱形和气泡曲线仍为近似，不能称像素级复刻。

本轮20次绘制均有新对象截图，绘后即时observe未再出现ax_attribute_unavailable；AXPress仍频繁-25204，工具切换由独立观察确认、fenced=false，没有重发动作。此为绘后观察稳定性样本，不等同完全后台兼容，也没有新增产品代码或测试结论。

保存文件418605 bytes，SHA256=b7436619f79118c2e09a22ecd12f9ee8d451a8f2fa99e7657528d4164a66ac9f。独立RWI预览work/v14-ax-diagnostics-20260920/header-icons-rwi-preview.png。后续主要剩余侧栏/头像/外框及整体比例与字体，不要重复修已足够的第一头像。未提交/推送。

### 2026-09-20 侧栏底部图标及布局变化

新增51×130浅灰遮盖（CUA）和RWI创建的邮件矩形/两条折线、手机矩形/底线、菜单三条横线、红色未读椭圆，总计10个可编辑对象。样式仍为近似：手机/信封尚未做圆角，浅灰补丁与旧侧栏底色不一致，后续统一整个侧栏，不能宣称完全匹配。

首个v14-sidebar-bottom-mask拖动returned但RWI截图无新层，CUA独立AX及截图也确认没有对象；CUA对照拖动才创建遮盖。再次证明后台首输入接收问题仍存在。之后RWI9个对象均经截图确认，绘后读回无AXWindows错误；AXPress仍uncertain但独立回读确认切换，无重放。

过程中窗口布局再次改变，当前主窗口bounds=[282,1190,1800,1131]，display1=[282,1152,1800,1169]；由控件/截图变化发现并重新discover/bind，红点使用新预览位置绘制，未沿用旧坐标。CUA辅助样式和Save，最终RWI独立回读，预览work/v14-ax-diagnostics-20260920/sidebar-bottom-rwi-preview.png。保存文件420389 bytes，SHA256=f40a4db077be500f88ae58195b9aed68181ccbb5914990530a3bd5336dc97ea5。无产品代码变更，无提交/推送。

### 2026-09-20 连贯性、全局AX超时与调用冗余

发现input/anchor_native.rs的point_element创建system-wide AX对象并设置0.05s超时。这不是单次读取预算，而是修改本Agent进程的AX全局默认值。Apple文档明确该作用域（https://developer.apple.com/documentation/applicationservices/1459345-axuielementsetmessagingtimeout）；动作回调内处理也可能超时（https://developer.apple.com/documentation/applicationservices/1462091-axuielementperformaction）。不能把频繁-25204直接解释成应用不支持动作。

修正为目标application级hit-test和局部50ms读取预算，保留PID/窗口/原生身份检查；实际AX动作在精确对象上设500ms有界预算，配置失败时不派发。没有动作重试或吞掉错误。fmt、445 passed/10 ignored、clippy与release通过，固定证书部署，备份work/v14-latency-20260920/previous-bundle；四权限保持，新Agent实例9fc47a55-6409-42af-832f-e7111296e279。

优化实际computer.js调用脚本：工具选择后直接复用该次新preview进行绘制定位，去掉紧接着的重复observe；分页找到唯一工具就停止；绘制前明确读回工具value=1、mutation_ready、blocked/fenced。选择到绘后验证共3次observe，旧调用通常4次；失败不再在每步输出重复长错误，正常仅一条摘要和最终截图，完整调用轨迹/操作回执仍保留。脚本work/v14-latency-20260920/shape-helper.js（这是验收调用层优化，非MCP协议全局瘦身）。

真实Plugin创建联系人遮盖、头部椭圆、身体圆角矩形3个可编辑对象。三个样本5780/7162/7404ms，均tool_action_uncertain=false，均由模型检查新对象截图确认。旧步骤常7–9秒且频繁AXPress-25204；不同布局/图形及样本少，不作严谨性能百分比比较。原生动作单独probe585ms并正常返回。此次首次绘制生效但不能宣称所有后台首次输入问题根治。输出保留goal_verified=false直到调用方独立检查，不把returned当成验收。样本细项work/v14-latency-20260920/live-samples.json。

联系人仅近似形状，CUA辅助颜色/边框设置及保存。文件421242 bytes，SHA256=67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。仍待：更多后台首输入场景、耗时拆分/紧凑协议、侧栏其余图标与整体复刻。未提交/推送。

### 2026-09-20 直调MCP工具：snapshot / click / drag

用户明确要求优化单步耗时、工具数量/便利性和临时JS依赖。复核实际CUA getAXState/getScreenshot、click、drag、setValue、pressKey及标题/增量输出，并研究Microsoft Playwright MCP的snapshot/ref工具、Anthropic Computer Use坐标动作。具体依据与参数例见docs/architecture/direct-computer-tools.md。

新增纯Rust、平台中立gateway组合工具computer.snapshot、computer.click、computer.drag，附人类可读title及read-only annotation。snapshot一次调用取得简短控件和映射图，默认64控件、行数据12KiB上限；完整原始观察保留observation_ref。click支持精确frame/element或view_id坐标；drag接收同一截图起终点并内部转为native偏移。两种输入内部claim/dispatch/release，稳定request_id，绝不重放，清理失败保留原动作和清理错误。动作后不自动截图，调用者明确snapshot核验；不是以returned为业务成功。JS仍保留高级编排，当前JS嵌套白名单尚未开放这三个组合入口，直接MCP调用不依赖JS进程。

回归450 passed / 10 ignored，fmt/clippy通过，release成功；测试覆盖参数/标题、真实MCP子进程工具目录、租约失败无派发、不确定动作不重放、清理失败原始证据、Windows控件保留和输出预算。这不是新工具的GUI端到端验收。

已原子部署稳定client-v1/bin/rwi-mcp，保留原客户端身份/profile，未重启Agent、未修改TCC；备份work/v14-direct-tools-20260920/previous-rwi-mcp。部署校验最初被rtk proxy对带空格可执行路径的拆分阻断，改为Python argv直接启动后成功。全新MCP进程tools/list确认29工具及三个新标题；证据work/v14-direct-tools-20260920/catalog-check.json。本会话的工具目录仍未刷新，新工具不可直接调用，故未绕过Plugin冒称实机验收；需Host重新连接/新会话加载后验证。未创建新任务，未提交/推送。

当前性能改进是减少临时代码和Host往返，原生AX/capture成本没有消失；未给未经对照的整体加速比例。后续：新工具真实Paint验收、type/key/scroll直调、语义检索/控件分页、增量快照和各阶段耗时。Paint保存版本仍为421242 bytes / 67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。

### 2026-09-20 重启后直调工具实测：部分通过，拖动未通过

重启后本会话已实际加载snapshot/click/drag，直接通过Plugin调用，没有computer.js脚本。mac-host四权限true，重新discover/bind Paint PID2578/cgwindow6491，bounds=[282,1190,1800,1131]。snapshot返回截图和原生控件，耗时约945–1087ms；click的精确Line AXPress耗时516ms，独立快照确认Line=1，无AX不确定。

drag在左侧空白画布测试，500ms路径调用返回1412/1437ms，但RWI后截图与CUA独立AX/截图均无新对象，因此拖动验收失败，不能当作性能成功。坐标工具选择以及CUA AX选择Line后的新测试仍未产生图形。随后CUA自身drag实测1.73s产生94px测试线，CUA Undo明确显示Undo Add Line并恢复原图层；没有撤销任何既有绘图，没有保存临时线。原文件哈希保持67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。

另定位到数次16–18ms的stale_reference为core/lib.rs的固定10秒snapshot年龄门禁（非窗口变化或原生命中失败），与独立模型工具调用往返不适配；未盲目重发，已确认派发前拒绝。需要设计显式有效期和保留全量revalidation的模型友好预算，不能靠每次临时JS把操作挤入10秒窗口。当前没有修改该门禁。

输出亦暴露改进点：snapshot包含Name:Move previous/Target/Selector等非标准AX动作噪音，64控件截断在编辑态会漏掉Undo；需要动作过滤、摘要归一化和直接控件搜索/分页。参考旧JS summary已有AX前缀过滤，而新Rust projection未沿用。本轮不宣称全链路通过；后续优先原生后台接收、引用有效期及输出筛选。回执保存在work/v14-direct-tools-20260920/live-receipts.json。未提交/推送。

### 2026-09-20 TTL/快照修正与流式架构补充

本轮将输入proposal年龄从10秒统一为120秒，与checked语义入口共享常量；保留逐次完整原生身份/窗口/布局/内容重验证，窗口placement旧10秒另行保留。观察的adapter_status由设备报告input_proposal_max_age_ms，新snapshot仅转述设备事实，旧设备缺失时不凭Host版本推断。增加30秒旧proposal成功、相同年龄布局改变失败、121秒超期拒绝测试，更新旧10秒假设测试。

Rust快照过滤非标准AX action字符串，将空name用description归一化，删除可省略的true/false显示字段但不虚构未知selected。完整原始元数据仍保留。新MCP二进制已原子安装client-v1，但本会话旧进程未重连，精简输出要下次连接生效。

背景输入移除以AXMain=true跳过AppKit激活准备的条件（AX主窗口不代表前台激活），并把该事件记入native dispatch计数。已部署Agent实例78019e68-5826-4adc-983a-891858b3625a，四权限保持。故意等待11秒后执行direct drag，1641ms返回且不再TTL拒绝，但第一条未画出；第二条新测试1467ms返回并实际新增94px Line。唯一新测试线已通过CUA Undo Add Line撤销，原文件未改。

进一步调研：web访问一度失败、GitHub tree API限流后，取得只读公开参考work/reference-cua-20260920，commit 9bbfa7dd3e27ca7f1861ede70aaca390174493f9。其libs/cua-driver/rust/crates/platform-macos/src/input/mouse.rs的post_mouse_moved_primer明确在目标点先发MouseMoved更新后台AppKit tracking。参考思路（未复制其双重派发/全局HID路径），RWI drag按下前增加已grounded起点的MouseMoved、12ms等待及重新guard；不warp系统光标、不额外点击、不重复down。最新源码453 passed/10 ignored、fmt/clippy与release构建通过；此MouseMoved候选尚未部署/实测。当前运行Agent仍是前一份AXMain/TTL修正版本。两份备份在work/v14-direct-fixes-20260920。

用户新增方向：考虑Streamable HTTP和双向流式状态。已查官方OpenAI MCP文档与MCP transport/progress规范：Codex支持STDIO及Streamable HTTP，stdio也可承载双向JSON-RPC notifications，换HTTP不等于模型获得实时视频/持续中途推理。计划将transport与执行事件流分层，先完善stage/progress、取消、断线事件游标与去重，再加可选本地Streamable HTTP入口；只恢复事件，不重放输入。暂无HTTP监听部署或配置迁移。

### 2026-09-20 MouseMoved候选部署通过样本与标准进度通知

部署上轮已通过453项回归的drag MouseMoved预定位候选；生产Agent实例393565eb-98ca-4ee8-a031-ae4899fadc99，四项权限true，持久journal保留。首次targets在服务重启瞬间无目标，第二次只读查询恢复，未发送输入或重放。重新绑定Paint后，完全通过直调MCP click/snapshot/drag执行：Line选择897ms、首条500ms drag返回1439ms，截图明确新增94px Line；随后从observation_ref分页取得Undo原生元素，用RWI click撤销597ms，独立截图确认测试线消失。没有借助CUA，也没有保存测试线；原文件哈希仍67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。该样本是Agent重启后的首次拖动，不是Paint进程冷启动/任意失焦情形的全面证明。回执work/v14-pointer-primer-20260920/first-drag-receipt.json。

实现标准MCP notifications/progress，接入三个直调工具的gateway阶段：接收、读取、映射、claim、唯一派发、release返回、最终结果。仅Host传progressToken时发送；单请求计数单调，无假百分比；通知最多等待25ms且不改变输入结果、不重放，cleanup先执行再通知；取消请求不继续推状态。未增加HTTP监听、设备端事件订阅或持续图片流。

真实MCP子进程测试确认有token时两条递增通知先于错误最终结果，无token时无通知。最终454 passed/10 ignored，fmt/clippy通过，release成功；已原子更新稳定client-v1/bin/rwi-mcp。部署后的release二进制也通过无设备/无GUI进度通知测试，证据work/v14-pointer-primer-20260920/progress-deployment.json。当前会话仍运行旧MCP进程，进度通知和快照精简需要下次重连，尚未声称Codex界面实际显示已验证。Agent拖动修正本轮已生效。未提交/推送。

### 2026-09-20 控件查找、文本与快捷键直调入口

新增computer.controls：使用snapshot的observation_ref+frame，对原始缓存控件按name/description/role子串过滤并分页，默认32条。缓存校验来源必须computer.observe且frame匹配；不截图、不RPC、不续期、不自动选择同名控件。返回所有匹配及next_offset，原生ID和未知状态保留。输出超预算导致无法前进时明确报错，不产生相同游标死循环。新增缓存有效期/不续期、来源/帧错配、歧义分页、未知查询与超预算测试。

新增computer.type_text、computer.press_key：前者1..4096 UTF-8字节原生输入，后者1..128字节平台原有key_chord；要求exact target/frame/element/stable request_id，复用checked action和短租约管理。没有自动focus、清空字段、clipboard、全局键盘或shell回退；受原生焦点与secure/unknown分类门禁约束。进度通知使用既有标准阶段，不包含输入文字。包含Unicode保持、参数边界和真实MCP子进程目录/无效参数回归。

最终459 passed/10 ignored，fmt/clippy通过，release构建成功。已原子部署稳定client-v1/bin/rwi-mcp，完整清单32工具；新release进程确认三工具标题/read-only标记和无效参数拒绝，证据work/v14-controls-keyboard-20260920/deployment-check.json，备份同目录previous-rwi-mcp。未改Agent、权限或profile，未触碰Paint文档。此为实现/协议验证，不是三工具的原生GUI验收；当前连接尚未更新目录，待下一次MCP重连统一验证，不要求逐个工具重启。现有MCP合成工具仍不在JS嵌套白名单；直接调用。未提交/推送。

### 2026-09-20 重启后的 controls / keyboard 实机验收（部分通过）

本会话已加载三个新直调工具，mac-host 原实例四权限为 true；重新 discover/bind Paint PID2578/cgwindow6491。controls 的 Text/Undo/不存在查询各约 5ms（工具内 elapsed_ms=0），正确保留多个匹配/零匹配；snapshot 实测 965–1239ms。精确 Text 工具点击 755ms，截图确认选择；左侧空白画布点击 1943ms 创建独立临时空文字对象。首次坐标点击因 content_changed 明确在输入前拒绝，重读后使用新请求，不重放。

type_text 请求 accept0920-unicode-1 输入 RWI 验收，509ms 返回 uncertain；原请求查询明确 dispatch_error=secure_or_unknown_text_field，input_domain_fenced=true。独立截图没有测试文字，不能宣称输入通过。源码 TextClass::classify 对缺失/AXUnknown subrole 保持 Unknown；需补充原生属性证据后修正分类，不直接放开 unknown。另需追踪派发前文本校验失败为何产生 uncertain/fence。snapshot 同时 mutation_ready=true 与 input_domain_fenced=true 的摘要易误导，也待修正。

查询原请求后，recover_input 独立验证零按键/按钮及 writers idle，清除输入域 fence，未重发文字。RWI Select 与 Undo 仅返回回执，编辑框仍存在；明确切换到 CUA 做清理，Escape 约 0.63s（含 AX 观察）退出空编辑，AX 与截图确认空 Text 层消失。这是混合清理，不是 RWI 键盘验收通过。随后在 CUA 确认的聚焦 scroll area 上，用新 RWI snapshot 精确元素执行 press_key escape，483ms returned；后续截图无变化、fence=false。此为按键通路无错冒烟，不是带可见结果的快捷键功能验收。

发现便利工具把 is_error=false 的 uncertain 回执当普通成功压缩，隐藏 dispatch_error。已修正为只有 dispatch=returned、无 dispatch_error、未 fenced 才精简，其余保留完整原生证据；新增回归。fmt、workspace locked offline tests 460 passed/10 ignored（38 suites）、全特性 clippy -D warnings 通过。此次新修正仅源码完成，未部署、未要求再次重启。

Paint 原文件 SHA256 仍 67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e；未保存临时测试、未提交/推送。结论：重启加载和缓存控件查询通过；文本输入未通过；press_key 仅冒烟；后台全过程与 Codex 中途 progress 显示仍未验收。下一步优先文本分类原生证据、派发前拒绝/不确定状态区分及摘要一致性，再统一部署验收，继续 Paint V14。

### 2026-09-20 文本分类修复部署，纯 RWI 短 Unicode 与 Escape 验收通过

区分 AXSubrole 原生明确缺省（-25205/-25212/成功空字符串）与读取失败；前者普通 TextField/TextArea 可分类 Plain，后者/AXUnknown 仍 Unknown，安全子类型继续拒绝。Apple nullable subrole 文档及截图证据见 docs/validation/2026-09-20-text-input-acceptance.md。键盘目标/payload 校验移至后台激活之前，每个事件批次仍重验。MCP 完整保留异常回执，fenced 快照不再 mutation_ready=true。

fmt、463 passed/10 ignored、clippy、release 均通过。已用原稳定证书部署 Agent，实例81a811a6-9018-4f48-b3ef-2eef75de1de1，四权限 true；MCP 原子更新，但现有 Host 进程仍使用旧摘要，下一次自然重连才生效。备份 work/v14-text-classification-20260920，不改 journal/profile。

纯 RWI 新建空白区临时 Text，输入 RWI 验收 680ms，AX value/截图一致；Escape 706ms 确认编辑器消失；Undo 624ms 删除该层。无 CUA、无保存、fence=false。无效键名测试240ms，原请求证据 unsupported_key/native_dispatch_entered=false/input_domain_fenced=false，无需恢复；core receipt 仍写 uncertain（状态精度待修），未隐藏此限制。短文本样本不代表多批长文本/IME及任意后台失焦场景已通过。

原文件哈希仍67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。三张按时间顺序实际截图在outputs/validation/2026-09-20-text-input/。未提交/推送。下一步细化明确零派发回执、长文本输入与增量快照/阶段耗时，继续 V14 原生编辑。

### 2026-09-20 零派发回执与多批长文本修复部署/验收

Core 输入/窗口移动失败仅以已知未饱和计数前后未变标记 rejected_before_dispatch；Observed 透传计数，unknown/已派发/饱和仍 uncertain。拒绝回执 durable 持久化，同 ID 跨重启仍禁止重放。实机非法键名217ms确认新状态、native_dispatch_entered=false/fence=false。

长文本首次只输入前20 UTF-16 单元，后续几何派生 ID 导致拒绝；首个增长候选仍失败，新增原生诊断证明同 AX 对象、原点不变，尺寸从7.5×16.5变为242.25×15.75。最终 Text 单次调用内以CFEqual对象/焦点、原身份/窗口/位置等连续性为约束允许尺寸变化，原点移动/替换/安全属性等仍拒绝，不续期或重放。最终43 UTF-16单元3批715ms输入完整，AX值逐字匹配、fence=false；RWI Escape676ms、Undo632ms，截图确认测试层消失。全过程无CUA；失败样本均独立查询/恢复/撤销，没有重试原输入。

最终fmt、465 passed/10 ignored、clippy -D warnings与release通过。Agent已按原证书部署实例3463cc61-c09d-4698-857f-7578e01cb41d，四权限true。证据与时间顺序截图：docs/validation/2026-09-20-longtext-acceptance.md、outputs/validation/2026-09-20-longtext/。原Paint文件哈希未变，未提交/推送。当前Host旧MCP摘要仍待自然重连；Agent改动已生效，无需用户重启。下一步V14实际编辑、增量观察与阶段耗时，多行/IME/极限长度仍需另验。

### 2026-09-20 分阶段耗时与 V14 侧栏 # 原生编辑

MCP direct tools增加stage_timings，按invoke工具名聚合次数/耗时，包含失败及release；亚毫秒先聚合再取整，不记录参数/输入文字。总elapsed含通知/呈现开销，阶段值是adapter墙钟时间，不宣称原生AX/capture细分或实际提速。新增聚合单测及真实MCP子进程测试：无效参数空阶段列表、非法目标保留失败observe阶段。最终fmt、466 passed/10 ignored、clippy通过；release已原子部署并核对安装二进制SHA一致。当前Host进程仍旧版，阶段字段待下一次自然重连，不要求立即重启。

纯RWI继续V14，新增侧栏#：1个矩形遮盖+4条可编辑Line，原底图保留。中途desktop_busy派发前拒绝，未清锁/抢占；后续重新观察的新请求成功。一次连续拖动误移矩形（Paint画完自动返回选择），已Undo Move Rectangle并核对恢复，后续每笔前重新选Line并检查value=1。已RWI cmd+s保存，文件422000bytes / SHA256 22fe04bf8331656d15e4d7a089228791c35b7403de41c78ddf815ad19c5c1d39；旧版备份work/v14-stage-timings-20260920/paint-before-hash.paint。无CUA、未提交/推送。

截图及边界见docs/validation/2026-09-20-sidebar-hash.md。遮盖底色与旧底图仍有轻微色差需修整，V14整体未完成；下一步继续侧栏其余图标/头像及背景统一，并用阶段耗时定位实际瓶颈。Agent未重启、权限未改。

### 2026-09-20 用户要求保留原生源文件、图层与操作历史

已新增交付快照outputs/paint-v14-source/paint-v14-20260920-sidebar-hash.paint，与当前工作源文件字节一致（422000 bytes，22fe04bf8331656d15e4d7a089228791c35b7403de41c78ddf815ad19c5c1d39）。只读解析两层NSKeyedArchiver确认117绘图对象：5图片/65线/26矩形/11椭圆/7圆角矩形/3文字，未重写归档。生成manifest及README操作记录，后续分阶段新增版本，不覆盖既有快照。尚未验证Paint Undo栈会持久化（未见history/undo/redo键），已明确告知不等同图层保存；没有关闭文档/清空会话撤销栈。

底色修整只完成诊断：现有侧栏像素217,217,218，与遮盖242,242,242不一致。填色popover在CG清单可见但不是独立AX窗口；owned_popup未实现，fresh initial绑定后observe报window_missing。回主窗口AX可见Input Color Manually；点击后popover关闭、没有新的颜色窗口或可见颜色更改，不能当作修色成功。本轮没有改色/新增图形，原文件与上一阶段一致。下一步需修正popover操作/核对原生动作路线后再继续底色和图标。备份work/v14-sidebar-polish-20260920/before-polish.paint。

### 2026-09-20 遮盖色修整完成、原生快照保存与双击便捷入口

RWI及CUA单次点击颜色控件未见弹窗，复核官方说明后CUA双击成功打开稳定popover（此前明确告知Raise做前台对照）。随后RWI显式AXPress手动颜色按钮返回uncertain/-25204，独立discover证明确已打开外接屏Input Color；查询原请求确认无held input/fence，不重放。RWI新鲜选择实际对话框，切HEX/输入#d9d9da/确认，并核对主窗AX填色。截图内外四处像素均217,217,218，补丁色差已消除。这是混合流程，不宣称纯RWI或完全后台通过。详见docs/validation/2026-09-20-mask-color.md。

已RWI保存原文件并新增outputs/paint-v14-source/paint-v14-20260920-mask-color.paint，不覆盖旧快照；422140bytes，SHA256 ab571735b04841b7b2914d48e96b5a68a649a45158054da8fef223ab3102d35a，仍117绘图对象。manifest与README记下过程；未关闭文档、未清撤销栈，跨重启Undo持久性不保证。

依据实际需求，computer.click增加可选click_count=1|2（默认1）；2映射单次native double_click intent，单request/lease，不循环重复click。显式AX action与2组合及其他count输入前拒绝。fmt、467 passed/10 ignored、clippy -D warnings及release通过；原子安装MCP，备份work/v14-doubleclick-20260920/previous-rwi-mcp。Host旧目录尚未刷新，新参数便捷入口实机验收待自然重连；不要求用户立即重启。Agent/TCC未改，未提交/推送。下一步继续侧栏其他图标、原生popover模型与后台双击验收。

### 2026-09-20 V14 收藏星形轮廓保存（128可编辑对象）

形状菜单CG存在但AX主窗无可读形状项目，独立绑定观察window_missing；CUA只用于Cancel菜单，没有猜选或绕过权限。RWI新建同底色Rectangle，并尝试Pen单次11路点闭合轮廓：Smooth开/关两次均丢失凹角，独立原始像素tile确认未达预期，均Undo Sketch，原因未确定、不归咎单一开关。

最终用10条独立Line完成星形轮廓，每笔重选Line/确认value=1并检查映射不变，错误或uncertain立即停止。最终放大截图确认五角轮廓，原图圆角及内部细节仍待精修。绘制/宽度输入/撤销/保存均通过RWI；CUA仅取消菜单。未修改产品代码、未提交/推送。

已保存并新增outputs/paint-v14-source/paint-v14-20260920-sidebar-star.paint，423768bytes，SHA256 3743a18e1d6379c9db029e58593d4598614f33acd481b49fac16b2912abcbd02。只读归档确认128对象（5图片/27矩形/75线/11椭圆/7圆角矩形/3文字），无Sketch残留，旧快照保留。证据docs/validation/2026-09-20-sidebar-star.md及outputs/validation/2026-09-20-sidebar-star/。下一步其余侧栏图标/头像、原生菜单/弹窗观察、多路点拖动真实几何保真验证。

### 2026-09-24 iPhone 副屏接入、mac-host TLS 配对到期修复

用户已将iPhone无线接为扩展副屏供RWI使用。系统只读显示器清单确认Sidecar在线，1434×660逻辑尺寸。最初computer.targets对mac-host返回证书过期；Agent进程与127.0.0.1:45129监听正常，问题不在显示器。查明provision固定3天leaf/7天CA；修改为180天leaf/365天CA，保持WebPKI时间/链和pin校验。fmt、467 passed/10 ignored、全workspace clippy及release provisioning构建通过。

新私有配对存于Host安装目录pairing-20260924，原配对/配置备份保留；只更新RWI Agent与mac-host profile，持久journal、Windows设备条目、TCC均不变。Agent重启后，fresh production MCP子进程initialize/computer.targets成功并报告四权限true。现有Codex任务MCP进程缓存旧pin，返回UnknownIssuer；终止本任务的该进程后工具Transport closed，未重启或终止其他任务进程。当前任务尚未重新获得Host原生RWI GUI工具，不能继续Paint实机操作或以standalone子进程冒充Plugin验收。需Codex重载MCP后按新显示器布局重新discover/bind/snapshot。证据docs/validation/2026-09-24-pairing-expiration.md；当前Paint源文件哈希仍3743a18e1d6379c9db029e58593d4598614f33acd481b49fac16b2912abcbd02，未移动窗口、未改图层、未提交/推送。

### 2026-09-24 BetterDisplay 虚拟工作区与反馈光标修正

用户重启后本任务实际Plugin恢复，四项TCC true。已创建唯一RWI Workspace虚拟屏（1920×1080 HiDPI、物理3840×2160，display7），通过RWI将Paint主文档与参考图移入；主屏与iPhone设置不变，遗留Open面板已取消。BetterDisplay本地PIP目标15fps、普通右下小窗观察虚拟屏。反馈光标旧版实际停主屏已复现并修正：显式frame/事务提交/最多50ms只读位置回读，最终三个不同点均落在display7且无error。静态截图底行抖动新增指纹边缘处理及对应输入边界约束，但选框动画仍误拒绝；后台菜单快捷键仍有回执无效果，未通过的PID-only候选已撤回。

第二行原图头像独立Image已定位48×48，最终CUA保存，不能算纯RWI闭环。新增outputs/paint-v14-source/paint-v14-20260924-avatar-row2.paint：438740bytes、f2b2d4e5615b24649355ceaba7cc5f514bae0b65f52d94ebf888e1db1f413f1c、129对象；旧快照保留，Undo跨重启持久性不保证。最终fmt、470 passed/10 ignored、全特性clippy、release通过；同证书部署Agent PID74634，四权限保持true，journal/profile/MCP未改。没有提交/推送。详情/实际截图/后续顺序：docs/validation/2026-09-24-virtual-workspace.md。用户要求另开归属Real World Interaction的任务专门研究虚拟屏方案，当前复刻任务继续保留。

### 2026-09-24 会话 Live 观察与本地 PIP 的范围澄清

用户指出 Computer Use Live 是当前会话的叠层，能随着应用调起系统文件、字体、颜色面板；不是具有经典三按钮的普通预览窗口。此为用户观察，尚未独立核对 Codex 内部关联规则。BetterDisplay PIP 是常驻应用进程的普通窗口，当前仅固定显示虚拟屏7；系统 Open 面板曾在内建屏1，说明固定屏画中画不能自动覆盖整个跨屏任务。RWI 当前 snapshot 单次绑定精确窗口，Mac popups_for_pid 范围仍是当前 PID，跨进程 owner-chain 切换未实现；反馈光标是 Agent 独立窗口，仅指示坐标。详细比较和下一步会话范围的多窗口观察需求已补到 docs/validation/2026-09-24-virtual-workspace.md。已把补充传给独立调研任务，并协调其短时 GUI 实测；本任务暂不触发桌面输入。

独立调研任务已归还桌面，报告了 TextEdit 在 BetterDisplay 虚拟屏成为真正前台应用的对照：Open 面板留在虚拟屏，Colors 面板转到内建主屏，RWI 单独发现/绑定/观察后者。逐步证据仍由该任务整理；这不等于 RWI `activate_window` 已实现或 Codex 会话 Live 机制已得到验证。本任务可继续 Paint V14，但跨屏观察设计仍应按相关窗口集合处理。

### 2026-09-24 BetterDisplay 移窗稳定回读与 V14 第三行头像

针对独立 TextEdit 实测的移动动画 `display:null`，Mac `move_window` 在唯一 AXPosition 后增加≤950ms只读稳定回读，目标显示器连续两次同几何才标 `placement_confirmed=true`；AX设置进入计数，错误不再误判零派发。真实Plugin将隔离 TextEdit `[152,72,586,488]` 从内建屏移到BetterDisplay display7 `[1536,24,586,488]`，2次回读/62ms确认，独立discover一致；未触碰V14。显示器桌面原点已从历史x2946变为x1512，旧坐标作废。

继续V14：参考图首次试选误画Ellipse，CUA Undo恢复，原参考PNG未改。RWI明确选择Rectangular Selection后取第三行企鹅头像82×82；被工作窗遮挡时键盘派发前拒绝，CUA Raise后RWI复制、切工作窗后RWI粘贴成第七个独立Image，RWI缩放至50×48、定位第三行。缩放提示由CUA关闭并结束编辑；一次无尺寸变化的短拖不算成功。前台/已结束编辑条件下RWI cmd+s此次实际改变磁盘，而非只收回执。新原生快照outputs/paint-v14-source/paint-v14-20260924-avatar-row3.paint，446815bytes、SHA256 b805184358b19b47e4a293f003a71349a4c044b4c40bf28ddcd84ce7cef53d23、130对象（7 Image）；旧版保留。来源仍是混合通道，不宣称后台Save或Live已通过。

曾试验放宽未知AX可见性以供聚焦滚动区快捷键使用，但现场拒绝来自明确 `visible=false/occluded=true`，故候选撤回并重新部署最终收紧版本。最终fmt、471 passed/10 ignored、clippy全目标全特性通过；固定证书签名Agent PID32328，四项权限true、input fence=false，MCP/profile/journal未改。本轮未提交或推送。详细分步证据见docs/validation/2026-09-24-placement-avatar-row3.md；剩余第三行宽2px精调、其他头像与主界面、Mac显式激活/跨屏面板、动态选框及会话Live。

### 2026-09-24 Live 只读观察底座与 BetterDisplay UUID

Mac `computer.capture_visual` 已实现并以当前任务真实 Plugin 在 Paint S 主窗口实测：ScreenCaptureKit 直接请求 512/1024/逻辑原尺寸，返回单窗 PNG，不遍历 AX、不创建输入 frame/引用；开始/结束复核原生窗口及显示器拓扑。已把当次所有 active displays 的 ColorSync UUID、id、bounds、像素尺寸/旋转加入 `computer.discover`，窗口记录携带所属显示器；`move_window.display.uuid` 可以与 id/built_in 同时 fail-closed 匹配。现场虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，当次 id7 / `[1512,0,1920,1080]`，与 BetterDisplay Console 记录一致。

离线门禁 fmt、workspace locked offline **475 passed / 10 ignored（38 suites）**、all-targets/all-features Clippy -D warnings 全过；Agent 本地证书重新签名部署，最终 PID45416、四项 TCC true；rwi-mcp 原路径更新且插件重装为 `0.2.0+codex.20260924114855`。本任务加载的旧 MCP schema 仍在使用；UUID 工具参数需在新任务加载新版插件后做 Host 实测，本轮未移动 Paint。原生画稿快照 SHA 未变，未提交/推送。证据与原样截图：[2026-09-24-live-visual-display-uuid.md](../validation/2026-09-24-live-visual-display-uuid.md)。

**边界**：这只是 Live 的单帧视觉底座，不是会话绑定叠层或跨 PID/跨屏系统面板自动跟随。后续仍需显式关联窗口图、订阅生命周期、隐私范围和 Codex Host UI 承载契约；BetterDisplay PIP 只能固定预览虚拟屏。Paint V14 第三行头像仍待宽度精调，后续对象未完成。

### 2026-09-24 重启后 UUID 闭环、Live context 预览与单调用移窗

当前任务已实际加载 UUID schema，并用 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41` 将 Paint 主窗口移到虚拟屏内 `[1536,24,1434,660]`，2 次/57ms稳定读回、独立discover确认。首次分步调用引用超时被拒绝，因此新增 `computer.place_window`，内部完成fresh observe/resolve/claim/move/release，保持单次派发和持久去重。

已实现 `computer.live`：现有 Agent 中的无标准标题栏、非激活浮层，当前 MCP device context 所有；显式新绑定切换来源，暂停/关闭清空图像，结束context/期限停止，迟到帧按generation丢弃；系统截图回调未释放时仍保留采集槽位。自动面板归属和 Codex 内部 Live UI 集成仍缺。新代码门禁481 passed/10 ignored、fmt/clippy全绿；Agent PID68622、四权限true，MCP及插件 `0.2.0+codex.20260924130816` 已部署。当前任务callable工具表尚无这两个新增入口，**Live浮层与place_window的Host验收待再次重载**；不能把编译/部署当作界面已通过。画稿快照SHA保持不变，未提交/推送。详情：[2026-09-24-live-context-preview.md](../validation/2026-09-24-live-context-preview.md)。

### 2026-09-24 Live Host 主路径验收、浮层修正与 V14 48×48 源文件

用户再次重载后，当前Plugin已实际加载live/place_window。place_window单调用773 ms并经原生稳定读回确认；这是已在目标虚拟屏的同屏放置样本，不宣称跨屏性能。Live主屏浮层实际显示虚拟屏Paint，显式bind可切换参考原图和内建屏上的提示弹窗；自Agent窗口排除有效。来源弹窗消失时paused/清空，显式重绑主文档后恢复；API pause/resume/stop、期限停止及CUA按钮对照已分别验证。仍是MCP context所属RWI浮层，不是Codex内置Live或自动面板归属。

实机发现并修复：自定义AppKit循环缺finishLaunching导致AX不可读；深色按钮对比度；Apple Silicon下暂停文字对齐枚举；增加来源屏幕/标题及清空占位、缓存UI chrome。又发现AppKit hidden先于窗口服务器收起动画结束，新增有界readback并禁用仅RWI浮层的默认窗口动画。最终start/pause/resume/stop的presenter_readback均settled=true；stop两次读回31 ms、on_screen=false，紧接着独立discover返回Agent窗口清单为空，worker_done=true、native_call_pending=false。保留未通过候选的原始记录，未重放不确定操作。

RWI拖动把第三行企鹅头像从50×48改为48×48；尺寸提示由CUA新鲜AX点击OK关闭，之后RWI Escape/cmd+s实际落盘。独立归档验证130对象（7 Image/27 Rectangle/75 Line/11 Ellipse/7 RoundRectangle/3 Text），第三行图片bounds=[476,361,48,48]。新快照 `outputs/paint-v14-source/paint-v14-20260924-avatar-row3-48x48.paint`，446815 bytes，SHA256 `1573f4f74b45a6c13014dd9c0efeb1d2d3a2185f9fa4368edab74c91ba8a9998`，与工作文件一致；旧快照保留，不承诺Undo栈跨重启持久化。

**新增未解决问题**：Paint NSAlert的可见CG3850与AXDialog3849不是同一ID/几何；CG全目录有逻辑3849但on-screen目录无它。Live视觉可读，RWI snapshot/语义绑定失败。已保存限定PID只读原生诊断，未靠同PID/相近几何放宽身份；后续需语义与采集目标分离。另一次RWI对自身浮层AXPress返回-25200/uncertain，但独立观察确认暂停；无重放，CUA使用相反的新操作验证继续/关闭。完全后台保存、其余头像/侧栏、自动系统面板关联仍待推进。

最终fmt、workspace locked offline **483 passed / 10 ignored（38 suites）**、all-targets/all-features Clippy -D warnings、diff check和release通过。签名部署Agent PID86935 / 实例ce37a10f-79d1-4f65-ae13-0bf97f480118，SHA256 `45fdf4c21756476d6db4de5b2f468e35256c458966badd86fdfa1a5d00be97f6`，四项TCC保持true。MCP/Plugin工具表未再改变，**无需再重启Codex**；未提交/推送，最终Live已停止、Paint保持打开并重新绑定。Jev咨询服务限流，不冒称其审查通过。完整逐步结果与实际截图：[Live Host验收](../validation/2026-09-24-live-host-acceptance.md)。


## 2026-10-10 综合原生验收推进（未封版）

- 用户确认 Live 叠层可见、任务切换返回；旧版晚期wrong-display失败保留，不宣称全面通过。
- 已实现、回滚验证并部署：可选自有环境sampler、Live首个停止原因保护、640点紧凑预览、失败几何保留。locked/offline workspace674pass/11ignored；fmt/Clippy/release0；Agent2234/66189a8f，MCP40与稳定签名/配对/权限保持。
- 两次独立5s拖拽均1/79/1/complete。第一次用户明确未切换；第二次确认框协调后在move44/68时捕捉不同NSWorkspace前台PID，durable释放且没有重放。接收者active/key/order改变仍是缺陷，不声称真后台隔离。
- 自有01–06均关闭退出0；第06完整发现0窗口发生在输入前，呈现/Space原因未知。新紧凑预览449/56帧呈现，无可见时几何读回。
- 当前源541份与历史231 commits secret扫描0；资产/日志/许可继续审，尚未commit/push/改public。标准PR #1已附。
- 完整A01–A07、F01–F16继续：原生Host Live绑定、前台接管/冲突/取消、tracking hover、modal/window-family、离屏/Space、DnD、browser route、压力与分层性能；补充问题不替代总计划。
- 详细记录：`docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md`。


### 后续原生无指针激活修正（同一目标、尚未封版）

- `needs_background_activation` intent policy：pointer全不发送AppKit激活primer，keyboard兼容保留；explicit hover guard未弱化。675workspace/11ignored、fmt/Clippy/release0；独立example32。
- 实机09在active/keyfalse且cover顺序保持时完成1/79/1蓝线。10在同样状态完成普通single/double3pairs、scrollclip y200、covered AXPress计数1；这些是自有app效果/状态证据，不是通用应用承诺。
- 11实际Unicode回读通过；keyboard仍会改变appactive，left-key仅receipt不可作goal。12显式Regular fixture已到期关闭并detach；晚期前台变化来源未知，未发拖拽，不计前台接管验收。
- 最新Agent60659/9ff5b99a，稳定签名/配对/四权限保持，MCP40不变；源码与可执行rollback原hash恢复/同输入对照完成。
- 07超短fixture在模型两段间到期；08完整段Live gate隐藏0帧。真正Host Live任务绑定、modal/stack、hover、其它功能/性能和发布完整计划都继续，未commit/push/public。

### 系统级前台键盘（用户补充后，未封版）

- 用户指出 Command+Tab 可由工具发送，无需人工按键；已新增显式 `computer.hotkey`（`target/lease/request_id/key/expected_foreground`），与定向 `press_key` 分开。原始桌面授权、共享 lease、持久不重放、原生 foreground/session/HID 复核；有界 modifier/key-down 后逆序配对释放，取消仍清理自己的已发送键。没有后台失败转全局、自动 focus 或文本桥接。
- 全部 locked/offline workspace **685 passed / 0 failed / 11 ignored / 42 suites**；fmt、all-target/all-feature Clippy -D warnings、release0。独立 receiver example32沿用。新增10项离线测试不等于真实 Cmd+Tab 通过。
- 同输入 contract：BASELINE101（无独立入口）、MODIFIED0、ROLLBACK101（恢复同一缺入口行为）、REAPPLIED0；10份旧文件哈希+4个原始不存在状态恢复，root仍为修改版，完整四角色继续扩展。
- 签名 Agent22357 / 09186440-f561-4a87-bf18-d9e0c7257124 已部署；MCP新进程实际列41工具，当前任务工具表仍40。稳定本地签名、CA/证书/config/profile及四权限均保持；这次需要 Host 重载新增工具，不是重新授权。
- MCP首轮部署只读探针出现 operator Python缩进错误1，自动恢复旧MCP；保留错误/回滚事件，修正脚本后R2真实部署/新MCP initialize/list/targets0。不是原生热键失败，不曾发送任何全局按键。
- 下次重载后启动新的自有测试窗口，由RWI发送一次 `cmd+tab` 并独立读前台/原请求清理；不再要求用户手动切窗。合成系统切换与真人 HID/目标接管的验收分开，完整 A01–A07、F01–F16没有关闭。


### 2026-10-10 R4 — 最新整体验收专项，完整计划保留

- 新项目任务 `01a1250e-25ec-71f3-9e1d-2ea6e7ce177d` 真loaded41，typed Cmd+Tab实际从53515转回15516（延迟独立回读，非立即receipt）；parent40不冒充升级，不再等待重启完成这项。
- 自有定时foreground演员默认关闭、只显式分配窗口使用。19中down后3964ms接收者inactive→active/key/frontmost4672；5秒drag1/79/1和蓝线完成，terminal/released/detached/exit0；不是真人竞争输入证明。
- 21中1500ms JS deadline→精准native取消；实际1048ms/16moves+自身up，native terminal/released/无pending；partial输入保留uncertain/fence。显式恢复只在idle及独立两表0键0按钮后解除，input_sent=false，旧receipt字节语义完全相同、no replay；退出0。20未知nested便捷工具是派发前拒绝，已改用支持的native调用，不重放。
- 最新workspace685/0/11/42suites，example35，fmt/Clippy-all/fixturebuild0；同CLI BASELINE101/MODIFIED0/ROLLBACK101/REAPPLIED0、3原hash恢复。产品Agent/MCP/config未变，无新部署/权限/证书更换。
- 旧截图及Git历史的公开范围已获用户明确授权；尚未commit/push/public，完整A01–A07/F01–F16及资产许可、标准PR拆commit仍未封版。下一项优化nested capability错误和继续Live/窗口族矩阵。
- 当前原始证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/self-foreground`；详见 [整体验收阶段报告](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)。同一四角色继续扩展，不替换历史baseline。


### 2026-10-10 R5 — 真正document-modal缺口修正并部署

- Source-only命名诊断检查101→0；真实零设备MCP同命令101/0/101/0，抑制参数回显/限定128字符/保留native catalog gate；冻结副本include_str缺失的compile101保留并修正。新安装MCP实际输出已独立验证。
- 自有modal fixture默认关闭，typed nullable Block/public beginSheet/endSheet，native父关系/visible/key/CG ID与按钮counter独立。Source同CLI101/0/101/0、3原hash恢复，38example测试。
- Native23真实父计数误改1→新增确切AXSheet-child/AXParent关系范围检查；Native24部署后父blocked/no input/counter0，Sheet safe1/close1/parent restored，全部terminal/released/no replay/detached/exit0。仅此准确专项通过，不冒充远程OS面板/全部sibling/所有模态。
- 新Agent9721/69c18019，signed8d90970…；新MCPf958512…/fresh41，原证书/DR/CA/profile/config与四权限保持。Original inode排他lease+当前域unfenced及旧reserves真实terminal/released后交换；历史21uncertain回执保留，不靠改写/清fence部署。
- 最新full workspace686/0/11/42、example38、fmt/Clippy-all/releaseAgent+MCP0。只ROOT产品源编辑，85 first-managed baselines继续保留同四角色，未新commit/push/public。
- 官方SessionEnd并不是UI切任务通知；下一项真实MCP UI resource capability gate，再宿主渲染viewer接续，不能用假visibility或私有CUA绕过冒充完成。完整A01–A07/F01–F16仍未封版。
- 当前报告：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md`；Native24证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/modal-fixture/NATIVE24_ACCEPTANCE_RESULT.json`。


### 2026-10-11 R10 — Live真实关闭通过与真人切窗差异修正

- 真实已加载MCP Apps组件与builtin预览：用户切任务隐藏／回来恢复、活动Close在剩余361s时requested停止41帧、worker结束、清图；源仍running，后来timer进程0。不是browser标签可见假设／到期停止／原生task-ID认证。
- Native39 typed后台drag1/79/1完整蓝线、release/no replay/进程0；用户确认切到目标，但81回调仍ChatGPT前台。差异保留，真人目标接管不算通过。
- ROOT自有fixture新增128条有界环境变化历史及自身bundle／name／策略元数据，修正策略false no-op误拒；两个同输入101/0/101/0，原3hash恢复、最终角色重应用。Native40实际1留存；唯一名称RWI Receiver41.app已在虚拟4启动、before0/ackfalse/after0实际读回，真人gate待开始。
- 最终workspace697/0/11/42、example45、UI7、fmt/全targets/features Clippy-Dwarnings/releasefixture0。无Agent/MCP/签名/CA/权限变更，无新commit/push/public；完整A01–A07/F01–F16继续，标准PR #1沿用。
- [当前完整报告](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md)；最新checkpoint `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/NEXT_CONTINUATION_STATE.json`。


#### R10 Native41收尾：真实切入目标发生在拖拽前

- 环境历史确认用户协调的前台17800切换，早于实际typeddrag1922ms。前台1/79/1正确接收、释放／不重放，源timer0并detach；不是中途接管通过。旧39差异保留，不归咎于用户。
- 后续按原生down／active-path可见信号协调，不再以回复按钮两秒为工具派发时钟；Native42待启动，尚未创建。source51a frozen、唯一.app模板沿用最新成功41 command；全部A01–A07/F01–F16及标准PR发布门继续。


### 2026-10-11 R15 · 系统浮层与sheet自动验收
- own Receiver51测试／workspace702及fmt/clippy/build/UI7均通过；新默认关闭system_panel_controls与独立原生oracle。
- Loaded RWI将真实Color／Fonts从内建屏移到虚拟屏；covered Color真实切tab；浮层下父count1，sheet下父dispatch前拒绝，独立sheet count1/close1，source exit0/全部detach/no replay。
- Native父Live420x386、Color252x420；明确Resume同source后实际采集，Color requested stop完成。父仅expiry，不称UIClose。
- 新问题：暂时window_missing就永久pause；失焦panel的AppKit visible与on-screen不同；父Live未合成独立sheet；字体无名／视口外rows输出和walk成本。继续解决而非缩减完整计划；不再请求人工配合。


### 2026-10-11 R16 · Live暂时丢失的有界只读恢复（已部署与实机验收）
- Mode::Recovering、State.capture_failed/source_retry_ready/capture_succeeded：同source最多8次失败、1s退避；权限／身份／超时／display错误不自动恢复。手动Hide/Pause/Stop、TTL/presence/native pending/no replay保持。
- Source contract101/0/101/0，原3hash精确恢复；7gates0，workspace708/Receiver51/UI7。Agent26064/b4e398..，签名226c167..、DR/证书/MCP/CA/profile与4权限不变；加载工具直接验收，无Codex重启或人工问询。
- 实际Color34733 running→recovering/window_missing→running，frames90→92→96；调用方明确恢复own52 app前台，worker仅再采同source，loss后没有LiveResume、rebind、自动focus/unhide或输入重发。长期失去来源达到8次真实paused，不冒充任意隐藏窗采集。
- 前台变化让旧view失效，首次drag unknown_frame／native_execution=false；独立newview/newrequest52b才实际1/79/1、16BG/65FG、own actor1，原输入未重放。
- own50/51/52/53实际close exit0/全部6detach/12mutations terminal/released/no replay；Live52 requestedStop worker_done/清图/no pending。IAB5/6临时页已关；不是MCPApps/Host任务绑定完成。
- Font默认输出优先级继续处理；完整A01–A07/F01–F16与标准多commit/PR/publication尚未完成。


### 2026-10-11 R17–R18 · 默认摘要保留关闭控件、小型Live部署与自主验收
- `compact_controls` 折叠无名不可用容器，匿名按钮保留；完整控件页原序／坐标／动作／总数／覆盖率不变。真实MCP进程65项夹具默认1close、完整分页65。不是AX-walk加速或当前旧Host新格式验收。
- `panel_size` 最长边360；SF隐藏eye.slash、×关闭、可访问标签／tooltip明确预览、28点hit area保持。同一Source54 PID65752/CG34996/1320x1104：实际420x386→360x336，面积-25.39%，采集质量不降。
- Agent67302/ad37bc3a…签名5fe8b595…已部署，同证书/DR/CA/profile/四权限。当前加载RWI实测原生hide→APIresume→原生close：2条自身AXPress仍uncertain(-25200)，独立Live状态达标、no replay；源随后单独close进程0/4detach/3原请求terminal释放；IAB7/8关，用户原页保留。不是任务ID或前台恒定证明。
- 两source合约101/0/101/0，原hash精确恢复；相同DR `.app` 副本0/0/0/0。R18七门0/workspace716/Receiver51/UI7，fmt/Clippy-all-Dwarnings。新MCP69fa4e26…另行更新，initialize/list/targets旧新回滚重应用各0/41，不重启Agent或当前父MCP。
- 用户睡觉不可配合：不问重启/展开/切窗，用分配窗口与机器演员自主验证，不能声称真人输入、真实Host渲染、解锁／授权被替代。完整A01–A07/F01–F16及PR/push/public保留；不在此提交或封版。
- 最新实机证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/small-live-r18/SMALL_LIVE_ACCEPTANCE_R18.json`；四角色仍同目录原97 baseline。


### 2026-10-11 R19 · 自主FG/BG冲突、取消与恢复（不缩减全计划）
- own56实际5秒FG→BG：26FG/55BG，firstBG1642.49ms，sameCG35390完整1/79/1/no orphan。机器own57 timer一次；不冒充真人。系统CmdTab在drag之后真实own57→56，paired4/释放；没有中途键盘介入证明。
- 新唯一drag1500ms JS deadline：native1340ms/1/20/1，release-only短线，不补完整路径；旧uncertain/fenced/terminal保留。明确idle/两表0keys0buttons后只读恢复，旧结果完全相同/no replay、当前unfenced。
- stale前台tuple foreground_changed/nativefalse，own57仍前台；旧56也已到期，仍需liveA/B隔离case。56expiry0/57独立close0/2detach/5原请求terminal释放。
- 新问题：inactive own57 AXWindows -25204两次但同窗SCK成功，active AX正常；不推论unsupported/destroyed、不自动focus，不复发输入。开始改进error保留+readonly visual degradation及真实MCP过程门。
- 产品源代码／Agent5fe8／MCP69fa／签名／权限未变，旧716gates不是本轮新跑。三managed docs和同四角色更新；PR/public/全scope未封版。证据 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/foreground-conflict-r19/FOREGROUND_CONFLICT_ACCEPTANCE_R19.json`。


### 2026-10-11 R30 · 精确 AX 读取恢复与无需人工的实机复验
- 单一产品根上修改 `NativeMac::resolve_window_element_traced`，`coverage.window_resolution_recovery` 保留原 AXWindows/-25204；仅 type/PID/CGWindowID/role/rect 全部符合时接受一条 CFEqual 去重的真实窗口别名。原成功列表路径、模态、输入、租约、让步和 no-replay 不变，不自动聚焦或换窗。
- R30 八门全0；provider变体后当前workspace723/0/11/42，Receiver53、全targets/features Clippy0。源合约1/0/1/0；四原fixture源精确恢复、新helper absent后重新应用；同DR完整 `.app` 副本签名0/0/0/0。首次在bundle外验证单文件返回invalid Info.plist，改用完整独立bundle，不归因于生产证书/权限。
- 已部署同证书Agent13847/96e02335…/签名36686a66…；当前loaded MCP已自动重连新实例，四权限true、不请求授权，CA/profile/MCP4a29未动。
- 专用虚拟屏68/69：真实120ms自身AXWindows getter被系统调用42次，真实读取-25204后精确AXMainWindow恢复；69后台safe0→1、drag1/14/1，16条mouse回调均AppInactive/KeyFalse/SystemBackground，蓝线像素确认，无orphan。真实owned sheet读取仍candidate0，原错误/无authority保留，parent输入在dispatch前拒绝。CUA只关闭自己的sheet作独立对照，不作为RWI通过。
- 不再需要人工：RWI明确focus69原生前台与receiver回读一致，随后全局cmd+tab实际69→68；4个平衡事件/paired release。receipt内瞬时after仍69，独立targets/两receiver已68，不将回执替代goal。
- Live当前实际360x336点/720x672像素，源1320x1104不降质，圆形28点eye.slash/×与可访问label/tooltip。真实兼容页面和API hide/resume均回读清图/恢复；MCPApps tabs=[]，不虚称Host任务切换通过。CUA浮层按钮模拟点击未观察到动作且无callback entry证明，待继续定位。浏览器关闭前已duration_expired，所以不归因关闭；terminal resume保持stopped。
- 8条原请求terminal/released/no pending/no replay，4detach/pointer隐藏；68显式AX close0非到期，69关闭控件在不完整RWI及CUA AX树均未列出，未猜点击，600s自己到期close0（expiry1）。以上差异保留，不宣称两源都显式关闭。
- 原97 baseline条目不变，当前139条延展同四角色。完整A01–A07/F01–F16、架构12债务与标准multi-commit/PR/push/public保持未完成；下一步精确AXFocusedUIElement→AXTopLevelUIElement读取支持sheet，并继续native按钮/Host与并发键盘验收。


### 2026-10-11 R31 · 精确sheet顶层恢复与原生Live按钮实测
- `resolve_window_element_traced` 在原 AXWindows/-25204 只读失败路径补 `AXFocusedUIElement→AXTopLevelUIElement`；focused provider必须真实AX type/同PID，最终对象仍严格type/PID/CGWindowID/role/rect，只有旧两alias无匹配才读取；不读focused值/文本、不把AXWindow当sheet、不焦点切换、不重放输入。
- 八门全0/workspace724/0/11/42、Receiver53/UI7、全targets/features Clippy0。源合约1/0/1/0，恢复原2hash后重应用；完整独立.app同DR签名0/0/0/0。已部署Agent57562/4be7b9d3…/签名ff542ac1…，loaded工具自动重连，四权限true、CA/profile/MCP4a29/证书未动。
- own70/CG37665 与独立真实sheetCG37670：120ms自身provider getter实际29次；真实AXWindows-25204后4属性读取恢复精确sheet720x300，子safe0→1/close1、parent0/前dispatch阻断。所有记录的NSButton回调inactive/system-background；KeyFalse仅最终独立parent/sheet回读，不能外推每callback。
- 原生Live自身按键使用loaded RWI AXPress各一次：Hide后确实hidden/clear，APIresume后同360x336恢复，Close后requested/stopped/worker_done/clear/no pending。两次selfAX仍返回-25200/uncertain，原回执保留；独立goal已观测，不伪称ACK、绝不重发。旧CUA模拟无效果不能当原生按钮故障的证据；模型通常应使用computer.live控制自己的预览。
- 新MCPApps tabs=[]；public navigate ack=true仍非实际可见任务证明，跨参考任务后预览仍visible，不能说task绑定通过，也不尝试绕过Codex原生访问限制。只创建/关闭自己的IAB15，不读取参考任务内容或操作游戏/文档。
- source70显式AX关闭exit0/expiry0，7原请求terminal/released/no pending/no uncertainty，3detach/pointer隐藏。原97baseline不变、139同四角色延展；完整A01–A07/F01–F16、架构12债务、PR/push/public未完成。
- 下一项回归原计划F12：真实分配71确认parent/owned sheet多层画面和精确关系，补齐layered Live；F13原生跨窗DnD、F14浏览器、其他范围均保留，不把一个alias修复当封版。


### 2026-10-11 R47 · 无需人工的执行中系统切窗与小型 Live 复验

- 唯一产品源仍为本仓库。本轮只修改 Rust 接收靶场的 `Options.command_tab` / `State.command_tab` 与默认关闭的 `command_tab::native::Actor::post_once`；不是新增生产后台桥。两个显式分配的虚拟屏靶场，由 loaded RWI 建立精确窗绑定及前台顺序；第一次真实后台 canvas down 触发一次原生系统 Command+Tab，拒绝/部分输入不重试。
- 保留 R46 的启动顺序/CG 发布延迟及慢 AX provider 失败；不把首个 PID 窗口误当主窗。按 UID、kernel birth、bundle、exact CG ID/geometry/title/layer 绑定；running 文件不等于 WindowServer 已发布，任何按键之前必须确认真实 peer surface。
- R47/102 暴露靶场自身 guard 错误：先误挡 combined-only 残留 key12，再把自己发出的 Command 误作 HID 外部键；真实只发 Command-down/up（2 调用），没有切窗。改成与生产 `native/hotkey.rs` 相同的分段：首次 down 前 held-resource preflight，之后仍逐 down 检查权限/foreground/peer/最近 HID；不把全局表当持续人类来源证明，不取消真实让步。失败的蓝线不重放，104 是新分配/新请求。
- R47/104 实机：固定系统 chord 4 调用及配对释放；同 CG40524 的5秒拖拽 1 down/78 dragged/1 up，18后台/62前台 callback，首次前台接收在 down 后1211.197ms，实际手势4970.729ms，completed1/orphan0。原生 NSWorkspace/接收记录独立确认30164→33600；receipt里的瞬时 after 仍是30164，未拿它替代成功。是机器系统切换，不是真人抢占证明。
- 当前已部署 Live 实际320×303点／截图640×606，源高清1320×1104未下降；原生圆形隐藏eye.slash、关闭xmark，AX名称明确，无经典三色窗口按钮。实际 Hide→清图/移除，显式Resume→恢复，活动Close→requested stop/worker done/清图，关闭时源仍running。两次自身AXPress仍返回-25200/uncertain，独立状态确认效果但不篡改回执、不重发。
- 兼容 viewer 先真实 queued，再通过公开任务导航显示，原生采集29帧/正确内建屏后才称可见；没有伪造 presence。它仍是实际 browser-panel visibility，不是正式 MCP Apps/Host task-ID 绑定。官方文档未给 PiP 精确尺寸，未声称1:1复制其私有界面。
- 102/103/104均真实 named-close、lifetime_close_requests0、进程exit0；11原请求全部terminal/released/no pending/no replay，两条AX effect uncertain如实保留。12个本轮及过期自有 binding detach；只读恢复观测0 keys/0 buttons/flags0、stale session key12保留，input_sent=false，无全局自动释放。
- 同一 CLI/input：BASELINE1（没有--check-only）、MODIFIED0（配置合法/noGUI/noinput/native binding false）、独立 binary copy执行ROLLBACK后1且SHA等于原始、重应用0。不能把纯配置检查当native成功。完整四角色继续沿用原150路径与原97基线；R47实际11门全部0：workspace736/0/11/42、Receiver63、UI7；fmt/全targets/features Clippy-Dwarnings、release Agent/MCP 与架构门通过（旧12债务仍保留），不借旧R45绿灯。
- 正式 Host 任务绑定、真人竞争输入、完整 F01–F16/其它应用覆盖仍未完成；小浮层是已有部署的实机复核，不是本轮新部署。证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/command-tab-native-r46`；截图按真实顺序保存在 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-native-r47`。用户休息期间不请求展开/切窗/重启/权限；不能自动代替管理员/系统授权。


### 2026-10-11 R48 · Live 原生 AX 回执修复、无标签页组件与新会话接力
- 本轮只在预览自己的 NSButton 子类实现公开 `accessibilityPerformPress`：enabled/明确target/已配置toggle或stop检查后，一次 `NSControl.sendAction:to:`，返回真实触发BOOL。失败false、不重试、不发键鼠、不改外部应用AX执行器；ACK仍不等于Live效果。
- 已部署Agent64062/2ead7588-7da3-44b4-b856-04df7f796bfb。复用同证书/DR，config/profile/CA/MCP哈希与四权限保持；排他原inode lease及全部当前durable终态门通过才交换APP。无新授权/密钥替换/Host重启。
- 自有105/CG40636位于虚拟4，预览实测320×303点/640×606：loaded RWI原生Hide及Close各一次，两个AX回执均returned；独立回读hidden/clear→APIresume/running→stopped/requested/worker_done/clear，源仍存在。旧R47两个-25200/uncertain结果不升级、不重放。
- 无任何IAB标签页时，`show_live`的旧v1会话组件已真采集并显示，live3同binding fffe4aca…：65帧/可见/有图→切另一任务76帧/隐藏/清图/暂停presence。mcpapps tabs=[]仅意味着没有侧栏tab，不能等同未内联渲染。返回第一次公开导航MessagePort错误保留，第二次导航ACK但76帧/仍隐藏，**完整隐藏/返回恢复仍未通过**，不拿导航ACK充数。
- 105后来原生lifetime到期closed/exit0（expiry1），不称显式关闭；业务safe0/drag0。两个原生按钮原请求terminal/released/no pending/no replay，两绑定退役；formal live3最终duration_expired，不把晚到Stop当活动Close。兼容测试页23已关闭。
- 当前源码11门均0：workspace737/0/11/42、Receiver63、UI7、fmt/全targets/features Clippy-Dwarnings/release Agent+MCP/架构；新helper纯单测1，不冒充所有App兼容性。匹配source配置probe1/0/1/0，独立副本执行rollback恢复两原hash/新module absent，再重应用。原97baseline保持，四角色扩展151。
- 上轮00f58ca真实Mac+Windows CI已success；本轮源码须单独提交与验证。依据用户已允许插件旧会话不可用时新会话接力，创建同项目「Real World Interaction：Live 新会话验收」01a12a1a-5f77-7153-b9e8-609f2c788d2a，只验新MCP v2组件、不改源码/设置、不操作用户应用，不要求人工配合。原A01–A07/F01–F16目标不缩减。
- 原生证据 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-button-accessibility-r48/NATIVE_RESULT_R48.json`；公开API依据：https://developer.apple.com/documentation/appkit/nsaccessibilityprotocol/accessibilityperformpress() （返回动作触发结果而非业务成功），https://developer.apple.com/documentation/appkit/nscontrol/sendaction(_:to:) 。


### 2026-10-11 R49 · 新Host Live v2闭环通过与真实WindowsCI失败修复
- 新会话Receiver106真加载Live v2，无IAB代替：实际内建屏预览截图320×303，切离131/131帧停采集/隐藏/清图，返回同live4/binding251c7283…恢复139帧与真实截图；原生Close returned，166/166帧 stopped/requested/worker_done/clear，源仍running。之后源namedClose/expiry0/exit0、2绑定detached。父会话已逐项回读原始工具ledger和所有manifest hashes，不单凭另一模型的all_passed。当前任务旧v1返回缺口仍保留；v2实际用户需求闭环已证实，不要求Host task-ID加密证明来替代行为验收。
- `a197d30`远程WindowsCI真实失败：`pending_native_ownership_survives_quota_cancel_and_context_close`把Instant::now往回减301秒，在年轻的Windows时钟epoch上panic overflow。保留完整原日志与watch exit1；不是推论、不是Live变更导致的native失败，亦不借旧00f58ca CI绿灯。
- 修正Execution::retirable_at/Runtime::resolve_execution_at，用私有显式query时钟做确定性测试；生产入口仍真实Instant::now，TTL常量/owner/session/principal/ref/cleanup/fence/no-replay都保持。测试改为完成时刻向前推进，验证截止前不退休、截止时stale，并额外确认未来query clock也不能退休pending native资源；不跳过、减弱断言或等待5分钟。
- 首个focused cargo调用没有fixture feature，因此实际0 tests，不能算通过；纠正命令加--features fixture并保留原结果。R49匹配source配置probe1/0/1/0，独立副本ROLLBACK原hash完全恢复后重应用，非Windows原生证明；真实新头Windows job仍须重跑验收。
- R49实际11项本地门禁均0，workspace737/0/11/42、Receiver63、UI7与strict Clippy通过；focused fixture case实际1 passed。结果见 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/execution-retention-clock-r49/final-gates/GATES_EVIDENCE_AGGREGATE.json` 为准，不宣称旧737覆盖新源。完整A01–A07/F01–F16仍保留；新Host专项见 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-mcpapps-v2-receiver106-2026-10-11/SUMMARY.md`。


## 2026-10-11 · R50：只读截图有界排队，复合预览实机闭环

- `Context::read_waiters` / `Runtime::dispatch` / `read_admission::acquire`：明确仅 `computer.observe`、`computer.capture_visual`、`computer.image` 可在同一上下文内等待；队列最多 8 项、等待最多 5 秒。执行器仍串行；输入、绑定、恢复等保持立即拒绝，不排队、不重放。取消、关闭和丢弃 future 释放队列名额。
- 实际加载 MCP、同规格专用虚拟屏窗口：旧版 8 次并发 snapshot 成功 1 次、7 次 context_busy；首个候选虽修复 observe，但预览 image 仍忙，依旧 1/8，失败证据保留。补齐 image 后最终 8/8 同时返回画面和 7 个控件，整批 2452ms。不是把快速报错当“性能好”，也不声称并发执行原生 API。
- 探索性 n=8：串行 snapshot512 中位 247.5ms；缓存 controls 中位 7ms；纯视觉 capture_visual512 中位 98.5ms。均为 Host/tool wall time，非原生 CPU；纯视觉不提供语义控件或输入帧，不能与 snapshot 等价比较；不足以估计稳定尾延迟或通用应用性能。
- 7 个 admission 测试包括 Runtime 的 8 路 observe→image 复合链、独立输入许可未使用、终态和资源回收。当前完整 11 门禁 exit0：workspace 744 passed / 0 failed / 11 ignored / 42 suites；Receiver 63、Live UI 7；fmt、严格 Clippy、发布构建及架构棘轮通过。旧字段误接编译101、错误 finish 前验收断言101、无PNG测试 fixture 参数101均保留并修正，不记作通过。
- 两轮 Agent 用既有签名身份原子部署；最终 Agent PID46886 / instance70635301-549b-4a30-b6ad-012de8a8d4f0。MCP、配对 CA/profile、签名证书/DR、四项权限不变，不要求用户授权或重启。Receiver108/109 均由实际 AXClose 关闭、进程0、expiry0、已解绑；不操作用户游戏或文档。
- Live 已有真实 Host v2 验收：320×303pt、无经典标题栏三按钮，隐藏 eye.slash／恢复 play.fill／关闭 xmark，原生 tooltip 和可访问动作名。当前复核 Apple Buttons 主源（https://developer.apple.com/design/human-interface-guidelines/buttons）；不是私有 Codex 资产克隆，也不把当前旧缓存 v1 的失败改写为成功。
- 本轮源文件配置验收 BASELINE false/exit1 → MODIFIED true/exit0 → separate-copy ROLLBACK false/exit1 → REAPPLIED true/exit0；精确恢复原始哈希/新文件不存在。不是生产服务回滚后的性能验收。四角色继续沿用原事务。
- 证据：`work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/native-readonly-pressure-r50/`。完整 F01–F16 仍保留；hover tracking/tooltips、隐藏/最小化等窗口状态及跨应用 DnD 等未完成项不冒充验收。用户休息期间不再请求配合，只用已分配 own fixture 自主推进。

R50 封版补查：隔离 `cargo test -p rwi-agent --no-default-features --lib read_admission` 实际发现 fixture 测试未加 feature 条件（exit101），已为仅测试的 RasterFixture/复合测试补齐 cfg；同命令现 6 passed。最终重跑全部11门禁仍 744/0/11/42；发布 Agent、MCP 和 Receiver 三个二进制哈希与已实机验收版本完全相同，故不重复部署或重放输入。


## 2026-10-11 · R51：补齐当前验收事实，保留悬停／系统共享菜单缺口

- HEAD `3e1998b` 的实际远程 CI `38129247364`：Mac、Windows 两个 job 均 success，独立 gh run view 与原 watch exit0 已核对。不是把排队/进行中视为通过。
- 专用 Receiver110 在虚拟屏4、默认 first responder 是按钮，应用 active/key/frontmost；一次真实 typed RWI hover 返回，原生 App/Window 均收到精确 own-window MouseMoved（点落 hover zone），但视图 move、enter、exit、tooltip 全0。未开启 cooperative responder 校准，不重放输入；前台并不因此自动获得 tooltip。这是待修复/适配证据，不是“不可能”的结论。
- CUA 的当前公开 native App 接口不含 hover，所以未执行或声称 CUA hover 对照。只读 getApp 开始观察后，系统 Window sharing 替换了三颗标题栏按钮；CUA 可展开菜单看到 Close Window，RWI 当前 selected-window snapshot 未列出菜单项。只读 Rust probe 观察到 AXMenu/-25205、AXChildren/-25212、AXWindow/-25212；不能把属性读取失败当菜单不存在，菜单消失原因也未证实。
- cleanup 使用 CUA 共享菜单 Close Window 一次；随后 getAXState 返回 noWindowsAvailable/-10005 的原始错误保留。独立 fixture state=closed、expiry0、进程exit0、RWI detach=true、原 hover 操作 terminal/resources_released；并非 RWI 的菜单关闭验收。首次 cleanup 脚本在缺失 Close window 控件时 TypeError，发生于输入前，保留而不重跑原 hover。
- R47 Cmd+Tab 原始日志复核：首down1791705773463，系统 chord1791705774483，独立环境采样1791705774739变成目标前台，动作仍1/78/1。actor 的即时 after 值仍是旧PID，不覆盖随后真实采样；保留完整原记录，不重演已完成输入。
- 修正文档里过期的“正式 Host MCP Apps 未验收”：R48 fresh Host106 真正 v2 渲染、away停采清图、return恢复同绑定、Close停止且source存活均已有独立证据。F12 的 foreign-PID/system-menu 缺口仍在，不能混为一谈。整个 F01–F16 计划继续，未标全量完成。
- 产品 Rust 代码及安装 Agent/MCP 本轮未修改；本轮只更新事实账本，不再次提交或触发部署。


## 2026-10-11 · R52：显式前台悬停闭环，默认后台路径不降级

- 新增 `MovePointerOptions::ForegroundSystem`；仅调用者明确提供 `move_pointer.value.delivery=foreground_system` 与精确 `expected_foreground` 时，使用一次 CGEventPost(HID)。仍要求当前选定同窗口/PID/出生身份、原生 foreground、既有 post/listen 权限、HID 空闲、共享输入 lease、原生引用及持久化不重放；不自动激活、不把后台失败改成全局输入。默认 `background_window` 完全保留。
- 用同一 Receiver112、同一视图点512/280、同一 foreground tuple 与 options 对照：旧 Agent `invalid_input_value` / before-dispatch / hover0；新 Agent enter1、move1、tooltip callback1，真实绿色高亮，默认 first responder 仍是按钮、无 receiver 校准和鼠标按键。生产 Agent 已用同证书/DR 原子部署，PID15620 / instance05e73ec4-2f49-4524-a926-e6fbcf2fe816；MCP、配对、四项权限均保持，无 Host 重启或人工配合。
- 首轮构建 ENOSPC exit101 原样保留。只清理本项目可重建 incremental/debug dependency 缓存，未清理别的项目、源码、安装程序或验收证据。按原11命令恢复后全部0：workspace750/0/11/42，Receiver63、Live UI7、fmt/严格全targets/features Clippy/发布构建/架构门通过。
- 运行中再次 ENOSPC：一次 foreground exit 在 durable reservation 前拒绝，无原生输入；原操作已终态/释放，不重放。Receiver112 实际关闭/expiry0/解绑，但输出曾失败，进程 exit1，不改写成完整清洁验收。原默认 addressed reenter 因空白像素整帧变化 stale_reference 拒绝；随后真实光标自然重入不能算该请求成功。
- 独立 Receiver113 清洁复验：先显式移动到自有按钮外区，再进入、区内移动、退出；相对外区基线新增 enter1/move1/exit1/tooltip1，光标原生回读对应所请求点，down/up/safe actions均0，无校准。实际同 PID tooltip 窗口曾被控制器按 PID 误选，TypeError 在输入前；已保留、退役，改用精确 main CG41271。错误 tooltip-window foreground tuple 真被拒绝 `foreground_scope_mismatch`；最后 AXClose、expiry0、进程exit0、全部原请求terminal/released/no-replay、两绑定均退役。
- Live 既有小预览320×303pt和原生 eye.slash/play.fill/xmark按钮、tooltip及可访问名称继续沿用；官方 Computer Use 文档只说明 picture-in-picture，不提供精确像素规格，不声称私有 UI 1:1 克隆。正式 v2 隐藏/返回/关闭已有 R48 独立 Host 证据，不重复请求用户配合。
- 本轮是显式前台能力，不能代替尚未完成的后台 cold-start tracking、系统共享菜单、隐藏/最小化、跨应用 DnD 等 F01–F16 项；全量目标保持未完成。证据与真实时间顺序截图见 `outputs/validation/r52-foreground-hover-2026-10-11/REPORT.md`。


## 2026-10-11 · R53：显式后台能力隔离、发送侧时钟修正与未完成验收

- 真实新回归先4pass/1fail、exit101：`BackgroundWindow` 显式选项无条件通过未声明路由的后端。`MovePointerOptions::require_capability` 现要求 pointer_move implemented 与 default_delivery=background_window；Engine 只为显式 value 做新增检查，bare/default路径不变。6项 focused0，fixture证明显式拒绝后backend calls0、bare move calls1；已拆分 commit a2b0030。
- 纯 Rust/公开 CGEvent getter 观测：NSEvent move/enter/exit 转换 CGEvent 后，字段58=1e9 与timestamp相同；写58=1把timestamp改成1。旧 `PRIVATE_WINDOW_ROUTING_FIELD` 命名错误，现移除该写入，PID/window51/坐标/标志仍保留。匹配确定性测试原123456789→1失败101，修正后原timestamp保留/exit0；旧“新event应自带当前时间”假设101（实际0）和漏清旧常量编译101均保留。旧 App/Window 原生接收记录已是系统更新后的正常时间，所以**未证明这是hover0的原因**。
- 当前复合源码11门全0，workspace753/0/11/42、Receiver63、Live UI7、严格fmt/Clippy/发布构建/架构；中间752是仅能力修正版本，不混用。同证书/DR原子部署 Agent4091 / instance67e9b9d1-8935-4a5f-94f1-aa24a1c7a729，配对、MCP、四权限不变，不要求重启或配合。
- 真实 loaded MCP 在自有116（虚拟4）系统后台接受显式 background_window；App/Window 收到精确MouseMoved、hit hover_zone，但view/enter/tooltip仍0、无校准/按钮，不升级成全后台hover完成。随后拖拽两次都在snapshot阶段 AXWindows/-25204 停止，未调用 drag、不重放。capture_visual 能返回真实图片；CUA对同自有116只读也timeoutReached/-10005，因此不武断认定只属于RWI。采样main stack在environment_sampled→flush_sampled→NSString/CFString创建；只是一秒样本，不足以认定死锁或锁屏。PID1588实际是iStat Menus而非loginwindow。
- 114–117均实际lifetime到期、closed、进程exit0；不是显式RWI Close验收。相关source绑定已退役、唯一实际BG move原请求terminal/released/no replay。source配置BASELINE false1/MODIFIED true0/独立copy ROLLBACK false1精确4hash恢复/REAPPLIED true0，原97 baseline与同四角色保持。
- 前台R52、实际v2 Live和R47执行中切窗结果保留，不复演。完整F01–F16仍未完成；下一步继续隔离环境采样与AX消息可用性，并处理真正后台tracking、系统菜单等。


## 2026-10-11 · R54：AX只读失败仍保留画面，不授予输入

- `ax_unavailable_for_visual` 仅为精确AXWindows只读CannotComplete(-25204)、unsupported/no-value或完整无歧义AX搜索放行独立CG/SCK画面；原三次settle保留，权限、身份、其它属性、action/replay错误仍拒绝。`visual_only_consistency` 保留原Fault及三次attempts，不将timeout解释成AX不存在；binding、mapping、controls、mutation_ready均不生成。
- 新focused基线assert失败101，首次候选u8/usize编译101保留，修正后同命令1pass/0。当前全部11门exit0，workspace755/0/11/42、Receiver63/UI7、fmt/全targets/features Clippy-Dwarnings、release及架构门通过。匹配source配置1/0/1/0，独立copy执行rollback恢复精确四hash再重应用；不是生产回滚或后台业务成功。
- 同证书/DR原子部署Agent46538 / a05ac614-8360-4dc5-8a59-e44c7d3a40ea；排他原inode lease与当前durable终态门先通过，CA/profile/MCP/四项权限未变，没有Host重启或人工请求。
- loaded MCP实际own119及独立second cover121在真实AXWindows25204/三次attempts后返回各自原生画面；pixel/window resolve两次都target_binding_unknown/native_execution=false。own120正常alias路径在部署前后均7controls，不能说是本轮修好了AX provider；后续fresh own120又变visual-only，drag未调用，不重放。
- 去掉environment sampling的own118仍25204，因此它不是充分根因解释；一秒采样255/no-longer-running原结果保留，不能认定deadlock或锁屏。own118–121均到期closed/expiry1/进程exit0而非显式Close，safe/drag输入全0；独立PID检查均不再运行，所有新binding已detached。
- 真实顺序图片保存在 `outputs/validation/r54-readonly-ax-2026-10-11/`；03是原1320×1104，不将显示缩放当源分辨率。Live既有320×303pt、eye.slash/play.fill/xmark、真实v2任务隐藏/恢复/关闭继续保留，不复演已完成操作。
- Apple的CannotComplete说明是消息失败/未响应，不能当AX缺失（https://developer.apple.com/documentation/applicationservices/1462091-axuielementperformaction）；API timeout可按对象设定（https://developer.apple.com/documentation/applicationservices/1459345-axuielementsetmessagingtimeout）。本轮只重试read、不重发action。
- 完整F01–F16仍未完成，尤其真正后台tracking/tooltips、系统共享菜单、隐藏/最小化、跨应用DnD等；用户休息期间不再请求切窗、展开、密码或权限。唯一源根及同四角色不变；基线从155扩为156，新增此前未纳入的native visual源，原97全部不变。首次重构副本漏文件的验收失败1保留，修正后重跑。


## 2026-10-11 · R55：自然冷启动后台拖拽实测，悬停缺口分开保留

- 按R54 checkpoint真实启动own122（PID64052/CG42266/虚拟4），regular app、无environment sampler、默认first responder为NSButton、无cooperative校准。初次AX在第二次read恢复，返回7控件/正向输入binding，未把只读图当输入。
- loaded RWI一次显式background hover：App/Window都收到type5、正确own window/hover zone；view/move/enter/exit/tooltip仍0。一次不同的新typed drag：实际NSView接收1down/16dragged/1up、completed1/orphan0，1320×1104实际蓝线可见；18个回调采样均app inactive/window nonkey/frontmostPID35390。不是连续前台/硬件来源证明，不外推所有应用或拖放DnD。
- 原hover与drag durable请求均terminal/resources_released/no pending/no uncertainty/replay=false；volatile execution refs在保留期后expired的原错误不改写，按明确request重新只读查询，不重放动作。第一次release错误字段target被schema拒绝；纠正device_id时lease已过期unknown_lease，不假称显式release成功，也不是原生资源未清理。
- 122后来真实lifetime到期closed/expiry1/进程exit0、binding detached；Close尝试停在process_identity_unknown，未调用点击，不称显式Close验收。
- 本轮只更新事实，不改Rust、不部署、不为了每轮对话触发提交/Action；R54同一源码的755/0/11/42门禁及精确8d3f705 Mac/Windows CI证据保留，不能说是新性能/全量兼容性结论。原四角色与156原字节基线延展，完整F01–F16继续未完成。
- 再复核已有50仓库/150正文研究及Apple activeAlways公开契约：允许inactive tracking并不代表一条外部PostToPid必然生成所有tracking/tooltip；不伪造第三方tracking指针、不偷偷激活/warp来充当后台成功。https://developer.apple.com/documentation/appkit/nstrackingarea/options-swift.struct/activealways ；Hammerspoon app post实用CGEventPostToPSN，非本进程sendEvent的跨进程接口：https://github.com/Hammerspoon/hammerspoon/blob/master/extensions/eventtap/libeventtap_event.m 。


## 2026-10-11 · R56：sheet隔离通过，区分AppKit可见与系统面板呈现

- 存储映射后继续原唯一源路径 `/Users/vincentadamnemessis/AI Projects/Real World Interaction`，实际卷 `/Volumes/Codex-RWI/Real World Interaction`；当前shell显式workdir，不进入旧cwd壳。只读复验旧123已到期closed/exit0；不重放原sheet/颜色动作。只比较当前156受管源，未把另一会话的282951全树校验报告当成本任务自行执行。
- 原123 sheet：parentCG42287/input blocked，parent Safe动作checked_context_unavailable/native_dispatch=false/计数0；独立sheetCG42292安全按钮计数1、Close后visiblefalse/attachedfalse。首次typed click漏at.kind是schema拒绝/计数0，纠正参数和新request后执行成功，不是原生不可用。
- 为自有fixture的SystemPanelOracle补只读hides_on_deactivate/on_active_space/occlusion_state/quartz_description；通过公开CGWindowListCreateDescriptionFromArray只查一枚pointer-sized windowID，再核对自身PID/id，Create/Get由CF RAII释放。无全桌面枚举、图像、输入授权、panel策略修改。未读取保持null；CG可选onscreen字段缺失仍null，不能改成false。focused基线缺诊断101，同命令修改后1pass0；完整11门0/workspace755/0/11/42、Receiver64/UI7。
- 新124：Colors与Fonts分别真实CG42817/42831，初色彩窗在内建屏且不完全落单屏（0,771,266,454/displaynull），既有place_window已能原生移到虚拟4且两次settle确认；不为一个并不存在的placement错误改后端。各自原生画面与控件可读。
- getter明确两panel hidesOnDeactivate=true。新peer125启动时已自然成为前台，随后RWI focus125是already_foreground/native_requests0，不能称工具造成该切换；source124 inactive时AppKit isVisible仍true，但两panel退出on-screen catalog，exact-CG描述仍同owner、onscreen可选键缺失。一次明确的RWI focus124经public AXRaise/AXFrontmost使54126→48954，独立记录同panelID/位置及catalog重新出现/onscreentrue。符合AppKit hide policy；不是旧123所有window_missing原因的普遍证明，也不是后台hidden panel输入验收。
- 清理时两panel再次window_missing，未点不存在的Close；只对fresh parent/peer执行各一次AXClose，124/125 closed/expiry0/进程0，retained panels isVisiblefalse。123是到期expiry1而非显式Close。13原durable请求全部terminal/released/no pending/no uncertainty/no replay，所有6相关binding退役。
- 生产Agent/MCP/四项权限不改、不需要Host重启。源配置BASELINE1/MODIFIED0/独立copy ROLLBACK1精确3hash恢复/REAPPLIED0，原四角色继续扩展；不将source配置试验当生产回滚效果。完整F01–F16仍未完成，background hover、foreign-provider menu、hidden/minimized输入、跨应用DnD等保留。
- 主源：Apple hidesOnDeactivate（https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate）与Quartz描述指定IDs（https://developer.apple.com/documentation/coregraphics/quartz-window-services）；正式报告/原图在 `outputs/validation/r56-layered-panels-2026-10-11/`。


## 2026-10-12 · R57：精确缺窗诊断部署及同源原生对照

- NativeMac::window_record / visual_window仅在精确on-screen lookup miss后，以public CGWindowListCreateDescriptionFromArray查询一枚pointer-sized ID，核对PID/id；CF Create由RAII释放。window_record_probe::missing保持window_missing，同时报告native_record_present/owner_matches/native_onscreen；hidden/destroyed未分类保持null。不返回Window/图像/frame/reference，不激活/unhide/fallback/replay；AX visual-only策略不因此扩大。
- 新wiring测试真实基线101→修改0，四projection测试0。首次Clippy len_zero101保留，is_empty修正后当前完整11门exit0，workspace760/0/11/42、Receiver64/UI7、fmt和全targets/features Clippy-Dwarnings、两个release和架构门均通过。
- 使用既有成功控制器模板，gate路径纠正为final-gates-current；持有原inode排他lease，旧instance全部durable资源终态后原子交换同签名Agent。新PID34800 / instance84129a56-ab4a-4672-91dc-0b18ef867255；installed SHA256 abc4f24b8bea2bbbb656cd5b8a5f2107383ffba02f43b28bd475832aa07e0cb1。稳定证书/DR、MCP5cf8424、CA/profile/config及四权限不变，无Host重启/人工请求。
- 自有126/CG43051与Colors CG43059在Agent交换前后同一原生实例：旧snapshot/capture_visual只报window_missing；新两路径仍错误，无新pixels/输入binding，但准确读出same-owner native_record_present=true、native_onscreen=null。fixture getter isVisible=true/hidesOnDeactivate=true、inactive，说明不可把该getter或catalog absence当呈现/销毁证明。显式focus与独立rediscovery是单独的新动作；不隐藏focus后再次退到后台导致的新miss。
- 原Colors AXPress/-25204仍uncertain，早期计数0、晚期计数1；没有重放。七原请求全部terminal/released/no pending/no uncertainty/no replay。126与127各fresh AXClose一次，closed/expiry0/进程exit0，six bindings detached；没有点击隐藏panel的Close。顺序原图、hash和结果在outputs/validation/r57-window-missing-context-2026-10-12/。
- 同输入source配置BASELINE false1→MODIFIED true0→独立copy ROLLBACK false1/三个hash+module absence精确恢复→REAPPLIED true0。沿用同四角色从156扩为158，原97immutable基线不改；不称生产回滚。Live已部署320pt紧凑比例与eye.slash/play.fill/xmark，R48真实v2显隐/返回/close证据保留；官方页面未给私有UI精确像素规格。
- 完整F01–F16未完成；继续background cold tracking、window-local/offscreen、foreign-provider菜单、隐藏/最小化、跨应用DnD等。用户休息，不请求人工配合；不以本次诊断补齐代替后续原生实现。


## 2026-10-12 · R58：独立只读窗口画面不再误要求完整单屏

- 用上一轮checkpoint实际启动严格window-display边界合同（1pass/0），不复演已完成GUI。新增record_for_visual的窗口局部合同真实1pass/1fail/101，保留旧strict行为；修正为可选containing_display后同命令2pass/0。只改独立visual record，保持on-screen精确PID/ID、session/process birth、重复行、SCK owner/frame/contentRect/scale、pre/post拓扑/几何检查；无display时仍null，不赋予输入或假称hidden可拍。
- 完整11门exit0，workspace762/0/11/42、Receiver66/UI7，fmt/全targets/features Clippy-Dwarnings、release/架构门通过。新增fixture --boundary-right只在显式allocation启用，3/4宽在所选screen、1/4跨出，精确frame与screen回读，不回主屏；默认完整单屏、重复/混合模式拒绝保留。
- 同证书/DR原子部署Agent68669 / 4a4e2cf7-a45c-4996-8f79-5f8f2a40a22a，signed SHA256 f55c53358783bcfb29858d0f25e219db9d6ada9aa273ef5c522014a076a37b10；原inode排他lease与durable终态资源门先过。四权限、MCP5cf8424、CA/profile/config不变；没有Host重启、权限/人工请求。
- own128（PID68206/CG43156）前后同一个process_start/CGID/位置：[3225,504,660,552]，Virtual4右缘3720，catalog.display=null。旧独立capture_visual报visual_display_unknown，新返回真实完整1320×1104/122552bytePNG；普通snapshot在旧版已可读7controls，新版仍可读，但两者mutation_ready=false、input_reference_available=false、original_to_desktop=null。不是修好了已正常的snapshot，不把响应图的visual_to_desktop借作输入frame。
- fresh AXClose在checked_context_unavailable前置拒绝、native_dispatch=false；另一次明确的cleanup placement在AX读降级后target_binding_unknown/native_execution=false，未move、不重放。三个binding退役；源最终真实到期closed/expiry1/进程exit0，所有Safe/drag/hover/text/scroll输入计数0，不称显式Close成功。保留这些window-local输入/语义动作缺口，继续真正实现，不以只读成功替代。
- 配置BASELINE false1/MODIFIED true0/独立copy ROLLBACK false1/REAPPLIED true0，六源pristine hash精确恢复，非生产回滚。沿用同四角色/累计158路径与原97基线；真实顺序原图/报告在outputs/validation/r58-window-local-visual-2026-10-12/。整个F01–F16保持未完成；过去CmdTab、Host v2 Live、sheet等已完成结果不重演。


## 2026-10-12 · R59：已恢复，窗口局部后台输入原生闭环

- 合并后根/旧Codex-RWI/旧AI Projects alias实际同对象，158受管源hash一致；不改迁移记录、不重建卷。先按存下的controller真实核对fa580ba对应38161295476：watch0、Mac+Windows两job success；这是恢复后的首个未完成操作，不重复旧GUI。Goal服务仍返回paused，未调用不支持的resume或新建替代goal；执行依据用户此前“搞完恢复会话”的条件授权与已确认目录状态。
- native::window_mapping_ready/display_layout将完整单display包含从input映射移除，改为有限Window几何+实际active unrotated交集；on-screen exact native Window/AX/SCK核验、layout digest、object/properties、lease/no-replay和modal等不变，完整离屏/旋转受影响Surface仍拒绝。display assignment仍null。foreground_move::guard单独验证point唯一命中一个unrotated active display，不clamp/global fallback；新负向point规则本轮仅纯测试，不称HID实机负向完成。
- 新mapping合同基线101→修改2pass0；首workspace旧wiring字符串失败101/partial490/1/9/16保留，纠正到guard(self,foreground,target.point)后完整11门0/workspace765/0/11/42、Receiver66/UI7、fmt/严格Clippy/release/架构通过。首保存大JSON argv被OS拒绝Argument list too long（未启动命令），纠正逐event写入，保留error来源而不捏造exit。
- 同证书/DR部署Agent18993 / 73001bbe-91f1-4177-9114-7f45a3b236f1，signed hash d44ee05a5ed5d84e99e3b44ada745110f64f3aa06f0d09168abbb4866380d7a7，原lease inode排他/durable资源门先过；四权限、MCP5cf8424、CA/profile/config未变，无Host重启或人工请求。
- 同native129/PID15825/CG43580/process birth和[3225,504,660,552]旧snapshot有图但mutReady/inputReffalse；新Frame自身map[.5,0,0,.5,3225,504]与两ready true。源仍display=null/右165pt出屏。一次AXPress Safe1；一次typed BG drag实际1/16/1/completed1/orphan0，完整1320×1104蓝线；18鼠标callback均inactive/nonkey/frontPID97732，非连续/hardware证明，AX语义非mouse input。
- outside ScrollArea advertised AXScrollDownByPage却返回-25205/uncertain，scroll events0/clip0，未重放或改走其他输入。保留四原请求terminal/released/no pending/no uncertainty/no replay。Fresh AXClose一次后源closed/expiry0/进程exit0，两binding退役；不靠timer冒充Close成功。
- 配置false1/true0/独立copy ROLLBACKfalse1/REAPPLIEDtrue0，七源pristine hash精确恢复，非生产回滚；同四角色158与原97不改。真实顺序原图/报告在outputs/validation/r59-window-local-input-2026-10-12/；F01–F16全目标仍未完成，不以这一个窗口的成功推断通用App/hidden/DnD/hover等。


## 2026-10-12 · R60：偏出显示器的局部后台输入实机闭环

- point_element先正向区分物理屏坐标与仍有有效屏交集的window-local点；后者只从精确AX窗口的有界原生子树取包含实际点的对象，不借用应用级hit、不clamp、不改delivery。application_hit_geometry校验本身rect，same-window fallback必须CFEqual原生关系；树选窗口不继承empty-window application-hit证明。Facts::validate保留hit_rect等guard并补紧凑数值诊断，声明只阻止当前batch、先前batch可能已发送，不诱导replay。
- 130原refusal/0输入/expiry1exit0保留。第一候选在同一131仍拒绝，真实hitBounds[1800,0,1920,30]不含point[3745,844]且same_native_window=false；不能把其角色或clamp原因凭矩形猜出来。第二候选同131、同point、一次新wheel原生生效：inactive/nonkey/foreground95007，scroll0→1、clipY0→240、原图row00→08；385ms是该适配器调用墙钟，不是性能优越性。
- 一次明确RWI focus131实际95007→98005，然后同点foreground_system move得到foreground_point_unmapped/rejected_before_dispatch/native_dispatch=false；这是实机前台点guard执行，而不是先前hit_rect失败冒充它。131/132明确AXClose、expiry0、进程0，两个binding退役；7原durable请求terminal/released/no pending/no release uncertainty/no replay。
- 最终Agent2772/89dec9f4稳定同证书部署，四权限true，安装MCP/CA/profile/config不变，无用户配合或Host重启。最终当前源全11门0/workspace768/0/11/42，Receiver66/UI7。原四角色扩展，同输入source配置1/0/1/0及精确hash恢复；不声称生产回滚。
- Live沿用已部署的小320点上限/原生hide-close图标；保留R48真实Host闭环，不伪造本轮用户可见验收。全F01–F16未完成，generic tracking/tooltip、跨显示器drag、hidden/minimized、foreign-provider菜单和跨应用DnD仍保留。当前修改不因一轮对话自动commit/push；既有远程CI不是本候选CI。
- 正式报告及按实际时间顺序的原图：outputs/validation/r60-offdisplay-background-input-2026-10-12/REPORT.md。


## 2026-10-12 · R61：精确窗口与物理命中分离、真实canvas跨屏界拖拽

- 134原quarter边界没有让canvas跨界，未将它冒充验收。新增fixture-only boundary_canvas_right/Rect::place_boundary_canvas_right，原view/role/responder/oracle不变；parser1→0/check-only无GUI及2个纯测试。source/pristine hash记录未改原97。
- 135同PID的Colors/Fonts先于main，第一次误选只观察，按已分配CG44120纠正后真实drag在hit_rect拒绝/0输入；hitRect[3205,740,470,270]不同于point[3676,829]。面板出现原因未知，不把同PID当owned-provider证明。原135请求保留不replay；135/136到期expiry1/exit0。
- point_element快速application hit必须native CFEqual精确Window且geometry含point，其余独立查selected native Window child tree；不借用foreign hit、改delivery或继承empty-window application proof。foreground_move::guard新require_foreground_pointer_hit做物理screen positive owner/window/geometry/readable检查；background metadata不授权foreground event。
- 修正部署后新137明确inactive/nonkey，peer138前台：global[3676,829]→[3781,829]跨3720右界，真实down1/move16/up1/completed1/orphan0，18NSResponder callbacks全部inactive/nonkey/fg29079/ownCG44174，真实PNG蓝线，局部[196,79]→[301,79]。随后明确focus137，samepoint foreground_hit_unproven/no native dispatch；另一有效foreground point真实hover enter1/move1/tooltip1。137/138明确Close/expiry0/exit0/detach；9durable terminal/released/no pending/no uncertainty/no replay。
- 同稳定证书Agent28828/e57ec955已部署，4权限/CA/profile/config/安装MCP不变，无人工配合或Host重启。完整11门0/workspace769/0/11/42、Receiver68/UI7；源skill小幅补精确window选择及两种grounding，validator0，未覆盖当前cache。四角色保持同集合并扩展、matching source配置1/0/1/0及精确hash恢复，不称生产回滚。完整目标/计划未完成，剩余generic background tracking、复杂layered provider/whole offscreen/DnD等继续保留。
- 正式报告：outputs/validation/r61-exact-window-cross-edge-drag-2026-10-12/REPORT.md；原图按实际顺序排列，raw截图不提交Git。


## 2026-10-12 · R62：框架pointer状态、真实Computer Use对照及private field58内存校准

- fixture-only DispatchEvent::framework_pointer/UI::framework_pointer公开getter诊断：只own window匹配时读NSEvent.mouseLocation与own-window/hover转换，未知null，不改responder、鼠标位置、active/key或tracking counters。增加2纯测试，初msg_send comma warning保留，修正后全11门0/workspace769/0/11/42、Receiver70/UI7。
- 原139 inactive默认move app/window各1但view/tooltip0；140 even active/key，eventWindowPoint[100,382]与framework[-1151.03125,130.53125]不同且insidefalse。Cua真实一次无害click与RWI同点click均实际down/up、inactive/nonkey、hover0；不杜撰Cua独立hover方法或作异intent耗时比较。subtype3/0与flags1048576/0记录，来源未知不猜。明确focus140+foreground_system move后位置匹配/insidetrue/enter1move1tooltip1。
- 最终源141启动时指针已经在其hover，enter1/tooltip1是既有状态；其前台addressed move0→1不可算后台。focus不覆盖它的右peer142后141 inactive/nonkey/fg76688，既有tracking仍inside，独立新BG move1→2，条件性view接受已证实但泛化cold hover未完成。139/141/142明确Close expiry0/进程0；140到期expiry1/进程0，process_identity_unknown之后的JS读取elements TypeError保留，未发送Close，不replay；11durable全部terminal/released/无pending无uncertainty。
- 原生Rust skylight_event_field_probe仅本地event内存：SLEventSetIntegerValueField(58,987654321)使CGEvent timestamp123456789→987654321，post/record只查symbol不调用，无authentication/窗口/输入/cursor改变。重取primary pinned tree源码；trycua desktop hover源码确为warp+HID且两次post，不能当真后台copy；不照抄field58 gesture注释。
- 生产Agent/MCP/稳定证书/权限不改；诊断和条件对照不是“hover已修复”。全计划继续，source四角色同集合扩展及1/0/1/0 hash恢复；正式原图/报告outputs/validation/r62-native-tracking-framework-state-2026-10-12/REPORT.md。


## 2026-10-12 · R63：显式SkyLight实验通道与后台tracking接收对照

- MovePointerOptions新增background_skylight_experimental严格opt-in；仅MouseMoved、一条fresh guard后计数的SLEventPostToPid，无默认切换／认证／field58／warp／自动fallback。符号进程期缓存、失败关闭library，无每次输入泄漏。新核心3／native内存2测试，完整11门0/workspace774/0/11/42、Receiver70/UI7。
- 同稳定证书部署Agent2300/509117ee；实际loaded MCP返回新capavailable=true，四权限／CA／profile／安装MCP不变、不需人工重启。143既有tracking后自然到期0；新144右侧cold hover0，peer145前台：public与SPI各一条type5准确达App/Window，仍view/tooltip0且framework转换区域外。明确FGmove才enter1/move1、dwelltooltip1；再focus145，144inactive/nonkey的新SPI move1→2仅条件性tracking证明。未泛化冷态后台hover。
- 144/145明确Close expiry0/进程0/detach；9durable terminal/released/no pending/uncertainty/replay。保留初编译101、源接线断言101、warning与unknown_frame/schema拒绝；focus确会失效同desktop aliases，fresh snapshot后新操作成功，不重放已投递输入。
- 同四角色扩展166，source-copy matching配置1/0/1/0与精确哈希恢复，非生产回滚。正式报告／原图outputs/validation/r63-explicit-skylight-pointer-2026-10-12/REPORT.md；全计划继续未完成，优先改善失效范围反馈并补通用tracking／复杂provider等。


## 2026-10-12 · R64：切窗后明确失效范围，避免旧frame误用

- Engine::focus_window新增grounding_invalidated：current_executor／same_native_desktop／target_count／snapshot_then_resolve；只有原始调用执行失效时返回，持久重复读取不假装新失效。核心返回／拒绝／uncertain／未知counter断言覆盖，baseline新检查101→modified0；完整11门0/workspace774/0/11、Receiver70/UI7。
- 同稳定签名部署Agent34348/cc9272d1，四权限／CA／profile／安装MCP不变。真实147前台后反馈3个本执行器目标失效；146旧frame只读unknown_frame/nativefalse。fresh snapshot+一次AXPress独立Safe0→1，146inactive/nonkey／fg34877。两个明确Close expiry0/进程0/detach；4durable全terminal/released/无pending/uncertainty/replay。
- 四角色源配置1/0/1/0和精确哈希恢复，非生产回滚。当前改动待与后续功能组合理提交；远程CI仍是R63 f2204d02/macOS+Windows success，不把旧CI当本轮证明。同步F03真实实验已实现状态，不再以陈旧not_implemented反复重做；全目标未完成。
- 报告／原始顺序截图：outputs/validation/r64-focus-grounding-feedback-2026-10-12/REPORT.md；下一步系统文件面板／provider层级，不依赖人工。


## 2026-10-12 · R65：真实系统文件面板／模态拒绝／取消后后台恢复

- 仅显式fixture system-panel-controls增加真正NSOpenPanel own-parent sheet与空目录。SystemPanels.file及真实completion response分离visibility/CG/sheetParent；队列上限/drop计数，可重入调用不借UI。record私有导致初编译101保留，窄completed方法修正；新增2测试，完整11门0/workspace774/0/11、Receiver72/UI7。生产Agent仍R64，未重复部署。
- 148 Open原始AX25204/uncertain不改，后续实际sheet44629/attached/display4/open1。父Safe modal负例nativefalse/Safe0；精确Cancel response0。148最终expiry1/exit0，晚到Safe/Close未发，两detach，不假装显式关闭。
- 新149 Open仍uncertain，首次有界只读未发现不当终止、不重启、不replay；继续同process/handle后sheet44684真实可见。Cancel returned；独立发现仅parent44674，fresh unblocked＋Safe0→1/inactive/nonkey/fg97732。明确Close expiry0/exit0，两detach；7durable terminal/released/无pending/uncertainty/replay。
- 此例真实NSOpenPanel渲染owner仍fixture PID，不冒充foreign-provider；没有放松physical point守卫或把window截图当遮挡证明。下一项已有LiveFamilyCapture与真正文件sheet的层叠合成实测，全A01–A07/F01–F16仍未完成。报告／原图：outputs/validation/r65-native-system-file-panel-2026-10-12/REPORT.md。


## 2026-10-12 · R66：真正系统文件sheet的Live层叠合成

- 当前加载RWI＋真实Host MCP Apps v2可见，own150根44695/系统sheet44713为已验证AXSheet/AXParent；actual Live内建屏320×239点/640×478 PNG，两层合成1760×1104，same-PID Colors/Fonts不自动纳入。
- 原Open25204/uncertain不重放；首Live180秒到期，Resume仍stopped，另新观察不是输入重放。Cancel之后首次recovering清图即Close，所以R66根恢复未验，不判失败或宣称通过。第二Live原生Close stopped/requested/worker_done/source survives，源单独Close expiry0/exit0，3 detach/4durable全终态。
- 同源实际11门0/workspace774/0/11/42、Receiver72/UI7；无部署/人工请求。顺序原图与报告：outputs/validation/r66-live-native-file-family-2026-10-12/REPORT.md。

## 2026-10-12 · R67：取消系统文件面板后根窗口Live恢复通过

- 新own151 PID62667/根44733/sheet44747/虚拟4，实际Host tab3可见，RWI发一次Open25204/uncertain；35秒只读超时保留，随后同原进程真实sheet/open1，不重启/重发。
- 实际Live两层complete，原生预览44755/320×239点；Cancel returned/真实response0/visiblefalse/解除own-parent。连续只读500ms采样231frames→recovering清图attempt1→232frames/root1/running；后276frames仍root1，原生预览新44763/320×303点/640×606 PNG。约1.5秒为本次观测不是性能上限。
- 对新preview fresh selection再AXClose，stopped/requested/worker_done/清图隐藏，源仍可读；源单独Close expiry0/exit0，4detach/4durable终态释放、nopending/uncertainty/replay。没有用户配合或用户眼见宣称。
- 同四角色补两个报告170项，原97不变，源配置1/0/1/0哈希恢复/非生产回滚。源码逐字节未变，使用R66当前实际11门而非旧CI；生产Agent/MCP/证书不改。
- Live已部署320点上限/eye.slash/play.fill/xmark/28点AX可访问控件；复核Apple HIG与Codex PiP用户issue，未获取官方精确尺寸，不称私有UI1:1。正式原图/报告：outputs/validation/r67-live-native-file-return-2026-10-12/REPORT.md。全F01–F16未完成，优先冷态hover/foreign-provider/隐藏最小化/DnD/压力等未验范围，不重复本闭环充进度。


## 2026-10-12 · R68：系统菜单语义正例、视觉及恢复缺口保留

- own152/74718/root44766/sheet44776/虚拟4；Open25204/uncertain不重放，实际后open1。Grid菜单AXPress→菜单项，List→List/AXOutline回读；source已active，非后台no-raise。
- 原图不带菜单，after-menu/after-List原byte同。Icons请求returned但即时仍List，未等即Cancel，不称恢复或API必坏。Live Stop最后worker_done=false，detach前没确认worker，保留缺口。
- File Cancel response0/visiblefalse/解除parent，源独立Close expiry0/exit0，两detach/7durable清理无重放，反馈marker隐藏。报告outputs/validation/r68-native-system-menu-2026-10-12/REPORT.md。

## 2026-10-12 · R69：显式AX菜单动作声明与稳定部署

- native_semantic_actions增加AXShowMenu/AXCancel/AXPick，真实控件advertise且caller显式才可；原dispatcher执行体/默认click/exact object/window/provider/lease/noReplay不改。SDK Pick列Obsolete，不默认替代Press。
- 核心新增能力/element/去重错payload测试，Mac声明与strict-reference/click-only测试扩展。初regex错误1不算baseline，纠正后原pristine1→修改0→独立copy窄rollback三hash/1→reapply0。11门0/workspace776/0/11/42、Receiver72/UI7，3focused各1pass不重复计数。
- 原inode排他lease、旧instance durable终态下同证书原子部署Agent89552/cabc313d；signed SHA05e6e1ad668e7c78ea0682d5013fec144c61a8f95a941911060fae12b0ffda10。4权限true/CA/profile/config/MCP原hash不变，无人工配合/Codex重启。当前loaded MCP实读newinstance/3动作。
- 报告outputs/validation/r69-explicit-native-menu-actions-2026-10-12/REPORT.md；同四角色扩展173/原97不变，全目标未完。

## 2026-10-12 · R70：新动作语义效果、有界settle与停止顺序实测

- 新loadedMCP对own153/90874/root44814/sheet44824：ShowMenu→菜单控件，Pick List→List/AXOutline；Icons第一次仍List，read-only500ms后第二次Grid/AXList，未重放。AXCancel后同窗树无原菜单，但coverage partial/absence_proven=false。
- 五原PNG全同SHA66e6b962，不能称菜单/视图像素变化。Live只root+sheet2层，foreign-provider视觉缺口仍在，不以新Pick案例推断旧Press坏或性能优势。
- File Cancel response0/visiblefalse/解除parent；Live Stop先等worker_done=true/清图隐藏，源仍可读，再源独立Close expiry0/exit0、两detach/9durable终态/no replay，marker隐藏。真实Host tab5已可见，无假presence/IAB/人类宣称。
- 报告outputs/validation/r70-native-menu-actions-2026-10-12/REPORT.md。下一项继续系统菜单视觉/provider与F01–F16，不重复已完成三动作验收。


## 2026-10-12 · R71：真实渲染等待和另一PID系统菜单被捕获

- own154/30584/root44864/file44875/Virtual4。ShowMenu＋3秒/noAX/noLive的exact capture仍基线0f289cbe；List Pick＋2.5秒/noAX/noLive真实fb983d85列表像素，AX另读；Live真tab6/原生640×478显示List，Icons＋2.5秒/Live运行回到0f289cbe。不是旧缓存普遍故障或延迟上限。
- menu render为AppKit XPC openAndSavePanelService31294/CG44922，uid501/ppid1；host proxy node_window44876不是sheet44875或真实menu44922。仅explicit独立绑定/read-only capture得560×582/6e3f9427菜单，0输入/no frame/没有自动Live关联。PID/名字/几何不当ownership证明。
- 154到期480s/expiry1/exit0但真实draw RefCell panic两次，不称干净关闭。迟到snapshot无elements TypeError保留，Menu/FileCancel/SourceClose未发。实际source/menu catalog空、Live止采/clear/hidden/workerdone、4detach/6durable清理。报告outputs/validation/r71-native-menu-visual-isolation-2026-10-12/REPORT.md。

## 2026-10-12 · R72：fixture重入关闭与漏记callback错误修复通过

- 只receiver appkit.rs：RAII DetachedSlot/with_detached_ui令AppKit teardown不持UI_STATE借用；window_closed、failed lifetime stop、run退出涵盖，unwind仍恢复。callback失败count在UI之外黏性记录，不被无法borrow吞掉；file cancel后重采状态。三pure tests通过，不修改生产输入/授权/Live。
- 新155 file-open→120秒expire，156 file+实际AXMenu-open→120秒expire，均expiry1/exit0、无panic/output_error，file visible/key/attachedfalse、实际response0。stderr正常保留own fallback close日志，非MCP输入重放；3durable终态/无pending/uncertainty、3detach、feedback隐藏。120秒不是480秒稳定性上限。
- 当前11门0/workspace776/0/11/42，Receiver75/UI7。source配置1/0/1/0及原09c9a03f哈希恢复，原97不改/同四角色175，不称生产rollback。生产release Agent/MCP6bd444eb/871aef7a没变，无部署/人工重启。
- 报告outputs/validation/r72-fixture-reentrant-teardown-2026-10-12/REPORT.md。全F01–F16未完；下一步foreign renderer归属与Live合成及其他矩阵，不重复语义正例充进度。

## 2026-10-12 · R73：Live进一步缩小并完成自主真实卡验收

- panel_size cap320×360→280×300点，竖向限制允许160点宽保持比例；修正首次portrait测试exit101后9相关项通过。采集2048/源1320×1104不降，28点eye.slash/xmark及提示语保留。
- 稳定签名Agent93892/c6480c95已部署，旧Agent租约互斥/durable终态后原子换包；本会话loadedMCP自动重连，权限全true/MCP+pairing+cert不变，无人工/重启。
- 真实Host tab7可见，专用157在虚拟4，内建1浮层实际280×269点/560×538原PNG；卡隐藏清图、恢复77→90新帧、卡关闭worker_done/source存活；再源Close一次干净exit0/expiry0/空stderr/2detach/1durable终态no replay。不是睡着用户亲眼验收。
- 11门0/workspace776/Receiver75/UI7，源码同输入1/0/1/0原hash恢复；同四角色176。Windows .ui-os旧正式门禁仍exit1不改，不宣称跨平台或Codex像素UI一致。
- 报告outputs/validation/r73-compact-live-refinement-2026-10-12/REPORT.md。完整计划保留，下项F12 foreign-provider归属与Live合成，不重放已完成157。

## 2026-10-12 · R74：明确额外视觉窗口实现，真实新参数正例待fresh Host

- LiveOptions.additional_windows≤3，根+sheet+额外共≤4，native exact UID/session/process/window指纹；额外源无输入／无owner推断；同层去重，原生z顺序前后一致；消失省略不换ID，root rebind清源。新visual/additional.rs职责拆分，未抬architecture600上限。
- 首轮architecture视觉635>600 exit1已保留；修复后完整11门0/workspace779/Receiver75/UI7，focused core3/macLive43/MCP UI3。源码copy1/0/1/0恢复9hash+absence，同四角色178。
- 稳定Agent21019/236e3e9a部署权限全true；新MCP24f133e5原子安装，新进程真实tools/list有additional_windows，现有Host旧adapter负例unknown field/0mutation。禁止操作Codex宿主的原工具错误保留，无绕过／无人工要求。
- own158 Files uncertain/-25204后真实file45054出现，单次Cancel response0，源Close exit0/expiry0/stderr空；3durable终态／2detach。新formal未挂载／compat queued，均0frame并stop workerDone，不称新familyLive或additional-source正例通过。
- 报告outputs/validation/r74-explicit-live-sources-2026-10-12/REPORT.md。总计划未完；下一步实际新MCP负例契约及fresh Host的显式foreign正例、自动ownergraph，保留其他F01–F16。

## 2026-10-12 · R75/R76：实际新MCP负例与原生容量／回收验收

- R75 installed新MCP实际进程3负例duplicate/4sources/owned_popup均invalid_live_sources，exit0/空stderr；不是Host GUI。
- R76 own159/31064/45089 Virtual4：15attachment+desktop16，越界target_capacity；2×4并发capture全部返回/原SHA5ab54928/106227B，101..519ms caller await含队列，不称nativeCPU。lease禁止detach，明确释放后回收新ID，retired target resolve foreign_target；原图缓存仍在有界64MiB证据预算，不授予旧frame输入。
- 4未挂载idle Live worker，第5拒绝；stop均done/clear/hidden/0frame。释放slot后1000ms到期terminal，resume不复活。16detach最终target1；源Close单次exit0/expiry0/空stderr/1durable终态/no replay，safe/drag/hover0。不是活跃视频／15应用／physical用户连续性通过。
- R74完整gate源码/binary hash仍同，11门0/workspace779，不冒充本轮新跑；same四角色180 sourceconfig1/0/1/0。默认Codex proxy sock不存在exit1，无globalreload／配置变更／安全绕过／人工请求。
- 下一项R77 min按钮ROOT_WINDOW_STYLE及responder.window_miniaturized独立oracle fixture patch；继续最小化/隐藏状态，保留fresh Host多源正例及全F01–F16，不重放159。

## 2026-10-12 · R78：浮层状态缓存修正，原签名部署与自主实机通过
- R76 hide响应false但targets缓存true，完整原生目录0；保留旧断言exit1，没有重复输入或hide。修正Rust Runtime::describe/refresh_volatile_target_facts 的 pointer_feedback 新鲜度，同身份刷新，未知/跨身份null，不新增输入授权或锁竞争。
- 同输入离线真实函数BASELINE1/MODIFIED0/独立副本ROLLBACK1/REAPPLIED0，两个原hash恢复。第一轮rtk空格错误及测试命名空间编译exit101保留，修正后11门全0，workspace782/0/11/42、Receiver75、UI7。
- 原证书/DR与配对复用、排他租约+durable终态检查、Agent原子更新：PID78581/c51be93a；4权限仍有效，MCP不换，本会话自动重连。一次纯视觉show/hide在虚拟屏4：targets分别true/false，独立窗口45113出现；600ms后完整目录0。没有用户应用键鼠/真实光标/激活，也不宣称窗口已销毁。
- Live更小的R73实机结果与隐藏恢复关闭保留，私有Codex浮层尺寸未查到，不虚构1:1尺寸。完整F01–F16及标准PR拆分发布仍未完成；先继续R77 fixture最小化候选与hidden/minimized实测，不向睡觉的用户索取配合。

## 2026-10-12 · R77实机：后台最小化确实达成，观察路径限制已定位
- 补专用根窗ROOT_WINDOW_STYLE最小化位、公有isMiniaturized原生回读；sheet不动。同输入源码配置1/0/1/0，独立回滚两个原hash恢复。11门0，workspace782/0/11/42、Receiver75、UI7。
- 自身160/PID98397/CG45116/Virtual4仅启动一次；动作前在后台。一次Minimize window/AXPress返回-25204 uncertain且永不重放，但独立原生getterfalse→true，前台PID两样本仍97732。API确认≠效果；此案例正是已发生效果却收到不确定确认。
- 原绑定capture_visual window_missing但exact native record present/owner matches，onscreen目录0不证明销毁。当前onScreen选择/SCK路径限制确实存在，未伪称最小化捕获成功。
- 沿用159原600000ms模板自然到期，原进程exit0/stderr空/status closed/output_error null。durable terminal/released无pending/uncertainty；detach、反馈隐藏、目标回到1。生命周期602122ms不是单步性能数字。
- R79 pending Rust候选已准备：仅视觉读取指定ID/PID的offscreen native record并按需扩SCK枚举；输入/默认发现仍on-screen，身份/租约/no-replay不变。尚未应用或验收，先真实执行候选和离线门，再原签名部署＋新161实机；不索取人工、不重放160。

## 2026-10-12 · R79：精确离屏描述＋最小化后台原像素实机通过
- CG IncludingWindow在161最小化后仍无row；复用description-from-array解析，只为response-only视觉启用离屏SCK，正常input/observe仍on-screen。新selection/policy九测与完整11门0，workspace791/0/11/42。architecture600行限制不提高，抽取语法/编译失败保留后重跑。
- 原证书/DR/配对/四权限不变，Agent70150/ad38部署；MCP不换自动重连。163/70685/45159后台、最小化原生gettertrue、前台自身actor164/71454，实际MCP PNG1320×1104/108919/SHA b9cabf…可见自身文字/控件，非空白；没有input frame/authority。
- snapshot仍拒绝；旧proposal负例实为expiry，不夸成Min专属guard。161最小化时实际FG，标签不作证据；164明确RWI focus后163才真BG。晚期外部前台变化/最终恢复false保留，不倒改capture-time true。
- 四原进程exit0/stderr空，六durable操作终态释放无pending/uncertainty。中断造成内存MCP输出丢失，保留本任务有限检索/精确摘录/原PNG/原生oracle，不伪造全返回，不重启或重放。
- 同输入源码1/0/1/0、3原hash+2absences恢复，四角色继续扩展。Minimized Live单根层序、完整F01–F16/标准拆分PR发布仍未完成，下一步真实执行Live层序验证与补齐。

## 2026-10-12 · R80：单个最小化根 Live 4 帧，正式控制卡异常继续修复
- capture_layer_order 单离屏根不虚构层间顺序；多层严格、身份/出现状态前后校验不变。完整11门0/workspace794/0/11/42，Receiver75/UI7；最终元数据文字修正版部署 Agent26360/ee76ea21，原证书/配对/权限/MCP不变。
- own165/6999/45311 已真BG/min gettertrue的新Live3捕获4帧；源600390ms正常exit0/空stderr，stop worker_done/detach/隐藏反馈/原输入不重放。无新的MinLive原图或人类确认，最后文字未再native验。
- 真Host卡DOM拒绝/disabled、0帧保留；兼容页确实198帧，但到期累计237不全当Min帧。实际embeddedUI回归7pass1fail，结束组件的新cap未重启heartbeat；R81下一项修正，未归因唯一原因。
- 同输入源码配置1/0/1/0，原hash恢复，非生产回滚。四角色188/原97不变；完整F01–F16/合理提交标准PR发布仍未完，无人工请求。报告 outputs/validation/r80-single-offscreen-live-root-2026-10-12/REPORT.md。

## 2026-10-12 · R81：新cap恢复、关闭/teardown竞态修复与MCP部署
- embedded Live UI 对真正新cap重置状态/队列/sequence并替换唯一heartbeat；旧响应/错误/异步teardown按对象隔离，旧相同cap不复活，hidden不发布visible。首恢复红测7/1、追加关闭/teardown两红测均保留；最终12/0。
- 完整11门0/workspace794/0/11/42、Receiver75/UI12；同输入12case BASE9pass3fail(exit1)/MOD12pass(exit0)/独立ROLL9pass3fail(exit1)/REAPPLY12pass(exit0)，两原hash恢复，非生产rollback。
- 原子安装MCP83736a8b，新真实stdio resourceHTML与源码byte相同，实际Agent26360健康。Agent/Codex不重启、不kill其他adapter、profile/配对/证书不动、无键鼠或截图。当前长会话旧adapter/Host缓存的新UI尚未实机证明，不伪造刷新或眼见；全F01–F16继续。
- 同四角色189/原97不变。报告 outputs/validation/r81-live-ui-replacement-recovery-2026-10-12/REPORT.md；下一项新专用166/167显式多源native Live，不索取睡觉用户配合。

## 2026-10-12 · R82：两进程显式源实机发现/移动/退出通过，Live仍0帧
- 新own166/40576/45420＋167/40790/45425 Virtual4；独立绑定成功，166单次place1824,24/rendered confirmed，167不搬。两张1320×1104原PNG可读，是源图而不是多源Live。
- show_live additional exact167被加载接受；真Host browser3/tab2 visible却DOM拒绝/disabled，0frame；compat open queued/实际IAB精确URL不存在，0frame/suspended。均明确stop（formal返回本轮工具输出没有另存完整RPC），不假presence/不重放；多源acceptance=false。
- 各源Close一次，原进程exit0/stderr空/expiry0/output_errornull；3durable terminal/released/no pending/uncertainty，2detach/markerhide/target1。源码byte同R81，引用11门0/794/UI12，不冒称本轮新门或新部署。
- 下一项R83精确拒绝reason诊断，随后查多实例/缓存/上下文链路，不以source截图或兼容0帧定义成功。完整F01–F16与最终PR/合理commit/push目标仍未完；报告 outputs/validation/r82-explicit-native-multisource-live-2026-10-12/REPORT.md。

## 2026-10-12 · R83：精确reason与打包修订已部署，Host旧资源仍未刷新
- UI只显示三种已知拒绝reason，不反射任意message；14case红测→全过。package()新增外部binary分块SHA/env配置identity，6case验证同路径不同内容修订变化、command/args/私有profile不复制、不启动后端。
- 最终12门0/workspace794/0/11/42、Receiver75/UI14/packager6。UI、pkg、descriptor独立copy均同输入1/0/1/0和原hash恢复，非生产rollback；两新managed既有pkg源码首次baseline冻结，原97不变/四角色193。
- MCPa0a0f645已原子安装，新真实stdio资源与源码相同；Agent26360/证书/配对/权限/profile不换，无GUI输入或重启。RWI登记env binaryhash更新后真实Host仍旧9153B/7aee9229，无R81/R83代码；不宣称刷新成功。
- 默认appserverproxy实际exit1/socket不存在，schema只有null参数globalreload；没发送reload、没kill三个RWI进程。三个PID/资源旧代码是实况，不直接推断UI究竟路由到谁；R84下一项新adapter_instance诊断，baselineNode15里14过1失败，尚未应用。
- 完整F01–F16/正式Live多源实机/合理commit标准PR推送仍未完，不以新stdio或packager替代正式目标。报告 outputs/validation/r83-live-rejection-and-package-revision-2026-10-12/REPORT.md。

## 2026-10-12 · R84：Gateway实例诊断、v3资源、同项目新会话接力
- Gateway.instance clone保持、每真实实例新UUID；start_meta与rejected_app_event带adapter_instance，UI可明示mismatch，但不跨实例授权/转发、不冒充认证。资源v3，v1/v2读兼容。UI15/新增Rust1。
- 首v3 workspace101因stdio固定v2；installerguard1保护未部署；修正契约后12门全0/workspace795/0/11/42、Receiver75/UI15/pkg6。同输入源码/descriptor各1/0/1/0原hash恢复，非生产rollback。
- 新MCP9fe5ae6d真实v3资源byte同源码；RWI env修订同步，Agent26360/profile/配对/证书/权限不动，旧Host真实只列v2，未伪称Live通过或强杀刷新。
- 按用户此前条件性新会话授权，在同RWI项目local创建01a1305c-f868-79f3-a9b9-e0f2e26597a0并受支持导航，wait确认真实inProgress/mac-host；先只读，父四角色ready才释放共享修改。不建源码复制、不要求人工，新任务承接完整未完成目标，父任务只读跟进审核。报告 outputs/validation/r84-live-owner-instance-and-v3-2026-10-12/REPORT.md。

## 2026-10-12 · R84 新会话：正式 Host 双 PID Live 与完整清理
- 真实loaded resources/list/read=v3/10460bytes/420f80f4…与源码byte相同，installed9fe5ae6d…，Agent26360/配对/证书/四权限不改；父角色ready才继续，不重做R84 patch或12门。
- 全新168/169只在授权Virtual4；真实Host DOM可见/“预览连接已确认”，native第59帧为两独立PID显式来源，原生浮层在内建屏1。session3最终147帧/真实240s expiry；新session4恢复后双源31帧→附加169关闭精确omission、根168继续110帧→根关闭paused/imagefalse→真实卡Close/stopped/worker_done，最终111帧。
- 两source具名AXPress各一次、exit0/stderr空/nonexpiry，三durables terminal/released、不重放，两detach/剩一desktop target/反馈隐藏。保留初始异步race、过期卡selector失败、精确浮层截图及最终卡观察timeout；不声称浮层截图已保存、用户肉眼见证或密码学task-ID。
- 同源码hash复用R84已执行795/0/11/42、UI15/pkg6/Receiver75及12门；原四角色/baseline继续扩展报告并实测独立copy rollback。正式双源/生命周期已接受，full F01–F16仍false，不把这一项替代full计划。报告 outputs/validation/r84-fresh-host-native-multisource-2026-10-12/REPORT.md，公开文字 docs/validation/2026-10-12-r84-fresh-host-live.md；raw screenshots不入Git。
