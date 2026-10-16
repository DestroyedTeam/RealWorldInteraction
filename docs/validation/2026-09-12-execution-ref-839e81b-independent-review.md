# ADR 0003 执行身份候选：839e81b 独立复核

## 固定结论

候选：`839e81b4a2a124b94cbc2026fddedb51ac5a5ca3`。

**执行引用与耐久请求分离的核心实现及主要 U/P 用例成立；R01 整体暂不关闭。** 本轮发现一个接受后取消失败的 P1 接口缺陷，以及一个新状态命名空间初始化的 P2 崩溃一致性问题。A01 owned-process worker 仍未实现，完整 ADR 0002 架构和模型 M 层均未验收。

本轮读取固定 Git object、测试源码及实现方补充的原始 gate 日志，没有修改产品源码/项目配置、运行 Cargo、启动 Host/native、连接 VM 或操作 GUI。后来收到的 `24fac6b` live 记录不混入本次 U/P 裁决，本报告不宣称完成了该组 live 证据的逐项复核。

## 1. 已确认的机制

### 服务签发身份，不再靠标签猜测

- device wire 为 v2，`OperationRef` 包含 reference version、device、agent instance、context generation 和服务生成的 execution nonce。真实 principal 在 Context 的服务记录中校验；不是依靠引用里的自报主体获得权限。
- 查询采用互斥 tagged selector。执行引用查询与明确的 device/request_id 耐久查询走不同分支；取消只接受 `operation_ref`。旧裸 monitor/auto 无公开查找入口。
- `resolve_execution` 校验当前设备/实例、主体所属 context、nonce 及完整引用相等。不存在或过期直接返回 `execution_ref_expired`，不退回 request 或 correlation 标签。
- correlation 在 Prepare 时从实际执行参数和耐久 fingerprint 中移除。历史 ledger 的旧字段保留原字节，但 monitor 不再用于查找或唯一性判断。
- adapter 的操作路由使用明确的 selector/reference envelope，且只发往已配对设备；不再从任意回包里的 monitor/request 标签构造操作映射。

### 接受和派发分开，但不增加模型业务步骤

- Prepare 只做参数/权限/context 检查、容量清理和内存接受记录，不调用 native provider，也不获取输入租约。输入授权的来源仍是服务配置，不是业务 manifest。
- Execute 和 Cancel 对同一个 Execution.phase 加锁；取消先于 begin 时不能 Execute，已 Running 的实例不能再次 Execute。
- 在真实 MCP 子进程的测试中，EOF 发生于 Prepare 回复之前时没有 Execute；在实际 Runtime/mTLS 的组件测试中，已拿到 ref 后就绪的 Stop/取消也没有 Execute。
- 已进入 Execute 的取消仍是 cooperative，不承诺撤销已发送输入或强制抢占 FFI。这个限制与 A01 分开保留。

### 未结束 native 所有权没有被普通过期/配额清理丢掉

`Execution::retirable` 要求 Running 已 terminal、没有 native_call_pending，并且资源已释放或处于域隔离。Context close/GC 还检查 gate 和 `native_quiescent`。worker 持有 context/domain permit、Finish 和 Context Arc，不把它们绑在 async 等待方寿命上。

已有单测保有一个真实 Core NativeCall 作用域，先请求 finish，再制造超过 64 项的接受/取消配额压力。原 ref 仍可解析、pending/非 terminal 保持、CloseContext 被拒绝；作用域结束后才允许终结记录过期。这不是实际 OS worker 挂死测试，但在本轮检查路径中没有发现“仅因 TTL/配额就删除 pending native 控制记录”的代码缺陷。

## 2. 阻塞问题 ER01 · P1：取消 RPC 失败时丢失已签发的 operation_ref

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway/cancellation.rs:20–45`；对照 `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway/invocation.rs:110–139`。

`dispatch_accepted` 已取得服务签发引用，并且在普通 Execute RPC 失败时会把该引用补进错误结果。但 Host Stop 选择取消分支后，`CancelDispatch::request` 遇到两秒超时或连接失败直接返回 `Err(e)`，没有把手中的 `operation_ref` 附回。

这对没有耐久 request_id 的长只读调用尤其重要：引用此前只存在于 adapter 内部，原 Execute 等待已被取消，Cancel 又未确认送达。agent 可能仍正确保有 pending native 操作，而调用者收到的 `cancellation_delivery_unconfirmed` 或 `device_connection_failed` 不含其唯一公开定位符；既不能以该 ref 查询/再请求取消，也没有 durable request 可用于定位。拥有权未必丢失，**调用方的可追踪性却丢失了**。

**修复要求：** 所有已接受后的错误/取消返回都保留精确 `operation_ref` 及取消交付是否确认；保留原错误代码和不确定性，不包装成“已停止”。不得重发原 Execute 或用 correlation 标签找回操作。

**关闭条件：** 分别令 Cancel timeout 和连接断开；覆盖接受后尚未 Execute、以及 Execute 已 pending 两种情况。返回结果仍含原 ref，恢复控制连接后用该 ref 查到同一执行；原始 Execute 计数不得增加。当前成功的取消测试没有覆盖这一错误回包分支。本轮结论来自固定源码推导，没有注入 native 网络故障。

## 3. ER02 · P2：device namespace 首次写入中断会留下不可重试的启动失败

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime/namespace.rs:7–29`；调用位置 `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:58–71`。

