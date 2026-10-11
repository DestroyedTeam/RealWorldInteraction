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

# RWI macOS 整体验收与公开准备（阶段记录）

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


### R28按真实调用顺序原图（仅专用虚拟屏测试窗口）

1. 66后台基线：Safe0、无蓝线，原生1320×1104。

![native66-baseline](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/01-native66-baseline.png)

2. 66明确AXPress后Safe1；应用inactive/keyfalse。

![native66-safe1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/02-native66-safe1.png)

3. 66原AXWindows读取拒绝后独立同窗SCK仍可见Safe1；没有输入授权，原drag未重放。

![native66-readonly-after-refusal](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/03-native66-readonly-after-refusal.png)

4. 67独立后台新笔画：蓝线完整，native down1/move16/up1、18回调App/key/frontmost均false。

![native67-background-blue-stroke](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/04-native67-background-blue-stroke.png)

5. 67独立document sheet初始Safe0；父旧输入因blocked变化拒绝，无父派发。

![sheet-baseline](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/05-sheet-baseline.png)

6. 67子sheet明确AXSafe1，随后独立Close1；父Safe仍0、visible/attached已false。

![sheet-safe1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-core-native-architecture/06-sheet-safe1.png)

## 2026-10-11 · 当前 R25/R26：MCP 职责拆分与真实协议等价通过，架构剩余13项

- 当前：同一产品根 / R25/R26 纯模块提取已写入，实际41工具的 initialize、完整说明、schema、资源清单与已安装4a29版本逐项一致 / 继续核心与原生架构债务、全部A01–A07/F01–F16；未新部署、提交、推送或开源。
- R25 convenience 2191行 → 188行入口及12个责任文件，全部≤600；原45 focused与2个真正MCP process正/poison测试通过。7门禁0、workspace722/0/11/42、receiver51/UI7。R26把投影测试、整数字符串边界、执行摘要、图像映射和worker测试分开，5个超限文件均≤600；架构检查仍exit1，但实际19→18→13项，未改规则或调高阈值。
- R26全workspace722通过后，Clippy真实因一个重复test-only导入exit101；原记录保留，仅删除重复导入，最终当前MCP110pass/1ignored、all-target/all-feature Clippy、fmt、fixture51、UI7及release均0。保留已完成前两门禁；不宣称在该import删除后又全跑了一次workspace，也不放宽-D warnings。
- 两轮同输入责任/大小契约均BASELINE1→MODIFIED0→可执行独立副本ROLLBACK精确原hash/新增路径消失→1→reapply0；这是大小契约，不是GUI验收。R25真实进程candidate9fcf与安装4a29完全等价；R26最终candidateb44ee同样等价。原输入、租约、窗口/设备身份、诊断、坐标、按钮、分页与no-replay策略保留。
- 原97 baseline逐entry保持、README第98原件和R25新增12不重置；R26首次管理trace原件加5个新增文件，同四角色为116管理路径。累计patch/default rollback/reapply全部hash/absences和原生360尺寸契约继续核验；此前失败的边界/导入/格式命令都保留，不包装成产品原生故障。
- 用户无需再配合。本轮只读loaded RWI仍设备available、仅desktop目标，不发送输入或截图。Live先前R18小浮层360×336/原图1320×1104与隐藏/恢复/关闭证据保留，不把这些纯代码移动说成新的GUI实测。真正Host任务绑定、composite window family和执行中独立系统键盘等未覆盖项仍公开记录。
- 已准备R27核心Engine/像素锚点/测试职责提取runner，只有parse与当前源码hash检查，尚未执行，不计完成。pointer依赖/旧workspace基线须依据ADR0002和真实依赖图评审，不直接修改allowlist骗绿。


日期：2026-10-10。对象始终是本仓库；产品源只在根工作区编辑。完整 A01–A07、F01–F16 仍保留，**不是整体验收完成**。

> **历史阶段（R6，非当前）**：workspace **690 passed / 0 failed / 11 ignored / 42 suites**；独立例程38、UI逻辑5。新MCP已实际部署（9f96321b…），Agent9721/69c18019及稳定信任/权限未变。新增标准会话内MCP Apps Live通道；实际Codex是否渲染、任务隐藏/恢复尚未验收，本任务旧MCP资源列表仍为空。Native25 `abcd`→Left→`X`得到 `abcXd`，补齐caret效果，不是只看按键回执。完整计划仍未封版；没有新commit/push/public。

> **历史阶段（R8，非当前）**：本轮重启真实加载42个模型工具和1个UI资源。用户已看到内建屏画面，Native32读回188/188帧及正确矩形；随后233帧出现位置未确认停止，源码已修复并用稳定证书部署Agent。最终workspace696/0/11/42、UI逻辑7、fmt/Clippy/release均0。新UI/MCP已部署，但当前Host资源实读仍旧HTML；独立展开、连续隐藏/恢复/关闭及任务切换待重新加载后验收，**不是整体验收通过**。

> **历史阶段（R11/R12，2026-10-11，非最新）**：执行中Live实时画线已由用户确认；用户休息后自主验收420×386紧凑原生浮层、圆形隐藏/关闭、恢复与停止清理。Agent稳定签名实际部署，四权限保持；workspace702/0/11/42、receiver45/UI7。并行模型status排队4957ms、AXPress−25200及真实人类执行中目标切前台仍明确保留，不宣称全部完成或已正式发布。

> **历史阶段（R13，2026-10-11，非最新）**：真正MCP零设备并发探针16回复/exit0；loaded Native45完整前台1/79/1及清理0。只读状态外侧5004ms、适配器内部2.386ms，等待主要不在这个Repl/native处理段；精确Host原因未定。没有改产品Rust/重新部署，不冒称R12回归是本轮重跑；全计划和自主执行中切窗仍继续。

> **历史阶段（R14，2026-10-11）**：真正loaded RWI完成自主执行中BG→FG连续drag；原生16后台+1中间态+64前台回调，完整1/79/1与清理。真实Cmd+Tab切到own companion，再次后台1/16/1；机器actor与物理人类分开，普通现代activate未生效的负例保留。workspace702/receiver49/UI7及全部所需gates通过；仅fixture实现与验收，不是全产品/公共发布完成。

> **当前 R20/R21（2026-10-11）**：722项workspace通过；只读AX错误降级MCP4a29已部署，Agent/信任/四权限不变，当前父MCP仍旧版。两个仍存活的自有窗口之间，旧前台tuple拒绝、精确租约边界与完整5秒drag1/79/1自主验到，原请求不重放、源关闭0/解绑。并发错误请求4583ms等待仍待分层定位；真实人类/Host新formatter/复合Live族不冒称通过。全A01–A07/F01–F16和PR公开准备未封版。

## 实际结果（按发生顺序）

