# 2026-10-10：重启后实机验收与 foreground 校验漏接修复

当前：RWI macOS / 本任务40工具已加载，真实调用发现校验漏接并完成修复部署 / 当前Host旧Adapter仍需重载，前台native goal未标通过。

## 1. 重启生效与实际窗口验收

用户本轮重启确实生效：本任务已出现 `computer.focus_window`、总计40工具。第一步复用成功控制器运行安装MCP的只读initialize/list/targets，exit0；当时Agent91566／6e601838…，四权限true。工具能列出不等于调用链路已接通。

[实际启动命令](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/NEXT.command_execution.json)、[loaded targets](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/01-loaded-targets.json)。

只使用已分配虚拟屏UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`（ID4）的own Receiver/Cover，没有操作游戏、飞书、Chrome、已有文档或主屏应用。GUI观察和focus尝试均来自本任务已加载RWI工具，不用独立stdio替代Host实机验收。

当前root重新构建fixture exit0、binary SHA256 `38db09ebea6f44d07713ac95adfb91454db3afef70853125e3e41d2563f751b0`。实际PID74864、主CG28373、Cover CG28374；发现、attach、frame均新建。AppKit active/key/frontmost=false、Cover在上方=true、Safe count=0。

顺序为fresh snapshot → resolve kind=window → fresh完整expected_foreground → claim30秒lease → 一次显式focus → finally release。原请求ID `own-explicit-foreground-after-codex-restart-20261010-01`，实际literal输出：

```json
{"code":"invalid_arguments","goal_verified":false,"message":"unknown capability"}
```

lease实际released=true；独立前台仍PID69398／CG28251，非目标PID74864；own oracle active/key/frontmost仍false、Cover在上方true、计数0。**前台切窗未通过。**

durable查询request_unknown；这本身不是无副作用证明。本轮结合下文RPC前参数分支来源与独立原生读回归因，未重放原请求、改走快捷键／鼠标或解除guard。

[原回执](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/12-focus-receipt.json)、[释放](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/13-focus-release.json)、[独立前台](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/14-independent-foreground-after.json)、[原请求查询](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/16-original-request-query.json)、[own读回](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/17-after-refusal-oracle.json)。

**01 · 新PID／新CG窗口：未聚焦，Safe count: 0**

![01 · 新PID／新CG窗口：未聚焦，Safe count: 0](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-post-restart-foreground/06-focus-before-snapshot.png)

**02 · 派发前新鲜图：窗口引用与完整前台tuple均重新读取**

![02 · 派发前新鲜图：窗口引用与完整前台tuple均重新读取](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-post-restart-foreground/09-dispatch-fresh-snapshot.png)

**03 · 拒绝后：仍非key、计数0，不是切窗成功**

![03 · 拒绝后：仍非key、计数0，不是切窗成功](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-post-restart-foreground/15-focus-after-snapshot.png)


fixture的300000ms自身timer最终真实closed，Cover visible=false、output_error=null、进程exit0；attachment已detach。没有为关闭而操作其他应用：[终态](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/21-fixture-final-oracle.json)、[进程退出](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/NATIVE_FOCUS.command_execution.json)、[detach](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/20-own-detach.json)。

## 2. 根因与最小修正

`ToolDefinition`和Agent executor已有新工具，但共享 [catalog.rs](/Users/vincentadamnemessis/AI Projects/Real World Interaction/crates/rwi-core/src/device/catalog.rs) 的 `validate` match漏了 `computer.focus_window`。[Gateway invocation](/Users/vincentadamnemessis/AI Projects/Real World Interaction/crates/rwi-mcp/src/gateway/invocation.rs) 在Prepare前就拒绝，非TCC、AppKit拒收、屏幕或模型问题。

仅补4行参数路由：

```rust
"computer.focus_window" => {
    let _: FocusWindowArgs = serde_json::from_value(args.clone())?;
    Ok(())
}
```

不改NativeAX请求，不删required字段、类型检查、未知工具拒绝、native身份、foreground/HID yield、lease或durable no-replay，也不把前台加入后台fallback。

新增3项回归：完整focus参数通过且缺字段／错类型拒绝；每个公布工具均有已知validator；真实MCP child+mTLS synthetic peer单次Prepare/Execute、不完整请求0转发。后者只证明真实process/transport，不冒充native focus效果。

旧实现先红：Core2测试101、真实MCP1测试101，重现unknown capability；最小补丁后2+1测试绿0。

## 3. 回归与实际部署

- 完整 **666 passed / 0 failed / 11 ignored / 41 suites**；fmt、locked/offline workspace、all-target/all-feature Clippy `-D warnings`、diff、release均exit0。ignored不计native覆盖；旧example27独立证据保留、不并入666。
- 复用成功部署控制器，仅换候选hash、当前PID/instance及本run路径。共享native lease锁下检查当前实例所有durable安全终态，停旧PID91566、原子App交换、bootstrap与签名/只读smoke核验；没有清fence/recovery或业务input重放。
- 最终Agent **PID92522／instance `57ac7afb-af79-41b5-a440-12c925145a3b`**，已安装SHA256 `d8c2bbb2fd4fc2f86c93740262f4b698bddb154bbdf3f4db71841c8b06d92dac`；MCP `b12fa991b9baaf59515739652154d2b41991eda007e5477edf72370a12e75878`。同本地证书DR、四权限true、CA/配对/profile不变；未导出私钥、改其他Codex配置或终止其他会话MCP。
- 新安装独立MCP仍40工具，对同一故意不完整输入 `computer.focus_window({})` 正确返回 **missing field target**，而非unknown capability。没有window/reference/lease，故不会native派发：[新进程probe](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/validator-deployment/MODIFIED_VALIDATOR_PROBE.json)。
- 部署后当前**loaded旧Adapter**只读JS probe调用一次 `rwi.call('computer.focus_window', {})`，仍unknown capability、mutation_attempts=0、dispatch_returned=0。JS只诊断local校验，不是GUI操作/绕过，也不重放原请求：[loaded probe](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/23-loaded-adapter-readonly-validator-probe.json)。

磁盘换新二进制不等于运行进程已重载。已通知方便时再重启Codex；这是本轮新修正需要重载，**不是上次重启没做**。前台native_goal_accepted=false；下一次必须新fixture、新绑定、新request，不能重发旧请求。

## 4. 实际回滚与同一输入

|范围与相同输入|BASELINE|MODIFIED|ROLLBACK|重新应用|
|---|---|---|---|---|
|Core focus_validator及真实MCP foreground_focus_real_mcp；同一断言harness|unknown capability；101/101|2+1通过；0/0|脚本还原两原文件、hash一致；只重插同一断言，101/101|0/0|
|原生App/MCP复制件；相同focus_window空参数|unknown capability，exit0|missing required target，exit0|原字节/同DR签名恢复，unknown capability，exit0|新字节/同DR签名恢复，missing required target，exit0|

所有二进制状态都是40工具，说明只验tools/list漏检实际调用问题。二进制probe不请求屏幕或native输入；没有生产回滚或GUI回放。源断言不是编译失败捷径，rollback不注入实现：[源码同输入](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/validator-fix/SAME_INPUT_RESULT.json)、[原生复制件](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/DEPLOYMENT_COPY_RESULT.json)。

准备错误也保留：旧debug hash不匹配导致创建窗口前assert退出1，改用当前root实际构建hash后运行；报告准备误读smoke JSON顶层导致KeyError/exit1，改读observed summary后完成。两者不是Native输入或权限错误：[binary准备](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/PREPARATION_BINARY_ERROR.command_execution.json)、[报告准备](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-codex-restart-focus/DOCUMENT_PREPARATION_ERROR.command_execution.json)。

同一四角色持续扩展，命令、literal stdout/stderr、exit、hashes、GUI实际回复继续追加而非替换：
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`

## 5. 全计划保持

F01–F16仍all_features_accepted=false。已验后台drag/文字/scroll/covered AXPress结果保留；本轮未验到前台成功。下一步：实际重载后显式前台goal；F07准备通知/down/client key-order隔离；F05 view/tracking/tooltip；offscreen/Space、multiwindow/popup Live、raw、hidden/minimized、跨app原生DnD、browser route、取消恢复与资源性能分开验收，不用单fix替代完整规划。
