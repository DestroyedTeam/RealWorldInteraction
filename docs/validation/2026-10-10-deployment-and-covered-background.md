# 2026-10-10：部署与原生遮挡后台验收

当前：Real World Interaction macOS / 新版 Agent 与 MCP 已部署、真实遮挡对照完成 / 整理回滚证据，前台切窗等待 Codex 实际重载。

## 1. 本轮部署与会话边界

- 对象仍为 `/Users/vincentadamnemessis/AI Projects/Real World Interaction`；没有创建另一套产品源、提交或推送，也没有操作游戏、飞书、Chrome、已有文档或主屏应用。
- 复用成功部署控制器的命令与独占 native lease／durable 安全终态检查。第一轮部署加入显式前台能力；第二轮部署只使后台能力描述符合下文实测边界，不宣称修复指针 key／堆叠变化。
- 最终 Agent PID `91566`，实例 `6e601838-8f71-48a5-bfd1-200de01503e0`；已安装 App 二进制 SHA-256 `f4b63cbcb16ab694bb333656b83f99f5c69d21cf29ca3bc902360379b546e4de`。MCP SHA-256 `97764d33a48b68e5ac17abac9e349034b9e9c87cdddd0ddb125caee1feb0a401`。
- 继续使用本地证书 `RWI Local Development` 和原 designated requirement；未换证书、CA、配对身份或 profile，未导出钥匙串私钥或清除 TCC。最终 loaded RWI 读回 accessibility／listen_event／post_event／screen_capture 均为 true，没有发起授权请求。
- 新安装 MCP 的独立只读进程实际列出 **40** 个工具；当前 Host 会话仍加载 **39** 个，缺少 `computer.focus_window`。**显式前台切窗仍待真实重载后的实机验收**。用户回复“稍后重启 Codex”，本轮不再次催促，也不从旧 JS catalog 绕过。

实际部署事件：[第一轮](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/DEPLOYMENT.command_execution.json)、[MCP](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/MCP_DEPLOYMENT.command_execution.json)、[第二轮](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/capability-deployment/DEPLOYMENT.command_execution.json)；[最终生产读回](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/FINAL_PRODUCTION_READBACK.json)。

## 2. 真实 GUI 顺序与逐项结论