1. 自有 Receiver/Cover 位于虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`。覆盖按钮的显式 AXPress：计数 0→1；普通指针路径 `visibility_unknown`，派发前拒绝，计数仍1。不能把语义动作当指针点击成功。
2. 明确 focus_window 读回切窗；紧接着的语义动作被 `user_active` 拒绝，原请求未重放。AXFocusedApplication 与 NSWorkspace 的观测口径不相同，不能混称为一个前台真值。
3. Receiver 新增 `--environment-sample-ms`（25–2000ms，默认关闭），只采样自有窗口和公开应用状态，不发输入、不截屏、不监听全局键值。关闭时 timer 失效；两次同输入原生测试均29tick、输入动作0、实际退出0。
4. 用户确认“已看到叠层并切换返回”。当时 Live 状态读回137/137帧，内建屏原生面板 `[736,209,1040,912]`；API hide/resume/pause/stop 分别验证隐藏、恢复、清图与清理。**任务隐藏间隔未独立连续采样**。稍后确有 `live_presenter_wrong_display`，但旧版未保存失败时几何，原因不明；不可归因于用户移动或显示器变化。
5. 修正 `State::stop` 的终态分支：已停止时保留首个停止原因/取消 generation。实机到期→Stop 仍为 `duration_expired`；worker_done=true、无 pending capture、面板移除。此测试0帧（viewer未打开），不是可见画质验收。
6. 继续修正 `panel_size`：高细节捕获的预览宽度上限640点；`max_edge=2048` 捕获上限未降低。新增 `presenter_failure`，错误位置分支在移除面板前保留精确自有 CGWindow ID/矩形、请求与包含显示器；`cause_attested=false`。该异常记录分支尚未实机复现触发。
7. 普通后台5秒拖拽：1 down、79 moves、1 up、1 completed；用户随后明确“还没切换”，因此该次不算 Command+Tab 验收。

![第一次拖拽前：自有测试窗口，无画线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/58-command-tab-before-snapshot.png)

![第一次拖拽后：原生蓝线和 completed=1，不是 selection overlay](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/63-command-tab-after-snapshot.png)
8. 使用确认框重新协调；用户选择“开始测试，2秒后切窗”。新的独立请求在另一自有窗口执行，旧请求未重放。自有环境/NSResponder 记录在 move44、move68时读到 NSWorkspace frontmost PID 从48574→25950→901，up时仍为901；并行 Describe 也记录了不同前台元组。真实蓝线、1/79/1、无 orphan、active=null；durable result terminal/released、无 native pending、未 fenced。**证明这条后台拖拽在观察到的前台变化期间完成，不证明所有前台冲突/目标被用户切到前台/每个应用都如此，也未截取用户实际按键。**

![重新协调后的独立窗口：拖拽前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/73-user-switch-fresh-05.png)

![观察到中途前台变化后：完整拖拽与原生蓝线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/80-command-tab-after-snapshot-02.png)
9. 两次拖拽都把接收者 AppKit active/key 从false变为true；系统前台不是接收者本身。旧 cover 的顺序也发生变化，不能宣传“应用 active/key/order 完全不变”。F07仍需修复、独立归因和前后台对照。
10. 新签名 Agent 的 Live 实际449/449帧、随后另一次56/56帧；source image_size=1320×1104，均到期清理、last_error=null。未在可见时保存新面板矩形，因此640点紧凑几何仍是离线验证+部署，不能写成实机矩形已读回。
11. 第06个30秒自有探针的首次完整 CG discovery 返回0个该PID窗口、semantic_app_visibility_unknown。控制器误用了“display mismatch”的统一错误；实际是缺少当前可见窗口证据，**没有观察到错误显示器**，未attach、未启动Live、未派发输入。探针自然关闭、退出0。启动就绪、WindowServer呈现/Space关系继续排查，不凭这一条判定原因。

## 回归、事务与部署

- fmt、workspace locked/offline、全target全feature Clippy `-D warnings`、release build均实际0；workspace 674 passed / 11 ignored / 41 suites。
- 同一命令/断言：sampler、停止原因、紧凑预览各自 BASELINE101 → MODIFIED0 → 可执行ROLLBACK恢复原hash → ROLLBACK101 → REAPPLIED0；每个断言恰好1 test，不接受0-tests成功。sampler原生同输入1→0→1→0；两次实际29tick/无输入/关闭。
- 保留真实失败：旧 context_closed 期望与首因保护冲突（workspace101）后分开验证 active Drop；copy2旧mtime导致复用旧测试二进制（reapply101）后用隔离target修正；Clippy manual_clamp101后改为clamp；旧fixture hash guard1发生在创建前；第06次0窗口 guard为JS执行错误而非原生派发失败。
- 最新 Agent PID `2234` / instance `66189a8f-eb5f-46df-aef6-58938b1bf233`；签名hash `17edb27af6b9fa3e7166e651672fb4adb0f5f4681f9ad37fbcbd9861262a4854`。MCP40工具，二进制 `b12fa991b9baaf59515739652154d2b41991eda007e5477edf72370a12e75878` 未替换；四项权限仍true。仍用原本地签名DR、配对、公有配置，未导出私钥、未更换证书/CA、无需本轮额外Codex重启。
- 两次实际部署均持有原共享native域的稳定inode排他lease，检查当前instance的durable input全部terminal/released，确认原进程退出后原子交换；回滚app与原始包保留。未清fence、未重放输入。
- 自有01–06窗口已全部实际关闭/退出0；绑定退役；只操作测试窗口，未对游戏、飞书、Chrome、已有文档发送输入；用户自己的切窗仅做元数据观测。

## 未完成 / 下一步

- A02：Live仍依赖任务浏览器面板visibility，未完成原生Host任务生命周期绑定，也不能将持续连接当成任务可见性。显式显示/隐藏结果与真正任务切换证据分开记录。
- A03：本次只接受“后台drag + 中途前台变化”。用户把目标切前台、foreground冲突、中途取消/up清理及恢复不重放仍需要分别验收。
- F05 view/tracking/tooltip回调仍未验收；F07接收者active/key/order隔离未修复；F08/F09离屏/Space矩阵、F12模态/系统窗口族/Live、F13 native跨应用DnD、F14 browser专用路由、F15压力清理、F16分层稳定p50/p95全部保留，不把当前测到的单样本当性能结论。
- 开源准备：gitleaks8.30.1全可达历史231 commits及当前541份选定产品源/文档均实际0、未发现secret。不是所有资产/日志/历史隐私与许可都验完；未扫描的资产继续审核。GPL-3.0既有许可未更改。
- 仓库仍private；尚未新commit/push/公开。标准PR复用 [PR #1](https://github.com/DestroyedTeam/RealWorldInteraction/pull/1)，已附到任务。验证完成后合理拆commit，显式stage，排除私有配置/凭据/二进制/原始截图/运行态，禁止force-push或release/tag。

原始命令、literal stdout/stderr、exit、hash、输入、native receipts与回读在本轮工作证据目录及同一套四角色内；截图为工具原始PNG，仅解码保存，无编辑，属于本地验收证据，不stage入Git。


## 后续：去掉隐式指针激活，真实 inactive 接收（本轮）

前一goal轮属于进展：真实部署、同输入回滚和用户中途前台变化证据已经存在；本轮不重定义最终目标。

- 第07个30秒探针首轮发现实际源窗口，后来进程到期后才再次调用；不是“活着但不能发现”。第08个完整连续执行段成功绑定，但当前任务viewer未显示，Live0帧/隐藏，Stop/移除清理已读回；不能把它写成尺寸/画质通过。标签页依赖仍未解决，后续不在未显示时反复启动同类Live。
- 根因已落在真实代码：`dispatch_input` 在指针之前发送了 `NSApplicationActivatedEventType`。现在 `needs_background_activation` 纳入 intent，只保留已校验键盘兼容准备；所有指针intent都不发送这种通知，hover的显式nonactivating guard保留。正常点击/drag/scroll不自动fallback到旧激活路径，capability明确说明。
- 旧四参数策略不支持新intent契约：同命令BASELINE101为E0061编译拒绝，**不是原生行为失败**；MODIFIED0/恰好1test，独立副本可执行ROLLBACK恢复4份原hash后同harness101，再REAPPLIED0。首轮全回归两条源码wiring检查失败101，保留既有hover显式guard后workspace675pass/11ignored/41suites；fmt/Clippy/release0。错误的仅ok-suite计数解析已保留、改为包含FAILED，未以部分277个通过声称全回归通过。
- 新Agent真实部署：PID 60659 / `9ff5b99a-161b-4ccd-8237-c33af9d66f73`；signed `cd852d4de81826755f5bf96e84993739bacc0c04862a554cddd0cf42972240f7`；稳定DR/四项权限/原CA配对保持，MCP40 `b12fa991b9baaf59515739652154d2b41991eda007e5477edf72370a12e75878` 未替换。沿用同一排他lease、durable terminal/released gate、原进程退出、原子交换、实际smoke0。
- **第09原生同坐标/同5秒drag对照**：旧已记录场景05 active/key false→true、cover顺序改变；新09 `[180,670]→[620,850]`：1down/79moves/1up/completed1，原生蓝线；全部NSResponder首尾/中间记录都active=false、key=false，cover_above=true，系统前台15516不变，无orphan、active=null。不能推广到所有应用/hidden/otherSpace/物理user键连续性，但这不是只改文案或只通过构造测试。

![新指针路径：在 inactive/non-key 状态画出原生蓝线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/POINTER09_AFTER_SNAPSHOT.png)

- **第10**：普通single+double共3 down/up、click_without_drag=3；native scroll delta_y=-200、clip origin `[0,0]→[0,200]`；被覆盖的Safe button显式AXPress计数0→1。期间active/key均false、cover顺序保持、系统前台不变，所有receipt未fenced。单击313ms、双击327ms、scroll476ms、AXPress159ms是单次adapter墙钟样本，不是p50/p95或CU优越性结论。

![真实滚动与计数读回](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/POINTER10_AFTER_AXPRESS_SNAPSHOT.png)

- **第11键盘兼容回归**：新非激活pointer点自己的普通文本框，然后typed Unicode实际value=`RWI 后台键盘11`，changes=1。保留的keyboard preparation把app_active变true，window_key仍false、cover顺序和系统前台保持；这不是完全无激活键盘。left key只有returned、缺少caret-range独立回读，不能宣称按键目标完成。无全局键日志、无setValue/clipboard替代。

![非安全自有文本框的Unicode实际内容](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/KEYBOARD11_AFTER_TEXT_SNAPSHOT.png)

- Receiver新增显式 `--regular-app`，默认仍Accessory；必须完整分配window/run/state/display且重复拒绝。public setter/getter校验Regular政策，不调用activate。独立example32test通过；同CLI断言BASELINE101未知flag→MODIFIED0→ROLLBACK101→REAPPLIED0，3份原hash恢复。Agent产品二进制不因fixture变化而改变。
- 第12普通Dock/Cmd+Tab测试app已实际启动在虚拟屏（PID84604），初始active/key/frontmost均false；只用于用户把后台目标切前台的下一项专项。已发确认框，**未有完成/实际切换证据前不标通过**。若窗口到期只读取其已完成状态，不能盲重放。

### 仍然未完成

完整A01–A07/F01–F16继续保留。特别是用户将目标切到前台的中途策略、前台冲突、取消/up释放、normal-window对照、stacked modal/system popups、hover tracking/tooltip、真正Host任务Live绑定及质量/稳定性、其它适用MCP、日志/资产审计及合理commit/PR/push/public仍未封版。本轮没有发布、推送或改public。

第12窗口已按5分钟上限自然关闭、真实进程退出0/无输出错误。末次自有采样读到active/key/frontmost均为该测试app；起因和变更时间没有完整历史、尚未收到开始答复且期间没有派发drag，因此不能把它写成“中途用户接管通过”。旧确认框中的PID已到期，后续先刷新分配目标，再开展该项。


## 用户补充：Command+Tab 应由 RWI 的系统键盘通道执行

你提出的三点方向都正确：系统快捷键不应被误当作仅发给某个输入框的事件；前台键盘与后台定向键盘是两条显式路线。本轮实际新增了 `computer.hotkey`，原 `press_key` 仍保持精确 focused native element 的定向语义，绝不自动全局 fallback。

### 代码与协议的实际变更

- Core `Engine::hotkey`：original desktop（拒绝 attached window）、确切 expected_foreground、同一 physical input lease；写前 durable reservation，原请求去重/冲突、未完成 journal 禁止重放、完成后废弃同桌面所有旧 proposals。原生副作用计数区分“发送前拒绝”与“不确定”；所有 receipt 仍 `goal_verified=false`。
- Native macOS：一个 Private event source，modifier flagsChanged → physical keyDown → keyUp → 逆序 modifier-up；CGEventPost(HID) 的显式系统事件流，不是 CGEventPostToPid，不请求激活、不 warp、不注入文本。整个序列先分配，每个 down 前检查 session/foreground/已有权限/近期HID；guard错误或取消时 Drop 仍发送已配对的自有 up，不能自动释放别人的键。
- Agent 注册/校验/执行/Observed 计数与平台能力全接通；MCP添加标题 `Send foreground/system shortcut`。没有窗口选中也可报告此桌面入口，Windows和其它未实现的backend明确不支持。
- `authorized_display` 代码复核显示它是已有的可选 placement-on-bind 策略，不能据字段名字杜撰为全局键盘禁令。现有 config 字节不改；输入权限仍受原 Principal/input 与确切桌面限制。

OS API依据：[Apple CGEventPost](https://developer.apple.com/documentation/coregraphics/cgevent/post%28tap%3A%29) 说明按指定位置进入事件流；[Apple CGEventSourceStateID](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid) 区分Private与既有状态表。Private来源、投递成功都不是“真人硬件来源”或“应用业务成功”证明。

### 已执行验收与没有执行的内容

| 层级 | 已观察结果 | 仍不能宣称 |
|---|---|---|
| 同一 `cmd+tab` schema/checker | 原版无入口101；新版0；独立 copy 执行 ROLLBACK 后同缺入口101；重应用0；10原hash+4absences restored | 不等于系统实际切窗 |
| 离线状态机 / cleanup | 本轮新增10项：guard/lease/attachment/去重/partial effect/取消松键/原键解析/平台能力；workspace685pass/11ignored/42suites、fmt/Clippy/release0 | 不等于所有应用接收此事件 |
| 签名服务部署 | 同证书/DR；旧PID60659实际退出，独占原inode域与全部current-instance durable清理后交换；Agent22357/09186440；权限true | 不等于新热键native输入已验收 |
| 新 MCP 真进程 | initialize / tools/list41 / mac-host targets全部0；入口implemented+authorized+available true，标题/schema存在 | 不冒充当前Host任务GUI调用 |
| 当前任务已加载Plugin | 真实RWI targets读到新Agent，但可调用表仍40、没有computer.hotkey | 不能通过临时stdio/旧press_key/隐藏JS桥去冒充新typed工具验收 |
| 旧Regular12终态 | 84604 timer关闭/exit0，已detach；没有拖拽派发，晚期active/key/frontmost true | 不知道变化来源，不计“拖拽中用户接管” |

本轮没有发送任何全局按键，也没有操作游戏、飞书、Chrome、已有文档；没有commit/push/改public。下一项是Host重载后**由RWI发送一次系统Command+Tab**，再独立读取前台身份与原操作cleanup/no-replay。合成切窗的结果与真人输入的证明分开；共享lease也不允许为了模拟真人而偷偷绕过并发保护。

### 本轮错误溯源与修正

1. 首次 targeted build101：test module漏引入 `fixture::Fixture`（E0425/E0433），保留原stderr，补import后同组测试0；没有改弱guard。
2. MCP首次atomic install后，operator只读probe因Python缩进错误1未启动其MCP子进程；controller实际atomic rollback至原b12。保存原事件，修正缩进/先syntax check，R2复用已校验原backup；fresh41 gate0。Agent不再次重启，config/profile/CA/签名身份不改。
3. 首次准备命令的shell `printf ---` 参数解析错误，整条命令仍exit0；后续读取改用rtk proxy cat。它不是产品执行失败或权限错误。

详细literal stdout/stderr、命令、status、原hash、修补与恢复结果继续存入同一 `VERIFICATION.txt`。原生09/10真实后台pointer效果与AppKit inactive/keyfalse/order保真继续保留；11键盘兼容仍改变appactive，left-key无caret oracle，不能笼统称全部键盘验收完成。

本轮当前安装 SHA：Agent签名 `a46b69d29ac413bc29ed6f5f08ce6161da47748056f39b5078320c17096c7729`；MCP `f40648f8c781404150dca2a5ac24f5ea3f28c2e31bda2267935f5a33ab9f68ec`。四角色绝对路径：
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`


## 后续 R4：系统快捷键、途中前台接管与取消恢复

### 已执行的新结果（按发生顺序）

1. **新任务原生41工具已验收**：本任务仍40工具，但用户曾明确允许插件加载不可用时新建项目任务接力，因此创建「RWI 系统键盘与切窗实机验收」（`01a1250e-25ec-71f3-9e1d-2ea6e7ce177d`）。其真实 loaded `computer.focus_window` 把自有53515切前台；随后真实 typed `computer.hotkey cmd+tab` 发送4个配对事件。立即回读仍是53515，稍后的独立 targets + fixture 均回到15516，不能把发送回执冒充切窗。租约释放、原请求terminal/released、无pending/fence、未重放；自有计时关闭/实际exit0。父任务已重新打开21份证据并核对SHA。**不再等待重启来验这项**，但不声称当前旧任务已加载第41个工具。

![系统快捷键验收：显式focus后的自有窗口，截图本身不是前台身份证明](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fresh-host-keyboard14/02-after-focus.original.png)

