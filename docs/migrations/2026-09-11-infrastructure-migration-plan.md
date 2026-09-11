# RCU 通用基础设施：小步迁移与真实验收计划

日期：2026-09-11。状态：**待评审的实施计划，本轮没有执行迁移或 live 验收**。

依据：[架构提案](</Users/vincentadamnemessis/AI Projects/Real Computer Use/docs/adr/0002-headless-control-infrastructure-review.md>)、[CrossMacro 源码对照](</Users/vincentadamnemessis/AI Projects/Real Computer Use/docs/architecture-crossmacro-comparison-2026-09-11.md>)。当前根目录是唯一可编辑产品源码，不再复制 `work/*/source` 开发分支。

## 1. 当前冻结点与迁移纪律

- 基线 HEAD：`aeb56851a37d570179eacb046ea0177b894a7002`。存在先前未提交的 privilege/GUI agent/window selection 修复、模块抽取与新 process-goal 代码。它们不是本轮新写内容，也不是已批准架构。
- 不 reset、覆盖或打包发布这些改动。先用逐文件 SHA 与 diff 分清“可保留的局部修复”和“应迁出核心的业务流程”，之后再按评审通过的方案形成小提交。
- 保留旧参考、失败记录、HOLD journal、截图和安装身份。**保留证据不代表继续把旧工作流作为生产入口。**
- 首个真实平台仍是 Windows VM。架构评审期间不连接 VM；评审通过后由唯一操作任务重新核实现场、目标与授权，旧 PID、HWND、grant、Model stage 都不得直接复用。
- 本轮不 commit/push/tag/发布；后续提交遵守已获本仓库 Git 授权，架构移动与行为修改分开评审。

## 2. 代码处置清单

### 2.1 保留，先提取再改行为

| 现有文件/符号 | 保留内容 | 需要的边界调整 |
|---|---|---|
| [rcu-core Identity / Scope / Scene / Receipt / Backend](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/lib.rs>) | 身份、观察、结果、引用与后端接口的已有基础 | 将必选 window/PID 的 Scope 拆成资源类型；能力查询变类型化，旧 Backend 作过渡门面 |
| [checked_action.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/checked_action.rs>) | 同请求 dedup、参数 fingerprint、派发前新鲜度/身份检查 | 动作需要什么前置条件由 capability/action 决定；document context 不再统辖全部输入 |
| [lease.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/lease.rs>) | 同桌面冲突键、文件锁、TTL、显式释放 | 所有 adapter 共用设备侧权威；加入主体、上下文代际与 fencing，不以客户端本地目录制造多个“独占” |
| [operation.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-core/src/operation.rs>) | 合作式取消、原生调用/资源释放状态 | 按 operation、主体和资源隔离，状态查询不依赖业务阶段 |
| [rcu-imaging](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-imaging/src/lib.rs>) | 有界编码/解码、原图/ROI/view 映射和失败用例 | 扩展显式坐标命名空间与多 display/region 映射，不默认跨屏等价 |
| [desktop.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/desktop.rs>)、[uia.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/uia.rs>)、[capture.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/capture.rs>)、[input.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/input.rs>) | Win32/UIA/GDI/SendInput，DPI 与 HWND/PID/birth/owner 证据、平衡输入/释放 | 原生事实层不知调用方文件 oracle；UIA 线程和卡死隔离属于 native worker |
| [agent_binding.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/agent_binding.rs>)、[agent_contract.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/agent_contract.rs>)、[pipe.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/pipe.rs>) | 原生令牌/会话、私密 ACL、保有进程句柄、pipe peer 验证 | 纯序列化证据类型移往核心契约；平台 FFI 留在 Windows；所有权不是业务 task 的别名 |
| [application.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/application.rs>)、[application_pin.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/application_pin.rs>) 与 package 模块 | 精确应用/文件/包身份、原生启动和窗口分类的局部修复 | 保留通用启动原语；Notepad/TaskMgr 仅是测试样例 |
| [rcu-macos native.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-macos/src/native.rs>)、[capture.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-macos/src/capture.rs>) | AX/CG/ScreenCaptureKit 实现及明确的局限 | 脱离固定进程启动 selector；以后实现动态 target 生命周期，未验收能力继续标未验收 |
| [rcu-link-trust](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-link-trust/src/lib.rs>)、[private_files.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-link-trust/src/private_files.rs>) | mTLS/证书/私密材料与 peer 验证 | 区分设备配对、主体认证、连接和操作授权；不把证书绑定一次业务任务 |
| [frame.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/protocol/frame.rs>)、[net.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/transport/net.rs>) | 有界帧、读取截止时间、TLS 基础机制 | 保留已验证低层机制，替换 attach 的业务字段和 Model 独占语义 |
| [model_output.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/integration/model_output.rs>) | 完整 RPC 留证、成功观察的精简展示、错误/回执保真 | 迁到 MCP presentation 层；证据通过受授权 MCP 资源接口读取，不强制依赖本机路径 |

