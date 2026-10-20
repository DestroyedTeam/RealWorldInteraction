# 2026-10-09：悬停接收层与局部锚点可用性

## 当前结论

- TARGET 始终是本仓库；实际输入只落在已授权的自有 Receiver，虚拟 display4（UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`）。游戏输入0、没有第二游戏 worker、没有部署或重启生产服务。
- 当前真实 Agent：PID73141 / instance `de7cc66c-8a96-4730-8f4d-a569b711e483`；本轮只读 targets 的四项权限 true，不申请权限。当前39tools已加载。
- 原 MouseMoved **进入自有 NSApplication 与 NSWindow，坐标命中 hover 区，但没有进入 hover view**。因此“未送到进程”已不适合描述本配置；不能泛化为所有应用或平台无法悬停。
- `grounding_diagnostic` 与 Engine.act 拒绝文案已补无锚点解释；匹配条件、阈值、原生校验、reservation、lease、no-replay没有改变。此源码补丁**尚未部署到当前 Agent**。
- 最终644passed/11ignored/41suites（接收层阶段643、随后新增构造对照1项）；fmt、locked offline workspace tests、all-targets/all-features Clippy -D warnings、diff check均exit0。Receiver example单独24tests/build/clippy通过，不能计入workspace643重复加总。

## 1. 溯源实际拒绝，而不是评判游戏画面

读取明确原始 durable request `navigation-user-return-map-open-lower-20261009-2`，未使用另一adapter的短期details_ref，未重放。
[原始持久回执](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-availability/GREEN_ORIGINAL_DURABLE_RECEIPT.json>)记录：

- stale_reference；content_changed=true；scope/focus/layout/native_epoch/mutation_context/mutation_ready/blocked changed均false。
- reference_kind=pixel；没有局部比较指标；native_input_dispatched=false / native_dispatch_entered=false。
- 操作terminal、资源已释放、无uncertainty、replay forbidden；不是goal通过，也不是输入失败后重试。

既有 `evaluate_grounding` 对保留锚点的比较会产生指标；本例没有指标与局部锚点，仅能定位到全帧比较路径。**创建时为何没有锚点，旧记录没有保存**；不猜测是真实GREEN纹理不足、权限、模型或用户活动导致。

新字段：`local_pixel_anchor_available`、`red_marker_anchor_available`、`pixel_content_policy`。
两种锚点均None时固定 `local_anchor_creation_reason=not_recorded`，无重新PNG decode；仍 `input_authority=false`。
像素ref无锚点且内容变化时下一步明确：
> Pixel reference has no local anchor; full-frame content changed. Inspect a fresh stable textured surface; no input sent.

这只解释事实，不忽略同时发生的其他binding变化；所有维度仍分别在诊断中。uniform离线fixture证明无anchor且远端内容变化仍拒绝、dispatch0、无receipt reservation。

## 2. Receiver透明探针与实际顺序

仅Receiver的 `model.rs/appkit.rs/README.md` 增加独立 `dispatch_probe`。首次sharedApplication之前注册自有子类，不swizzle；两个sendEvent各正常super一次，没有跨super RefCell借用。只读取自有进程接到的限定mouse NSEvent属性；不装global/local monitor、不读取按键字符/其他应用内容、不调用或伪造view/tooltip回调。
hitTest仅匹配自有window且坐标finite；日志128条、分层计数独立，JSON-only写入发生在super返回后，fsync观察者开销真实存在。

运行前磁盘可执行SHA `46b28c93c2ced21125949b483fb8309dda5dcabad79d6b0b7afa99ceb9b8db2f`，随exec记录；不是对内存映像的硬件证明。
rtk wrapper PID6575，**真实Receiver PID6576 / CG15045**。初查wrapper PID无窗口保留为操作员身份错误，随后按独立state的真实PID重新discover/attach，没有猜窗口或输入fallback。

| 顺序 | 命令/动作 | 独立观察 | 结果与限制 |
|---|---|---|---|
| 1 | 新Receiver --open-window --display-id4 --lifetime-ms600000 | state.running，app_active=false、frontmost=false；fresh window bounds[1824,504,660,552] | 仅自有窗口，初始计数0 |
| 2 | snapshot原始1320×1104 | 2×映射[0.5,0,0,0.5,1824,504] | 没有模糊downscale；非原子AX/pixels仍明确 |
| 3 | typed hover view[500,360]，新request ID，269ms | App/Window记录sequence35/36：type5、subtype0、window15045、point[250,372]、own_window_match=true、hit=hover_zone | **两层都到了，view moves/enters/tooltip仍0**；回执effect=unverified |
| 4 | 明确AXPress safe count，192ms | Safe count0→1；动作不伪装为mouse receipt | 正向semantic路径有效 |
| 5 | fresh snapshot，typed drag500ms view[250,540]→[500,660]，728ms | 真实1down、8drag、1up、completed1、orphan0；App/Window各记route，蓝色path可见 | super正常转发已交叉验证；不是无观察者性能benchmark |
| 6 | 运行中Live start300000ms，正确虚拟preview display | 第一次open_in_codex MessagePort error；第二次queued，本session采集/显示0，隐藏无图 | 不把用户“旧页面已出现”套成这个新session的显示证明；没有伪造visibility |
| 7 | Live stop | status stopped / reason requested / worker_done=true / image false | 清理完成，不称此次运行中任务切换通过 |
| 8 | fresh observed自有close control AXPress，119ms | state.closed / lifetime requests0 / process exit0 / stderr空；detach完成 | 输入回执与独立windowWillClose/进程结束分列 |

顺序实际截图（由loaded RWI返回；没有绘制或修改截图）：

1. 初始状态与hover区域：
![Receiver初始0计数](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/probeSnapshot.0.png>)

2. AXPress计数1，drag前：
![Safe count1](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/probeBeforeDrag.0.png>)

3. 完整drag蓝色路径、hover仍0：
![Drag实收完整路径](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/probeAfterDrag.0.png>)

[完整native工具回执](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/LOADED_MCP_DISPATCH_PROBE.json>)、[hover后的两层日志](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/STATE_AFTER_HOVER.json>)、
[drag状态](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/STATE_AFTER_DRAG.json>)、[关闭/进程exit](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/LAUNCH.command_execution.json>)均保留。
two-stage计数不是两次物理event；hit_region只是公共hitTest，不证明view回调。若super阻塞/崩溃，post-super未持久化不能独自证明上游未送达。AppKit active/frontmost状态只在原既有callbacks采样，不把最后一次样本误当后续时刻状态。

## 3. BASELINE / MODIFIED / ROLLBACK（同输入）

| 范围 | BASELINE | MODIFIED | 主ROLLBACK.sh独立副本 | REAPPLIED |
|---|---|---|---|---|
| no-anchor diagnostic，同一uniform→远端变化fixture | 101，新availability字段Null | 0，1test通过，仍拒绝0dispatch | 101，同一字段缺失断言 | 0 |
| receiver schema，同一个新schema-only test | 101，新dispatch_probe字段Null | 0，字段0且view计数独立0 | 101，同一断言 | 0 |

101是**测试先行暴露旧行为**，不是生产命令、GUI或cargo编译失败。旧生产bytes的hash在加test-only harness**之前**核对相等；根源码从未执行回滚。两个scoped rollback都真实执行同一个四角色中的ROLLBACK.sh，副本恢复后重施，三文件原hash逐项相等。
receiver主controller首次copy误用了harness/model.rs：真实exit1、FileNotFoundError，cargo尚未启动；更正为观察到的完整harness子路径后完成，不覆盖错误。

[no-anchor三态证据](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/anchor-availability/MAIN_ROLLBACK_RESULT.json>)；
[receiver主rollback三态证据](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/receiver-dispatch-probe/MAIN_RECEIVER_ROLLBACK_RESULT.json>)。
所有command_execution与literal stdout/stderr（含路径对应真实原始文件）并入同一VERIFICATION.txt；没有删旧成功结果或不成功步骤。

## 3a. 继续执行的纯离线构造对照

新增 test-only `probe_move_constructor` 与 `hover_constructor_public_nsevent_vs_current_cg`；测试直接调用生产 `event(P)`，没有复制或改写生产构造器。
Moved只读公开SDK允许的type/windowNumber/location/flags/timestamp/subtype/delta；不复用down-only getter集合，不读取eventNumber/clickCount/buttonNumber/pressure。没有post、窗口、capture、warp或输入。

同一namespace命令 `rtk proxy cargo test -p rwi-macos --lib --locked --offline native::input::move_pointer::tests -- --nocapture`：
BASELINE **5pass** → MODIFIED **6pass** → 主ROLLBACK.sh副本 **5pass** → REAPPLIED **6pass**，四次exit0，两原hash在harness之外恢复一致。
新增测量而非“修复hover”或“修复坐标”；首次编译E0369/101（pinned CGEventType无PartialEq）原始stderr保留，改为u32 discriminant比较后完成。

本次实际同数值点[-400.25,200.5]、synthetic windowNumber0/1048573两组：

| 字段 | 当前CG构造 | public NSEvent转CG |
|---|---|---|
| event type / subtype / delta / flags | 5 / 0 / [0,0] / 0 | 相同 |
| Quartz location | [-400.25,200.5] | [-400.25,968.5] |
| source_state_id（单次样本） | 1779289672，当前Private生成值 | 0，factory隐含值 |
| NSEvent roundtrip fixture windowNumber | 当前无路由字段、0 | 分别0/1048573保留 |

这证明不能把factory转CG当作字节等价替换；**不能据此证明真实目标窗口坐标错了**：fixture未提供真实目标geometry、addressing在生产后续阶段设置，current和factory的数值不是已证明同一frame。
不能偷换Private source策略，也没有把subtype3设为默认。
[原始测量](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/hover-constructor-probe/PROBE_MEASUREMENT.json>)、[主rollback与hash](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/hover-constructor-probe/MAIN_ROLLBACK_RESULT.json>)、[最终644/11/41回归](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-09/hover-constructor-probe/FINAL_COUNTS.json>)均保留。

## 4. 延续的四角色与下一步

- [MODIFIED_FILE](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE>)
- [DIFF_FILE.patch](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch>)
- [VERIFICATION.txt](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt>)
- [ROLLBACK.sh](</Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh>)

补充证据只扩展既有角色。新诊断不需要更换MCP工具表，但Agent源码尚未部署，不能把离线绿色测试当运行版本修复。
原CG与public NSEvent的**离线构造对照已执行**；下一未完成hover动作是在自有窗口下验证真实frame转换并保持Private source，再做独立派发/兼容subtype/post通道对照。每种独立request、先offline再隔离实机；无activation/real-cursor warp/buttons/global fallback、未知delta不造历史。不在游戏正在使用生产服务时热换。

完整计划还包括运行中Live任务切换/Hide/Resume、keyboard/scroll、AppKit/Electron/Wine/SDL/GLFW状态矩阵、
覆盖/不同Space/minimized/hidden的诚实支持度、资源并发回收、cold/warm/P50/P95、MCP输出易读性。
它没有被缩成仅多窗口或仅本次诊断；已通过整体MCP项目见[此前整体验收](2026-10-09-mcp-functional-acceptance.md)。
