# RCU 新设备服务与 MCP gateway：独立静态复核 r1

日期：2026-09-11 UTC（本地交付时间见证据 JSON）。

## 结论与范围

**新分层在源码层面已转向批准的架构，但当前候选存在 4 项 P1、2 项 P2，尚不能据此宣布“可稳定供模型使用”。** 这些是开发中候选的静态问题，不是声称已在用户 VM 上复现的故障。

审阅基于 HEAD `979f6f0c84a55b9bf8c6534378f258501f055938` 上的未提交工作区；实现任务同时在修改代码。每个引用以证据包内的文件 SHA-256 和摘录为准，不把整个工作区说成某一时刻的原子快照。后续修复需要按新版本重新核对。

本轮只读源码、配置结构和已有测试源码，并生成评审文件：**没有运行 Cargo 测试/构建，没有调用产品 MCP，没有连接或操作 VM，没有安装或重启软件，没有修改产品源码、Codex 配置、证书或持久化操作记录。** 所列复现步骤均为待执行测试设计。

参考项目身份已由用户明确确认为 `alper-han/CrossMacro`；原项目与本地改动的对照继续沿用已交付的源码审计，不重新混同为 Python 后端。本轮关注的是新 Rust 服务的具体行为边界，而不是用实现语言推断性能、内存安全或模型信任。

### 已有的正确方向

- 生产 adapter 使用稳定设备 profile；工具目录来自编译进程序的 catalog，不从业务任务或测试 golden 文件加载。
- 默认依赖声明将旧业务工作流和本机平台后端置于显式 legacy feature；新设备服务按 principal、context、原生执行、持久化预约分层。这里是源码检查，不冒充重新跑过的依赖图验证。
- 设备不可用和存储问题不会直接否定服务元数据的可读性；设备 RPC 不自动重试输入。
- Core 在原生调用报错时保留 `dispatch=uncertain`；不能因为外层拿到一个返回对象，就误判为成功派发或业务目标完成。

## S01 · P1：裸字符串操作句柄会把查询/取消路由到另一台设备

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway.rs:79–95,196–200`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/device/api.rs:151–155`。

`remember` 把 `target/frame/artifact_id/lease/view_id/operation_id` 全部放进同一个 `String → device_id` 表，并以 `insert` 后写覆盖。`computer.operation` 和 `computer.cancel` 的 `device_id` 是可选项，缺省时直接使用此表。因此两个合法设备使用相同的 caller-supplied `operation_id` 后，先前设备的记录会被覆盖；没有歧义检查，也没有句柄类型或设备代际绑定。

**可确定的错误路径：** 在设备 A、B 各启动 `operation_id=op-1`；分别带明确 `device_id` 查询两者状态，让返回值先后进入 `remember`；随后只传 `operation_id=op-1`。结果被路由到最后写入的设备 B，而不是报告歧义。在两个操作均未结束的情况下，同样路径可取消 B 的操作，尽管调用者意图取消 A。操作完成后的查询也会混淆证据来源。这里不依赖随机 UUID 碰撞。

**修复要求：** 用含类型、设备和 context/instance 绑定的 opaque handle，或保留多值映射并在歧义时要求明确设备；不得静默选择最后出现的设备。耐久操作 ID 与短期 context handle 的生命周期要区分；重连失效不能靠猜测新路由修复。

**关闭条件（待运行）：** U/P 两设备同名操作，交错 status/cancel；验证 A/B 接收计数、返回设备身份及仅正确操作的 `cancel_requested` 变化；再测跨类型同字符串与 agent instance 改变。关联架构验收 V07/V16。