“保留”不等于原样直接用于新服务：每项仍需在新作用域下通过回归和身份/失败隔离用例。

### 2.2 迁到验收/调用方，不再被生产运行时依赖

| 现有内容 | 处置 |
|---|---|
| [deployment/plan.rs 的 NotepadSeed / ProcessCleanupSeed / notepad / process_cleanup](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/deployment/plan.rs>) | 迁为验收场景构造器；部署模块只处理安装/配对/服务配置，不产生业务流程 |
| [orchestration/process_goal.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/process_goal.rs>)、[oracle.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/oracle.rs>) | 文件/进程业务 oracle 移到测试 harness 或调用方 workflow；不得作为服务成功退出/新任务准入条件 |
| [Stage / Step / Approval 中业务字段](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/protocol/types.rs>) | 八阶段、期望文本、文件 manifest 和 process goal 从设备 wire/domain 剥离；保留对应历史类型供只读旧证据解析 |
| [budget.rs 中 Model/Oracle/Cleanup 固定预算](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/budget.rs>) | 测试执行预算归 harness；通用 monotonic deadline、容量边界可提取保留 |
| [worker.rs / worker/windows.rs 的阶段 worker 编排](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/worker.rs>) | 区分进程所有权/回收原语与按 Stage 建 worker；前者进 agent，后者仅 legacy 验收 |
| [control.rs 的业务 prepare/execute/oracle/cleanup 及 catalog 限制](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/src/orchestration/control.rs>) | 将通用目录分页/窗口身份校验提取给平台能力；Stage 次序留验收，不再约束正常工具 |
| [full_link.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/tests/full_link.rs>)、[preparation.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-native-link/tests/preparation.rs>) | 保留历史结果与适用原语测试；旧的“必须先完成文档阶段”断言仅留 legacy 场景，不能作为新产品契约 |

暂用根目录下的 `tests/acceptance/` 或显式 `xtask`/验收 crate 承载场景，**只在确实需要共享构建入口时建 crate**；不建立另一份产品源码树。

### 2.3 必须替换的生产耦合

1. `AgentConfig.approval` 与 `ClientConfig.task/approval_sha256`：改成稳定设备/profile、主体认证及链路配置；业务操作另携有效授权。
2. `Runtime::new`：改为不依赖窗口/文件任务的 service runtime。设备能力可处于 locked/permission_limited，但 service control plane 仍工作。
3. `Store::state: Option<State>` 和 `installation_has_unfinished_or_held_task`：换成设备/客户端/授权/operation 的独立索引；旧记录只读保留。
4. `Model attach → Stage::Model`：换成普通认证调用通道；多个只读客户端不得为取得工具目录竞争独占 Model。
5. `GuardedBackend` 的阶段/业务预算部分：改成通用主体/能力/目标/原生代际的 dispatch guard，保留 peer、焦点和资源检查。
6. `Server` 单全局执行 gate：状态/目录与原生执行隔离；按 device/native-context 队列、线程亲和性及冲突域管理，不无限制并发同一桌面输入。
7. `ProcessAbsenceGoal` 所在 [process_inventory.rs](</Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-windows/src/process_inventory.rs>)：先提取通用 `ProcessSnapshot`/`ProcessIdentity` 和完整性事实；匹配条件与业务 absence oracle 迁出去。现在的字符串匹配不是通用进程终止授权。
8. task-bound plugin exporter/Skill：稳定安装入口只绑定服务/profile，不能要求另一个 agent 先推进五个阶段。保留“安装不授权输入”，去掉“每次准备 Model stage 才能连接”。

