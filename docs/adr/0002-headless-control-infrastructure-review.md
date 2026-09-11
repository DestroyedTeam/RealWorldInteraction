# RCU 架构评审：无自有 UI 的多端控制基础设施

日期：2026-09-11。状态：**正式架构基线 / 用户已批准；尚非实现或能力验收声明**。

批准依据：2026-09-11 主任务中用户明确要求“不是首期采用，而是你要完全遵循这套架构设计去实现落地”。核心分层与解耦原则作为完整实现的约束；P0–P6 仅是迁移顺序，不是临时任务耦合的豁免。原始只读审计包保持不变。

评审对象：`/Users/vincentadamnemessis/AI Projects/Real Computer Use`，HEAD `aeb56851a37d570179eacb046ea0177b894a7002` 及本次记录的未提交工作树。代码身份以配套 SHA 清单为准，不能把未提交修复说成该 commit 已包含。本轮只读源码与本地旧证据，新增文档；没有连接 Windows、调用控制 MCP、部署、输入、运行模型、修改产品源码或提交代码。

## 1. 架构确认

**RCU 应是 AI Host 与受控系统之间的观察、操作和证据基础设施；不是固定 Mac↔Windows 遥控器，也不是预置业务工作流引擎。**

已由用户确定的方向：

- 无自有产品 UI；提供通用 MCP 工具、Plugin/Skills 和 headless native agent。
- Codex 是首个 AI Host；Mac 是首个接入宿主，Windows VM 是首个真实验收目标，不是架构中的固定角色。
- 面向本机、远程主机、VM；Windows/Linux/macOS/Android/iOS 以实际平台能力分级。未来设备扩展保留接口，不提前宣称支持空调、电视。
- 产品后端、执行、传输和 MCP runtime 使用 Rust；OS FFI 允许。Python/.NET/PowerShell/AppleScript 不得充当生产截图、输入或传输的隐形桥。
- 用户确认参考项目是 `alper-han/CrossMacro`，不是另一个待猜测的 CrossMark 项目。已核实的基线、补丁身份及移植缺口见[源码对照表](</Users/vincentadamnemessis/AI Projects/Real Computer Use/docs/architecture-crossmacro-comparison-2026-09-11.md>)。

本评审的关键结论不是“取消安全检查”，而是：**将安全检查放回正确的层次和影响范围。**

## 2. 当前偏离的源码证据

| 已核实的位置 | 事实 | 架构处理 |
|---|---|---|
| [Runtime::new，51–103](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/runtime.rs>) | agent 启动必须读取 `approved-task.json`、校验业务 Approval，创建 task 文件目录和总预算 | 服务配置不再持有某个业务任务；授权按调用作用域提供 |
| [AgentConfig / ClientConfig / Approval，83–121、205–251](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/protocol/types.rs>) | 安装配置、连接配置携带 task、approval SHA、文件 manifest、期望 UI 文本或 process goal | 拆出设备身份、客户端主体、授权和操作；业务目标迁往调用方/验收 |
| [connection，68–105](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/transport/server.rs>) | Model 建链先推进固定 `Stage::Model`，失败进入 task HOLD | 建链不推进业务阶段；连接与操作分别记账 |
| [Store::open / start_task，137–176](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/store.rs>) | 重启未完成任务产生 HOLD；旧任务不 complete 时拒绝新 task | 不确定操作记录保持不可重放，但不阻止建立新客户端或查询其他目标 |
| [EndProof::permits，38–52](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/store.rs>) | 资源结束证明还要求 model goal / file oracle / process oracle 成功 | 资源已释放与业务成功独立；失败业务也能正常结束会话 |
| [process_cleanup / application，45–110](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/deployment/plan.rs>) | ProcessCleanupSeed 复用 NotepadSeed，限定一个 Task Manager 窗口和 `Taskmgr.exe` | 用例不能进入部署/通用协议；不再增加更多业务 Seed |
| [worker_binding，842–846](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/control.rs>) | catalog 只允许 Activate worker；新清理流程在 Launch 调用时失败 | 目录/窗口发现是能力调用，不应受业务阶段限定 |
| [Server::execute_operation，59–101](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/server/mod.rs>) | 一个全局 semaphore 和 Engine mutex 覆盖工具执行 | 按设备/原生执行上下文隔离；状态、目录查询不能被一个 UIA 调用拖住 |
| [act_checked，75–116](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/checked_action.rs>) | 所有 checked action 都要求完整 document context | 保留新鲜度与语义校验，但按动作类型要求必要证据，不把整套文档扫描作为菜单/桌面/普通按钮的共同门槛 |

