# RWI macOS：前台状态刷新与真实切窗验收（2026-10-10）

当前：同一个产品根仓库 / 已部署并验收显式 A→B 前台切窗 / F07 no-raise 与 F05 tracking 分支继续，F01–F16 未全部完成。

## 结果与改动

- 本次 Codex 重启生效：本任务 loaded MCP 40 tools，`focus_window` 已进入 required-field 校验，不再是 unknown capability。
- 首次实机仅 `already_foreground`、0 native requests，不能算真正切窗。另一个请求被 `foreground_changed` 拒绝，原 request 不重放。
- 根因：Executor 的 owner/binding catalog 合理缓存，但 `Describe` 把其旧 `current_foreground` / `user_input_recent_seconds` 原样当成当前值；新鲜 observation 的 foreground 不同。Native 护栏拒绝陈旧 tuple 是正确行为，不是要删掉护栏。
- 修正 `Runtime::describe` + `refresh_volatile_target_facts` + `Call::Describe`：用既有独立 native probe 的采样结果覆盖响应中两个易变字段。只匹配同 desktop identity；未知/不匹配明确 null；不改缓存中的 owner、target、selection、input_opt_in，不等待 native executor mutex，不把采样当原子输入授权。
- 3 项新增回归；完整 workspace **669 passed / 11 ignored / 41 suites**，fmt、locked offline tests、all-target/all-feature Clippy `-D warnings`、diff、release build 均 exit 0。此前 example27 是独立旧记录，未混入669。

## 实际部署（Agent only）

已重用上一轮成功的 controller，更新候选/当前实例/路径，取得同 native lease inode 排他、核对本实例 durable writes 都 terminal/released 后，停止确切旧 PID92522，原子交换 App 并 bootstrap、签名及只读 smoke 均0。

- 新 Agent PID **84167** / instance `a72260d6-98b1-4f92-82a2-208a006b446a`。
- signed Agent SHA256 `aff8ada5b363c42726f5347f7455ea866fea4537545b5143916418504f91d20e`；unsigned candidate `ef488833298ad84e92be607ecebe15b9008ae24160a3343c87ba28d0ba377a7a`。
- MCP 仍为 `b12fa991b9baaf59515739652154d2b41991eda007e5477edf72370a12e75878`，未替换 MCP，**无需再次重启 Codex**。
- 原稳定证书 designated requirement、CA、配对、public config/profile 字节不变；accessibility/screen_capture/listen_event/post_event 四权限 true；未读取或导出私钥，未要求重授权。

## 按执行顺序的真实截图

