# 完整实施计划：当前状态

更新：2026-09-14。以已批准 ADR 0002 和 requirements-and-acceptance.md 为准。
用户临时发现的问题只改变优先级；提交、推送、一个成功截图均不是总完成标准。

## 正在执行的顺序

| 顺序 | 交付项 | 当前状态 / 下一步 |
|---|---|---|
| 1 | 焦点变化、窗口切换、重新接续 | 已补单次显式激活、异步完成/用户退让/最终事件收集、独立前台事实、旧观察失效；再补最小化恢复与前台分步，真实Host约105ms恢复Explorer。窗口展开仍未获得精确控件焦点时，显式点击地址栏后才发键。整Paint进程关闭、服务仍健康、GUI启动新Paint并重开原图均已验证。真实人工介入与异常窗口重用仍单列验收 |
| 2 | 语义动作性能 | 精确 runtime ID/祖先重验已落地、公开负向回归及 Paint/Explorer/文本实测通过；旧版仅一个有效基线，不声称完整 A/B。公共 observe 仍有 UIA 遍历延迟，继续第 3 项 |
| 3 | 可视化与操作通道分离 | capture_visual 已由重启后的真实 Host 加载并实测，不产生input frame；新版面积平均1024px预览保留本次细线，原生512px切片SHA升级前后相同。ROI source_region重验已实机画出第三个闭合菱形。公共语义 observe 仍需缩减全树遍历开销 |
| 4 | 连贯多步操作 | withInput 已补同target多wrapper独占、单调调度预算、重复ID/dedup停止、在途读/输入与异常留证；真实三步工具按钮批次约6.54s，另已用一个Host原生MCP JS单元完成两笔640ms轨迹，整批约5.33s，原生512px切片确认两个闭合菱形。缩略细线已修且实测；菜单focus/pointer身份表示继续修正，不称效率完全达标 |
| 5 | 完整真实验证与简洁交付 | 重启后真实Host已加载45ae版；agent重启后的首次查询自动单次重连成功，原操作记录未变。1200ms截止后的原生停止/一次租约清理成功，显式恢复仍保留不确定历史。最小化修订独立393默认/480全feature门禁通过并实测，无需再重启Host。原Paint PID2024已退出，新PID7372由GUI重开原图；文件与512px切片SHA均不变。另按新发现的窗口尺寸重绑，3次观察约712/591/616ms，只读截图约140ms，不算跨版本A/B。截图36步，历史失败全部保留 |
| 6 | Mac 受控端 | 通用 agent 装配、无选窗身份、动态CG发现/绑定、AX/CG输入与取消/回调资源已实现，独立默认424/全feature511通过；实际无窗口身份预检通过。R1长文本/双击自身状态变化导致中断的问题已审出并修复。本机用户级开发版已部署：新私有配对（agent-mac-v1）、loopback mTLS 连通、运行进程 accessibility/screen_capture/post_event/listen_event 预检为真，证据见 work/macos-deployment-20260914/；Host 原生 GUI 验收待做；Pointer/drag/ROI/capture_visual/lifecycle等仍未实现，详见本批验证文档 |
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