## S02 · P1：MCP 取消没有传播到设备侧实际操作

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway.rs:352–360`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/device.rs:66–107,148–157`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:375–400`。

当前 `call_tool` 丢弃 `RequestContext`。本地锁定的 rmcp 1.7.0 在收到 `notifications/cancelled` 后取消该 context 的 token，但不会替 gateway 自动调用设备 RPC `Cancel`。设备连接断开也不等于取消：服务端在处理阶段仅 `await handler`，没有消费断线信号；已开始的 `spawn_blocking` 原生工作不会因为等待方消失就自动停止。

因此，Host 若以 MCP cancellation 或丢弃连接表达停止，当前链路不能把该意图传到正确的 device/context/operation。现有显式 `computer.cancel` 和 Core 的 cooperative checkpoint 不能证明 Host Stop 已接通。该判断不假定 Codex 的停止按钮必然采用某一种内部实现。

**修复要求：** 在派发前建立 MCP request 与设备操作的精确关联，消费取消 token；为已派发调用发送有界的、身份绑定的 Cancel，并处理“取消先到、预约后到”的竞态。停止后不再发送后续输入批次；已经进入不可抢占原生调用的部分必须继续报告 pending/uncertain，不能提前释放租约或声称已撤销。不得通过重新发送原始输入来“恢复”取消。

**关闭条件（待运行）：** U 阻塞/多批次 fixture 的 dispatch 计数；P 实际 stdio 发送 cancellation、EOF 和断连；分别覆盖预约前、首批后、原生调用中。记录无后续批次、正确操作被标记、资源未虚报释放、另一 context 的只读操作仍可用。Host/N/M 效果另外验证。关联 V04/V05/V08/V15。

## S03 · P1：一次不确定写入会持久封住整台设备，缺少恢复协议

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:41–51,275–281,404–427`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/ledger.rs:82,117–121`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/device.rs:39–45`。

Runtime 只有一个设备级 `input_uncertain`。进入原生派发后的不确定结果或结果落盘失败会将它设为 true；没有解除/对账路径。保存的 `input_domain_fenced=true` 结果又被 `Ledger::unfinished` 在重启时继续视为未完成，因此新 context、新 principal、重启服务都不能恢复该设备的写入。RPC enum 中不存在 reconciliation 操作。

**这不是要求在未知状态下冒险放行。** 不确定输入必须防重放，原生执行尚未结束时也可能必须封住整个 desktop 冲突域。问题在于当前持久化预约只存 principal、两个操作 ID、payload hash 和状态，缺少原生冲突域、执行代际和派发阶段；服务既不能证明可以恢复，也没有提供让合法新操作重新准入的协议。相比旧服务 HOLD，它保住了只读，但“一个任务故障不应使后续任务永久失效”的要求仍未完成。

新 `Observed` 已增加原生输入计数，用于排除明确的派发前拒绝，这是有价值的修正；它不能消除真正不确定派发后的上述永久封锁。

**修复要求：** 持久记录受影响 native context/资源域、worker/instance 代际、已知派发阶段及证据；提供可核验、受授权的恢复/新代际准入流程。只有确认旧执行者不能继续派发并满足资源恢复条件后，才接纳新的操作 ID。旧预约和不确定效果必须保留，禁止清空 journal、直接重置全局 flag，或把“允许新操作”写成“旧操作未发生”。

**关闭条件（待运行）：** U/P 注入预约后崩溃、原生调用中断、结果丢失及存储故障；重启后旧 ID 不重放、受影响写入维持隔离、独立设备只读可用；通过恢复协议后仅允许合格的新操作。关联 V03/V08/V09/V18。

## S04 · P1：外层与 Core 的幂等命名空间不一致

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/ledger.rs:48–56`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:149–159`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/executor.rs:80–101`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/lib.rs:716–719,752–774`。

外层持久化键为 `hash(principal, request_id)`，明确允许两个 principal 使用同一个请求 ID。然而所有 context 的 Engine 共享 `state_root/native`，executor 把原始 `request_id` 不变地传给 Core；Core 的日志路径只含 `native Identity.key + request_id`。原生 Identity 是 host/guest/session/seat/desktop，不含 mTLS principal。

两个获授权的客户端在同一桌面顺序执行不同的新动作、都使用 `request_id=one` 时，外层均允许创建预约；第二个客户端却会命中第一个客户端的 Core 日志。动作 fingerprint 含各自不同的 context target，因而返回 `idempotency_conflict`，阻断一个本应独立的新动作。`act_checked` 的路径有相同问题。互斥 lease 不能解决这个顺序发生的冲突。