### 2.4 在替代实现验收后删除的运行路径

**以下是计划，不是本轮删除动作。**

- 生产配置中的 task-bound `.mcp.json` 字段与指向 `work/.../run-name/...` 的临时路径。
- normal service 路径对 `approved-task.json`、business Stage、Notepad/TaskMgr Seed、file/process oracle 的依赖。
- 生产 `StartupFailure → Unavailable` 的“记录 19 工具、所有业务都拒绝”替代服务器；保留有界错误呈现机制，替换为真实路由器的 offline/permission/capability 状态。
- 以业务成功决定服务退出/安装目录能否重用的条件；会话结束只处理真实资源、操作和授权状态。
- agent package 内的测试业务命令。可以保留显式 legacy evidence reader，但不能默认启动旧任务。
- 不删除原生输入/截图、身份验证、租约、去重、partial coverage 或错误证据。更不能删除旧 journal 来规避 HOLD。

## 3. 拟议依赖约束

| 模块 | 可以知道 | 不可以知道 |
|---|---|---|
| core contracts/policy/operations | 主体、Device/Target/Capability、grant、lease、operation、evidence | Codex 配置格式、notepad.exe、UIA COM、TLS socket |
| native-link | 认证连接、协议帧、路由上下文、有界收发 | 文档内容、Oracle、固定 Model 次序、Windows FFI 类型 |
| native platform | 本 OS 原生身份、观察/输入/进程/窗口与权限事实 | AI 模型名称、业务文件应该改成什么、是否完成整项任务 |
| agent composition | 驱动装配、线程/进程生命周期、设备侧权威状态 | AI Host 的业务编排与最终业务断言 |
| MCP adapter/presentation | 标准 MCP、工具契约、请求路由、错误和证据展示 | 为满足某用例自动打开应用、清理进程、续授权或重放操作 |
| evaluation / Skill | 高层目标、选择工具、业务 oracle、指标 | 越过 native 权限、修改效果回执或让服务启动依赖目标成功 |

阶段性仅新增 `rcu-agent` 作为确有独立职责的装配 crate；Linux/Android/iOS 不提前创建空“支持”模块。每次移动后检查 Cargo 依赖与生产 feature 图，保证生产 crate 无反向依赖验收。

## 4. 小步实施次序

### P0 — 审阅本方案并固定边界

交付：角色/层次确认、来源对照、兼容策略与文件处置清单。建立“服务启动不得读取业务 manifest”的架构规则。旧现场停留在已记录状态，不为评审重启/重连 VM。

通过条件：用户/项目负责人接受核心边界；跨平台与无 UI 限制按事实分级。**本轮到此文档交付，不继续写产品代码。**

### P1 — 分开服务配置与业务配置，建立真正的 control plane

- 从原生场景调用中提取稳定 DeviceConfig、Identity、CapabilityDescriptor、TargetRef。
- 增加/装配 headless `rcu-agent`；运行配置只有安装、认证、资源限制和明确设备上下文，不含业务目标。
- 将 MCP server 的 schema/目录/设备状态放在 adapter 本地独立执行路径；设备离线/窗口为空也能正确回应。
- 初期 native capabilities 可诚实 unavailable；不得用仅诊断的假成功冒充已执行 native。离线时“已配对设备离线”与“未发现任何设备”也要区分。

提交拆分：先纯类型/装配提取，再切换启动行为。通过 V01、V02 的离线/实际 Host 层；这尚不是 native 操作验收。