所有原生 GUI 输入经 **本任务已加载的 RWI MCP typed tools** 发出，只有专用 Receiver／Cover。已分配虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`、本次 ID 4；新 PID／CGWindow 均重新发现并分别绑定。没有用独立 stdio 客户端替代 Host GUI 验收。

### 2.1 场景一：先建立 capture 对照，不冒称已验堆叠

Receiver PID 70517、主窗口 CG 24391、Cover CG 24392。Cover 在虚拟屏盖住 Safe count 按钮；两个窗口的精确截图分别采集。主窗口 SCK 图仍包含被遮控件，说明**精确窗口截图与桌面遮挡／输入命中不是同一个事实**。

这一版尚无 `orderedWindows` oracle，只接受窗口／屏幕／capture 场景证据，不计完整遮挡顺序保持验收；没有点击。到期后两自有窗口真实 closed、进程 exit 0、cover_visible=false。

**01 · 精确主窗口图仍包含 Safe count: 0；不是桌面无遮挡证明**

![01 · 精确主窗口图仍包含 Safe count: 0；不是桌面无遮挡证明](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/01-03-base-covered-snapshot.png)


**02 · 同一场景的独立不透明 Cover 窗口**

![02 · 同一场景的独立不透明 Cover 窗口](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/02-04-cover-snapshot.png)


### 2.2 场景二：普通指针效果通过，严格遮挡／key 保持失败

新 binary 加入 public `NSApplication.orderedWindows` 前后采样；Receiver PID 76141、CG 24395，Cover CG 24396。原生 CG 主窗口 bounds `[1824,504,660,552]`，Cover `[1840,580,205,58]` 包含按钮点 `[1938,609]`；AppKit frame 与 CG 桌面坐标分开保存，Retina 原图至桌面 scale=0.5。

只发送一次普通 `computer.click`，request_id `covered-ordered-pointer-click-20261010-02`，不是 AXPress。回执 dispatch=returned、goal_verified=false、replay=forbidden；单次 adapter wall **241 ms**，不是稳态性能比较。

|独立读回|输入前|输入后|判断|
|---|---:|---:|---|
|Safe count|0|1|按钮目标生效|
|Cover visible|true|true|仍有 Cover，但不等于仍在上方|
|Cover above primary|true|false|窗口堆叠保持未通过|
|AppKit active / primary key|false / false|true / true|不取 key 的严格后台条件未通过|
|OS frontmost PID|40411|40411|系统前台未变，不足以证明真后台全部条件|

[输入前 oracle](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/12-ordered-before-oracle.json)、[原回执](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/13-ordered-pointer-receipt.json)、[输入后 oracle](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/14-ordered-pointer-oracle.json)。原生状态采样不是原子连续监测。

**03 · 新场景指针输入前：Safe count: 0；堆叠由原生 oracle 确认**

![03 · 新场景指针输入前：Safe count: 0；堆叠由原生 oracle 确认](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/03-11-ordered-base-before.png)


**04 · 指针输入后：Safe count: 1、key 高亮；Cover 已不在主窗口上方**

![04 · 指针输入后：Safe count: 1、key 高亮；Cover 已不在主窗口上方](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/04-15-ordered-pointer-after.png)


### 2.3 场景三：独立 AXPress 保持真实遮挡，未自动 fallback

另开独立新 Receiver PID 80115、CG 24405、Cover CG 24406，不是失败后换路重发同一操作。对本次快照明确 advertised 的同类 NSButton 发送一次 `at.action="AXPress"`，request_id `covered-independent-axpress-20261010-03`。

回执 dispatch=returned、goal_verified=false、replay=forbidden；单次 adapter wall **188 ms**。独立读回：Safe count **0→1**；Cover visible／above primary 在输入前后均 **true**；AppKit active／key 前后均 **false**；系统前台 PID 前后均 57285。

**该原生语义目标与 sampled 遮挡状态保持通过**，仅覆盖自有标准 NSButton；不外推为通用覆盖窗口 pointer／hover／drag、hidden/minimized 或任意第三方应用保证。

[输入前](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/22-semantic-before-oracle.json)、[原回执](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/23-semantic-axpress-receipt.json)、[输入后](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/25-semantic-after-oracle.json)。

**05 · 独立 AXPress 场景输入前：Safe count: 0，非 key 状态**

![05 · 独立 AXPress 场景输入前：Safe count: 0，非 key 状态](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/05-21-semantic-covered-before.png)


**06 · AXPress 后：Safe count: 1；前后原生读回 Cover 仍在上方、AppKit key=false**

![06 · AXPress 后：Safe count: 1；前后原生读回 Cover 仍在上方、AppKit key=false](/Users/vincentadamnemessis/AI Projects/Real World Interaction/outputs/acceptance/2026-10-10-covered-background/06-24-semantic-covered-after.png)


### 2.4 清理与已有错误保留

三个 fixture 最终均 `status=closed`、cover_visible=false、output_error=null，进程 exit 0；相应 attachment 已 detach。第一例由实际 timer 路径关闭，第二／第三例用重新观察的自有 AX close。没有借强制全局输入关闭其他软件。

首次 cover 编译出现 **E0283**（嵌套 `msg_send` 的 windowNumber 类型不明确），保留 [ATTEMPT1](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/ATTEMPT1.command_execution.json) 的原 stderr／exit；改为显式 `isize base_number` 后重新构建和验收。不是 runtime 或权限错误。

前次 AXScrollUpByPage `-25205` 与 clip150→344 的 uncertainty 继续保留，未重放／自动 fallback。本轮复核原始前后 PNG，前图 row05 与150一致、后图 row11与344一致，未得到“原归因已修复”的证据：[原图审计](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/AX_SCROLL_ORIGINAL_IMAGE_AUDIT.json)。

## 3. 具体修正与未证明的因果

- Receiver 显式 `--cover-controls` opt-in；重复／缺完整授权参数拒绝；cover 使用自身 screen/frame/window 读回及真实 close 后 visible 检查。
- `State.cover_above_primary` 读 public own `orderedWindows` 的两个对象索引；任一未找到则 null，不拿创建时 order 请求冒充当前顺序。
- `desktop.rs` 的 `background_operation` 文案与 `background_input` 三个字段已部署并由 loaded RWI 读回：`foreground_transition_scope` 明确仅指无显式系统前台请求；`target_appkit_state_unchanged_attested=false`、`window_order_unchanged_attested=false`。它们不取消 native identity、hit/mapping、cooperative lease、durable no-replay 或独立 goal readback。
- [实际 background.rs](/Users/vincentadamnemessis/AI Projects/Real World Interaction/crates/rwi-macos/src/background.rs) 明确构造并向目标进程发送 `NSApplicationActivatedEventType`。这是 AppKit active/key/order 变化的**待隔离因素**；本轮没有证明究竟是准备通知、普通 down 还是 client 策略导致变化。不能直接删掉准备并宣称所有后台能力修好。

## 4. 离线同输入／回滚与原始输出

fmt check、locked/offline workspace test、all-target/all-feature locked/offline Clippy `-D warnings`、diff check、release build 全部 exit 0。工作区 **663 passed / 0 failed / 11 ignored / 41 suites**；ignored 不计实机覆盖。

最终专用 example 独立重新运行 **27 passed / 0 failed / 0 ignored**，不并入工作区 663：[真实命令与原始输出](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/FINAL_EXAMPLE_TEST.command_execution.json)。

|复制件范围与同一输入|BASELINE|MODIFIED|ROLLBACK|重新应用|
|---|---|---|---|---|
|`covered_receiver_contract`（相同 test-only harness）|101；旧 Options 不接受 opt-in|0；合约通过|101；真实脚本还原旧3文件，原始hash一致|0|
|`background_claim_does_not_attest`（相同 test-only harness）|101；原字段缺失/Null|0；显式false合约通过|101；真实脚本还原旧2文件，原始hash一致|0|
|原生 App/MCP **部署复制件**：相同只读 initialize/list/targets|39工具，exit0|40工具，exit0|39工具，exit0；旧App/MCP原字节与同DR签名校验0|40工具，exit0；新签名校验0|

测试 harness 只是同一断言，不向旧代码补入实现；所有 literal stdout、stderr、exit、command 和 hashes 写入同一 `VERIFICATION.txt`。没有生产回滚、GUI 回放或替换证书。二进制复制件的只读客户端连接当前生产 Agent，所以该项证明的是 **MCP catalog／二进制／签名恢复**，不是旧 Agent native 行为复演。

[源代码两范围](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/covered-receiver/SAME_INPUT_RESULT.json)、[能力合约范围](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/capability-contract/SAME_INPUT_RESULT.json)、[部署复制件](/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/post-resume-acceptance/DEPLOYMENT_COPY_RESULT.json)。

## 5. 同一四角色与下一步

旧四角色继续扩展，不换成果路径：
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt`
- `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh`

全量 managed 源码／文档补丁重建、复制件 portable rollback、原始 hash／原先缺席文件恢复、重新应用及 root 保持修改状态单独核对；这是源文档复制件验证，不是完整生产运行回放。

完整 F01–F16 仍 `all_features_accepted=false`：[当前矩阵](/Users/vincentadamnemessis/AI Projects/Real World Interaction/docs/product/macos-background-acceptance-status.json)。接下来按优先级：实际 Host 重载后仅在 own fixture 验收显式前台切窗；单因素定位 F07 准备通知／mouse down／client key-order；继续 F05 view/tracking/tooltip；F08/F09 offscreen/window-local、F12 多窗/popup/Live，再保持 raw、hidden/minimized、native跨app DnD、browser route、取消恢复、性能资源压力的各自范围。不能用本轮 AXPress 成功代替这些验收。