以下都是该次 MCP 返回的原始 PNG，1320×1104，逐字节复制、哈希/尺寸复核，无拼接/重画/后处理。仅操作明确分配的自有 Receiver/Cover，虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`、当时 id4；无游戏/飞书/Chrome/已有文档/主屏键鼠。

1. 旧 Agent 的第一个场景：fresh native tuple 已是该目标，因此是 no-op，而非切窗。

![01 已在前台分支](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/01-old-agent-before-noop.png)

2. 旧 Agent 第二场景：cached targets 仍报旧 PID52729，而新鲜 observation 报 PID48574/CG29129；请求发出前拒绝、lease 释放。保留原始拒绝，不擅自归因于用户干扰。

![02 陈旧状态拒绝场景](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/02-old-agent-stale-tuple-refusal.png)

3. 部署状态刷新后：新的 own scene03 也命中 `already_foreground`。targets 与 fresh observation 都是 PID88597/CG29516；独立计数仍0。启动时 oracle active=false 是历史样本，不作为持续 foreground 状态。

![03 新 Agent 的 no-op 场景](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/03-new-agent-before-noop.png)

4. 创建第二个独立 own app，两个窗口 fresh snapshot/resolve 后，从新鲜 targets 选定**不是当前前台的目标**，不用更改用户应用或重复旧 request。切换前 native 是 A PID88597/CG29516。

![04 切换前 A](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/04-two-apps-source-A.png)

5. 目标 B PID89640/CG29532，独立 attach、与 A 共享 physical desktop lease；不是同 app 的多窗/跨 PID OS popup owner graph/Live 全验收。

![05 目标 B 切换前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/05-two-apps-destination-B-before.png)

6. 唯一请求 `own-freshfacts-two-apps-explicit-switch-20261010-01`：public AXRaise/必要 AXFrontmost 各一次，receipt returned、resources released。**另一次 fresh targets 与另一次 fresh observation metadata 均读回 B PID89640/CG29532**，证明实际 A→B foreground transition；回执仍正确保留 goal_verified=false，不把它当应用业务成功或键盘输入授权。

![06 独立读回 B](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/06-real-switch-B-after.png)

7. F05 继续对照：先向 native 前台 B 发送一次 hover，再向 background A 同一 own hover zone 发送一次；每次都是 fresh view 660/374、native point 到 own window [330,365]。两次 App/Window 各接收1，hit_region=hover_zone，view/tracking enter/move/exit/tooltip **仍均0**。foreground before/after 保持 B；不是只因窗口在后台才未触发。其后 oracle 的 AppKit flags 来自缓存样本，不据此推断当前 active/key 状态。

![07 foreground hover](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/07-foreground-hover-before.png)

![08 background hover](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance-2026-10-10-fresh-foreground-facts/08-background-hover-before.png)

8. 陈旧 tuple 负例：新的 request 故意带 A 旧 tuple；fresh observation 前台与随后 fresh targets 均为 PID48574/CG29129，`foreground_changed`、native_dispatch_entered=false、lease released。保留有效护栏，不强迫激活、不重放。

9. scene03/04 均 fresh exact AX close 一次，独立 oracle closed/cover_visible=false/output_error=null，两个真实 process exit0，attachments detached。最终只剩原 desktop target、service healthy、input_domain_fenced=false。此前 scene01/02 的 timer closed/exit0 记录保留。

## 同输入 BASELINE / MODIFIED / ROLLBACK

```text
rtk proxy cargo test -p rwi-agent --features fixture --locked --offline volatile_describe_drops_cached_foreground -- --nocapture
BASELINE: exit101；同断言检测到 cached foreground 没有变成 fresh unknown/null。
MODIFIED: exit0，恰好1个选中测试通过；native executor mutex 被占用时 Describe 不等待它。
ROLLBACK: 对独立源码副本实际运行同一 ROLLBACK.sh --volatile-facts-only，原3文件hash完全恢复；插入同一 test-only assertion 后同命令 exit101。
REAPPLIED: 回到修正源码、同命令 exit0；产品根文件hash始终保持修正值。
```

字面 stdout/stderr、命令、环境、状态、hash、原始 GUI replies 进入同四角色的 VERIFICATION.txt；pristine 72项 baseline 不替换，本轮增加两个第一次纳入的源文件与本报告。完整 unified patch 和全路径 rollback 在独立副本运行并读回；不是生产回滚，也不是重新派发 GUI。

## 错误保留与性能边界

- evaluator 最初未开 fixture feature，选中0 tests的 exit0不能算验收；修正后必须恰好1 test。
- 原始 frozen/root 共享 Cargo target 复用了旧 executable；保留错误流，改为独立 target dirs 重编译，不能把这个 operator cache 问题说成 Native 回归。
- 其他真实错误：controller 首次缺 root argv、fixture build 后 hash guard、不存在的 evaluator 路径、CodeGraph 不支持 --file，均保留来源/exit1并更正命令。缺 argv 发生在任何部署步骤之前，hash guard 在该 fixture 创建之前；另外两项是只读命令错误，不是原生输入失败。
- `computer.targets` 的 schema 不接受 include_timings，尽管公共工具说明里有该示例；本次调用明确 invalid_arguments 后去掉该字段完成。此描述/schema 易用性缺口进入下一轮，不隐瞒失败或声称成功率100%。
- 单次 hover B373ms / A299ms，snapshot after switch158ms、AX close119/111ms，仅各一次 adapter wall样本，不是稳态 p50/p95、不证明与 Computer Use 的性能胜负。

## 下一步（完整计划保留）

1. F07：逐项隔离普通 pointer 的 preparation/down/client key/order；不能把 explicit focus 成功当后台 no-raise 或用户键盘连续性/恢复通过。
2. F05：依据两侧都到 App/Window、却无 view/tooltip 的分层结果，继续 transport/NSWindow tracking route；不伪造应用侧 tracking pointer，不判定“不可能”。
3. F08/F09 offscreen/window-local；F12 popup/window-family Live；F06 raw/client、F10 hidden/minimized、F13 native DnD、F14 browser route；F15 cleanup/stress、F16 分层性能/画质/资源回收，均保留且未全验收。

证据根：[post-validator-reload-acceptance](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-validator-reload-acceptance)；[机器状态](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/product/macos-background-acceptance-status.json)；同四角色不换路径。