这里并不是说 `act_checked` 写死了 Notepad：`Documents::finish` 在完整遍历且没有文档节点时也能生成空集合摘要。问题是通用输入仍统一依赖该 document-context 的覆盖/预算要求，而非仅校验对应动作实际需要的上下文。

最新保存的 [process-cleanup3-model-setup.json](</Users/vincentadamnemessis/AI Projects/Real Computer Use/work/windows-native-plugin-20260911/process-cleanup3-model-setup.json>) 为 `ready_for_model_attachment=false`，Launch 错误是 `catalog_only_current_activate_worker`。这与上表代码路径吻合，**不是新的 Windows 业务成功记录**。

当前诊断服务器能暴露 19 项工具，但[其实现，22–74](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/integration/startup_diagnostic.rs>)来自记录的 schema，并让全部业务调用返回 `native_session_unavailable`。它解决了错误可见性，不是目标服务架构；不能以它替代真正的设备路由与通用执行层。

## 3. 角色、组件与连接方向

| 术语 | 精确定义 | 不代表什么 |
|---|---|---|
| AI Host | Codex 或其他承载模型/权限交互的应用，包含 MCP client | 不等于 Mac；模型本身不直接拥有 OS 权限 |
| MCP client | Host 中调用 MCP server 的协议端 | 不由 TCP 谁先连接决定 |
| MCP server / adapter | 对 AI 暴露工具、校验 schema、映射调用与结果、路由设备 | 不应包含 Notepad/CrossMacro 清理等任务流程 |
| native agent | 受控设备上的无自有 UI Rust 服务及必要原生 worker | 不因为反向连接就变成产品“控制端” |
| native worker | 在正确 OS 用户/会话/桌面和线程模型下执行平台调用的受控进程/执行上下文 | 不按每个业务阶段反复重建整套 MCP |
| operator / policy authority | 配对设备、授权主体、制定范围、撤销权限的一方 | 不应由模型伪造一个 approval JSON 自我授权 |
| workflow / evaluation | AI Host、Skill 或测试层对“我要完成什么”的组合与判断 | 不控制通用服务能否启动或接受下一任务 |

