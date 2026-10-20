# 游戏最终输入检查：准备事件与请求输入分离（2026-10-02已部署，失败路径待实机复验）

## 实际来源与状态

- 「观察并自动化冰牙鲨副本」会话 `01a0ecbc-5896-7323-ba39-87c9cd7b00d9` 反馈：部署empty-AXWindow适配后，PID61792/CGWindow1671的“奇兽”打开管理页、“休息”后读回“骑乘”、关闭与再次打开均已真实成功。此为来源会话的GUI验收，本任务没有重放其点击，也没有以自己的派发回执替代页面效果。
- 随后关闭管理页的新请求 `creature-close-following-seahorse-for-two-native-runs-20261001-2352` 失败。**本任务独立查询其durable request**，实际为 `dispatch=uncertain`、`dispatch_error.code=stale_reference`、旧文案“no input sent”、`native_dispatch_entered=true`、`resources_released=true`、`resource_release_uncertain=false`、`input_domain_fenced=true`，`replay_allowed=false`。[原始只读查询证据](../../outputs/validation/2026-10-01-animated-empty-ax-window/03-final-guard-original-receipt.json)。没有查询后重放、换输入路径或代替游戏会话解围。
- 来源会话按正规 `recover_input` 恢复后，本任务的只读 `computer.targets` 返回当时已部署Agent PID64545、instance `d738b7f6-8623-4359-937f-ca17da6da636`、healthy、输入域false、四项权限true。先在不重启服务时完成源码/离线测试/候选签名；随后来源会话反馈不再发送RWI游戏输入、部署可继续，按用户此前明确要求修正后部署，在只读时机更新服务，实际验收见后文。

## 已确定的实现缺口

`NativeMac::dispatch_input` 在应用非前台时，先向绑定PID/窗口post一个 `NSApplicationActivatedEventType` 的AppKit准备事件，将它记入同一native effect counter；随后第一次 `read_input_batch` 再截取/校验目标，才可能发请求的mouse down/up。

旧最终检查若此时拒绝，错误文案称“no input sent”，却没有区分已post的准备事件。Core按变化的native effect counter保留`uncertain`，Observed保留`native_dispatch_entered=true`，Agent则因此围栏整个输入域。**“没有请求点击”不等于“全操作零原生事件”**。这个路径解释了回执与文案的矛盾；不能以文案或错误code为依据直接解围/允许重放。

旧回执没有记录该最终检查的逐维度差异，无法从现在的画面补算当时究竟是局部patch、窗口epoch、focus、modal、layout还是mutation readiness改变。此次没有把其笼统归因于游戏不接收事件，也没有放宽像素阈值或重试原请求。

## 实现修正

1. 合成输入返回失败时，记录准备事件后、请求输入前的invocation-local计数；仅当已post准备事件且之后计数**未饱和并完全未变**，附上版本化原生证明：`macos_background_preparation_only`、API `CGEventPostToPid`、准备事件1、请求输入事件0、`held_input_resources=false/replay_allowed=false`。保留原生失败的诊断对象。任何请求鼠标/键盘/滚动/拖拽/AX效果已计数、计数饱和/重置或panic，都不能获得此证明。
2. 准备事件仍留在effect counter，durable receipt仍`uncertain/replay=forbidden/goal_verified=false`，不冒充`rejected_before_dispatch`或“零效果”。文案明确“请求输入未发；已post一个后台准备事件”。
3. Agent仅在完整、严格类型匹配的上述backend证明下免于**新增**输入围栏；native worker/panic、清理未完成/不确定、记录落盘失败和不完整证明仍围栏。不解除已有围栏，不改旧账本；后续只允许新观察/新绑定下的新独立意图，不能重放本请求。
4. 最终 `validate_batch_scene` 失败诊断加入stage、identity/scope/focus/layout/native epoch/mutation context/content变化、grounding结果、reference kind、local anchor可用性、mutation readiness和blocked，且不暴露图像/文档文字。此比较本身不声称不存在持有输入；只有上层计数证明才能区分准备事件。没有新增截图次数。
5. PixelAnchor的96px patch、对比度/变化阈值、blank full-frame规则和确切AX canvas/PID/CF/window号证明，及modal/权限/身份/焦点/租约/取消/不可重放均不变；该检查继续拒绝真实stale目标。