### P2 — 认证、授权、租约和 operation ledger 分离

- 以设备侧唯一授权执行点管理 principal、grant、桌面写租约、原生 worker 代际。
- 新连接独立于业务 task；重连允许查状态但不得重发输入。
- reservation 与 payload fingerprint 在副作用前持久化；冲突、未知、取消、已结束分别记录。
- 账本按 operation/资源隔离。保留预算和存储上限，但某个目录/图像容量耗尽不应让全部设备查询失效；无法持久化时明确拒绝对应写入。
- **有界保留不能破坏防重放**：过期 operation namespace 一律拒绝新派发，不能因回执已归档就视为新操作。采用主体/上下文代际+序号或等价的可验证命名空间；旧 ID 的历史查询可以返回 history_expired，不可以重新执行。

通过 V04–V10、V16–V18 的离线故障注入。UIA/native call 的隔离仍需下一阶段真实验证。

### P3 — 接入已有 Windows 原生原语，而不是接入某个业务用例

- 接入真实的设备/会话、窗口目录、UIA/截图、绑定、输入、验证和资源回收。
- 改 document 前置条件为动作类型相关：编辑器文本动作可以要求文档上下文；菜单/按钮/桌面动作使用适当的原生条件。
- 保留普通/管理员原生部署验证成果、GUI subsystem、IME/window 分类、Owned popup、DPI 和进程身份局部修复。
- 将会话/窗口变化作为能力/目标状态；不得调用 `runtime.hold()` 使别的客户端目录读取永久不可用。

先实际 Windows 做 V01–V06、V09–V15、V19–V20 的适用部分；真实输入前仍需当前有效授权。通过了多少就报告多少，未测的多屏/多设备明确列出。

### P4 — 将所有业务 oracle 和 runpack 迁到验收层

- 将 Notepad、CrossMacro 清理等场景改成只调用公开工具/API 的测试工作流。
- 保留原始失败日志。已有测试中验证身份/去重/资源的断言原样保留；仅把错误的“业务阶段决定服务可用”断言从生产契约退役并记录原因。
- 不通过增加 timeout、自动重试、放宽 UIA/身份或简单删测试来做出全绿。
- 进程 absence 是一个采样时点/区间的事实，不是永久保证；名称匹配、路径/身份、权限覆盖不足必须进入 oracle 的 unknown/partial 结果。

通过 V11、V14、V19 的独立事实/业务拆分，再做真实模型业务验收。

### P5 — 稳定的 Codex MCP + Plugin/Skills 安装体验

- 只维护一个真实 RCU 入口；旧 M1 不重新加入默认配置。
- 入口绑定稳定安装的 Rust adapter 与 profile，不绑定业务 task。升级采用受控版本路径/manifest 切换，运行会话不热换二进制；旧状态/证据保留。
- 验证真实用户 effective config、项目 trust、工具发现和原生可用性，分别留证。
- 不再以禁用 MCP 表达设备 HOLD；恢复设备链路不要求反复重装插件或重启 Codex。
- 需要改变信任、OS 权限或重启客户端时，提前给用户明确说明，不用“工具不信任”概括。

### P6 — 完成 Windows 基础验收后再逐平台扩展

先补齐真正影响通用软件操作的窗口布局/拖拽/显示器/菜单/弹窗能力；再做 macOS 受控端、Linux X11/Wayland 和移动端能力验证。平台可共用契约与 fixture，不以交叉编译或 Rust feature 名称宣称已支持。

## 5. 旧安装与旧 HOLD 的迁移方式

1. 归档旧配置、程序 SHA、journal、旧未决 operation 与资源所有权证据；不改写旧成功/失败状态。
2. 新服务使用版本化的新状态模型，旧文件只作为只读 legacy 证据导入。不要通过复用新 task 名绕过旧不可重放 ID。
3. 任何 native 写操作前验证旧 owned worker 已停止/不能继续派发，建立当前输入域的统一锁与 fencing。证明不足只隔离相应写域，查询仍可用。
4. 旧的业务失败不需要伪造 Oracle 成功来解锁新服务；旧不确定物理效果仍保留，并在相关资源的新操作前重新读取/确认。
5. 验证新目录/状态不依赖旧任务后，才切换唯一项目入口；全局桥接、旧 CrossMacro 安装和其他软件不属于这次清理范围。
6. 回滚只切换合法版本/配置，并保留全部 operation 记录和防重放边界；不把备份的旧 active session 当成可以恢复执行的快照。