**修复要求：** 明确操作的稳定命名空间，并在进入 Core 时做一致、无歧义的 principal/device/logical-operation 映射；同步覆盖 receipt、verify 和 lifecycle 的相关引用。不要简单将整个 Engine state 按 principal 分目录，以免顺手隔离了本应跨客户端共享的原生输入租约。

**关闭条件（待运行）：** 两个真实认证 principal 在同一 native identity 上分别获得租约、执行同名但各自新的请求，应各派发一次；同一 principal 的重试仍不得再次派发；改参数冲突、重连和重启同样覆盖。当前 Ledger unit 只断言第二个 principal 的外层预约是 New，不足以证明经过 Engine 的行为。关联 V06/V08/V16。

## S05 · P2：能力声明与真实授权不一致

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:54–89,263–267`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/device/catalog.rs:33–37`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/native/service_policy.rs:76–85`。

Describe 将所有工具的 `implemented` 与 `available` 同时设置为“backend 是否构造成功”，没有使用各项 native capability 和当前授权/状态。一个明确矛盾是：`observe=true,input=false` 的 principal 会看到 `computer.claim.authorized=true`，因为 catalog 将 claim 标为非写入；真实 Invoke 却特判 claim、返回 `permission_denied`。Windows 当前 service policy 没有 launch permission，也不能因 backend 构造成功就将 launch 统一报为可用。

**修复要求：** 分开实现支持、当前原生可用、principal 权限、所需资源/租约和动态封锁状态；Describe 与 Invoke 共享同一授权判定。不要只把 `claim.writes` 改成 true：这个字段还控制写入预约与 `request_id` 要求，直接改会引入另一种不兼容。

**关闭条件（待运行）：** U 权限/平台/输入封锁矩阵，逐能力比对 Describe 与 Invoke；unsupported 平台应诚实报告，不能把暂时没权限当成“未实现”。关联 V10/V21。这一项直接影响模型是否相信工具给出的能力信息。

## S06 · P2：EOF 清理逐设备串行等待，缺少整体退出期限

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway.rs:236–242,400–404`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/device.rs:15–16`。

stdio service 结束后先 `await gateway.close()`，后面才进入 runtime 的两秒 shutdown。close 对所有已建立 context 的设备串行发送 RPC；单设备连接等待上限五秒，调用等待上限九十秒。只要此前打开过的某个设备不再响应 CloseContext，就能拖住后续设备的清理和 adapter 的正常退出；多个设备的等待还会累加。末尾的两秒不能覆盖此前的网络等待。

**修复要求：** 对 gateway 清理设置总时限、有限并发和明确的 best-effort 语义；超时只说明客户端清理不完整，不得宣称远端操作/资源已停止释放。与 S02 的操作取消分开处理。

**关闭条件（待运行）：** P 在已打开两个以上设备 context 后令其中一个 CloseContext 无响应，再关闭 stdio；验证其他设备清理能继续、adapter 在设计的总期限内退出。报告实测时间，而不是把上述超时常数当成已跑过的性能数据。关联 V02/V05。

## 如何关闭本轮评审

1. 按问题提交可审阅修复，给出候选 commit/源码 SHA 和新增测试路径。
2. 先用对应的 U/P 场景验证代码边界；Windows 原生、Host 接入、实际模型任务分别留存各自证据，不互相替代。
3. 本轮没有增设“所有离线回归先过才能做任何实机实验”的人为门槛；但未验证的项不得写成已完成验收。
4. 不重建任务专用服务，不删除旧不确定操作，不重新发送旧输入，也不擅自变更现有 MCP 安装。

### 证据校正

首轮快速反馈中将 `observed_backend.rs` 与 runtime/ledger/gateway 一并写成“回读 SHA 未变化”不准确：前三个文件在本轮引用快照中保持原 SHA；`observed_backend.rs` 从 `847683f7…` 变为 `38c8fd67…`，已按新文件回读。`checked_action.rs` 也已相对最初清单变化。本报告不依赖旧的派发前误封锁推断，并明确承认并行开发的版本变化。

机器可读记录包含初始清单、逐文件摘录 SHA、本轮发现与待执行断言。审计包仅含文本/JSON 证据，不含密钥、原生截图、可执行文件或开发源码副本。