## 离线门禁与负例

- macOS **98 passed/6 ignored**；全仓默认离线 **537 passed/10 ignored，41 suites**；显式`rwi-agent --features fixture`的preparation-only用例 **2 passed/45 filtered**。
- Native helper矩阵覆盖无准备事件、请求输入计数已变、计数回退/饱和，均不能产生豁免证明；原stale诊断保留。最终检查矩阵区分session/layout/element revision/readiness/unknown blocked/binding missing。
- Agent矩阵覆盖各字段缺失/错类型/错误API或event/version、请求输入非零、held资源true、cleanup不确定，均仍围栏。
- 真实服务层的**synthetic offline fixture**通过Engine→Observed→Agent→durable ledger：准备事件1/请求输入0的回执仍uncertain、不围栏；重复request不再post；重启保留这一分类、durable查询仍不允许重放，新独立请求可以执行。请求输入非零的对照仍在重启后围栏。此fixture无AX/屏幕/OS输入，不能替代真实游戏验收。
- fmt check、全目标/全特性Clippy `-D warnings`、release构建、diff check通过。

## 2026-10-02 部署与实际只读验收

- 同leaf候选 `/Users/vincentadamnemessis/AI Projects/Real World Interaction/work/animated-window-canvas-20261001/RWI Agent.guard-candidate.app` 严格签名验证成功，binary SHA256 **`2045fc748f334afd3cf3ad6bb2e04491d85301e39d2ec9b0765fef58108d7ab4`**，designated requirement仍是原identifier＋leaf `24b346a95ec8f450f7d8433a2e1876568dbcb2f8`。
- 原实际安装binary **`b59d96a375adc0d64a0b193da80b94438ae286b49955b38e0e7c0cd733a5ed0a`** 已备份在 `work/animated-window-canvas-20261001/RWI Agent.before-guard.app` 和 `RWI Agent.replaced-before-guard.app`。随后替换同路径应用、strict codesign verify并bootstrapped既有LaunchAgent；**实际安装SHA256为上述2045fc…，与签名候选一致**。
- 北京时间 **2026-10-02 00:03**，已加载MCP直接读到新Agent **PID92469**、instance **`08ac04cf-a444-403d-9839-8933259b245d`**、healthy、input fence false；四项TCC与screenshot API均true，LaunchAgent PID独立相符。未改MCP/Plugin/配对/配置、未重置权限、无需重启Codex，未提交或推送，Paint215对象源未改。
- 新target **`mac-host:08ac04cf-a444-403d-9839-8933259b245d:0d6f8bc1-eaa4-4cd0-967a-7172fb9ba980`** 下重新精确发现/绑定PID61792/CGWindow1671：process start `61792:1790826329:801087`、native epoch `f9e9cfa03d0fa3f8da60574f35d9eb14d942c3cd5abfd826b4f3732620960bc6`、bounds `[0,0,1800,1169]`，仍为内建主屏；`activated=false/input_sent=false`且资源已释放。未移动/操作游戏，未对来源worker发命令。
- 实际snapshot得到 **1512×981** preview、原图 **3600×2338**、0controls、`visual_only=false/blocked=false/mutation_ready=true/input_domain_fenced=false`，总2476ms（observe1993ms、image481ms）。实际[部署后游戏画面](../../outputs/validation/2026-10-02-game-final-guard/01-post-deploy-game.png)和[部署只读证据](../../outputs/validation/2026-10-02-game-final-guard/02-deployment-readonly.json)保存于outputs，不加入Git原始截图。此前d738…下所有target/frame/view停止使用，游戏会话应自行refresh；这里frame/view只是短期验收证据，不能交给另一context使用。
- 新服务再次durable query原失败请求，**原result字节结构未变**，仍记录当时uncertain/fence true且`replay_allowed=false`。当前域false来自此前正规恢复保留，而不是新版删除/改写旧回执。
- **实际最终拒绝/准备事件only路径尚未在游戏中复验**：本任务只进行了新服务与绑定/截图验证，没有发送游戏点击、故意触发stale或重放旧请求。后续由游戏会话在新的授权步骤/独立意图里检查逐维度诊断和输入域状态；不能称所有stale已解决、豁免在真实游戏失败中已验或完成V14复刻。