## 6. 真实验收矩阵

层次：**U**=离线单元/状态模型；**P**=实际 MCP 进程；**H**=真实 Codex Host 配置/工具调用但不等于模型推理；**N**=授权目标上的原生操作；**M**=真实模型自主选用工具完成业务。没有执行的格子不能填通过。**下表均是新架构待执行项。**

| ID | 场景与故障注入 | 通过标准 | 必须具备的证据/层次 |
|---|---|---|---|
| V01 | 没有打开 Notepad/Task Manager；没有业务 manifest；尚无输入 grant | MCP 可发现工具；已配对设备/上下文状态可查询；不自动打开应用或截图 | P/H + N 设备状态；启动依赖/副作用计数 |
| V02 | 一个已配置设备离线，或 agent 未启动 | schema 仍可发现；只对该设备返回 offline；其他设备/本机控制面可访问 | U/P/H，native 启停另需授权 |
| V03 | 已绑定窗口被用户关闭 | 对旧引用返回 target_gone/stale；服务健康；同设备另一窗口可以重新发现并选择 | U/P/N，旧/新窗口身份与服务状态 |
| V04 | 一个客户端发送非法参数或错误操作 | 该调用有明确错误；另一有权限客户端仍能只读查询 | U/P/H；两客户端交错请求记录 |
| V05 | 一个 UIA/provider 调用卡住 | 受限 worker/队列可诊断和隔离；状态/目录/取消控制面仍响应；不谎报 native 已停止 | U/P/N，owned worker 状态和资源回执 |
| V06 | 两客户端争用同一桌面输入，第三个客户端读状态 | 只一个写 lease；另一个不派发、不抢焦点；只读控制面不被封锁 | U/P/N，input 调用计数与 lease 所有权 |
| V07 | 两设备/两目标相同窗口标题、PID 或图像坐标 | 路由严格按设备/上下文/代际；没有 fallback 到另一设备 | U/P + N 双目标；第二真实设备未分配时明确未验收多设备 |
| V08 | 原生动作派发后响应丢失，再连接并提交同 operation ID | 不再次派发；返回已知状态或 unknown；payload 变化返回 conflict | U/P/N，agent 原生调用计数、持久化 ledger |
| V09 | agent/worker 在 reservation 或派发期间重启 | 旧记录保留；旧瞬时引用失效；只读可用；相关写入先完成 fencing/资源核查 | U/P/N，旧进程实例、lease、代际与 ledger |
| V10 | grant 到期、撤销、只读授权或跨完整性级别访问 | 操作按具体权限失败；不影响工具目录/其他授权；不自动提权 | U/P/N 普通/Admin 对照与真实 token 证据 |
| V11 | 进程/窗口查询分页未完、权限拒绝、编码/容量失败 | coverage=partial/unknown；不得以空结果证明 absence | U/P/N；独立原生采样，不通过脚本代执行 |
| V12 | 新客户端、新业务目标、新窗口，不更换安装目录 | 可正常观察并按授权操作；不需要完成上一业务 Oracle | U/P/H/N；安装/profile 保持不变 |
| V13 | DPI 改变、负原点、旋转、跨屏、分辨率/显示器热插拔 | 原图/view/input 坐标来源明确；旧映射失效；不支持的组合明确拒绝而非错点 | U 离屏数学 + N 实际显示配置；两者不可替代 |
| V14 | 菜单、设置页、桌面、弹窗，没有文档内容 | 支持的操作可按适当证据执行；不要求 Notepad 文本摘要；owned popup 不串台 | U/P/N/M，至少两种非文档界面 |
| V15 | 用户中途输入、切焦点、锁屏、lease 过期 | 停止后续批次并退让；必要 release-only 回收；不自动抢回焦点/恢复旧输入 | U/P/N，事件时间线与派发/回收计数 |
| V16 | 旧连接延迟包、旧 frame、伪造 target/session/clientInfo | 明确拒绝过时/越权调用；不得影响无关主体或目标 | U/P，多主体和 generation 负例 |
| V17 | 降采样图、多文本块、回执/错误、证据取回 | compact 不改错误/授权/结果事实；原始证据可经 MCP 取回且 hash 一致 | U/P/H，不靠 Python 读文件完成产品功能 |
| V18 | 回执归档、磁盘上限、操作 namespace 到期 | 旧操作永不因“查不到”重新派发；资源耗尽明确限制写入；目录诊断不消失 | U/P，持久化/崩溃/保留期负例 |
| V19 | 同一服务分别做文档修改与通用进程查询/GUI 清理 | 不增加 task 类型、不变更 agent 启动配置；业务 oracle 在调用方；失败业务仍可正常结束上下文 | P/N/M；实际 prompt、MCP 调用和独立业务证据 |
| V20 | 安装/升级/连接与无控制台启动 | agent 无黑窗；stdio 不混入日志；只一个 RCU 入口；设备恢复无需重装插件 | H/N，真实配置与 native 进程启动方式 |
| V21 | Linux/移动端未实现或 OS 权限受限 | 正确报告 unsupported/permission_required；不借旧桥/脚本冒充已实现 | U/P + 对应平台 N；未分配平台则不宣称覆盖 |
| V22 | 长时间空闲、反复观察/断线、大小图负载 | 缓存/队列/worker 有界；记录 RSS/CPU/句柄与调用延迟，无无界增长；错误路径资源可核查 | 分别 U 离屏/P 实进程/N 原生；M 的理解和总耗时另计 |

