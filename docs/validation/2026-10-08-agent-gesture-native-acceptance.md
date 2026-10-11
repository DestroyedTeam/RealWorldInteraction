# 2026-10-08 Agent gesture 与真实接收验收

本轮完成的是基础 gesture、诊断和一次已分配窗口验收，不是全部 RWI 实现。
唯一开发对象为原仓库；原四角色及所有历史结果沿用，不将研究完成混同后台实现完成。

## 实施与部署

- `MouseGesture` 为一次 click/double-click/drag 保留一个 Private source；明确 button/eventNumber/clickCount/pressure/delta，fractional remainder 在投递尝试后提交。原 identity/focus/hit/session/lease/cancel/释放-only 防护保持。
- `Executor::invoke(computer.resolve_view)` true 分支走 RedMarkerCluster，不静默变 Pixel；Describe 分离 executor 参数支持与每 view 的 native anchor 可用性。
- `operation_journal::recent` 为 Engine/Agent 共享的只读尾部读取：至多 65536+1 bytes/20 valid rows，跳过被截断的首行、未完成末行与局部坏记录。旧实现读文件头，超过64KiB后误报历史动作。离线实际回归：BASELINE 101（1通过/3失败），MODIFIED 0（4通过）。
- 真实调用发现 `include_timings:true` 的成功 compact 回执漏掉两字段；源码已保留 present `elapsed_ms/stage_timings`，默认/false仍 trim、默认JSON不增加。两项组合回归101→0；缓存的原始耗时此前已有，输入/schema/cleanup未变。
- Agent 第一次原子替换/signature均通过，但 startup smoke 在 xpcproxy 阶段读取，availability缺失导致子进程exit1；控制器真实执行原子回滚，恢复旧hash。未把这个 harness/启动时序错误称为原生事件不可用。
- 修正为 bounded（20s）只读 targets 就绪探测、失败保留日志和退出子进程；第二次部署exit0。跨切换持同inode BSD flock，锁下重扫本域当前instance全部durable结果；只排除合作RWI input writers，不宣称HID/OS队列/读workers全局静止。无recover/清fence/重配对。
- Stable DR/“RWI Local Development”同leaf，profile/config/public CA及TLS public certificates hash均未变；未读/导出私钥。新Agent PID52174/instance efd220c1-8188-458a-942e-6d5de084178d；loaded MCP实际返回四项权限true，permission prompt false。

## 按实际顺序验收（只操作自有 fixture）

用户明确授权专用窗口位于虚拟屏。NSScreenNumber=4，RWI fresh discovery核对 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`，不回落主屏、不操作游戏/飞书/已有文档。

1. 旧Agent：Rust/AppKit fixture PID34066/CG12649，初始app/frontmost false；按钮实际0→1。这个button receipt当时frontmost为fixture，不能据此宣称按钮完全无焦点影响；人为切换/OS时序未单独隔离。
2. 第一个拖拽在按钮动画后的旧截图上被 `stale_reference/content_changed` 拒绝，0 native input，保留原结果、不放松门槛。
3. 新观察+新request后，旧Agent已完成相同100,300→250,360 Quartz窗口点路径、700ms：1down/10drag/1up，completed=1，无orphan；其全部mouse callback OS前台是另一PID，eventNumber为0。旧实现不是“完全不能后台拖拽”。
4. RWI AXPress关闭这个fixture，state.closed且进程exit0；随后只启动第二个delta版自有fixture，PID52864/CG12704，位置同display4。部署后使用新target/frame/view，未重放旧引用。
5. 当前frame原图1320×1104、affine0.5；旧frame660×552、affine1。按真实映射换算像素，而不是照搬旧view坐标。只说明这些实际frame，不推导性能优势或所有显示器缩放已校准。
6. true red-marker点击在无cluster点返回 `red_marker_cluster_not_found`，cached-read、0native，明确无fallback；这项未到native Executor，用于adapter负例，不代替Executor的独立PNG/分支回归。
7. 新Agent按钮实际0→1；新drag同Quartz路径700ms：1down/11drag/1up，completed=1，无orphan/down遗留，start[76,54]→end[226,114]。全部mouse callback OS前台仍是另一PID（56396），receiving_app_frontmost=false；AppKit内部isActive/key为true，二者没有被混淆。
8. NSEvent实际收到所有down/drag/up相同eventNumber=2，pressure 1→0；raw delta依次14/5、13/6等，累计[150,60]。这个2×截图配置未要求把事件delta自动乘2；仅此配置有效，不外推任意DPI/OS/客户端。Private source生命周期另由真实CG构造/owner-drop tests验证，不说receiver独立attest了source。
9. 回执goal_verified仍false；独立receiver JSON+截图才证实上述目标。cached details显示click877ms、drag943ms（含700ms请求路径）、close188ms；单样本，不作速度/成功率百分比。operation terminal、无native pending、resources_released true/uncertainty false。
10. RWI关闭第二fixture，state.closed、进程exit0；无残留测试窗。

### 真实截图（按时间顺序，未编辑）

旧版本初始

![旧版本初始](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/native-fixture/fixtureBaselineSnapshot.1.png)

旧按钮已计数，后续动画拒绝保留

![旧按钮已计数，后续动画拒绝保留](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/native-fixture/fixtureBaselineDragSnapshot.1.png)

旧拖拽完成，关闭前

![旧拖拽完成，关闭前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/native-fixture/closeBaselineFixture.snapshot.1.png)

新版本初始（原图2×）

![新版本初始（原图2×）](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/native-fixture/fixtureModifiedSnapshot.1.png)

新拖拽接收完成

![新拖拽接收完成](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08/native-fixture/fixtureModifiedAfterDragSnapshot.1.png)

## 未完成项（继续原完整计划）

- typed Move/Hover 尚无公共 Intent/便捷工具；tooltip/tracking/鼠标轮询需要独立接收验收，不能用drag/moved探针冒充hover成功。
- fully occluded/offscreen/other-Space/minimized/hidden能力仍需分级实现与实机负例；这轮只是非OS-frontmost且显示在虚拟屏上的窗口，不是所有“真后台”状态。
- 新MCP timing源码已通过最终fmt/workspace618pass+10ignored（41 suites）/all-target/all-feature Clippy -D warnings，release与安装strict signature/独立stdio2025-11-25/38tools/mac-host读取均exit0、stderr空、0images/0input。安装SHA256 `dbbfc8cdfb497a664e8a4e336b01b1812f6e302275dd9c383b536c65bc12e23c`；当前Host重载后的成功耗时输出仍待验收。
- AppKit/Electron/Wine/SDL/GLFW的同状态CUA对照、动态guard成功率、分层cold/warm/P50/P95、Native事件/捕获/AX/传输的耗时拆解继续；Live、资源并发/回收与多窗口完整计划未被本轮一项修复替代。

## 证据

所有原始loaded MCP、receiver JSON/哈希、原生PNG、签名/两次部署/失败回滚、literal stdout/stderr与exit均在：
/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-08

## 原四角色同输入事务闭环

journal回归BASELINE101（3fail/1pass）→MODIFIED0（4pass）→执行主ROLLBACK.sh的独立副本恢复原hash、同test-only harness同输入ROLLBACK101→重施修复0；timing组合回归101（2fail）→0（2pass）→主脚本恢复后同harness101→重施0。明确区分这项源码回滚、已真实执行的Agent部署回滚，以及未重复的GUI输入；没有冒称GUI回滚后的业务目标复验。已安装MCP另在独立copy恢复e5c1…hash并只读smoke0，再恢复modified hash。所有41个源路径diff重构与完整原hash回滚的结果见本轮最终事务ledger。