2. **自有fixture定时前台演员**：新增 `Options.self_foreground_after_ms`、`legacy_self_foreground`、`State.self_foreground_requests` 与 `UI.self_foreground_timer`；必须显式分配window/run/state/display、Regular policy、100..60000ms且短于自身lifetime。默认关闭；timer只调用自己的AppKit生命周期，不发送键鼠、不操控其它程序，不是RWI后台注入桥。普通现代 `activate` 在16中只产生请求，全部真实拖拽回调仍inactive/non-key/系统前台15516，蓝线1/79/1只算后台拖拽，不算接管。Apple明确现代激活请求不保证成功，旧ignoringOtherApps方法已弃用，因此旧方法只作为**显式 `--legacy-self-foreground` 测试演员**，没有加入产品fallback。[Apple AppKit激活说明](https://developer.apple.com/documentation/macos-release-notes/appkit-release-notes-for-macos-14?language=objc)，[旧方法](https://developer.apple.com/documentation/appkit/nsapplication/activate%28ignoringotherapps%3A%29?changes=__1&language=objc)。

3. **19：拖拽途中目标真的转前台**。后台起步：active/key=false，系统前台98541。拖拽down后约3964ms，NSResponder sequence64开始读到自身4672/frontmost=true/active=true/key=true，随后drag/up在同一目标继续。真实蓝线、1down/79moves/1up/completed1、无orphan/active=null；独立targets确认4672；原请求terminal/released、无pending/fence、禁止重放；detach及timer关闭/exit0。这是fixture诱导的前台转换，**不是真人Command+Tab接管、竞争输入或所有应用保证**。

![19：后台开始，没有线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/FOREGROUND19_BEFORE_SNAPSHOT.png)

![19：途中转到前台后完整线与原生计数](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/FOREGROUND19_AFTER_SNAPSHOT.png)

4. **20：保留失败而非假成功**。把MCP便捷 `computer.drag` 嵌入 `rwi.call` 时，现有Rust REPL只接device catalog原生工具，实际 `js_unknown_capability`、calls=[]、mutation_attempts=0；自有计数全0。`request_unknown` 单独不能证明未派发，本次还有上述独立零动作证据。没有重放20；正常单步drag继续首选typed工具。此处工具说明/错误未指出被拒工具，作为后续可读性修复保留。

5. **21：真实中途取消/松开/恢复**。换新独立请求，使用已支持的native resolve_view映射、在loaded MCP JS内claim→act；1500ms cell上限触发准确服务operation_ref的协作取消。原生实际1048ms/16moves后自身配对up：1down/16moves/1up，无orphan/active=null；完整5000ms目标没有完成，不能把fixture的completed=1（见到up）当业务目标完成。trace确认原生quiescence、租约释放；独立exact execution查询terminal/cancel_requested/released、无pending。部分输入诚实记录uncertain并置fence。之后显式 `computer.recover_input` 核对当前域、所有native writer idle、HID+combined两表0键/0按钮/flags0，再恢复域；**恢复过程 input_sent=false、无额外松键事件**。恢复后旧receipt完全相同、仍禁止重放；fixture关闭/实际exit0、detach。

![21：取消后的短线，不是完整请求目标；松开已收到](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/CANCEL21_AFTER_SNAPSHOT.png)

### 回归、回滚、异常与公开范围

- 同一allocated CLI命令/输入：**BASELINE101（未知新flag）→ MODIFIED0（恰好1test）→ 可执行ROLLBACK恢复3份原hash → ROLLBACK101（恰好1test、同未知flag）→ REAPPLIED0**。独立副本只用于冻结验证，不是另一个开发源码。最新workspace685pass/0fail/11ignored/42suites、独立example35test；fmt、locked/offline、all-target/all-feature Clippy `-D warnings`、fixture build均实际0。
- 异常均保留：13旧binary hash assertion实际exit1，root原83hash稳定，重新实际build0后13退出0；15prepared/null窗口就绪判断错、无输入；17exec先yield尚无stdout导致JSON parse错误、无输入；18初次模板自引用FileNotFoundError实际exit1（没有fixture），改命令后真启动；18startup CG几何 `[1823,503,662,554]` 与刚读epoch变化导致真实 `window_epoch_changed`，未派发drag，后续取稳定状态并全新显式发现再attach；19成功。没有把准备脚本错误都归为RWI不可用。两次apply_patch定位/格式失败仅在接续摘要有记载，原工具输出未另行落盘；不将其当完整literal证据。成功修补后才跑回归。
- 13/15/16/17/18/19/20/21都自然关闭且实际进程exit0；19/21目标已detach。整个本轮没有给游戏、飞书、Chrome或已有文档发送输入/截屏；系统快捷键仅14一次、目标都是显式自有fixture，主屏应用只有短暂系统切换。产品Agent/MCP/config SHA重新核对仍为原a46b69d…/f40648f…/39d4b3…；本轮fixture变更没有再部署Agent，没有改证书/CA/私钥/TCC/profile。
- 用户直接确认 **“确认旧截图及相关 Git 历史可以公开”**，已记录到 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/PUBLICATION_SCREENSHOT_AUTHORIZATION.json`。此前截图范围等待解除；仓库尚未新commit/push/改public，资产许可/其它历史审计、完整验收与合理commit/标准PR仍待完成，不能把截图授权当全套验收通过。
- **完整A01–A07/F01–F16不删减**：Live原生Host任务生命周期绑定、真实任务隐藏间隔、原生Hide/Close/层叠弹窗对照、真人前台接管/真实竞争输入、hover view/tracking/tooltip、offscreen/Space/hidden矩阵、系统模态族、跨应用DnD、浏览器路由、并发/清理压力及分层p50/p95仍保留。19/21只关闭其准确专项，不替代整计划。下一项先验证并修正REPL拒绝诊断的可读性，随后继续剩余窗口/Live矩阵。


## R5：REPL拒绝诊断、真实文档模态窗口及范围修复

1. 接续dispatcher实际启动源码提取Rust诊断检查：原branch只返回 `js_unknown_capability`，1test失败/exit101。最小修改在 `crates/rwi-mcp/src/gateway/repl.rs`：拒绝信息现在包含实际工具名（最多128字符，非标识字符替换）、原生catalog边界与便捷工具应直接调用的提示；不回显参数、不开放nested REPL/便捷工具、不放松native权限/租约。更新工具说明，避免此前“全部advertised tools都能rwi.call”的歧义。
2. 真正生产MCP entry（零devices）执行3种拒绝：computer.drag、computer.js递归和4096字符名称，验证calls=[]/mutation_attempts=0/无参数回显/有界消息。相同命令原版101→新版0→可执行ROLLBACK恢复2原hash→恢复版同harness101→重应用0，恰好1test。首个冻结副本遗漏include_str!的 `integrations/javascript/rwi-client.mjs`，实际compile101**不算语义负例**；补该明确输入后同检查才得到预期失败。新安装MCP的独立真实进程也验证了命名拒绝输出/exit0，非GUI验收。
3. 新增自有fixture `--modal-controls`、`State.modal` 独立native oracle和真正public beginSheet/endSheet。默认关闭、完整window/run/state/display分配、正确typed nullable Block、只创建/关闭自有NSPanel，不是键鼠注入桥；原生visible/key/number/frame/display/parent关系与按钮计数独立。源码CLI BASELINE101未知flag→MODIFIED0→ROLLBACK原3hash恢复且同输入101→REAPPLIED0；example38test。
4. Native22真实打开Sheet30810/own parent关系/virtual4，own AppKit active=true、sheetkey=true；后续操作准备过久，实际自身120秒timer关闭/exit0。不能把后来无窗口判为活应用发现失败，未派发safe/close。并发现shared target/debug example在Cargo gates期间换hash、source3仍相同：保留binary guard实际exit1，重新build0并冻结内容寻址runtime binary；不再依赖可变target路径。
5. **Native23发现真实bug**：独立父30818与Sheet30827绑定、sheet safe0→1、关闭和父继续观察都成功；但是Sheet打开时父blocked=false/mutation_ready=true，显式父AXPress实际把父Safe count从0改为1。这是模态范围缺口，不能标全通过。

![23：弹窗按钮实际收到操作，但父窗口错误地仍可修改](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/MODAL23_SHEET_AFTER_SAFE_SNAPSHOT.png)

6. 修正 `NativeMac::modal_state`：正向检查所选窗口直接AXChildren里的AXSheet及其确切CFEqual AXParent关系，再与既有modal判断合并；type-check子对象、64项有界探查、已知hidden不当active；不把缺失属性当已知否、不新增应用级PID锁。只阻止确切父窗口；单独绑定的Sheet不会自阻塞。跨PID子对象的正向父关系可以阻止父窗口，但不为该对象自动授予任何输入权限。这不是全App modal/所有深层provider完成证明。
7. 同helper test旧缺入口E0425/101（不是native负例）→新增0→独立ROLLBACK原hash恢复/同harness101→重应用0；实际Native23父计数1是独立原生反例。最终workspace**686pass/0fail/11ignored/42suites**，example38；fmt/locked-offline/Clippy all-target/all-feature -D warnings/release Agent+MCP均0。
8. 真部署：沿用原稳定cert/DR，原inode排他lease内检查所有current-instance reservations terminal/released/no pending；此前21的历史fenced=true回执未改写，fresh当前域检查unfenced后才允许交换。原Agent22357实际退出；新Agent**9721 / 69c18019-b329-40b4-98ab-c77c37b7da96**，signedhash `8d90970f21245d4725a94c753af0adea610ef81acbf97a8b21efc7716f4c2645`；MCP新hash `f9585125eee427a67eff88cb489cc753c9d89d82262ce99726d5c5e3dbf5cdfd`。四权限仍true，config/profile/CA/cert未变、无私钥导出或额外授权。旧MCP进程不强杀，Root loaded tool schema仍按实际表，不冒充新进程；实际loaded typed工具读到新Agent。
9. **Native24部署后复验通过准确专项**：父30889、Sheet30898。打开后父blocked=true/mutation_ready=false；父AXPress16ms实际 `checked_context_unavailable`、native_dispatch_entered=false，父计数保持0；独立sheet blocked=false，safe0→1，close1，visible=false/attached=false；父恢复blocked=false/mutation_ready=true。4个原请求terminal/released/no pending/no replay，parent/sheet均detach，自有30秒timer关闭/实际exit0。没有重放23。

![24：修正后弹窗safe计数为0；父窗口已阻止操作](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/MODAL24_SHEET_SNAPSHOT.png)

![24：独立弹窗safe计数1，父计数仍0，随后正常关闭](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/MODAL24_SHEET_AFTER_SAFE_SNAPSHOT.png)

### Live接续及剩余范围（未缩减）

重新核对[官方Hooks文档](https://learn.chatgpt.com/docs/hooks)：SessionEnd不会在切走会话或thread/unsubscribe时立即触发；**不能拿它假装任务切换隐藏**。官方[可选MCP UI资源](https://developers.openai.com/plugins/build/chatgpt-ui)提供会话内宿主渲染候选，而不是要求打开独立浏览器标签。下一项先执行真实MCP resource能力门槛，再实现/验证宿主viewer候选；当前还没有该resource或真正Host任务ID/可见性证明，不称标签页依赖已移除。未使用私有Codex UI读取绕过既有CUA拒绝，也未伪造visibility事件。

Remote系统面板、其它文档/同PID sibling、nested modal、多source Live、实际任务隐藏间隔、真人输入冲突/hover-tracking-tooltip、offscreen/Space/hidden、raw游戏、跨App DnD、browser route、压力和分层p50/p95等完整A01–A07/F01–F16继续保留。24只接受自有原生文档Sheet的范围/动作/关闭，不推广所有应用；无游戏、飞书、Chrome或已有文档输入。公开旧截图+Git历史授权仍有效；未新commit/push/public，标准PR逻辑拆分仍待总验收。


## R6：标准 MCP Apps Live 通道 + Left 独立消费验收

### Live：已做出的代码与实际边界

- `Gateway::get_info/list_tools/list_resources/read_resource` 增加标准资源能力；`ui://real-world-interaction/live/v1.html` 以 `text/html;profile=mcp-app` 返回。只有 Host 明确声明 `extensions.io.modelcontextprotocol/ui.mimeTypes` 包含该 MIME，`computer.show_live` 才提供渲染入口和 `_meta.ui.resourceUri`，`computer.live` 仅作原数据/控制入口；默认/experimental-only/错误 MIME 仍是原文本/浏览器路线。不是凭Host名称假定支持。
- 新 UI-only `computer.live_panel` 标记 `visibility=[app]`，另加一个仅在UI协商时显示的模型入口 `computer.show_live`；没有 `target`/device/窗口/坐标/截图/键鼠参数。能力仅限当前适配器的随机 view_key/page、正安全整数 sequence；替换page、过期、乱序和未分配事件拒绝。close先撤销视图，再仅对已绑定的native Live发送stop，不重试未知结果。
- 标准 `ui/initialize` / `ui/notifications/tool-result` / `tools/call` / `ui/resource-teardown` 消息桥；资源无fetch/WebSocket、外部域/iframe/字体/权限请求。私有pane能力只放在tool-result `_meta`，不放在文本或structuredContent。原native捕获和权限/会话/桌面边界未改。组件未握手、没有已分配能力、缺IntersectionObserver/缺可见信号时不发布visible=true；只允许一个请求在途，隐藏替换排队heartbeat，关闭不重试，到期自动清空。
- 新组件初始化先清除旧page的预览，失联依赖短native presence TTL；准备阶段若失败保留已返回的start与presence错误，不能把前面的native效果丢掉。原浏览器兼容页可接管page，但不能与旧组件同时续租。
- 这是**已实现的可选会话内组件通道**，而不是已完成原生Host任务ID绑定。Intersection/文档可见性证明的是组件；不同Host隐藏iframe的行为仍需要真实切任务验收。官方[UI说明](https://developers.openai.com/plugins/build/chatgpt-ui)把这种资源作为兼容Host的可选UI；[MCP Apps规范](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx)规定显式协商和teardown。不得将[SessionEnd](https://learn.chatgpt.com/docs/hooks)等同即时任务切换，更没有读取私有Codex窗口/对话来伪造活动任务。

### 同输入与回归

- 实际零device MCP进程，两份相同协议输入（未协商、已协商）：**BASELINE1 → MODIFIED0 → executable ROLLBACK还原6个原hash/不存在项 → ROLLBACK1 → REAPPLIED0**。每个MCP子进程正常退出0；红的是资源/协商验证，不是原生输入失败。根工作区从未回滚，冻结环境仅用于构建/验证。
- 首次测试真有 `E0308`/101（rmcp扩展字段是BTreeMap而非JSON Map），已修正；首次协议runner的绝对可执行路径被RTK截成 `/Users/…/AI`，进程1，**不计作预期baseline失败**。保存原stdout/stderr，改用同工作环境的 `./bin/rwi-mcp` 后重跑四状态，才接受1/0/1/0。均无窗口/截图/键鼠。
- 三个Rust策略/资源/事件测试、一个真实MCP可选协商进程测试；5项嵌入JS VM测试覆盖握手+父消息来源、无能力不续租、单在途及隐藏优先、hide/resume、close失败不重放、teardown等待隐藏、缺API保持隐藏。**VM是组件逻辑，不是真实Codex或GUI通过**。
- 最终FINAL9全workspace **690/0/11/42**，fmt check、locked/offline tests、all-target/all-feature Clippy `-D warnings`、release MCP全部实际0。既有例程38独立，UI逻辑5独立，不并入690。仓库macOS Skill已补齐两种viewer输出的分流，quick_validate实际0；未改已安装cache或其它插件。

### 实际部署与当前Host

- 仅原子替换RWI MCP：旧 `f9585125eee427a67eff88cb489cc753c9d89d82262ce99726d5c5e3dbf5cdfd` → 首版UI `8e0792c73cb473bb6b7f9370634fc476fbf7dba2f3482aa0a8a2be54176bbba5` → 渲染/控制分离后的当前 `9f96321be0fb8c51d32f05129105542dd27b43c13f5a4230b3b738de032775e8`。部署和fresh smoke实际0；没有终止旧MCP、重启Agent/Codex、重发输入或改证书/配对/profile/config。
- Fresh进程实际未协商**41工具**；协商HTML后**43工具（模型42 + app-only1）**，资源/list/read、闭网络CSP、未知URI及未分配presence拒绝均通过。该进程读取资源不等于当前Codex已展示。
- 实际 `list_mcp_resources(server=real_world_interaction)` 在当前已加载Host仍是空列表，当前仍40个工具。下一步重载后再看真实能力/渲染；不能宣称Codex不支持，也不提前宣称已经切换到会话内UI。
- Agent仍 PID9721 / `69c18019-b329-40b4-98ab-c77c37b7da96`，signed `8d90970f…`、原公有CA/certificate/config/profile hash全部相同，4项权限true、fence=false；不读取/导出私钥，不重新授权。

### Native25：Left不是只收到returned

1. 只启动冻结的自有Receiver（PID47486 / CG30945），虚拟屏4/既定UUID；新分配、独立attach、初始field空，未复放任何旧请求。
2. typed click普通自有非安全文本框，typed text=`abcd`；原生PNG与AX value实际为`abcd`。
3. typed `press_key key=left`只发送一次；此时文本仍`abcd`，**单凭回执和文本未变仍不足以验证caret**。
4. 同一原生控件typed text=`X`，实际PNG、AX value、接收方独立State.text均=`abcXd`、changes=2，而非`abcdX`。因此接受这次Left移动caret的消费效果。未用setValue/剪贴板/终端GUI桥替代。

![Native25：实际输入abcd，光标在尾部](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/KEYBOARD25_BASE_SNAPSHOT.png)

![Native25：Left一次，再插入X，原生结果abcXd](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/KEYBOARD25_AFTER_INSERT_SNAPSHOT.png)

5. 接收方文字回调active/key/frontmost=true，仍是保留的兼容键盘准备路径，**不是纯无激活后台键盘**。4原始输入请求各自terminal/released/no native pending/replay=false；detach、timer closed、真实exit0；feedback隐藏读回visible=false，未留输入资源。到期后的fresh targets系统前台回到原PID48574（只读观测，不操作该应用）。
6. 单次adapter墙钟：click140ms、文本107ms、Left247ms、插入83ms，均有原literal stage timings；同轮有并行Cargo回归，不能由这些单点样本作p50/p95、整体任务耗时或优于CU的结论。

### 保留的整体验收与公开工作

Native24 parent blocked/no dispatch/counter0、sheet safe1/close1/parent恢复的原证据保留；JS命名拒绝、系统hotkey14、拖拽接管19和取消/up21均不重做、不重放。A01–A07/F01–F16仍未全通过；Host Live切任务/隐藏关闭/多源、hover view/tooltip、离屏/Space/hidden/raw/跨appDnD、资源压力/分层p50p95与实际客户端矩阵继续，不能退化成单个Live修复。

用户授权“确认旧截图及相关 Git 历史可以公开”继续有效。仓库实际仍PRIVATE、PR#1 OPEN；此轮没有commit/push/public。当前源文本审计将按新内容刷新；可达历史未发生Git变更，旧secret扫描仍保留，不能把源文本审计误报为所有图片/许可审计。完整验收后再按合理多commit/标准PR发布，不因这次单项成功提前封版。

**R6源码审阅后的生命周期优化**：通用 `computer.live` 的start/status/hide/resume/stop若全挂UI资源，兼容Host可能在状态查询时重建组件。这是规范提示支持的风险，**未声称在当前Codex实际复现**。已分离 `computer.show_live` 一次start+render 与无资源关联的 `computer.live` 数据/控制；真实进程断言后者在任何协商状态都不带resource URI、render入口仅在HTML协商时出现。UI协议四状态重新1/0/1/0、原6hash/absences恢复；FINAL9 690/0/11/42、fmt/Clippy/release/UI5再次0；二次MCP-only原子安装和fresh进程41/43也实际0。首版实现/安装/协议记录保留，不覆盖为从未发生。


## R7：显式原生动作契约、错误可读性及多 Live 生命周期（最新）

### 延续与边界

- 用户答复是“稍后重启 Codex”，不是暂停项目。当前真实 Host 仍为40个工具、资源列表为空；R6安装MCP `9f96321b…` 保持，未强杀旧MCP、重启Codex或伪造pane可见事件。会话内渲染／隐藏恢复关闭仍未验收。
- dispatcher实际启动Native26并把原进程轮询到exit0；180秒到期前未派发输入／未启动Live。Native27原生parent31104、sheet31113分别绑定、打开关闭；两个Live分别分配，但0frames。父会话在停止子会话前已duration_expired，不能算“运行中的父Live保持”。原进程600秒timer closed／exit0，原请求终态、释放、无重放，旧绑定已detach。

### 修复一：显式 AX 动作不得丢失语义

1. **纠正初始推断**：Native27的AXShowMenu没有鼠标事件；原macOS仍从Click.value调用AX，并不是已观察到鼠标降级。AX API returned也不证明菜单可见。实际缺口是Core固定8个动作列表与macOS按值提升指针引用的契约不一致。纯Core pointer-only backend的独立baseline确实把显式AXShowMenu当普通引用并返回successful receipt；这一证明不能套成macOS实际鼠标事件。
2. `Engine::act_checked` 现在把显式Click.value保留为native action：无backend支持直接native_action_unavailable，不回退指针。macOS明确声明 `native_semantic_action_policy=advertised_element_action`；仍需确切已保留元素真的公布该动作。静态8项只保留兼容示例，不作为每个provider的完整清单。
3. `native_action_request`／`NativeMac::dispatch_input` 在输入前要求marked reference与value完全一致；移除按value把普通pointer引用提升成AX的旧分支。原CF对象／窗口／provider／epoch／modal／租约／user-yield／no-replay检查不放松，普通pointer路径保留正向可见性检查。
4. 初次添加能力字段使大json!触发recursion-limit编译101，记录保留；改为构造后赋值，原macOS同命令实际0，不用全局提高宏限制掩盖问题。

### 修复二：拒绝理由不再误导模型

- Native28（24317／31242）部署后真实AXPress把Safe count0→1，AX／原生PNG／接收方计数一致，鼠标记录0；打开sheet31255一次。AXUnknown确实未派发，但错误visibility_unknown把“不公布动作”混称为遮挡。
- `Engine::resolve_action` 拆分为unsupported_action、element_disabled和真正的pointer visibility_unknown。纯测试覆盖可见／不可见、启用／禁用、公布／不公布共8种组合。改前实际1test红101（visibility_unknown != element_disabled）；改后0。
- Native28后来120秒timer关闭／exit0，late snapshot process_identity_unknown不是活窗口适配bug；sheet safe／close未派发，不能把timer关闭当原生Close按钮验收。10秒catalog先到期、fresh discovery后才attach的真实拒绝也保留，不放宽TTL或把输入重放来“补成功”。

![28：AXPress真实消费，Safe count为1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE28_PARENT_AFTER_SAFE_1.png)

### 匹配输入、部署及完整回归

- 同名同输入Core checker：BASELINE101（错误successful receipt）→MODIFIED0（native_action_unavailable／backend calls0，普通click仍正常）→可执行ROLLBACK恢复5个原hash后同checker101→REAPPLIED0。诊断checker另按相同命令101→0→101→0，原hash在注入独立测试输入前恢复；Root产品源始终保持新版，冻结目录仅为验收，不是开发分支。之前4文件事务和首个宏编译错误均保留。
- 最终fmt、locked/offline workspace **693pass／0fail／11ignored／42suites**、all-target/all-feature Clippy -D warnings、release Agent+MCP均实际0；own example38与UI VM5独立0，不并入693，不冒充GUI。源码审计另按本轮最新内容刷新。
- 两次Agent-only部署沿用同稳定cert／DR，原inode排他租约内复核current-domain unfenced和原请求terminal/released/no pending；旧PID9721和22871先真实退出，再原子交换。最终PID36549／instance `4741ed85-d39d-4957-b521-07d0c6bb2862`，signed `f7a28ff6460858898db7ddb9438441de45286be0ef9695e72de561ab8488552f`，unsigned `070c8bd661a67be21b40ab74a155c7df2ef62442ee59b518db530d8fdfc98f2c`。四权限true，原profile/config/publicCA/cert/MCP hash不变；未读导出私钥、重配对、重新授权、清历史fence或重放输入。

### Native29：真实动作、模态范围和 Live 生命周期

1. 首次launcher因operator替换把模板指向尚不存在的29结果，真实exit1／FileNotFound；未创建fixture或派发输入。改为已完成28的metadata，新run29实际启动38988／parent31268／sheet31277，virtual4，冻结receiver二进制与4源码hash未变。
2. AXUnknown实际unsupported_action；已知disabled的AXPress实际element_disabled，均native_dispatch_entered=false。正向父按钮0→1、打开sheet1次，直接原生PNG、AX值及接收oracle佐证，mouse records0。
3. 父while-sheet blocked=true／mutation_ready=false；使用打开前旧proposal的父AXPress返回checked_precondition_changed，diagnostic.blocked_changed=true／native_input_dispatched=false。runner最初只接受checked_context_unavailable，误把**正确拒绝**算失败；保留错误记录、修正判定，不重放原请求。释放旧绑定后fresh rediscovery重新附着同一原窗口族，继续第一个尚未执行的child步骤。

![29：父窗口被自己的模态sheet阻止](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE29_PARENT_WHILE_SHEET_1.png)

4. 独立sheet Safe0→1，关闭按钮1次；原生visible=false／attached=false，父blocked=false／mutation_ready=true，父计数保持1。7个原ID均terminal／released／no pending／replay=false；两批绑定全detach，feedback visible=false，fixture timer closed／原进程实际exit0。

![29：独立弹窗收到AXPress，Modal safe count为1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE29_SHEET_AFTER_SAFE_1.png)

![29：关闭弹窗后，父窗口恢复、计数仍1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE29_PARENT_AFTER_CLOSE_1.png)

5. 父／子分配不同session与presence binding；停止子Live后父status仍running、worker_done=false，再显式停止父，两个worker终态／无pending／无image。这次补上27遗漏的**运行中父Live生命周期隔离**。没有打开viewer／伪造visibility，因此两者frames0；不是多source像素渲染、正确显示器放置、任务切换、hide/resume/close UI或画质通过。

### 剩余计划与发布

A01–A07／F01–F16、真实Host任务隐藏／恢复／Close／多源渲染、hover view/tracking/tooltip、no-raise、真人冲突、离屏／Space／hidden、系统弹窗族、raw游戏／跨App DnD、浏览器路由、资源压力与分层p50/p95均继续。保留14/19/21/24/25结果，不复放、不把本轮语义按钮或0frames生命周期当全后台鼠标／全应用适配完成。下一项继续独立hover/事件消费矩阵；用户重载后再按真实Host能力补Live。

公开旧截图及Git历史授权保留；仓库／PR状态只读取核验，仍未commit／push／public。相同四角色保留并扩展为91 managed paths，diff重构／完整rollback原hash及absences恢复／reapply／四角色reopen在证据账本记录；提交仍等完整验收后合理多commit／标准PR。

## R8：重启后实测、用户不可见问题与实际修复

1. 重启已生效：模型工具42，UI资源1，URI ui://real-world-interaction/live/v1.html。computer.show_live实际启动会话内组件；模型看不到私有_meta不意味着Host没有收到。
2. Native30仅一次新的typed hover：自有PID62060/CG31359，像素[660,374]→桌面[2154,691]。独立App/window记录同一type5/subtype0、window point[330,365]命中hover_zone；当时app active/key/frontmost=true，但view moves/enter/exit/tooltip均0。它是接收层差异的窄失败，不是“所有后台悬停不可能”。输入前已有subtype3/负窗口点的未归因事件，不归因于RWI或物理用户；final ring淘汰早期匹配记录，冻结after-hover仍保留285/286。
3. Native31用户确认组件可见并展开，采集/呈现60帧，但预览指定虚拟屏4，控制卡本身不含画面。用户随后明确点过关闭且看不到应用/窗口，不能把停止归因于API Hide，也不能把帧计数当用户可见验收。关闭后Resume不能重新启动。
4. Native32源仍虚拟屏4，仅预览改内建屏1。用户回答“能看到测试窗口画面”；读回188/188、presenter_visible/image_present=true、on_requested_display=true，矩形[1136,544,640,577]，源图1320×1104。随后233帧读到live_presenter_placement_unconfirmed，旧分类presenter_wrong_display且failure=null。没有证据表明实际移到错误屏幕，不把之后Hide/Resume回执当视觉恢复成功。
5. 修复Panel.placement_hidden / restart_placement_after_hidden：真实hide→reshow给一次新250ms出现预算，即使上次尚未确认；pending ticks不延长预算，旧确认/检查缓存失效。placement_stop_reason将未确认与真正错误显示器分开；预算耗尽保留精确自有window、requested display和inventory覆盖，cause_attested=false。确定错误屏幕仍停止清图，不放宽身份/权限/输入策略。
6. 稳定证书Agent-only部署实际0：原36549/4741…实际退出，原共享lease inode排他后原子交换；新4544/14d4ace3…、签名eb769ac8…、release2440ec0d…。DR、证书、CA、profile/config、四权限未替换，无私钥导出或TCC修改。Native33用户确认可见、74帧；API确实hidden→running/无error，但采样时presence suspended，恢复后未读到新增帧/可见面板，完整视觉roundtrip仍未接受。
7. 控制卡新增inline/fullscreen标准显示模式、原生画面位置及Close终态说明；preview_stopped停止心跳并禁用误导恢复。依据[MCP Apps显示模式声明](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/draft/apps.mdx)，不保证Host一定提供独立面板，也不等同原生Session ID绑定。
8. Root和当前插件缓存rwi-macos Skill同步8045f442…，补充源/用户预览屏独立选择及Close终态。发现并修复旧缓存仅描述viewer.url的部署遗漏；其他插件/签名/权限未改。
9. 同输入：Skill/位置时钟/UI逻辑/实际MCP复制件均BASELINE1→MODIFIED0→ROLLBACK1→REAPPLIED0，隔离复制件原hash恢复；UI是5pass/2fail→7pass→5pass/2fail→7pass。时钟checker执行真实分支/helper的纯Rust摘取，不冒充Native GUI。Core R7 101/0/101/0保留，未冒称本轮重跑。
10. 最终fmt、workspace696/0/11/42、all-target/all-feature Clippy-Dwarnings、UI7、Agent及MCPrelease实际0。独立receiver38是R7结果，不混入696、不冒称本轮重跑。
11. 新MCP安装23ce3345…；fresh零设备进程无UI41/协商43（模型42+app-only1），真实返回新HTML6807a637…，无原生截图/输入。当前Host公开resource实读旧c9ba2395…，新独立展开/终态反馈需重新加载；工具数42不证明新HTML加载。
12. Native30–33均timer关闭、原始command实际exit0、绑定退役；Live停止清图/worker_done、无pending native call、反馈隐藏，旧输入未重放。regular-app fixture启动可成为前台，不能从“未发键鼠”推断前台绝不改变。未操作用户游戏/飞书/Chrome/已有文档。

### 按顺序的实际源截图（不是Live叠层截图）

![Native30 hover之前，自有接收者原图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE30_FRESH_HOVER_FRAME_1.png)

![Native32 Live位置异常后，独立源仍可截图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE32_SOURCE_AFTER_LIVE_FAILURE_1.png)

![Native33 API Resume之后的独立源图，不证明叠层已恢复](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-10-comprehensive-native/NATIVE33_SOURCE_AFTER_API_RESUME_1.png)

完整A01–A07/F01–F16、F05 first-responder/tracking校准、F07隔离、F08/F09、F12系统窗口、F13跨应用DnD、F14、F15压力、F16分层p50/p95及公开前资产/许可审查保留。未新commit/push/public，PR#1继续复用。安装复制元数据最初误标命令事件已留原并更正installation_metadata；正式进程/同输入均有真实命令、literal stdout/stderr和exit，不把虚构命令标签当执行证据。

## R9 · 2026-10-11：后台视图接收、探针原生异常及实际 Live 隐藏恢复

1. 原接续 Native34 已真实 timer closed／exit0，没有绑定、输入或Live；保留结果，不再启动同run。R9 matching parser先实际101（拒绝新flag），修正源0、执行portable rollback原3hash恢复后101、重新应用0；初次serde/tooling错误、锁文件no-target、缺import101及路径2均留存，非native delivery错误。
2. Receiver新增默认关闭的 `Options.hover_first_responder`：专用own hover子类接受first responder、public own-window makeFirstResponder一次；`State.responder`独立读回配置/类/region/acceptsMoved/tracking数。没有修改RWI输入后端或向外部应用安装hook。依据[Apple Event Architecture](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/EventArchitecture/EventArchitecture.html)，普通mouseMoved与tracking owner不可混作同一接收层。
3. Native35自有48964/31852、虚拟4：inactive/key/frontmost均false，firstResponder NSButton。仅一次typed hover[660,374]→[2154,691]；App/Window同type5/subtype0、native[330,365]命中hover_zone，但view0。随后44秒**异常exit134**、最后JSON仍running/lifetime_request0，不伪称正常关闭。崩溃日志有NSEvent assertion；触发的具体事件未持久化，不能归因于用户或某次RWI输入。
4. 独立Rust真实public NSEvent enter/exit(type8/9)构造，window0、不创建GUI或派发输入：原subtype getter两者都实际134。修正 `DispatchEvent.subtype: Option<i16>`／`dispatch_subtype_supported`／lazy `dispatch_subtype`，tracking8/9不读非法字段而记null；仍super转发原事件一次。相同构造输入MODIFIED0→可执行ROLLBACK原3hash恢复后134→REAPPLIED0。它是观察者修复，不是新的跨进程输入transport。
5. 新冻结receiver d90ed06b…，Native36自有56151/31931虚拟4：校准accepted=true、firstResponder hover；前后inactive/key/frontmost=false。新的一次typed hover787ms，独立App/window匹配type5/subtype0、window[330,365]、真实view moved1／local[306,27]，截图也显示0/1/0。**只通过条件化视图回调**，enter/exit/tooltip此刻0。Native37同新binary、默认NSButton但active/key/frontmost=true：同点typed487ms仍只有App/Window、view0；两例不同foreground状态，不冒称完全单因素对照。Native35的642ms亦保留；三单点不是p50/p95或性能优势。

![35：默认接收者，新hover之前的原始窗口](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE35_DEFAULT_BEFORE_HOVER.png)

![36：显式校准后，输入之前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE36_CALIBRATED_SNAPSHOT_BEFORE.png)

![36：真实视图mouseMoved计数1，仍无tracking/tooltip成功](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE36_CALIBRATED_SNAPSHOT_AFTER.png)

![37：修复探针后，默认firstResponder的对照原图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE37_DEFAULT_AFTER_OBSERVER_FIX_BEFORE_HOVER.png)

6. Native36后来出现未归因的subtype3 moves及type8/9；新探针真实记录subtype=null而继续存活。最终moves115、enter1/exit1不能归为RWI或物理用户；精确RWI单次后冻结oracle仍为moved1，未被final环淘汰改写。36/37均timer closed／原命令exit0／output_error=null，全部35–37绑定detach，原请求terminal/released/no pending/replayfalse，feedbackfalse；35异常134如实保留。无游戏/飞书/Chrome/已有文档或主屏键鼠；regular-app启动可能成为前台，不从“没发输入”推断foreground不变。
7. 新Live source36在虚拟4、preview明确内建1。起初0frames/suspended；用户实际回复“新卡已展开，画面可见”后，**即时dispatcher真正执行**API status/hide/readback/resume/readback：49/49 visible/image=true，bounds[1136,544,640,577]／on_requested_display=true；hide后image/visible=false、on_screen=false；resume50→51新增帧、原生图和正确内建位置恢复，last_error=null。后台capture_pending在运行时可true，不冒称已cleanup。**通过API原生隐藏／恢复roundtrip，不等同任务切换或原生Close全通过。**
8. 随后源timer关闭，Live最终83frames、duration_expired／worker_done／imagefalse／no native pending；用户稍后回复“已关闭预览，浮层消失”被保留，但到期记录先于回复，不以此假称active Close按钮通过。原绑定已经detach，再status真实foreign_target；这是旧identity退役，非新窗口适配故障。下一有效Close样本采用更长明确lifetime，不能重放旧presence或旧窗口输入。
9. 实际 `computer.targets` 拒绝include_timings，暴露全局初始化说明的真实矛盾。纠正 `Gateway::get_info / SERVER_INSTRUCTIONS` 为只在input schema公布时使用该参数；新增永久契约测试，无字段/工具权限放宽。真实零设备MCP同命令BASELINE1→MODIFIED0→可执行rollback原23ce二进制后1→REAPPLIED0；源副本原hash也恢复。初始化说明UTF8 339→302字节，**不是输出token或模型成功率改善的统计证明**。
10. 最终fmt/locked-offline workspace **697pass／0fail／11ignored／42suites**、all-target/all-feature Clippy-Dwarnings、releaseMCP实际0；receiver41、UI7及release receiver独立0，不混入697。MCP-only真实原子安装 **5ca2d986…**，fresh process无UI41／HTML协商43（模型42+app-only1）、newHTML6807、严格schema说明实际通过；Agent14d4ace3/4544、签名eb769及publicCA/profile/config/四权限保持，没有服务重启、重配对或私钥导出。当前Host实际仍oldHTMLc9，所以新fullscreen/终态说明尚待Host重载；现有APIroundtrip无需把重启作为已通过项目的重做理由。

完整A01–A07/F01–F16继续：RWI-driven tracking/tooltip及真实client、foreground隔离/真人冲突、offscreen/Space/hidden、系统窗口族、多source/任务Live/active Close、raw/DnD/browser、压力/分层p50p95及公开前资产许可审查。保留14/19/21/24/25及R8成果，不新commit/push/public，不提前封版；同四角色91条源延续，literal commands/errors/streams、三套同输入、累计diff/rollback/hash/reapply/reopen均追加。


## 2026-10-11 R10 — 真实任务隐藏／恢复／活动关闭通过，切窗证据差异保留

- Native38：真实 MCP Apps 控制卡，没有另开 browser tab 或伪造 presence；连续只读原生回读捕捉3可见→20隐藏清图→21恢复，再33→35、38→39两组变化。UI关闭时并发 status 出现 context_busy，原 identity 复查为 requested 停止、41帧、worker_done、清图、无pending，剩余361026ms，源窗口仍running。用户确认切换、返回和关闭；这次不是到期。随后源timer关闭、实际进程0、detach。没有原生Host task-id／物理事件来源认证。
- Native39：一次全新typed5s drag真实1 down／79 move／1 up、完整蓝线、5.259s adapter pre-cache wall time。原receipt terminal/released/no pending/replayfalse、进程timer0并detach；不作性能统计。用户确认已切到测试应用，但81条原生mouse回调均inactive/keyfalse/frontPID58877（独立ps为ChatGPT）。保留双方记录，不当真人目标接管通过，不重复旧输入。
- 新增自身环境历史：初次及tuple变化单独保留128条、时间／probe tick／输入sequence，未知保持null，不增加输入、不读取其他应用内容、不激活、不证明硬件来源；采样先读取再记录，不再只留最后状态。自身application identity记录bundle/name/native policy。
- Native40：无输入且无窗口创建，真实exit1：`explicit fixture activation policy not observed`，原stdout/stderr保存。旧分支把已是目标policy的false setter no-op也拒绝。`activation_policy_ready`仅在before/after均精确等于0或1时接受此no-op；未确认的变化和错误最终状态仍拒绝。Native41新冻结.app实测before0／ackfalse／after0，localizedName=`RWI Receiver 41`，bundle id确切，虚拟4，已绑定独立窗口；真人拖拽尚待开始。不是更换生产Agent或解除权限。
- 两项匹配输入：历史BASELINE缺字段／MODIFIED三条状态（false→true→false）／ROLLBACK恢复缺字段，101/0/101/0；策略旧分支拒绝已匹配no-op／新分支接受／恢复旧拒绝，同为101/0/101/0。原3文件hash恢复、patch重建、ROOT维持修改版。首轮44与最终45 example结果分别保留，workspace697/0/11/42、UI7、fmt/Clippy-all-Dwarnings/build0。工具名称误用TypeError、缺失operator文件exit1、suite统计21/42的operator断言exit1都独立保留，不冒充原生输入失败或测试通过。
- 完整A01–A07／F01–F16保留，系统／窗口族、真实竞争输入、压力、发布审核及标准PR提交仍未封版。当前加载旧c9组件已完成上述真实UI路径；安装5ca含新6807界面，实际Host fullscreen仍待reload验证。

### Chronological actual source screenshots（原始MCP PNG，仅解码，无重绘）

1. Native38活动Live前源窗口（不是浮层截图）：

![Native38 source before Live](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE38_SOURCE_BEFORE_LIVE_TASK_CLOSE_R10.png)

2. Native39真实用户切窗协调前，目标canvas为空：

![Native39 before](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE39_SOURCE_BEFORE_HUMAN_START_R10.png)

3. 同一个目标、同一次新drag后完整蓝线；它证明后台接收，不证明真人前台接管：

![Native39 after](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/NATIVE39_SOURCE_AFTER_HUMAN_START_R10.png)

### Sources and evidence

- Apple：[Regular activation policy](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum/regular)、[running application identity](https://developer.apple.com/documentation/appkit/nsrunningapplication/bundleidentifier)、[time-varying application observations](https://developer.apple.com/documentation/appkit/nsrunningapplication)。文档支持元数据／策略语义；具体false no-op与前台差异来自上述实际进程记录，不从文档推导已切窗。
- R10 Live：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/explicit-native-action/first-responder-r9/NATIVE38_ACCEPTANCE_RESULT_R10.json`；原生回调差异：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/explicit-native-action/first-responder-r9/NATIVE39_HUMAN_TAKEOVER_DISCREPANCY_R10.json`。
- 源／回滚／完整命令证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/explicit-native-action/environment-history-r10`；四角色仍沿用原B路径、91首管文件baseline不重置。


### R10 Native41 final — 用户切换确实成功，但早于派发

- 新环境历史独立记录 ChatGPT58877→另一前台48382→目标17800，随后target active/key true；最后切入目标的sample Unix1791652498985，实际typed drag启动1791652500907，提前**1922ms**。这证明本次用户协调的切换确实发生，不把旧39差异解释成用户没操作。硬件来源仍未认证；native42不要照搬“点回复两秒后”的时序假设。
- Native41全81条mouse回调均active/key/frontmost17800，完整1/79/1、起点36,24／终点286,179；因此接受**前台gesture接收**，不接受“后台目标在drag期间转前台”。原request terminal/released/no pending/replayfalse、无fence。源在600s按自身timer退出0，detach；延迟post-snapshot已window missing，没有伪造后图／画面验收。
- 新唯一bundle与策略no-op真实配置已验，测序问题在caller协调层：下一轮Live预览中看到原生down／active-path再切，GUI动作仍从已加载typed工具发出。完整其它窗口族／工具覆盖／发布门未缩小。
- 原记录：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/explicit-native-action/environment-history-r10/NATIVE41_TYPED_HUMAN_COORDINATION_R10.json`；可复核结果：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/explicit-native-action/environment-history-r10/NATIVE41_ACCEPTANCE_RESULT_R10.json`。

## 2026-10-11 · R11/R12：执行中反馈与自主紧凑预览（按发生顺序）

1. **Native42真实问题**：原请求后台drag 1 down/79 moves/1 up/completed1、释放完成且不重放；但用户说“结束后才整条出现”。旧Runtime context gate与Executor mutex使Live presence/control排队，不是绘图只支持completed path。到期停止保留duration_expired，不冒充用户Close。
2. **R11最小架构修正**：新增`LiveController: Send+Sync`及`Context.live_gate`/精确目标controller目录；start/输入仍走原串行lane，已创建Live的status/presence/hide/resume/pause/stop走有界控制lane。Session退役会stop且旧handle不能恢复；principal/目标/auth/取消/单执行/TTL1500ms与无重放不变，控制lane不得清输入entered/fence。4 Runtime测试与1 SessionDrop回归新增通过。架构边界见[Live控制lane](../architecture/live-control-lane.md)。
3. **稳定部署与Native43实测**：签名227e0539…的R11 Agent安装、PID68509/3296be46…、四权限true。新的自有Receiver43仍虚拟4；用户看到内建预览后，唯一新请求5秒后台drag完整1/79/1。用户确认“5 秒内逐渐伸长”，实时反馈通过。另一并行模型`computer.live(status)`从401ms发起到5358ms返回，排队4957ms；UI presence能继续不等于所有MCP调用已低延迟，这一残余需分Host/adapter/transport/Agent继续诊断。

![Native43拖拽前：工具原始PNG，无编辑](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/r11StartSnapshot43_ORIGINAL_R11_1.png)

![Native43拖拽后：1/79/1与原生画线；实时逐渐伸长来自用户确认，不由静态截图推断](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-receiver-calibration/r11PostSnapshot43_ORIGINAL_R11_1.png)

4. **保留不成立的切窗验收**：43B是另一个唯一请求，不是重复43输入。完整第二条1/79/1已消费，但81个新回调仍全部inactive/non-key/前台58877。不能称“执行中人类把目标切前台”。用户此后准备睡觉，不再发人工配合问题；自动参与者的后续结果与真人来源分层，不丢失A03。42/43真实退出0、到期清图、解绑，未重放。
5. **用户补充与当前网络研究**：用户要求浮层更小、隐藏/关闭样式参考Computer Use。[Codex PiP公开问题报告](https://github.com/openai/codex/issues/32534)描述小型浮层的Close/弹出控件和圆形柔和材质，同时报告hover黑框；这是用户的一手问题记录而非官方精确规范。该页和当前官方设置页未给出窗口精确宽高，不能虚构“官方420点”。[Apple NSImage系统符号API](https://developer.apple.com/documentation/appkit/nsimage/init(systemsymbolname:accessibilitydescription:))与本机SDK提供minus/xmark/play.fill及原生圆形按钮实现。未读取私有Desktop源码/资产，未绕过此前Codex原生工具拒绝。
6. **R12实现**：`crates/rwi-macos/src/live/presenter.rs::panel_size`将max宽/高压到420、`CHROME_HEIGHT`55→48；保持真实源宽高比、边距、小屏限制与capture edge2048。`button_symbol`/chrome使用28点NSButton圆形minus/X、隐藏/关闭AX标签与tooltip，pause使用继续图标、缺符号保留文字fallback。没有新增UI流程或停止源应用的动作；非激活popover、不加外层经典三个窗口按钮。Micro可编辑[前后几何稿](./2026-10-11-rwi-live-compact-before-after.svg)只是明确标注的设计/布局意图，不是原生截图或正式像素parity。
7. **R12部署**：原R11签名227e0539…→实际c2c116f112e750b0f5afd5a925ea3c89f567dd3beeacd706a5b8ca6076d5e43d，PID94874/21e75450…。原稳定签名DR/证书/CA/profile/config/MCP5ca完全保留，四权限true、requests_permission=false。持原native domain稳定inode排他lease，输入已terminal/released才停止精确旧Agent并原子交换。未导出钥匙串/配对私钥、未授权TCC、未重启Codex，未调用recovery。旧签名二进制在独立副本执行ROLLBACK后hash完全相等，实际安装仍新版。
8. **Native44全自动可见验收**：真正由RWI返回的兼容浏览器页打开并产生自身visibility心跳，未模型伪造presence；不是MCP Apps展开的替代证据、也不是Host task-ID attestation。源PID95343/CG33074留虚拟4；预览CG33090在内建1，原生bounds `[1356,735,420,386]`、工具原图840×772、AX“隐藏”“关闭”及AXPress实际发现。没有操作游戏/飞书/Chrome/旧文档、没有截主屏全桌面。

![Native44实际紧凑浮层：420×386点／840×772原始PNG，减号和X是原生控件](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-compact-live/r12AgentPanelSnapshot_ORIGINAL_1.png)

9. **自动按钮效果与诚实诊断**：一次显式新AXPress隐藏、一次新恢复、一次新AXPress关闭。两个AXPress回执均`native_action_unconfirmed / AXError−25200 / effect unknown`；未将API错误伪装Success、未重发/改鼠标降级。独立回读隐藏=hidden/清图/原生面板离屏；恢复=running/新帧/原420×386与正确显示器；Close=stopped/stop_reason requested/worker_done/无pending/已清图。即时worker_done=false也保留，等原执行自身完成后再接受。源仍运行/safe0，说明只关闭预览；随后独立关闭自有源、native进程0、各绑定退役，临时创建浏览器页关闭，不碰用户页。API不确定仍是下一项可用性诊断债务，不用效果回读抹掉receipt。

![Native44恢复后的原生新截图：相同紧凑几何、正确源，非重用旧PNG](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-compact-live/r12ResumedPanelSnapshot_ORIGINAL_1.png)

10. **本轮同输入与回归**：R11旧busy→context_busy/101，新独立lane→保留真实capability_unavailable/0，ROLLBACK原11hash/absence恢复→101，重施GOAL→0。R12同1320×1104源/2048 capture输入：BASELINE640×576.89/101，MODIFIED420×385.89/0，ROLLBACK同原hash640×576.89/101，REAPPLIED420×385.89/0。全部只在验证副本恢复，原开发根保持新版；累计四角色仍是10/06同一组，扩展96份不可变原hash/absence。最终fmt、workspace locked/offline702/0/11/42、全target全feature Clippy−Dwarnings、releaseAgent、receiver45、UI VM7均实际0。保留真实operator错误：null mouse解析AttributeError1、证据列表误用items AttributeError1、CodeGraph未知--depth1后改有效命令，不称原生权限错误。
11. **UIOS与发布边界**：用户原始反馈按项目Micro范围记录、bounded recall0仅历史本地经验。未改既有Windows Pointer `.ui-os`草案。该旧contract的release-check/report均实际1，stale证据/不同project id/未选direction/未有release-authoritative capture等问题保留。macOS本轮只是用户授权的可逆本地开发安装与聚焦运行验收，没有独立正式fidelity review、人类审美验收、发布资格或1:1 Codex像素声明。原始native PNG不stage进Git。

**继续而非等人**：后续先跑隔离实际MCP进程并发诊断，再按自主专用窗口补其余A03/F05/F12/F15/F16；真实物理人类来源不能由脚本代替，单列未证明，不作为所有工作等待门槛。全文旧结果保留日期与失败，不重新执行旧输入。完整A01–A07/F01–F16、公开敏感/许可/审查和合理多commit/PR/推送仍未完成；未新commit/push/改仓库可见性。

## 2026-10-11 · R13：真正协议进程与loaded输入时序

1. 控制接续第一工具已读五个最小checkpoint字段并实际启动pending runner。`rtk proxy`误处理含空格的绝对可执行名，真实child退出1，stderr为`Failed to execute command: /Users/vincentadamnemessis/Library/Application: Permission denied (os error 13)`。这不是RWI/TCC拒绝；未改系统权限，只将`command[2]`改`./rwi-mcp`、cwd设原installed bin。匹配输入BASELINE1→MODIFIED0→执行ROLLBACK恢复原source hash后1→REAPPLIED0，失败原因与literal stdout/stderr全部保留。
2. 正确探针启动**真正已安装5ca MCP**，无profile、零设备，校验service healthy/targets空再发送8对并行`computer.targets`与未知target `computer.live(status)`；未知resource返回真实resource_unknown，不是假成功。16个request id都只返回一次、存在乱序关联，进程退出0、无native input/capture。第一轮样本p50 0.123ms、p95 0.272ms，是调用者墙钟与response UTF-8 bytes，不是CPU/GPT token数/有设备GUI性能，更不是公平对照或通用成功率。
3. 继续通过**当前Host已加载的typed RWI入口**，新唯一Receiver45/CG33541/PID25136在分配virtual4。新启动时原生前台已是该fixture；RWI attach未activate。截图新frame/view精确映射，唯一5秒drag `loaded-parallel-walltime-drag-20261011-45` 得到1down/79moves/1up/completed1、原生蓝线、无orphan/active残留。81个接收回调全部active/frontmost=true，因此这是前台input和诊断对照，不谎称后台或中途目标切窗。

![Native45操作前：真正Host RWI原始截图，源在虚拟屏](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-parallel/r13Snapshot45_ORIGINAL_1.png)

![Native45操作后：真正view消费1/79/1与蓝线，无编辑](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-parallel/r13AfterSnapshot45_ORIGINAL_2.png)

4. 为区分等在哪一层，先warm一次**只读**`computer.js`内的native `computer.live(status)`（9.59ms handler），不写临时JS驱动点击。再与typed drag并行发两个只读探针：JS从403ms到5407ms返回=5004ms；其真实`adapter_elapsed_ms=2.385792`、nested live=1.469917ms、worker计时2ms。直接Live从702ms到5413ms返回=4711ms。约5001.6ms发生在测得的Repl handler之外，与上游Host/tool-dispatch调度等候一致；未读私有Desktop实现，不能声称已定位哪一个私有队列。**不据此撤销R11真实Live输入lane修复，也不重构已测得很短的处理段。**独立来源的时间范围和误差保留，非端到端服务trace。
5. Live本身未start，无UI心跳或抓图，status stopped是真实既有状态，因此这组数据不证明Live画面刷新；R11用户43的逐渐画线确认仍独立保留。后台/前台不同接收条件不能混做公平p50/p95。长5秒是明确试验输入，不把期望持续时间当性能退化。
6. 只查询原durable请求，terminal/released、native pending=false、replay=false。源Close第一次缺`at.kind`在schema阶段返回invalid_arguments/elapsed0/stages[]，未forward；保留失败，不同新有效ID在fresh observed AXButton明确AXPress后关闭真实source/exit0，再detach。没有鼠标fallback、没有重放drag/Close、没有用户应用/文档输入、没有主屏截图，未启动Live或UIOS新设计。随后源失焦发生于81输入结束后，源头未归因于人类，不拿采样PID当物理人类证明。
7. 同四角色增加**一份operator脚本**，总97个源/文档/operator管理路径；不是97份产品源码。累计baseline原1040×911.44/101、modified420×385.89/0、portable rollback恢复全部原hash/absence后1040×911.44/101、reapply0另验证。最新R12单轮640×576.89→420×385.89→回滚640×576.89的101/0/101/0继续保留。R13没有产品Rust改动，workspace702/0/11/42/receiver45/UI7是R12已执行结果；本轮不称重跑或新部署，安装Agent c2c116/MCP5ca/原证书/CA/四权限保持。

下一项改为**自主、按首次真实canvas down触发的自有regular应用前台actor**，不再依赖聊天回复/外侧排队/人工数2秒。现有fixture只有launch计时，所需first-down opt-in尚未实现；先真实CLI baseline，再只在根fixture补参数、定时及状态观察，最终通过loaded MCP独立接受中途BG→FG或干净释放。自动actor不冒充Command+Tab物理人类；原14真实系统hotkey与其他34/41等证据保持独立。其余A01–A07/F01–F16/栈式系统窗口/压力/审查/多commit/PR/public仍不删，未新commit/push/public。

## 2026-10-11 · R14：自主因果触发的切窗（按实际顺序）

1. 原始checkpoint第一工具已真正执行冻存receiver，新`--self-foreground-on-first-down-after-ms`在旧版child返回1/unknown argument；未创建state。另一次完全相同纯parser断言实际101，不接受0-tests假绿。根fixture新增显式opt-in：只在running/正常regular own-window的第一条有效Canvas LeftDown上arm一次100..5000ms定时；不从聊天/启动计时推断输入开始。
2. 原三个Root源hash89131…/17dcaa…/8c359…逐份备份并保持10/06累计baseline不变；新State cause sequence/unix/event timestamp只记因果，不增加input/request、不设前台成功。现有own-app激活callback核对精确timer和running，并撤销owner-close/expiry timer。没有他应用input/global monitor/私有SPI或生产backend依赖。
3. 配套`--right-side`只是分配NSScreen里的own-window setup，保留尺寸和24pt边距，不足拒绝。47真实位于[3036,504,660,552]，目标46/48在[1824,504,660,552]，均virtual4 UUID13E4…；不覆盖目标、不借main屏、也不是新产品任意坐标移动能力。
4. Native46/69737：输入前background/non-key、actor47/69718 foreground；first-down仅arm1，普通public modern activate请求1，但81回调都仍后台。完整1/79/1/blue line正常，**未接受切窗目标**。actor请求不是系统效果，源后续真实close/exit0/detach，原动作从未重放。

![Native46输入前：自有后台source，未arm](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-takeover/r14BeforeSnapshot46_ORIGINAL.png)

![Native46完整后台画线；现代activate负例留存，不伪装切窗](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-takeover/r14AfterSnapshot46_ORIGINAL.png)

5. **Native48/70515新独立场景**：创建前明确选fixture-only legacy public activateIgnoringOtherApps(true)，不是输入失败后的自动fallback。初始AppKit inactive/non-key/后台、foreground47；唯一5秒请求`autonomous-explicit-first-down-takeover-20261011-48`在first canvas down/sequence1后arm，约1010.843ms的NSEvent timestamp偏移首次观察system foreground目标48。16回调后台、1条系统foreground但AppKit inactive/non-key中间态、64条active/key/foreground；down前台=false、up前台=true。独立环境历史在input_sequence18读到新native tuple。**此次实际继续完成1/79/1，没有中断、orphan或partial path。**时间来自event timestamp与callback采样，不是精确显示延迟；自动actor不冒称物理人类Command+Tab。

![Native48输入前：实际background](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-takeover/r14BeforeSnapshot48_ORIGINAL.png)

![Native48中途真实转前台后，完整蓝线与消费计数](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-takeover/r14AfterSnapshot48_ORIGINAL.png)

6. 原unique请求durable terminal/resources_released/nativependingfalse/replayfalse；没有把compact receipt的goal_verified=false改为true。视图callbacks+像素+native foreground变化才支持这一项业务接受；不能外推全部软件、键盘归属/全局modifier干扰或完整physical-user连续性。
7. 第一拖拽已结束后，通过**当前Host已加载computer.hotkey**在原desktop fresh expected foreground48和短shared lease下，一次`cmd+tab`，实际CGEventPost(HID)4个配对事件、paired release；native即时after仍48保留，而fresh Describe独立看到47，证明后续系统切换效果。显式release已成功、原durable terminal/released/no pending/no replay。仅源48与own companion47参与，不发用户app文本，也不声称这个键盘试验发生在拖拽执行中。
8. 旧snapshot因global shortcut失效，实际重新snapshot48后独立读它回到后台。new unique48b、1000ms drag1/16/1，18个新callback均foreground47/target background，actor未重新arm、请求计数仍1；完整第二条线、no orphan/active残留，原请求terminal/released/no pending。证明在这套已测own-AppKit条件里，切走后仍能继续新后台操作，不只是先前ACK。

![Native48经真实Cmd+Tab返回后台后的第二条新轨迹，累计2/95/2](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-autonomous-takeover/r14AfterSecondSnapshot48_ORIGINAL.png)

9. 46/48/47都从fresh observed native close button明确AXPress关闭；真实进程全部0、所有绑定detach，lease已释放，无重放/恢复/清fence。没有Live start或假presence、没有主屏全桌面capture、没有游戏/飞书/Chrome/旧文档input、没有Agent/MCP/CA/权限/签名部署变动。
10. 最终pure matching actor参数断言：BASELINE101（unknown CLI）→MODIFIED0→可执行ROLLBACK恢复preceding三个exact hashes→ROLLBACK101→REAPPLIED0；临时matcher overlay移除且原环境hash恢复。两轮full fmt/workspace/clippy-all-target-all-feature/fixturebuild/example/UI均0，最新workspace702/0/11/42、receiver49/UI7。原right helper误期望Rect.place(&self)的operator AssertionError1在写源前发生，后续fmt把shell结尾变0不当作成功；修正为真实self signature后全gates再跑。大ledger1.76MB超过exec argv导致**创建失败、无process/无exit可宣称**，改native file patch并reopen/JSON parse后保存原PNG，不编辑图。

完整A01–A07/F01–F16未被actor验收取代。A03执行中target BG→FG（机器actor）与真实系统快捷键后后台继续已补；其他foreground冲突/取消恢复、系统浮动或模态家族/Live跟随、更多client/window状态、最终审查/分commit/PR/public继续。下一步是own NSColorPanel/NSFontPanel系统原生控件接收场景；fixture接口尚未有该模式，当前只是pending baseline，不能写成实现/验收通过。不再要求用户人工配合，物理来源不明仍分层标注。


## R15：实际 AppKit 色彩／字体浮层＋document sheet（2026-10-11）

只用专用 Receiver49（PID 95099）和已加载 typed RWI；测试源位于虚拟屏4，预览明确指定内建屏1。无人工问询、无游戏／已有文档操作，也没有将终端模拟输入桥当 RWI 验收。

1. 加入默认关闭的 `Options.system_panel_controls`；真实 Colors／Fonts 按钮分别调用本测试 App 的 public NSColorPanel／NSFontManager。原生 open request 与 `window_number/visible/key/frame/display/is_sheet/sheetParent==own-parent` 分开；不替换系统 panel delegate、不预先搬动，默认缺少完整 own-window 分配则拒绝。
2. 原始色彩窗口34197实际在内建屏边缘，字体34206也在内建屏。fresh exact-PID discovery 后独立 attach，真实 `computer.place_window` 搬到虚拟屏；每项 placement/readback 都独立确认。Color首次紧跟AXPress的CG枚举尚没有新窗口，后续fresh只读枚举出现；未重放打开按钮。
3. Fonts遮住Color上部时，精确Color的AXPress切换 Color Sliders，原始图片与 Gray 控件独立证明真实页面变化，不把回执当goal。两个浮动系统面板不是document-modal；父窗口 Safe count 实际0→1。
4. 开启真实sheet34237，父窗口 `blocked=true`；对父计数的负例在native dispatch前拒绝，计数仍1。独立sheet target `blocked=false`，实际 Modal safe count 0→1，再Close sheet，原生 `visible=false/attached_to_own_parent=false/close_actions=1`。
5. 原始父预览420×386；Color独立预览252×420，均在内建屏、原图细节没有因缩小面板而降低采集分辨率。父预览只含父窗口，sheet／其他浮层并未自动合成，这是现有功能边界，**不是全层级Live完成**。
6. **新复现问题**：Color Live 首次 `window_missing` 后长期 paused；later同PID枚举实际仍能看到来源。一次明确的只读 Resume 即采到同一个34197，未重新bind／focus／输入。这说明临时丢失的自动恢复仍待修复，不能称永久关闭或已修复。父Live实际因duration_expired终止；Color实际requested stop，worker_done、清图且无pending，两者原因不能混淆。
7. 关闭验收IAB页后，App inactive／非frontmost；AppKit色彩、字体 `isVisible=true`，但on-screen CG枚举只剩父窗。Font close因此native dispatch前拒绝，未重发。最后明确关闭own父窗，fixture自身清理它保留的两个shared panels：visible=false，实际进程exit0，7个attachment全部detach，12个原始mutation全部terminal/released/no replay。
8. 另发现字体树约1.5s，默认输出包含大量无名、视口外AXRow；limit8确实减输出，但不减少原生walk成本。后续继续做树／输出优先级优化，不能宣称耗时已解决。两个真实CUA兼容IAB页也可能同时发可见心跳；这不是Codex任务切换绑定的证明。

### 原始截图时间顺序

- [打开前的专用窗口](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15BeforeSnapshot49_ORIGINAL.png)
- [系统色盘移到虚拟屏后的原图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15ColorSnapshot49_ORIGINAL.png)
- [独立字体窗口原图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15FontSnapshot49_ORIGINAL.png)
- [被字体遮挡的色盘仍切到Sliders](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15ColorSlidersSnapshot49_ORIGINAL.png)
- [独立模态sheet原图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15SheetSnapshot49_ORIGINAL.png)
- [父窗口Live明确不含另外的sheet](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15ParentPreviewSnapshot49_ORIGINAL.png)
- [Color来源丢失的真实占位状态](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15ColorPreviewSnapshot49_ORIGINAL.png)
- [明确Resume后的紧凑Color预览](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-system-panels/r15ColorRunningPreviewSnapshot49_ORIGINAL.png)

纯匹配输入 BASELINE101→MODIFIED0→执行ROLLBACK后的同输入101→REAPPLIED0；preceding三个源文件hash全部恢复，root最终仍为modified。六项gate均0，workspace702/0/11/42、Receiver51、Live UI7。Agent/MCP/配对CA/TCC均未替换。

权威原始记录：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/system-panels-r15/SYSTEM_PANELS_ACCEPTANCE_R15.json`、`TYPED_NATIVE_EVENTS_R15.json`、`GATES_R15.json`、`SAME_INPUT_RESULT_R15.json`。所有A01–A07/F01–F16仍保留，未完成PR／公开步骤不冒充完成。

AppKit公开API：[shared color panel](https://developer.apple.com/documentation/appkit/nscolorpanel/shared?language=objc)、[shared font panel](https://developer.apple.com/documentation/appkit/nsfontmanager/orderfrontfontpanel%28_%3A%29?language=objc)。同PID不是父子关系证据；本例provenance只来自明确触发的own framework返回对象与原生sheetParent读回。


## R16：来源暂失的自动恢复（2026-10-11，已部署）
旧版实际Color临时missing后永久paused。新增Mode::Recovering，只对exact-source的window_missing／visual_window_changed／capture_binding_changed有界只读重观察（8次失败，1s退避）；原PID／process／session／window／geometry、native pending与presence检查保持，权限／身份／capture timeout／display错误仍暂停。不会自动focus、unhide、换来源或重发输入，不是任意隐藏窗的后台采集。

稳定Agent部署：PID26064，instance b4e398bb-4d5b-4bd5-ba67-5e45447bdfa3，签名226c1675a3481b156e967b544e87d2c5cb2a990dbcc3146f23b9718d57c020b3；MCP/CA/profile/本地证书不变，四权限true，无Codex重启／人工問询。

1. 第一实际用例失焦Color34733 recovering attempts1–4，持续missing达到8后paused/exhausted，没有无限轮询。旧view因focus变化失效，首次drag在native前拒绝；独立freshview/newrequest52b才1/79/1、16BG/65FG、own actor1。当时预算已耗尽，故不算自动恢复通过。
2. 第二独立用例显式Resume仅在来源丢失之前reset。caller focus52→53，Color实际recovering/window_missing/attempt1；caller focus53→52恢复own app。之后无LiveResume／rebind，worker自动running并再采同34733，frames90→92→96、attempts0。应用前台恢复是明确caller操作；自动恢复是截图恢复，不混称自动激活。
3. own50/51/52/53全部close进程exit0；6bindings detach、12原始mutations terminal/released/no replay；Live52 requestedStop/worker_done/清图/no pending。Live50expiry不冒充UIClose。真实兼容IAB5/6已清理，user tabs未操作，未伪造presence或Host任务ID。
4. 同一DR完整.app copy验证BASELINE0／MODIFIED0／执行ROLLBACK后0／REAPPLIED0，恢复原c2c116..hash再应用226c167..；production未回滚。最初将Mach-O移出.app时codesign报Info.plist错误，原记录保留；改在真实.app context后通过，不是生产授权失效。
5. Source同输入101→0→rollback101→reapply0，原三个hash恢复；7gates0（workspace708/0/11/42，Receiver51，UI7，release build0）。AXWindows -25204一次真实read failure、下游缺frame的invalid_arguments、catalog_expired selection拒绝与derived callback字段错误均保留并纠正，未作为input重试。

[拖拽后原始图](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-source-recovery/r16AfterActualDragSnapshot52_ORIGINAL.png)。动态轨迹与权威结论：/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/source-recovery-r16/TYPED_NATIVE_EVENTS_R16.json；/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/source-recovery-r16/LIVE_SOURCE_RECOVERY_ACCEPTANCE_R16.json。

[Apple hidesOnDeactivate](https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate)规定NSPanel默认在应用inactive时移出屏幕；R15未直接读own panel该属性，对该case仍为机制推断。[SCK筛选API](https://developer.apple.com/documentation/screencapturekit/scshareablecontent/getexcludingdesktopwindows%28_%3Aonscreenwindowsonly%3Acompletionhandler%3A%29?language=objc)明确onScreenWindowsOnly的作用，不把“不可采集”解释为已销毁。

parent/sheet/float composite Live、真正Host/MCPApps任务绑定、Font输出排序／原生walk效率仍继续；完整A01–A07/F01–F16和标准PR/公开目标未缩减或完成。


## 2026-10-11 R17–R18：控件摘要与小型 Live，自主实机验收

- 修改：MCP 的 `compact_controls` 默认折叠无名且不可见/遮挡的无状态容器，匿名关闭按钮不折叠；`compact_controls_in_order` 保留完整分页顺序。坐标、原生动作、总数和 partial coverage 保留。真实 MCP 子进程的65项夹具：默认宽页只显示1个关闭控件，完整16项分页仍返回65项。此项是 transport/formatting 测试，不是新格式的 Host GUI 验收；AX walk 耗时没有因此改善的证据。
- Live 的 `panel_size` 最长边上限360点；纵向来源可缩到约200点宽，保持比例。`preview_control_icon` 用原生 `eye.slash`／`xmark`／`play.fill`，可访问名称与 tooltip 明确“预览”，28点点击区域不缩小。没有复制 Codex 私有资产，也没有修改源截图的采集精度。
- 官方参考：[Computer Use 使用说明](https://learn.chatgpt.com/use-cases/use-your-computer-with-codex)说明macOS画中画可打开／移开；[Apple Panels](https://developer.apple.com/design/human-interface-guidelines/panels)建议面板通常不提供最小化按钮，[Apple Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons)建议以符号／文字表达用途、保持同组尺寸一致。官方未公开 Computer Use 预览固定尺寸，360点是本产品有界设计，不宣称1:1。原文与联网错误记录在 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/small-live-r18`。

### 按实际发生顺序

1. **基线**：同一自有 Receiver54，PID65752／CG34996，源在虚拟屏4。旧 Agent 原生回读 `[1356,735,420,386]`，来源1320×1104像素。

![旧版实际预览：420×386点](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-small-live/r18BaselinePreviewCapture_ORIGINAL.png)

2. **部署**：沿用原证书与 designated requirement，排他 cooperative lease 与全部 durable input 结束检查后原子换 .app。新版 Agent67302／ad37bc3a…，签名5fe8b595…，四权限仍为true；CA／profile／证书未变，不要求用户重启Codex。
3. **新版同源**：相同PID／CG／窗口660×552点，采集仍1320×1104。预览实际 `[1416,785,360,336]`，面积减少25.39%，原始PNG720×672。隐藏图标和×可见；两按钮AX名称为“隐藏预览”“关闭预览”。

![新版实际预览：360×336点，隐藏与关闭图标](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-small-live/r18ModifiedPreviewCapture_ORIGINAL.png)

4. **真实按钮自主验收**：通过当前加载 RWI 对其自身预览的原生AX按钮各发一次唯一请求。两个AXPress均返回-25200／uncertain，不能改写为API成功；独立只读 Live 回读分别得到 hidden／清图，再由明确API恢复为running，最后 stopped／requested／worker_done／无pending／清图。原请求已查回，全部 released，未重放或降级输入。
5. **源与清理**：关闭预览后源仍running／Safe count0／drag0／text changes0，随后独立关闭源，实际进程exit0，非timer到期；4个绑定detach。源环境记录有App active/key变化和1个非绘图区mouseMoved探针，因此不宣称完全无事件或系统前台恒定。自有IAB页7、8关闭，用户原页未动；这只验browser-panel可见性，不冒充真正Host任务ID绑定。

![预览关闭后仍运行的源；随后才单独关闭](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-small-live/r18FreshSourcePreCleanupSnapshot54_ORIGINAL.png)

### 同输入、回滚与范围

- R17原代码同输入关闭优先级断言失败101 → 修改0 → executable rollback副本恢复原2hash／同断言101 → 重应用0。首轮错误包／缺SDK／分页limit128错误与Clippy格式诊断均保留，修正命令，不归咎于TCC。
- R18同输入：420×385.89／252×420／420×233.82，断言101 → 360×335.71／203.2×360／360×206.23，0 → 原3依赖hash精确恢复／101 → 重应用0。签名 `.app` 副本的相同DR验证 BASELINE/MODIFIED/ROLLBACK/REAPPLIED 全0；生产未回滚。
- R17七门全0，workspace714；R18七门全0，workspace716／0failed／11ignored／42suites，Receiver51／UI7，fmt与Clippy全targets/features `-D warnings`。新MCP69fa4e26…已单独部署；同initialize/list/targets输入旧／新／副本回滚／重应用全exit0／41tools，没有Agent再重启。当前父会话未重载MCP，所以其新格式不算本会话实机通过。
- 不再安排人工参与；专用fixture／机器切换演员的结果与真人来源证明分开。真正Host/MCPApps任务切换、多窗族合成、其余前后台冲突／取消／资源恢复、完整F01–F16、PR拆分提交／推送／公开审核仍未完成，保持A01–A07。
- 原始证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/small-live-r18/SMALL_LIVE_ACCEPTANCE_R18.json`、`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/control-priority-r17/SAME_INPUT_RESULT_R17.json`；沿用同四角色与原97 baseline，不替换历史结果。


## 2026-10-11 R19：自主前台冲突、系统切换、取消和恢复

1. **执行中前台→后台**：current loaded RWI 对虚拟屏4 own56（PID92593/CG35390）发一次5秒drag。own57只显式启用自身3秒activation timer；实际第27回调在down后1642.49ms变为后台，26FG/55BG；同CG35390接收1/79/1、无orphan、完整蓝线。启动时间不代替native回调；这是机器自有窗口演员，不是物理真人来源证明。

![执行中退到后台仍完成的原生蓝线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-foreground-conflict/r19AfterConflictSnapshot56_ORIGINAL.png)

2. **系统Command+Tab**：拖拽结束后，原始desktop typed global hotkey，在own57的精确前台tuple下只发送4个paired事件。即时receipt仍显示57，不当作goal；随后独立Describe实际own56/CG35390。此项不是中途键盘介入。
3. **中途取消**：另一个唯一5秒drag，通过1500ms bounded JS deadline（相关原生claim/act/cleanup编排，不是临时单击包装）。原生1340ms后停止移动，新增1/20/1且只补自身up；短蓝线终点不同于完整终点。`completed`只表示receiver收到了up，不表示完整5秒goal。原请求uncertain/fenced、terminal/released/no pending，无自动重放。

![取消后仅保留真实短线，不补做剩余路径](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-foreground-conflict/r19PartialCancelledStrokeSnapshot56_ORIGINAL.png)

4. **显式安全恢复**：确认writer idle、两表0键0按钮后reconcile，input_sent=false，无自动release事件。旧cancel result逐字JSON相同、旧fenced=true/no replay；当前domain=false。没有修改旧结果或重发目标输入。
5. **过期前台tuple拒绝**：own56的timer随后真实退出0，系统前台own57。旧56 tuple globalhotkey被foreground_changed、rejected_before_dispatch，native_entered=false；own57保持前台。此case还包含旧窗口过期，不能冒充“两个活窗口只切前台”的隔离case。
6. **新发现**：own57在后台时AXWindows两次-25204／约0.5秒读失败，但同PID/CG/几何的SCK只读图像成功；后来57前台AX snapshot成功。没有自动focus或把visual-only授予输入；一次准备新return chord因56不再是前台，在claim/input之前被operator guard停止。Apple SDK `AXError.h` 把-25204定义为通信失败或目标busy/unresponsive，不是attributeUnsupported，也不证明窗口已销毁；目前不能证明根因。公开网页/Markdown404错误、本机SDK和既有Hammerspoon参考均保留。
7. **收尾**：own56是lifetime expiry（不是按钮关闭）；own57是独立AXClose、实际进程0，未到期；2detach，5个原操作查回terminal/released/no replay，产品Agent/MCP未再次部署。Derived assembler误读timestamp字段，保留KeyError，改用原生 `mouse.timestamp_seconds` 后0；原始input没有重放。

完整A01–A07/F01–F16未关闭，继续AX只读可视降级、活窗口前台guard/并发与其余Live/窗口族及发布门；无人配合要求。原始证据 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/foreground-conflict-r19/FOREGROUND_CONFLICT_ACCEPTANCE_R19.json`；同四角色继续，代码候选与签名不因这轮native测试改变。


## R20/R21 · 当前补充：错误仍可见，输入权限不从截图推导

- 当前：同一 RWI 产品根 / workspace **722/0/11/42**、receiver51/UI7、七项门禁0；MCP **4a29eebf…** 已安装 / 继续全 A01–A07/F01–F16、分层延迟与窗口族，非整计划封版。
- 用户休息后不再提人工配合/重启问题。Live 的 R18 原生实测仍有效：同源 **420×386 → 360×336**，面积减少25.39%，捕获1320×1104未缩；隐藏 eye.slash、关闭 xmark、暂停恢复 play，28点命中区域与中文无障碍标签保留。公开资料参照不等于 Codex 私有界面1:1或公开精确尺寸。
- R20 只改 MCP 的 snapshot 错误分支与测试：明确 AXWindows/-25204/read、原读取终态/资源释放/无 pending/无清理不确定/无输入派发时，才单独尝试一次当前绑定窗口的 capture_visual。原 AX 错误和 isError=true 保留；控件数未知，不说不存在；frame/view/mapping 为空，没有点击权限、自动聚焦、重绑或重放。视觉错误/悬挂/错误绑定附带的媒体不会泄漏。
- 实际 MCP 进程 + 认证合成 peer 的两项测试0，正例1张fixture PNG、poison0张；这是协议/输出策略，不是原生 GUI 图像验收。匹配源行为 **101 → 0 → 可执行ROLLBACK原2hash → 101 → reapply0**。最初mock分支漏匹配导致101已修复，原失败保留。七门禁0；新旧/回滚副本/重应用进程启动均0、41工具、只读targets健康。
- Agent仍67302/ad37/5fe8…，同DR/证书/CA/配对/权限；安装未重启当前父MCP，因此本会话仍旧formatter，不能冒称Host已使用R20截图降级，也不再要求用户重启。
- R21 只用loaded typed RWI操作虚拟4的Receiver58/59：两个窗口仍存活时，先58→59实读切窗，再用旧58前台tuple请求cmd+tab，原生派发前拒绝foreground_changed，59保持前台；补上R19“旧窗口已过期”造成的隔离缺口。旧focus引用拒绝no-dispatch保留；新鲜独立观察后另立请求正常切窗，不重放旧请求。
- 两种精确租约边界均实际拒绝：desktop租约不能拖拽child，child租约不能发desktop全局键。第一对中的drag已拒绝，cmd+tab仍真实58→59，**不算拖拽期间切窗**。新目标租约上的5秒drag实际1/79/1、81FG回调、原生时间4986.726ms/完整蓝线/无orphan；其间发起的错域chord无native dispatch，controller等待4583ms。等待层未测明，不能归咎某个私有Host组件，也不删除租约或durable reservation绕过。
- 12个原请求都查询到terminal/released/no pending/replayfalse；两自有源明确AX关闭且进程0（非到期），2解绑，反馈指针隐藏，11张实际交付PNG按序保存。R21未启动Live，不把“存活窗口”误说成Live预览/真实人类输入。未新commit/push/public；完整窗口族、framework/stress、审查与发布仍继续。


### R21 实际交付截图（按原调用顺序，未编辑）

![007-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/007-computer-snapshot-1.png)

![015-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/015-computer-snapshot-1.png)

![021-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/021-computer-snapshot-1.png)

![032-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/032-computer-snapshot-1.png)

![038-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/038-computer-snapshot-1.png)

![045-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/045-computer-snapshot-1.png)

![047-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/047-computer-snapshot-1.png)

![053-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/053-computer-snapshot-1.png)

![060-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/060-computer-snapshot-1.png)

![061-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/061-computer-snapshot-1.png)

![063-computer-snapshot-1 · loaded RWI交付预览，不冒称native full-original](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-live-window-isolation/063-computer-snapshot-1.png)

证据：/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/degraded-observation-r20/SAME_INPUT_RESULT_R20.json、/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/degraded-observation-r20/GATES_R20.json、/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/degraded-observation-r20/deployment/DEPLOYED.json、/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/live-tuple-and-keyboard-r21/LIVE_WINDOW_ISOLATION_ACCEPTANCE_R21.json。每个command_execution保留命令、输入、stdout/stderr、exit；四角色继续同一97个管理路径，原hash/absence不改。


## R23/R24 · 开源预检与独立Computer Use actor：实际结果

## 2026-10-11 · 当前 R23/R24：开源预检有结果，架构门禁真实失败，切窗actor负例保留

- 当前：同一 RWI 根 / Gitleaks全部231提交及当前源码目录0，72初筛逐项排除；SPDX许可证0/2warnings、JS client50pass；README已对齐实际41/42工具、Rust/QuickJS与Native Live / 继续19条架构问题、完整原生验收和PR/多commit，未开源或推送。
- 初筛1741历史blob，128 binary与3 overbudget不做四regex秘密不存在证明。实际Gitleaks默认规则、无baseline/allowlist/忽略注释的全history0；crates/integrations/architecture/tools/docs及README均0。两条只有header的非法PEM测试字符串无密钥正文；70条为Microsoft Learn ask-learn属性名子串，均逐条检查。未读取生产密钥、未调整原GPL-3.0-only；license inventory通过不作法律保证。
- 额外执行真正architecture checker：exit1，19项，不包装成通过。包含convenience2191>600等文件责任边界、旧baseline membership以及pointer依赖规则问题；应评审和重构，不调高阈值骗绿。已准备最大convenience纯模块提取runner，未执行，不计作实现完成。
- R24原本准备“RWI拖拽期间独立系统Cmd+Tab”actor。Computer Use仅选虚拟4自有Receiver64：super+Tab返回但native前台仍64，焦点Colors→Fonts；新Mac literal CMD+TAB报keyNotFound CMD。这些不是全局切窗成功，也不是RWI拖拽故障；未发RWI drag、不试第三种猜测键名、不重放任何RWI请求、不碰Codex native denial/用户应用。
- Computer Use自己的Window sharing菜单实际提供Close Window；只关闭own64，native状态closed/exit0/non-expiry。own65通过RWI fresh observed AX close也closed0；3原RWI请求terminal/released/no pending/replayfalse、2detach/反馈指针hidden。对照PNG按实际调用顺序保存，不把下降采样预览说成native full-original。实际during-input系统keyboard/物理人类覆盖仍未证明；R14/R19原生状态转换与独立全局shortcut结果保留。
- README旧的20/21工具、JS未部署、plugin未安装陈述已更正，明确开发候选、未知范围与全部未完成项；同输入1→0→可执行rollback原READMEhash→1→reapply0。原97 baseline每个entry不变，第一次管理README为新增第98路径，同四角色扩展而不重置原件。


![005-computer-snapshot-1 · RWI实际交付预览](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-cua-actor-comparison/005-computer-snapshot-1.png)

![012-computer-snapshot-1 · RWI实际交付预览](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-cua-actor-comparison/012-computer-snapshot-1.png)

![018-computer-snapshot-1 · RWI实际交付预览](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-cua-actor-comparison/018-computer-snapshot-1.png)

![020-computer-snapshot-1 · RWI实际交付预览](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-cua-actor-comparison/020-computer-snapshot-1.png)

Gitleaks/许可证、原生与工具错误分别留证：/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/publication-audit-r23/PUBLICATION_PREFLIGHT_RESULT_R23.json；/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/system-command-tab-r24/COMPUTER_USE_COMPARISON_ACCEPTANCE_R24.json。完整A01–A07/F01–F16保留，不因预检通过或actor失败缩减。


## 2026-10-11 R30：无需人工配合的读取恢复、后台输入、前台切窗与紧凑 Live

只操作自行分配的 Receiver 68/69（虚拟屏4）与 RWI 自己的预览，不操作用户游戏、文档或真实会话。新的Agent已经部署，当前加载工具已重连；证书、配对、MCP和四项权限未换。本轮完整八门全0，更新provider夹具后 workspace **723通过／0失败／11忽略／42结果组**，独立Receiver53测试、全targets/features Clippy0。Baseline／Modified／Rollback源合约为 **1／0／1**，回滚精确恢复此前两源hash并删除新helper，随后重新应用为0。provider源也为1／0／1／0；签名完整.app副本同DR为0／0／0／0，不在生产服务重放或回滚输入。

### 按实际发生顺序的原始截图

1. 普通provider的68基线：Safe0、无拖拽，图片1320×1104。

![Receiver 68基线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-ax-relay/01-native68-baseline.png)

2. 69自己的公开AXWindows getter延迟120ms，确实被系统调用；RWI实际收到-25204，再经type/PID/CGWindowID/role/rect严格核对恢复AXMainWindow。原读取错误仍在coverage里；没有聚焦、换窗或重发输入。

![真实AX读取失败后的精确同窗恢复](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-ax-relay/02-native69-actual-AX-timeout-recovered.png)

3. 69后台AXPress一次：独立receiver和像素均Safe0→1。

![后台Safe1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-ax-relay/03-native69-background-safe1.png)

4. 一条新的1秒后台拖拽：down1／move14／up1、completed1、orphan0；16条mouse callbacks均inactive/keyfalse/system-background，实际蓝线确认。强制慢provider的该次receipt总耗时1696ms，不当作普通生产耗时或语言性能优越性。

![后台实际蓝线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-ax-relay/04-native69-background-blue-stroke.png)

5. Live原生浮层：实际360×336点、720×672像素，源1320×1104不降质。28点圆形eye.slash隐藏、×关闭，可访问label/tooltip区别“停止预览”与“关闭源窗口”。图像由loaded RWI仅捕获自身浮层，原PNG未缩放、未重编码。

![紧凑Live实际原生界面](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-ax-relay/05-live-compact-native.png)

RWI明确focus69后，独立原生foreground/receiver都确认69；再以原desktop的cmd+tab发送4个平衡事件，独立targets/两receiver证实69→68。回执内即时after仍69是异步切换的采样，不代表最终未切换。操作中的真实用户键盘介入、全局键盘与已持租约drag的调度仍不是本轮通过项。

### 没有掩盖的缺口与清理

- 同一69真实attached sheet（独立CG id）在慢provider下仍candidate0；parent阻断在native dispatch前拒绝。CUA同一时间能读到sheet并只关闭该专用sheet，作为明确差异，不计RWI通过；下一轮将依据公开AXTopLevelUIElement语义补齐，不借父窗像素或几何猜配。
- Live兼容页面按钮与API的hide/resume都确实清图、恢复。新MCPApps卡未挂载（tabs=[]）；使用真实显示的临时IAB页面自主复验，没有伪造presence、task id或人工确认。Native按钮的CUA模拟click未观察到动作，也没有callback entry证明，不能归因或声称通过。页面关闭观察时Live已经duration_expired，关闭触发与到期无法分离，保留未验；terminal resume仍stopped。
- 68显式AX关闭，进程0/expiry0。69的新不完整RWI和CUA树都未列出关闭控件，未猜坐标，最终600s自身到期close0/expiry1。8条原输入全部terminal/released/no pending/no uncertainty，4个独立target退休，pointer隐藏，临时Live页关闭；原错误/请求不可变，绝未重放。
- 首次把已签名二进制移出.app直接做codesign回滚验证，实际返回invalid Info.plist；保留错误，改用完整独立.app副本，相同DR四次均0、旧hash恢复准确。它是验证器的bundle上下文错误，不是生产证书或TCC失效。

[OpenAI官方说明](https://learn.chatgpt.com/use-cases/use-your-computer-with-codex)确认macOS后台画中画观察和可移动预览，但已打开的文档没有精确像素尺寸、按钮样式规范；360点、系统符号与控件布局是RWI自身实现与实测，不声称1:1复制私有Codex实现。[Apple AXTopLevelUIElement语义](https://developer.apple.com/documentation/applicationservices/kaxtopleveluielementattribute)区分window/sheet/drawer，不能把AXWindow当作sheet身份的等价物。

证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/exact-ax-relay-r30/NATIVE_ACCEPTANCE_RESULT_R30.json`。同四角色延展139条，原97 baseline不变；完整A01–A07/F01–F16、架构12债务、PR/push/public未完成，本轮不封版。


## 2026-10-11 R31：真实慢provider的精确sheet恢复，原生Live Hide／Resume／Close

新版Agent已签名部署，当前loaded RWI重连新实例57562／4be7…／ff54…。八门全0：workspace **724通过／0失败／11忽略／42结果组**，Receiver53、UI7、全targets/features Clippy0。证书／CA／profile／MCP未替换，四项权限有效，无人工请求。

### 实际发生顺序（原PNG保持字节、尺寸和hash）

1. parent70原图：CG37665、Safe0，无用户文档。其AXWindows真实-25204时仍经精确main alias恢复。

![父窗基线](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-top-level-sheet/01-parent-baseline.png)

2. RWI打开自身document-modal sheet，独立CG37670。只读的新`AXFocusedUIElement→AXTopLevelUIElement`链找到真正sheet，不使用代表enclosing parent的AXWindow；窗口type/PID/CGID/role/rect完全对应，原AXWindows-25204仍在coverage，attrs4／candidate1，图片720×300。

![精确sheet恢复后的Safe0](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-top-level-sheet/02-exact-top-level-sheet-safe0.png)

3. 父窗旧Safe提案native dispatch前被拒绝，父值保持0；对子窗AXPress一次，独立接收记录和图像都Safe0→1。关闭sheet1次，visible=false／attached=false；原provider getter确实调用29次，非请求参数假装错误。所有NSButton回调记录inactive/system-background；window-key只在独立最终readback为false，回调没有该字段，不扩张断言。

![子窗Safe1，父窗隔离不变](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-top-level-sheet/03-exact-top-level-sheet-safe1.png)

4. 原生浮层自身的隐藏与关闭按钮由loaded RWI各发AXPress一次。实际Hide清图且hidden，随后API Resume恢复同360×336；Close使requested／stopped／worker_done／clear／no pending，源窗口继续存在。自己的AX回路两次返回-25200／uncertain，但独立效果已经看到；不改回执、不称native ACK、不重放。旧CUA模拟no-op不足以判定按钮损坏；日常模型控制自己的预览优先`computer.live`。

![真正的原生Live按钮与画面](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/validation/2026-10-11-rwi-exact-top-level-sheet/04-native-live-controls.png)

### 仍不通过／未证明的范围

新MCPApps组件未挂到可控tab（tabs=[]）；public navigation返回navigated=true，但浏览器open仍queued，导航另一参考任务后原生预览仍visible。导航ACK不是实际渲染任务切换、当前任务ID或窗口可见性的证明，因此**不把强任务绑定验收标通过**，不借其他通道绕过Codex原生应用访问限制。只浏览自己的临时Live页面，未读参考任务内容；IAB15已经关闭。

完整F12的多源／owned窗口层叠画面、foreign-PID system family仍待；接下来启动另一独立自身fixture71实测并实现，不借不同窗口像素授权输入。F13跨窗DnD、F14浏览器及其余F01–F16／A01–A07保留。真正用户物理来源和正在drag时全局键盘介入仍不冒充机器actor证据。

source70最后单独AX关闭，进程0／expiry0。7条原mutation全terminal／released／no pending／no uncertainty，3targets退休、pointer隐藏；Live的两条uncertain receipt与独立goal分别记录。源码同输入BASELINE1／MODIFIED0／ROLLBACK1（原2hash）／REAPPLIED0；完整独立.app同DR四次0，不回滚生产、不重放GUI。139条延展同四角色，原97baseline不变；暂不commit／push／public或宣称全目标完成。

Apple语义已复核本机Xcode SDK原始`AXAttributeConstants.h`，AXWindow572–587／AXTopLevel590–605；web页需JS，实际markdown404也保留，非只凭搜索snippet。证据：`/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/exact-top-level-relay-r31/NATIVE_ACCEPTANCE_RESULT_R31.json`。

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
