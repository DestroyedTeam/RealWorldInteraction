# 7fbbeeb：通用设备服务与 MCP adapter 独立复审

## 结论

**确认本次迁移的局部成果；不批准为完整 ADR 0002 架构验收通过。** 原 S01、S04、S05、S06 的具体缺陷已有修复和对应现成 U/P 证据；S02、S03 部分闭合。本轮发现 **3 项 P1 剩余缺陷**，另确认原生 worker 隔离等结构仍未完成。

- 固定候选：`7fbbeeb714dd0f5274c8d3fab16f614bcc240d59`。
- 正式架构基线提交：`98dc3aec1d8d262df4b2492e2bdf196afa87977f`。ADR 0002 是整套必须遵循的架构，P0–P6 只是交付拆分。
- 源码从指定 Git object 读取；实现任务后续修改不改变本报告对象。
- 本轮未改产品代码、未运行 Cargo/产品测试、未启动 Host/native 进程、未连接 VM、未输入或截图。视觉检查使用的是已保存 RPC 中原有 PNG，解码后 SHA 与原回包一致，不是新采集。

“关闭原问题”仅指对应缺陷及本次检查范围；不等于整个子系统、操作系统或模型任务验收完成。

## 1. 已核实的证据，而不是仅接受自报

### U/P 原日志

实现方证据索引列出的 **20 份原始 response 文件，其大小与 SHA-256 全部一致**。两份回归日志的 SHA 与索引一致，逐条 `test result` 汇总为：

| 实现方保存的日志 | passed | failed | ignored | 本轮行为 |
|---|---:|---:|---:|---|
| default-regression-current.log | 192 | 0 | 1 | 只读核对，没有独立重跑 |
| all-feature-regression-current.log | 283 | 0 | 2 | 只读核对，没有独立重跑 |

已核对对应测试源码与日志中的成功条目，包括双设备冲突/取消/EOF、双 principal 回执与共享租约、取消先于注册、按域持久恢复及零设备正式目录。Clippy 和 ratchet 保存日志显示成功；ratchet 本身明确 `architecture_complete=false`。本轮没有重新执行 fmt 或推断完整构建命令的参数。

默认 transport 依赖树保存文本未出现 `rcu-windows`、`rcu-macos`、`rmcp`；adapter 树未出现平台 driver。候选 Cargo feature 和模块门控与这些记录一致。旧 workflow 仍可通过显式 legacy feature 编译，不能称已物理迁出。

### H 与有限 N

- 两份实际 Codex Host 的 server inventory 中均有 **20 个 `computer.*` 正式工具**，未出现 `DIAGNOSTIC ONLY`。这证明所记录的 Host 接入，不证明全局插件安装或模型自主推理。
- 已目视复核 response-0009、0013、0017 的既有原图：**Processes → Performance/CPU → Processes**，回到 Processes 后搜索框仍是 `rcu`。三个 PNG 均与对应 `png_sha256` 匹配。
- 三个新观察的绑定均为 Task Manager 同一 HWND/PID/native epoch，`document_state=null`。checked action 的源码已按 native-element policy 不再一律要求文档内容。
- 计数依次为 SendInput `0 → 1 → 2`、插入事件 `0 → 3 → 6`；两次动作都保留 `goal_verified=false`，两次 release 回包均为 true。原图/UIA 页面变化与输入 ACK 分开检查。
- 记录中的后续 Host 查询返回第一项操作的耐久回执，`replay_allowed=false`；这组无 ID 冲突的实例成立，不能覆盖第 3 节 R01 的交叉命名场景。
- 最后两次独立标题采样匹配 Task Manager，只证明那个标题谓词；不是 Performance 页面变化的替代证据，也不是完整业务成功证明。

没有将两张任务管理器的内存读数用于性能优越性、泄漏与否或长期稳定性结论。没有重新验证 PID 6820 当下仍然存活；安装状态和进程身份是实现方保存的历史证据，不是本轮实时探测。

## 2. S01–S06 逐项裁决

