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
  （computer.launch_application/lifecycle_preflight/lifecycle_receipt，19 项不变）；rcu-core
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
  （自洽校验），rcu-mcp 网关侧未发现对实时工具目录的数量/哈希断言。
- **施工清单（按序照单执行，每步一编译）**：
  1) `crates/rcu-core/src/lib.rs`：新增引擎方法 `move_window(target, lease, reference, display)` ——
     参数借 `Resolved`（元素/像素接地引用）+ 目标显示器（`built_in` 或 `id`）；语义=原生移动，
     含尺寸预检、失败 fail-closed、持久台账（复用 act 的 journal 路径与 request_id 去重）。
  2) `crates/rcu-macos/src/native/metadata.rs`：已具 `place_window_on`（含 `placement_impossible`
     预检）→ 仅需暴露一个"按显示器 id/内置判定"的包装与 `Display` 查找。
  3) `crates/rcu-windows/src/...`：同名后端方法返回显式不支持（fail-closed，不静默）。
  4) `crates/rcu-agent/src/runtime`：在工具目录（`catalog::executable/validate`）注册
     `computer.move_window`（write 工具，需 input 授权与租约），并加入 `capabilities.rs`
     的工具清单；执行分支调用引擎方法。
  5) `crates/rcu-mcp/src/server/mod.rs`：注册 `#[tool] computer.move_window`（args：
     target/lease/request_id + reference + display 选择），走既有 Prepare/Execute。
  6) 验收：`work/pua-loop/live-acceptance.sh` 增用例——把测试窗口在授权屏↔非授权屏往返一次，
     证据 JSON 落盘；Windows 侧仅断言"显式不支持"。
- **改动面（已勘定，比预想小）**：设备线协议是 **tool 泛型**（`Call::Prepare{tool,arguments}` /
  `Call::Execute{operation_ref}` 按工具名路由，无 per-operation 枚举），所以不需要动
  device.rs 的 Call/Reply；实现集中在：① rcu-mcp `#[tool]` 注册（args 结构 + 描述）；
  ② agent 侧按工具名注册执行器（runtime/execution + operation Registry）；③ 引擎方法
  （rcu-core）+ macOS 后端（复用 `place_window_on`，含尺寸预检与 fail-closed）；
  ④ Windows 明确不支持；⑤ 验收脚本用例（跨屏往返 + 证据 JSON）。
- 实现路径（跨 crate，一步一验）：① rcu-mcp 注册 `computer.move_window`
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
- 结构：rcu-core `Popup` + `popup_kind`（纯函数 + 单测）、`Observation.popups`、
  `Backend::popups` 默认空（Windows/fixture 显式不实现）；rcu-macos 盘点 + display 富化；
  `Observed` 包装层转发。
- 系统面板策略：#3a 的 bind 变更已解开"跨屏面板被拒"的死结（选择无副作用）；面板不可搬移时
  move 回执如实报告实际 bounds/display（不以请求为准）。此点为文档级结论，未新增 fail-closed 检查，
  避免过度严格重现陷阱。
- 活体验收：`docs/validation/popup-acceptance.py` → 13/13 PASS（清单基线 → Add Image 开出面板 →
  下一次 observe 即交付 `cgwindow:N` kind=unknown + 事实标题 "Open" → owned_popup 绑定 → Cancel 关闭），
  证据 `docs/validation/evidence-popup-1789462163.json`。
- 提交：`3b83a8b`（代码）、`50fa1c2`（验收）。