MCP 本身区分 Host、client 和 server；RCU 的控制/受控角色应建立在其上，不借用连接方向重新定义术语。[MCP 官方架构](https://modelcontextprotocol.io/docs/2026-07-28/learn/architecture)

```text
                        ┌───────────────────────────────┐
                        │ Codex / 其他 AI Host           │
                        │ 模型 + MCP client + 业务工作流  │
                        └───────────────┬───────────────┘
                          标准 MCP：stdio / HTTP
                     ┌─────────────────▼─────────────────┐
                     │ Rust MCP adapter                  │
                     │ 工具目录 / schema / 路由 / 展示    │
                     │ 不依赖业务 task、HWND 或 oracle    │
                     └───────────┬─────────────┬─────────┘
                          本地认证 IPC      认证设备链路
                     ┌───────────▼───┐     ┌───▼────────────────┐
                     │ 本机 agent     │     │ 远程 / VM agent    │
                     └───────────┬───┘     └───┬────────────────┘
                                 │              │
                      ┌──────────▼──────────────▼───────────┐
                      │ 身份 / 能力 / 授权 / 操作账本         │
                      │ 按资源冲突域调度，原生 worker 隔离    │
                      └──────────┬──────────────┬───────────┘
                         Windows / macOS    Linux / 后续平台
                                OS API / native FFI
```

这张图是逻辑分层，不要求每个盒子独立进程：本机适配可以共用库、远程服务可以独立部署。需要进程隔离的主要原因是 OS 会话/权限边界、线程亲和性与原生调用故障，而不是为了复制业务阶段。

支持的部署拓扑设计：

1. **本机**：Host→同机 adapter→本机 agent/worker；可使用受保护 Unix socket 或 Windows named pipe，不能要求为本机绕一圈远程 VM。
2. **远程/VM**：Host→任意可用宿主的 adapter→受控设备 agent；连接 IP 只是地址，不是设备身份。
3. **多 Host、多设备**：多个经认证的主体可访问同一 agent；允许独立只读调用，一个桌面输入流只允许一个 RCU 写租约。
4. **反向连接**：未来可由 agent 主动拨出，但身份、授权、路由语义不变；本轮不建设中继云、穿透平台或常驻网络扫描。
5. **嵌套 VM/RDP**：宿主远程桌面窗口与客体原生桌面是两个不同 target。没有 guest agent 时，只能声明操作宿主的远程画面，不能伪称取得 guest 原生窗口树和坐标。

## 4. MCP 与设备链路：需要独立协议吗？

MCP 支持 stdio、Streamable HTTP 和合规自定义 transport；额外设备协议不是 MCP 的强制要求。[官方 transport 定义](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)

| 方案 | 优点 | 成本/限制 | 本提案选择 |
|---|---|---|---|
| agent 直接提供标准 MCP，远程用 HTTP | 协议复用、其他 Host 可直接连接 | 多设备聚合、原生二进制图像、OS helper 生命周期仍需领域契约；远程认证也要真实实现 | 合理备选；不是错误方案，也不要求每个用户一定安装 Mac 中转 |
| MCP adapter + 小型 Rust 设备 RPC | 可复用已有 mTLS/帧/peer 绑定；MCP 与 OS 生命周期容易隔离 | 会承担一个内部版本化协议，必须防止重造业务状态机 | **推荐首期采用**，但删掉业务 Stage/Approval 绑定；共享领域类型，不复制两套工具逻辑 |
| 完整设备云/插件总线/IoT 工作流平台 | 远期扩展空间大 | 当前无已验证需求，成本远高于问题 | 不采用 |

首期内部 RPC 仅需：认证后的 `hello`、设备/能力查询、类型化调用、操作状态/取消、有限证据读取。每条消息有协议版本、关联 ID、目标引用、主体上下文、截止时间和容量边界；**不得出现 `NotepadSeed`、`ProcessCleanupSeed`、Oracle 阶段或预期业务文本**。状态/撤销不应与大图传输共享不可抢占的无限队列。首期有界并发请求即可，不强行引入通用事件总线、任意远程代码加载或多路流框架。

**协议版本事实**：本仓库锁定 `rmcp=1.7.0`，本机该 SDK 的 `ProtocolVersion::LATEST` 是 `2025-11-25`。2026-07-28 官方规范已改为逐请求版本/能力 metadata，不再需要旧式连接初始化，并定义 `server/discover`。因此本方案中的“客户端会话”是 RCU 调用/授权上下文，不能再绑定某一种 MCP 握手或业务会话。当前保留已测试的 legacy adapter，现代协议支持另立兼容验收，**不暗中升级 SDK 或宣称现已支持新规范**。[官方版本兼容说明](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)

## 5. 目标和能力模型

### 5.1 身份不等于地址，不等于窗口

- `DeviceId`：与受信设备身份关联的稳定 ID；IP、主机名、用户显示名只是属性。
- `DeviceEpoch`：设备/agent 实例代际；重启不丢失设备身份，但使原始句柄、frame、lease 等瞬时引用失效。
- `NativeContext`：OS 实际证明的 user/logon/session/seat/desktop。Windows 保留 SID、token/LUID、WTS 会话、WinSta/Desktop；其他 OS 按真实语义表示，不伪造 Windows 字段。
- `TargetRef`：服务端产生的 opaque 路由引用，包含/关联 DeviceId、上下文代际及资源身份。显示器、桌面、窗口、应用、进程不是同一资源类型。
- `ObservationRef`：绑定目标、捕获区间、拓扑代际、元素/图像来源；绝不是可以跨任务/设备复用的裸坐标。
- `ResourceScope`：先实现 Desktop/Surface/Window/Process；未来 DeviceFunction 不强制含 PID、HWND 或屏幕。不存在的平台字段用“不适用/不可获得”的类型表达，不用猜测的字符串填满 schema。

所有 ID、epoch、纳秒/100ns 时间和可能超过 JavaScript 精确整数范围的计数采用 opaque string 或声明过的十进制字符串；不让 FILETIME 经 JSON Number 往返失真。真实身份与路由配置的关联强度必须回报，TLS 认证不等于硬件/虚拟机证明。

当前部分 native 回包使用 `native-attested` 字样，不能据此声称硬件远程认证；已读实现是本机 OS API 事实。新契约应把 `native_api_observed` 与真正设备证明分开命名。

### 5.2 能力至少分开五个维度

```text
CapabilityDescriptor
  id + version                 例如 desktop.capture / input.pointer / window.list
  implemented                  代码是否存在
  availability + reason        当前 OS/设备/权限/会话是否可提供
  authorization                当前主体是否允许使用（独立于 availability）
  evidence + constraints       native / derived / partial / unverified
  limits + generation          大小、频率、坐标域、代际等
```

工具注册、设备能力、当下权限、当前授权、一次操作成功是五件事。`tools/list` 来自同一份编译期工具定义；平台能力通过设备查询返回。一个设备离线不应导致 adapter 工具目录消失；授权不可见的设备不应泄露给其他主体。离线列表可返回已配对设备及标明过时的 last-seen，不能伪造当前原生窗口清单。

### 5.3 图像、坐标和语义必须共同设计

- 每个图像明确原始/派生、像素尺寸、裁剪范围、颜色/alpha 规范、捕获时间、目标与拓扑代际、图像哈希。
- 坐标携带命名空间：图像像素、surface-local、display-native、desktop-native 或规范化输入域；不要裸传一个 `(x,y)` 就猜它属于 Mac 还是 guest。
- 混合 DPI、旋转、负原点、跨屏窗口采用逐 display/region 映射；不能假设一个全局仿射矩阵永远成立。映射不可证明时明确 unavailable。
- UIA/AX 树和截图不是天然原子快照；报告各自采样范围/新鲜度，派发前校验必要状态，不宣称消除了 OS 的所有 TOCTOU。
- 语义和视觉操作是显式能力/策略：支持 UIA Invoke 不代表支持鼠标可见点击，图像可见不代表能访问目标语义树；不偷偷改用 shell 或旧桥。
- 截图按需，语义-only 请求无需每次 PNG 编码；原图/preview/ROI 的关系可验证，完整证据可经 MCP 取回，不要求模型启动 Python 读取临时文件。

## 6. 各层独立生命周期

| 层 | 状态/事件示例 | 失效应影响的范围 | 不应依赖 |
|---|---|---|---|
| Deployment | absent→installed→upgraded/removed | 文件/服务版本、安装登记 | 文本目标、某窗口存在 |
| Service | starting→healthy/degraded→stopping | 当前服务实例；重启生成新代际 | workflow success / Oracle |
| Transport | disconnected→authenticated→connected→backoff | 一条设备链路/连接 | Notepad、业务 task ID |
| Client context | identified→authorized→closed/revoked | 当前主体/客户端上下文 | 单个应用生命周期；MCP 新旧握手差异 |
| Native context | available / locked / logged_out / permission_limited | 对应 OS 会话/能力 | agent 进程必须退出 |
| Target | live / changed / destroyed / stale | 对应 target/frame | 其他窗口和设备可用性 |
| Authorization | valid / expired / revoked | grant 指定的主体、能力、资源 | 安装是否存在；工作流是否成功 |
| Input lease | free→owned→released/expired/fenced | 同一物理桌面输入冲突域 | 只读状态/其他设备 |
| Action | reserved→dispatch_started→returned/uncertain/reconciled | 一个 operation ID；必要时其冲突资源 | 整个设备必须永久 HOLD |
| Workflow/evaluation | running→success/failed/cancelled | 调用方目标及验收记录 | 服务准入、下一个客户端 |

“设备 offline”“窗口关闭”“权限不足”“业务失败”都不是 `MCP server disabled`。MCP 配置开关只表达用户是否启用这个集成，不用来承载运行期状态。

### 6.1 重新连接不是重新执行

1. 网络中断可以重新建立认证连接；新连接生成自己的传输/调用上下文。
2. 操作 ID 与 JSON-RPC request ID 分离。操作 ID 在重连前后保持可查询；同 ID 不再次派发，同 ID 改 payload 返回冲突。
3. 重连后只恢复能力/状态/订阅（如以后实现），不自动重发鼠标、键盘、启动、关闭或进程终止。
4. 原 frame、元素和窗口 generation 失效时重新观察。**重新观察本身不是重放动作**；不能像当前 Skill 一样把正常恢复一概禁止。
5. 只有新的用户/模型意图、有效授权和新鲜状态可以形成新操作；不能通过换 request ID 把不确定旧动作伪装成重试。

### 6.2 不确定效果保持记录，但不永久封死服务

- agent 在 OS 副作用之前持久化不可重用的 reservation 与 dispatch 意图。这里只能保证明确约束下的“至多一次派发”，不能承诺物理 OS 副作用的 exactly-once。
- 回复丢失、worker 崩溃或超时可能留下 `effect_unknown`；结果查询保留它，不能伪造为失败且未执行。
- 在尚未证明旧 worker 停止、所有权/fencing 更新、已按下键和鼠标释放状态安全之前，**该桌面输入域**可以禁止新写操作。其他设备及经许可的只读访问继续可用。
- 仅业务结果未知、但已证明原生调用结束且资源释放时，不应继续全局封锁；同资源新操作需先读当前事实并重新规划。无法判定是否会重复高影响动作时，向调用方要求该范围的明确决定。
- agent 重启恢复账本、使旧 lease/瞬时引用失效，并检查旧 worker 所有权；不清除历史 reservation 来获得“干净开始”。账本损坏时降级对应写入能力，保留独立控制面；不能静默丢弃证据。
- 调用方 Cancel 的 ACK 不等于原生调用已停止。卡住的 FFI/UIA 由受控 worker 隔离；终止 worker 也不能倒推出已发 OS 输入被撤销。

### 6.3 多主体、共享桌面和用户中断

MCP `clientInfo.name`、请求里的 client_id 或 task 文本不是认证凭据。远程以真实认证主体授权；本机以受保护 IPC 的 OS 身份及受信 adapter 授权。若 adapter 代表多个不同权限的 Host，必须有真实委托边界，不接受它随意转发一个字符串就提升权限。

输入互斥必须在受控设备侧统一执行，不能让不同 Mac adapter 的本地状态目录各自取得“独占”。原生线程有亲和性时可使用每上下文 actor/worker；只读控制面与慢调用队列分离，不能把所有请求放进一把全局大锁。

RCU 的租约只协调 RCU，不会阻止人类或其他软件输入。检测到焦点/输入/会话变化后退让、失效旧引用并停止后续批次；检测覆盖不足须在能力中明示。多个窗口不等于多个独立鼠标/键盘流。

## 7. 工具接口与业务边界

| 工具族 | 设计职责 | 不应出现的业务耦合 |
|---|---|---|
| targets / status / capabilities | 经许可的设备/上下文/能力与可用状态；无应用也可查询 | task seed、先启动 Notepad |
| discover / bind_window | 发现、选择和绑定原生目标；分页、覆盖与过期明确 | 只在 Launch/Activate 阶段允许 |
| observe / image / artifact / resolve_view | 原生观察、派生图像和坐标映射、证据按需读取 | 每次必须截图；外部脚本才能拿到结果 |
| act_checked / act | 类型化输入及必要前置条件、派发一次和回执 | 所有按钮必须完整 document digest；自动改用另一工具实施同一不确定动作 |
| claim / release / operation / cancel | 冲突域互斥、取消与结果查询 | 独占整个 Model 连接或全服务 |
| launch_application / activate_window | 经许可的通用应用/窗口原语 | 只能启动 notepad.exe / Taskmgr.exe |
| verify | 独立读取一个明确原生谓词及其覆盖/采样时点 | 替整个业务流程盖“成功章”并解锁服务 |
| 后续平台专用能力 | 例如有界进程查询、窗口布局、剪贴板、会话信息 | 为“清理 CrossMacro”添加专属工具或名称黑名单流程 |

工具名和 v1 兼容策略在迁移阶段逐项确认，不把今天的 19 个名字当永久数量指标。`computer.targets` 可保持熟悉入口；如引入新 TargetDescriptor，必须有显式 schema 版本与兼容适配。通用受权进程查询/终止可以是平台能力，但**不能用它替代要求 GUI 路径的验收**；进程名模糊匹配不是安全的终止授权。

通用 `verify` 返回 `predicate_matched + coverage + evidence + time`；业务成功由调用方根据目标判断。现有 `goal_verified` 字段需明确旧语义并渐进迁移，不把一个属性值匹配当成“整项任务完成”。

错误结果必须有层次、机器可读 code、影响 scope、可重试范围（连接/只读/禁止重发动作）、已派发/不确定状态及下一步。协议错误与工具执行错误区分；执行错误通过 MCP `isError` 和结构化事实表达，不能一律“成功返回空列表”。[MCP 工具结果与错误语义](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)

`execute` 若以后作为用户显式需要的 OS 能力，应采用结构化 argv、cwd、权限和输出限制；这与偷偷用 Python/PowerShell 实现后端完全不同。首期不以万能 shell 工具弥补截图/输入能力缺失。

## 8. 平台能力分级与无 UI 边界

“无自有 UI”不代表“没有系统授权、没有前台服务通知、绕过 OS 沙箱”。权限状态应可查询，用户批准仍使用 OS/AI Host 提供的交互。

| 平台/场景 | 代码/证据现状 | 合理能力边界与后续路线 |
|---|---|---|
| Windows | 有 Rust UIA、GDI、SendInput、窗口/会话/权限/进程证据代码和部分历史 live 记录；当前通用服务未通过 | 优先完成新架构真实 VM 验收。GUI subsystem 消除 agent 黑窗；普通/管理员是权限配置，不是两套工具目录。桌面 worker 在正确交互会话内运行，不让 Session 0 服务假扮用户桌面 |
| macOS | 有 Rust AX/CG 受控窗口实现和离线/离屏证据；没有本轮通用受控端 live 验收 | Mac Codex adapter 优先；本机 agent 另验证 TCC、AX、显示器和窗口。安装 MCP 不自动取得辅助功能/屏幕权限 |
| Linux X11 | 参考有实现，RCU workspace 尚无 Linux native crate | 按 X11/输入设备权限分别协商，不以 Linux 名称等同 GNOME Wayland |
| Linux GNOME/Wayland | 参考有 portal、多个 compositor/provider 与 GNOME JS 路径；未移植到 Rust | 先验证 portal ScreenCast/RemoteDesktop、PipeWire/libei、可获得的 AT-SPI 语义；全局窗口表/坐标无法证明时不伪造。不把旧 JS extension 偷装成 Rust 后端 |
| Android | 未实现 | AccessibilityService/截图需实际 OS 能力与用户授权；MediaProjection 有会话与前台服务要求。Rust 核心+JNI/平台必要声明壳需单独实现验证，不承诺静默任意控制 |
| iOS/iPadOS | 未实现 | 普通第三方应用受沙箱、entitlement 与后台 API 限制。不能承诺像 Windows 一样安装 headless agent 后任意全系统输入；仅公开且已验证的能力，特殊开发/管理通道另列，不充当一般产品覆盖 |
| VM / RDP | 是部署/路由关系，不是单独 OS 后端 | guest agent 和宿主画面代理分级；最小化宿主 VM 窗口、RDP 断开、guest 锁屏是不同场景，分别验证 |
| 空调/电视等 | 仅预留 DeviceKind + capability 命名空间 | 无 agent/API 的设备不凭想象宣称可控；不增加空 crate、云平台或业务流程框架 |

平台依据：Windows 服务的 Session 0 约束见[Microsoft](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services)，输入的完整性级别限制见[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)；Mac 辅助功能授权见[Apple](https://support.apple.com/en-euro/guide/mac-help/mh43185/mac)。Wayland 输入和 stream mapping 由[RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)具体接口约束；Android 的[AccessibilityService](https://developer.android.com/reference/android/accessibilityservice/AccessibilityService)与[MediaProjection](https://developer.android.com/media/grow/media-projection)不是无条件通用控制许可。iOS 边界依据[Apple 运行期安全说明](https://support.apple.com/guide/security/security-of-runtime-process-sec15bfe098e/web)，这里的产品分级是据此作出的保守设计决定，不是已经完成的 iOS 实现审计。

## 9. Rust workspace 目标依赖

先模块内解耦，再在需要独立构建/部署时抽 crate；不为每层新建一个空项目。建议在现有七个 crate 上，仅新增一个明确的进程装配 crate `rcu-agent`：

```text
rcu-core                  纯领域契约、授权/目标/操作接口与有界状态原语
  ↑ rcu-imaging           图像编码/解码/映射
  ↑ rcu-windows / rcu-macos（未来平台仅在有真实实现时加入）

rcu-link-trust            配对、认证、私密材料；无业务目标
  ↑ rcu-native-link → rcu-core
       设备 RPC / IPC / transport；无具体 Windows 平台依赖、无 rmcp 展示逻辑

rcu-agent → rcu-core + rcu-native-link + 当前平台 driver
       服务装配、worker 管理；设备侧唯一授权/租约/副作用账本执行点

rcu-mcp → rcu-core + rcu-native-link（+ 需要的展示/图像映射）
       MCP SDK / tools / adapter / Host 集成；不负责业务 oracle

验收 harness → 公开 adapter/client API
       Notepad / CrossMacro 等用例、故障注入、文件及进程 oracle
       生产服务不能反向依赖 harness
```

当前 `rcu-native-link → rcu-windows` 和含 Windows 类型的 `protocol/types.rs` 必须去掉；不是简单移动文件夹就算解耦。平台证据转换为核心可序列化契约，具体 FFI 保留在平台 crate。当前 `rcu-core::Backend` 可以暂作兼容门面，内部按能力拆接口，不要求每个非桌面设备伪装成有窗口的 Backend。

具体保留、迁出、替换和最终删除项见[迁移清单](</Users/vincentadamnemessis/AI Projects/Real Computer Use/docs/migrations/2026-09-11-infrastructure-migration-plan.md>)。

## 10. 接入与运维要求

- 一次安装一个稳定 RCU MCP 入口；Plugin/Skill 只指向稳定 profile/设备注册配置，不指向 `work/.../admin5/model/.mcp.json` 或业务成功后即失效的路径。
- 设备配对、普通/管理员运行方式、可用性与操作授权可分别诊断。设备 offline 时 MCP 服务仍正常提供目录/状态，不强制刷新/重启 Codex 才能恢复。
- Skill 描述选择目标、按需观察、受权操作、独立验证与恢复方法；不负责弥补缺失的能力，也不在每步禁止合理的重新观察。
- 实际 Codex effective config、工具发现、原生连接和任务验收分别记录。项目 trusted、插件安装、`required=true`、启动等待只是 Host 设置，不能修复服务生命周期耦合。
- 当前首个 Host 仍用已核实的桌面 CLI，不降级用户配置；若后续加载确需重启，先通知用户。项目级 MCP 配置的信任条件及 stdio/HTTP 支持以[OpenAI 官方文档](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)为准。
- 有界缓存/队列、连接背压、控制面与大图分离、按需截图、FFI worker 隔离和资源释放必须测量；不因 Rust 语言就宣称无泄漏或更快。测试模型理解、token/回包体积、native 延迟与总任务时间分别计量。

## 11. 待评审的选择，不阻塞本轮文档

1. **建议批准**“MCP adapter + 小型设备 RPC”的首期路线；直接 MCP agent 是可替换部署模式，不同时建设两套完整栈。
2. **建议批准**首期授权以“主体 + 明确设备/桌面 + 能力集合”为基础，窗口级限制可加；不要求每次更换窗口重新安装 agent。高影响能力可单独再授权。
3. **建议批准**移动端仅承诺公开 API 能力分级；如果平台确需最薄声明/注册胶水，允许其只调用 Rust 核心，禁止业务逻辑/截图输入偷偷迁到另一后端语言。若坚持连声明壳也不得有，移动端范围需要进一步缩小。
4. **建议暂缓**万能远程 shell、持久化用户宏/调度平台、IoT 云、中继穿透与 App UI。它们不是本次架构整改的前置条件。

下一步不是再补一个清理任务类型，而是先通过“不打开指定应用也能列工具/设备、窗口关闭仍健康、另一客户端只读不被错误动作封锁、新会话可用、路由不串台、重连不重放输入”的基础验收。完整矩阵与实施次序见[迁移计划](</Users/vincentadamnemessis/AI Projects/Real Computer Use/docs/migrations/2026-09-11-infrastructure-migration-plan.md>)。所有这些新架构验收在本轮均为**待执行**。