| 原问题 | 本轮裁决 | 已核对的修复与证据 | 不能扩大的结论 |
|---|---|---|---|
| S01 双设备同名操作后写覆盖 | **原缺陷关闭** | gateway 使用 `kind:value → set<device>`，交集路由与歧义拒绝；P 测试确实使用两个认证 synthetic peer，并验证明确 device ID | 不是所有 ID/epoch 场景完成；耐久查询仍有 R01，瞬时操作句柄跨 context/instance 的测试仍需补齐 |
| S02 Host 停止不传播 | **部分关闭** | 消费 MCP cancellation token；有限定设备/context 的 Cancel 和 future-drop guard；设备侧记录注册前取消；P 覆盖已开始 Invoke 后的 cancellation/EOF | R03 的无订阅者 EOF 闩锁仍有缺陷；P peer 只验证 Cancel 收到，不是原生多批次输入停止与资源释放的 N 证明 |
| S03 不确定写入永久封锁整个设备 | **部分关闭** | 按 native identity 建域，保有服务状态所有权；RecoverInput 检查域、写者空闲及独立按键状态；恢复不删除旧预约 | R02 的恢复记录崩溃窗口仍可永久封住该域；原生 worker 隔离尚未实现，详见 A01 |
| S04 两 principal 的 Core 幂等冲突 | **原缺陷关闭** | `Engine::with_lease_root` 分离 principal receipt root 与 shared-native lease root；U 测试真的进入两次 Engine checked action，并检查跨 principal 争用被拒绝 | 不代表所有 ID 别名与跨 Host 查询正确；R01 单独跟踪 |
| S05 readonly claim 被报 authorized | **原误报关闭** | capabilities 明确将 claim/recover 归为需要 input 权限；只读 principal 的 claim 为 implemented=true、authorized/available=false；未配置 launch 明示不可用 | 设备级粗粒度能力不是全部动态前提的完整 CapabilityDescriptor；不能由 20 个工具推断 20 项当前都能执行 |
| S06 多设备 CloseContext 串行拖住 EOF | **原缺陷关闭** | 清理改为并发且共享三秒总预算；P 对两个都不返回 CloseContext 的设备断开 Host，断言在六秒内退出 | 这是该 P 场景的上界断言，不是精确退出延迟测量，也不代表远端 native worker 已终止 |

主要源码与测试定位均在附带 JSON 和固定候选摘录中。下文行号全部指 `7fbbeeb`，而不是后续工作区。

## 3. 必须修复的剩余缺陷

### R01 · P1：耐久查询混用 request_id 与 monitor_id，重连后可能返回另一动作

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/ledger.rs:105–112,124–130`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:480–507`。

调用参数的 `request_id` 被保存为 record 的 `operation_id`，调用参数的 `operation_id` 被保存为 `monitor_id`。reserve 只在各自用途上检查复用，没有阻止下面两个合法动作：

| 动作 | request_id | operation_id（monitor） |
|---|---|---|
| A | req-a | op-b |
| B | op-b | op-c |

同一 principal 顺序完成两次动作后，在原 context 查询 `computer.operation(operation_id=op-b)`，registry 返回 A；新 Host/context 的 registry 没有 A，便进入 Ledger::status。该方法**先**查 `hash(principal, "op-b")`，命中 B 的 request key，根本不再尝试 A 的 monitor。于是同一个查询 ID 在重连前后对应了不同动作的证据。这里不是跨主体权限绕过，但会直接破坏结果归属与模型的后续判断。

**修复边界：** 用类型化的查询引用/明确 ID kind，或对多种候选进行完整歧义检测；也可建立真正统一且受验证的唯一 ID 空间。不能只把两个查找分支调换顺序，否则只是换另一种串号。已有记录必须可查且不得自动重放。

**关闭条件：** U 构造上述合法交叉 ID；P/Host 在旧、新 context 分别查询，均返回 A 或明确拒绝歧义，不得静默返回 B；追加同字符串不同类型、不同设备/主体以及旧瞬时 operation 跨 instance 的用例。该故障由源码路径推导，本轮未运行复现。

### R02 · P1：恢复记录的半写文件会再次制造永久输入封锁

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/ledger.rs:163–201,284–301`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime/recovery.rs:99–109`。

恢复记录使用固定 `recoveries/<pending-snapshot>.json`，直接 `create_new` 后写入并 fsync。若在创建文件后、完整写入/同步前崩溃，空或截断文件会留在该路径。重启后读取失败让 `fenced=true`，这是正确的 fail-closed；但下一次由合法 principal 发起、且原生资源检查已经通过的 recover，仍对同一路径 `create_new`，只会得到 AlreadyExists。snapshot 没变，写入被封住，也无法用正常新动作改变 snapshot，因此 API 没有走出该状态的路径。

现有 `corrupt_reservation_or_recovery_never_unfences` 测试只证明坏文件不盲目放行；未证明重新检查资源后能提交一份新的合法恢复证明。**防止误放行与允许受验证恢复必须同时成立。**

**修复边界：** 使用崩溃安全的记录提交，以及能保留坏记录证据、为同一 snapshot 提交新证明的重试机制。保留原操作预约与效果未知结论，不用删除 operation journal、直接清标记或重发原动作“解决”。

**关闭条件：** 在 create、部分 write、完整 write、sync/提交各边界注入中断；重启保持隔离；再次核查资源后可恢复新操作准入；原 ID 仍不可重放；其他 domain 状态不变。本轮未做磁盘故障注入。