### 第一轮 Windows 原生验收的最小范围

先做 V01、V03、V04、V06、V08、V10、V12 的核心组合：**不依赖特定应用启动的目录 → 普通只读 → 受权最小输入 → 窗口关闭 → 第二客户端/新会话 → 断线不重放**。不等 Linux/Android/iOS 全部完成才开始实测，也不再用一个文件的写入成功代表整个平台。

随后使用至少两个不同界面的高层业务任务检验模型是否能自行选择 RCU 工具，加入焦点变化、窗口遮挡和用户中断。任务步骤不能硬编码进 agent；Host 直接调用脚本执行固定 RPC 序列只能算 H/P，不算 M。

## 7. 指标与报告方式

- 分开统计：安装可用、工具发现、认证链路、原生读取、原生派发、实际效果、业务完成、干预恢复。
- 每轮记录成功/失败、错误分类、MCP 调用次数、额外 shell/旧桥次数、模型重新观察次数、总耗时与人工干预次数；不能只展示顺利样例。
- 性能分开：MCP 冷/热启动、只读目录、UIA 读取、capture/encode、图片传输、输入到独立回读的延迟；RSS/CPU/句柄分别测 Mac adapter 与 guest agent/worker。
- 先建立同硬件/分辨率/权限/任务的基线，再设改善目标。不因 Rust 语言或 JSON 字节减少宣布“性能/模型信任已改善”。
- 生产回归仍执行 AGENTS 要求的 fmt、workspace locked tests、host all-target/all-feature Clippy；Windows 编译检查不是 Windows runtime 测试。合理保留并标明 ignored 场景。

## 8. 完成标准与未完成声明

本轮完成标准：有真实源码支撑的架构结论、来源映射、代码处置与可执行的迁移/验收计划；源文件、配置、参考与现场未被本轮改动。

**本轮不声称**：新架构已实现、已进行 Windows 新任务准入、已解除任何 HOLD、已清理进程、已原生输入/保存成功、已跑全部回归、已提升模型理解/性能、已发布跨平台产品。