新增 namespace 能拒绝已经绑定的状态根被另一 DeviceId 复用，这项正常路径已有单测。但初始化对最终文件 `device-namespace.json` 直接 create_new→write→sync。若创建后进程结束，或 write 因磁盘满等原因留下空/截断内容，下一次启动会走 AlreadyExists 分支并解析失败。即使 IO 条件恢复、DeviceId 未改变，Runtime 仍在打开 Ledger 和提供服务前退出，没有受验证的重试提交路径。

这不表示应忽略坏身份记录，更不应删除 namespace/账本重新绑定设备。应使用崩溃安全的首次提交策略，并区分未提交初始化与已有权威绑定；已有坏绑定的修复必须保留证据、验证原设备身份，不能默许换 DeviceId。

**关闭条件：** 首次创建/部分写/完整写/同步/发布各边界中断后，同一已证明的设备可正确重试或进入明确的修复流程；不同 DeviceId 仍被拒绝，旧预约不变。本项是启动恢复缺陷，不是目前已成功创建 namespace 的现场 agent 已发生故障的声明。

## 4. 已有测试与日志的核对

最初 gate 文件只有 RTK 汇总。本轮要求并获得了未过滤原日志，结束标记已存在，开始/结束记录均为 `839e81b4a2a124b94cbc2026fddedb51ac5a5ca3`。

| 原日志 | 独立汇总结果 |
|---|---|
| default-tests.raw.log | 215 passed / 0 failed / 1 ignored，31 个结果块 |
| all-features-tests.raw.log | 302 passed / 0 failed / 2 ignored，35 个结果块 |
| clippy.raw.log | 完整结束记录；本轮没有重新执行 |
| architecture.json | passed=true、无 violations；architecture_complete=false |

已从原日志确认以下具体用例有成功条目，并对照源码：

- context 关闭/agent 重建后，旧只读 ref 不命中复用标签的新执行。
- 更改 device/instance/context/nonce 或使用另一主体，Execute/Cancel 被拒绝。
- Prepare claim 不取得 lease，另一个主体仍可取得共享桌面租约。
- 原 durable request 跨 epoch 查询一致，改变 correlation 不导致重放，旧文件字节保留。
- 直接记录 Fixture.calls 的测试在换 context/agent 后仍保持两次，而非只凭返回状态判断防重放。
- 未结束 native scope 承受配额压力且拒绝 context close。
- 真实 MCP stdio 子进程在 Prepare 回复前退出，不发送 Execute。
- 新 MCP Host 没有旧路由缓存时仍可查询合法 live ref；原 context 明确关闭后 ref stale，显式 request 历史查询仍正确。

这些是对实现方保存日志的读取、统计和源码对应，不是评审者自己运行测试。补充的 raw 原日志和 SHA 已纳入本次审计包，不以最初 54 字节摘要代替逐测试证据。

## 5. 仍需保留的验证边界

- R01 原先的标签串号/代际混淆在这份候选中已被结构性替换；但 ER01 仍使接受后故障路径丢失执行引用，因此**不将 R01 整体标记完成**。
- 已开始后的 MCP 取消测试使用 synthetic peer；pending 所有权单测直接持有 Core native scope。还应补“实际 Runtime.dispatch → 阻塞 fixture → 经 ref 取消 → 后续批次计数为零”的组合用例，避免把分层测试说成已经验证了该整条路径。
- A01 owned-process worker 仍未实现。cooperative cancel、pending 状态和域封锁不能冒充强制终止/崩溃隔离。
- 本轮不判定新增 Windows live 证据，也不判定全局 Plugin 安装、多平台/复杂桌面矩阵、性能优势或模型 M 层。
- ER01/ER02 修复后应给新固定 SHA 和对应新增测试。不要移动本报告基线，或修改旧成功/失败证据来获得通过状态。

完整架构验收继续为 **未完成**。