### R03 · P1：EOF 状态在没有 watch receiver 时会丢失

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway.rs:65,256–267,534–540`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/tests/gateway_transport.rs:266–282`。

Gateway 构造只保留 `watch::channel(false).0`，没有常驻 receiver。Stop::drop 使用 `send(true)` 并忽略返回值。本地锁定的 Tokio 1.50.0 源码明确：没有 receiver 时，`send` 返回错误且**不保存新值**；后来 subscribe 的 receiver 仍会读到 false。

因此 EOF 在当前没有 Invoke subscriber、但调用尚在 stdin/任务调度缓冲区的窗口到达时，所谓“Host EOF 后不得接纳新操作”的闩锁未置位。后续 invoke 不能从这条状态可靠判断 Host 已断开，是否仍被另一个 MCP cancellation token 或传输关闭时序挡住取决于调度；不能靠这种竞态保证不再转发。

现有 P 测试特意等待设备收到 `slow-eof` Invoke 后才关闭 stdin，那时 receiver 已存在，所以不会覆盖此边界。

**修复边界：** 使用能在无订阅者时保持状态的单调闩锁，例如 `send_replace(true)`/适当的持久取消原语；同时保证退出路径与后续订阅者都看到停止。取消仍不得等同于撤销已发送输入。

**关闭条件：** 确定性地把调用暂停在 subscribe/forward 之前，先触发 EOF，再放行调度；断言没有原始 Invoke 发到设备。保留“已进入调用后取消”的既有 P 场景。这里确认的是状态丢失的源码事实，未声称本轮在 VM 上复现了 EOF 后点击。

## 4. ADR 0002 的剩余偏差与交付缺口

### A01：原生 provider 的隔离与可回收执行者仍需实现，不仅缺测试

`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/runtime.rs:400–461` 仍在 agent 同一进程中用 `spawn_blocking` 执行 native backend，并持有 context/domain permit。当前 Windows backend 诚实声明没有 strong native cancellation，UIA 配置的 250 ms provider 超时与 cooperative checkpoint 不构成通用的进程级挂死/崩溃隔离。

如果原生调用不返回，取消只设置标志，恢复检查仍会发现 pending；它没有一个可被独立回收、确认已停止且不连带杀掉设备服务的 owned worker。因此 ADR §6.2/§10、V05/V09/V22 仍有**实现缺口**。应实现受控执行者生命周期、隔离、所有权与代际 fencing，再验证挂死/崩溃；不能把“诚实报告 pending”当作完整恢复能力，也不能把杀整个服务作为所有 context 的唯一恢复方式。本轮未注入真实 UIA 挂死。

### 其余必须继续完成的项

| 项目 | 本轮定位 |
|---|---|
| compact、按需语义/图像与完整证据取回 | 实现报告已承认未完成；本轮观察回包约 230–347 KB，structured JSON 约 82–115 KB，仍不能宣称模型 token/理解效率改善。原图按 MCP artifact 获取与完整语义/审计证据获取要区分，不能让外部脚本成为产品必经路径 |
| 原生应用启动策略 | 当前 capability 明报 `application_policy_not_configured` 是正确降级，不是通用 launch 已实现的证明；不得重新增加业务 seed 才能启动 |
| legacy 物理迁移 | 默认 production graph 的边界成立；显式 legacy 模块仍在库中，应按已批准清单迁到验收层。只在 feature 后面不等于整体迁移完成 |
| 原生与权限矩阵 | 当前只有 Task Manager 非文档按钮的有限 N；键盘、焦点/用户中断、窗口关闭/弹窗、多显示器/DPI、ordinary/admin、锁屏、取消/崩溃均须分别验证 |
| 平台与资源类型 | agent 当前接入 Windows，Fixture 不代表 Linux/macOS/其他系统实现；桌面、显示器、窗口、进程的通用资源契约与各平台限制仍需完成 |
| Host/Plugin/M | 已记录项目级真实 Host 工具接入；全局插件安装、模型自主选择/操作/恢复和业务完成度仍未验收 |

没有把这些缺口解释为“批准一个暂时不同的架构”。它们仍属于完整 ADR 的交付义务。本轮也不增设任何新的 VM 授权门槛或“必须全套离线回归后才能继续做实机实验”的流程限制。

## 5. 下一次复审需要的最小材料

1. R01–R03 的可定位修复提交，及上述边界对应的测试源码/原日志。
2. A01 的执行者隔离与代际恢复设计/实现证据；若尚未实施，进度报告直接列为结构缺口。
3. 保留 H、N、M 分层证据：工具被发现、原生点击生效、模型自主完成任务是不同结论。
4. 保持旧操作不可重放；不以清理旧账本、换业务任务或禁用正常 MCP 入口绕过问题。

本轮只确认固定候选和保存证据的上述范围。完整架构验收状态继续为 **未完成**。
