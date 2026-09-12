# RCU 调用体验与 Codex Computer Use JS 实现对照

日期：2026-09-12。性质：实际任务记录 + 本机已安装实现的只读分析，**不是优化发布或性能胜出声明**。

> **样本纠正：** 本文统计的是 11:01–11:04 的安装验收任务，不是用户指出的
> 11:08–11:17 实际清理任务。原数字保留为安装样本，但不能用来解释用户所说的
> 九分钟体验。正确对象为 `01a09395-5b92-7812-ba25-878aceb09466`，完整分析见
> `2026-09-12-crossmacro-full-flow-and-dual-channel.md`，含全部 126 次 MCP 与
> 30 个模型工具批次的时间表。本文 Codex JS 源码对照仍是独立有效的静态分析。

## 结论

用户对输出冗长、操作不够连贯的反馈有依据。但本次新任务的两个只读 MCP
调用本身没有显示出秒级延迟：Host 记录分别为 21 ms 和 35 ms，而首轮总时长
为 77,896 ms。首先需要整改模型侧输出和交互次数，不能把整轮时间归因于
Windows 或 MCP 网络。与此同时，源码确实存在每次设备 RPC 重建 TCP/TLS
连接的实现，远距离连接上的代价需要单独测量和改进。

本机安装包包含可读的 Codex CUA JS SDK、MCP 启动器和本机 IPC 客户端。
其连贯性不是因为“JS 比 Rust 快”，而是持久调用环境、资源对象、按需输出、
确定性动作组合、状态差分接口与连接复用共同降低了模型往返次数。

应借鉴这些机制，继续保留 ADR 0002 的 Rust 多端设备服务与认证通信架构；
不是把 Codex 的本地服务、权限体系或某个业务脚本直接搬进 RCU。

## 1. 新任务实际做了什么

任务：**验收全局 RCU MCP**，`01a09390-6394-7d13-8119-460e93c6b804`。
只读取这个任务的记录，没有扫描其他项目历史。

| 顺序 | 操作 | 实际结果/代价 |
| --- | --- | --- |
| 1 | 查 `ALL_TOOLS`，打印全部 RCU 元数据 | 20 个工具；输出 20,556 字节 |
| 2 | 直接 `computer.targets` | MCP 21 ms；打印 17,237 字节 |
| 3 | 直接 `computer.operation` 查询旧 request | MCP 35 ms；输出 1,446 字节 |
| 4 | 检查局部配置和 PATH 上的 CLI | 局部配置不存在；旧 CLI 无法解析 `max` |
| 5 | 输出首轮报告 | 整轮共 77,896 ms |
| 6 | 根任务发补充提示，用应用自带 CLI 查询 | 命令成功；另一个模型回合共 38,374 ms |

字节数取实际 `custom_tool_call_output` 的可见文本之和，包括执行工具的短
完成提示；不是 JSONL 文件大小、模型 token 数或网络包长度。
计时来自 Host 的 task/tool 记录，不是手工秒表。

不能把剩余的 77 秒全部称为“模型思考”：其中可能有模型生成、Host 调度、
输出处理和其他开销，记录没有给出完整分解。也不能把这两个只读接口的
21/35 ms 外推成截图、UIA 查询、鼠标键盘输入的性能。

### 核心指标：结果返回后，到下一步调用发出之前

这部分是用户关心的 Codex 理解/思考/决策与续接操作成本，不能被排除在
“MCP 使用效率”之外。按该任务原始记录的时间戳逐段计算：

| 已收到的结果 → 下一步 | 间隔 | 下一响应记录的 reasoning tokens | 解释 |
| --- | ---: | ---: | --- |
| 工具目录 → `computer.targets` | **10.637 s** | 187 | 消化工具说明、选择入口、生成调用 |
| `computer.targets` → `computer.operation` | **4.598 s** | 0 | 读取设备返回，继续已指定的只读查询 |
| `computer.operation` → 配置检查工具调用 | **21.865 s** | 405 | 决定附加检查，期间还有一条 98 字符旁白，再生成检查代码 |
| 配置检查返回 → 首轮最终报告 | **28.219 s** | 516 | 结果解释及 819 字符最终报告；无下一 MCP 调用 |
| 补查 CLI 返回 → 补查最终报告 | **29.450 s** | 516 | 重述已验证边界及 836 字符报告；没有新增设备操作 |

这里计时的两个端点是 **Host 记录工具输出** 与 **Host 记录下一工具调用/
最终消息**。它包括结果理解、计划、旁白和参数/代码生成，也可能包括
Host调度/模型服务等待；不是仅工具耗时，也不能冒充已经分离出的纯推理
计算时长。reasoning token 数是该响应的真实 usage 计数，不是按时间估算，
也不能将 0 个 reported reasoning tokens 解读成模型没有处理输入。

首轮这四段间隔合计 **65.319 s**；此外还有首次生成/选取入口、工具执行、
消息尾部及 Host 开销。两个回合的 Host 时长合计 **116.270 s**；中间还有
约 **33.42 s** 根任务协调间隔。按日志起止观察，用户看到的首轮开始到补查
结束约 **149.63 s**，不能只拿 56 ms 的两个 MCP 调用之和代表体感效率。

同一组记录还显示输入上下文从 29,871 增至 34,727、38,977、39,532、40,431
tokens；这包含工具文本、前序生成和协议内容，不等于某个工具独占的 token
数。缓存能降低部分重复输入成本，但不能消除多轮调用与报告生成。

**理解正确性也要计入验收：** 这次两个原生调用参数正确且均成功，没有
出现 MCP schema 纠错。实际可见的浪费是全目录回显、冗余包装、额外 CLI
失败及长篇复述。不能声称已证明模型“读不懂”某个字段。另一方面，targets
里服务级 `computer.act available=true` 与目标级 `input=false` 并存，是需要
在简洁输出中明确区分“实现/策略支持”与“当前可执行条件”的设计风险。

这次大 targets 输出之后只等了 4.598 s，而更小的历史查询之后等了
21.865 s，也说明单个样本不能证明“输出越多，思考必然越久”的因果关系。
后续必须同时控制任务、模型和流程，再比较输出方案。

### 三种可确认的冗余

1. **验证调用方式冗余**：只需确认工具存在，却打印了全部说明与调用声明。
   目录中 20 个 description 共 18,424 字节，共有前缀 349 字节重复了 20 次。
   首次选取必要工具元数据即可，不应在正常操作中重复完整枚举。
   这是验证代理调用 `text` 回显整个元数据对象的行为，不是 MCP 强制模型
   打印目录；验证代理的提示/输出策略也在整改范围内。
2. **MCP 包装被整体打印**：`targets` 使用 `text(result)`；其 text content
   的 JSON 与 `structuredContent` 完全相同。完整 MCP 对象为 17,190 字节，
   单份结构化 payload 为 8,241 字节。后一个 request 查询已经选择
   `structuredContent`，没有同样的外层重复。
3. **产品 payload 自身冗余**：`targets` 的顶层目标列表与
   `devices[0].state.targets` 完全相同，列表本身 2,624 字节；详细能力、
   策略说明与动态状态又混合在同一响应里。仅改变调用者打印方式并不能
   解决这一层，也不会减少网络发送的原始 payload。

此外，补充 CLI 检查本可以一开始使用应用自带的确定路径，避免一个失败
检查和后续模型回合。这是验证流程问题，不是 RCU 设备服务问题。

精确数值保存于 `2026-09-12-rcu-session-output-metrics.json`。
原始选定样本保留在忽略目录 `work/cua-flow-analysis-20260912/`，不提交
完整任务历史、运行身份数据或第三方实现源码。

## 2. 实际读取的 Codex 实现

来源编号、绝对路径、文件大小及 SHA-256 保存于同目录
`2026-09-12-cua-source-manifest.json`。本轮未 import/执行这些包，也未改动
Codex 安装、调用 CUA 桌面、录屏或向虚拟机发送输入。

- `unified-computer-use`：`26.908.40834`。
- `@oai/cua`：`0.2.4`。
- `@oai/sky`：`0.6.32`。
- RCU 仓库快照：`099c4f2`；已安装 Rust 二进制来源：`e2f3598`。

### 2.1 确认到的调用链

```text
Codex Host
  -> unified-computer-use plugin: cua_repl.js / js_reset
  -> @oai/cua-repl launcher
  -> persistent Node REPL + initialized cua facade
  -> app/tab object methods
  -> nodeRepl.rpc("sky", typed method + arguments)
  -> trusted @oai/sky/service
  -> platform client
  -> native pipe / native service
```

证据：C01–C09。`@oai/cua` 打包的 Sky proxy 调用受信 `sky` RPC，配置再将
该服务映射至独立的 `@oai/sky/service`，并非直接把模型 JS 当成本机原生
后端执行。launcher 负责启动 REPL 和注入 API 指令，不实现 OS 输入。

### 2.2 值得借鉴的机制

| 机制 | 读到的具体实现 | 对 RCU 的启示 |
| --- | --- | --- |
| 持久状态 | REPL 模块/变量保留；`globals.js` 初始化 `cua` | 不让模型每次拼接完整设备、窗口、帧参数；但服务权威身份仍不可省略 |
| 资源对象 | `getApp` 返回闭包绑定 app identity 的方法；tab 也暴露同类接口 | Host SDK 封装 device/window/frame，原生执行仍由 Rust agent 决定 |
| 初次状态合并 | `getApp` 获取一次完整状态，返回对象时同时输出状态 | 选择资源后不要再多做一次纯重复观察 |
| 状态/图片按需输出 | `getAXState`、`getScreenshot`、`getAXStateAndScreenshot`；`emit:false` 控制输出 | 默认紧凑语义状态，图片按需；避免自动输出后再 `write` 一遍 |
| 差分接口 | `disableDiffing` 映射到 native `disableDiff`；默认保留差分路径 | 建立带基准帧的差分协议，不只截断 JSON |
| 文档复用 | 核心/浏览器文档与 request metadata 有缓存，提供显式重写方法 | 公共规则无需重复嵌入每个工具的大段描述 |
| 确定性动作组合 | 安装包文档明确允许一段 JS 内执行已确定动作，最后取一次新状态 | 减少模型回合；不是取消每步前置条件，也不是盲目播放任务脚本 |
| 连接复用 | Mac client 按 API 版本缓存 transport；native pipe 保留连接、序号和 pending map | 设备通信也应复用已认证连接，连接与设备/上下文身份分开 |
| 串行请求队列 | Mac native pipe 用 Promise 链安排请求；Windows pipe 也有队列 | 同一输入资源串行；不要把这种队列无区别用于 RCU 的取消/状态控制通道 |
| 错误传播 | Mac请求失败传播错误，关闭连接后丢弃缓存；这里未见自动重放动作 | 重连与重放是两件事，RCU 的 durable no-replay 必须保留 |
| 回合结束通知 | plugin 有 Stop/Interrupt/SubagentStop 的 `turn_ended` hook | Host生命周期需要接到资源释放/取消，但不能推断 hook 返回即原生调用已停止 |

具体源：资源对象/输出 C05；RPC proxy C06–C07；连接与队列 C10–C12、C15；
状态返回 C13–C14；行为契约 C16；hooks C02。

**关键区别：一段 JS 可以包含多次 IPC/RPC。** 它减少的是模型↔工具之间
的回合，不自动把远程设备的多次操作合成一个网络请求。RCU 仅增加一个
`js` 工具而不处理设备 RPC 和输出，远程延迟仍会存在。

### 2.3 不能假装已经看透的部分

- Mac 原生 `SkyComputerUseService` 是 ARM64 Mach-O 编译程序（C17），本轮
  没有获得其完整原生源码。JS 能证明差分标志被透传、状态被返回，不能
  证明底层 AX diff 的完整算法、元素生命周期或所有失败分支。
- 安装包文档说明观察前有内部等待；Mac JS 路径把观察交给原生服务。
  因此这里没有把具体稳定检测算法、固定等待时长当成已源码验证的事实。
- hooks 能证明 Host 发送结束通知，不能证明 REPL/native server 在所有
  Stop、超时、崩溃情况下都完成了资源回收。不可借此宣称优于 RCU 的取消模型。
- 读到的平台工厂有 Mac/Windows/Linux 分支，默认依 `process.platform`
  选择；本版本 unified `getApp` 又明确限制 Mac。不能说 Codex 完全没有
  多 OS 实现，也不能从这些本机管道推出已具备 RCU 的配对多设备远程协议。
- [官方说明](https://learn.chatgpt.com/docs/computer-use)也区分 macOS 与
  Windows 使用方式；这些官方平台能力不等于本机某个 facade 的完整能力表。

## 3. RCU 当前实现的对应缺口

| RCU 位置 | 已确认事实 | 影响 |
| --- | --- | --- |
| R01 `gateway.rs::targets` | 完整 `state` 内含 targets，随后又生成顶层 targets | 同一份能力/身份在一次响应内重复 |
| R01 `mcp_result` | 使用 rmcp structured helper；本次输出证实有 text + structured 两份 | `text(result)` 时放大模型输出；不能不测客户端兼容就删任意一份 |
| R01 `get_info` + R04 catalog | 公共 server instructions 较长；Host 展开后每个工具重复 | 20 个工具的目录检索成本增加；本次验证还全量打印了它们 |
| R03 `Client::call` | 每次 call 新建 TCP、TLS，发送一个 request、读取一个 reply | 不是持久网络连接；跨高 RTT 链路可能放大开销 |
| R03 `serve` | 每条连接处理一个 request 后 shutdown | 客户端单方面连接池无法解决，服务端协议也需配套 |
| R02 `invoke/dispatch_accepted` | 可执行操作先 Prepare，再 Execute | 两次设备 RPC；取消身份保证不能为了减包直接删除 |
| R05 + R06 | 独立 observe/claim/act/verify/release 等工具，无通用动作组合执行接口 | 模型很容易每个低层步骤都发一轮；目前没有资源对象 SDK 来降低这一负担 |
| R06 observe | 先 `observe_with_region`，之后才按 `inline_image` 决定附图 | `inline_image:false` 不是“只读 AX、不截图”的原生能力 |
| R07 observe | 保存完整 scene、帧与图像 artifact；当前返回路径未提供 AX delta schema | 无法照搬“只输出变化”而不设计基准帧与恢复机制 |
| R08 checked action | 新鲜状态、元素、租约与 durable request 前置条件在服务内 | 应保留，不能用无检查 JS 点击队列绕开 |

这解释了为什么通用架构已经建立，但模型操作体验仍然偏向低层接口拼装。
工具存在、基础通信通了，不代表交互层已经做完。

## 4. 建议落地顺序（尚未实现）

### A. 优先减输出，不先替换协议

1. 原生 MCP 保留稳定目录；正常使用只取所需工具元数据，禁止整包枚举回显。
2. Host 输出只选 text 或 structured 的一份，图片单独输出；任何精简都必须
   保留 `isError/code`、执行引用、request 身份、uncertainty、terminal、
   `native_call_pending/resources_released` 等安全相关状态。
3. 产品区分简洁的动态状态与显式详情查询：能力以版本/引用复用，错误原因
   当场可读，完整审计可检索。不要把关键故障藏进只有额外工具才能读的日志。
4. 为文本型 MCP Host 与结构化 Host 都做真实兼容验收。只在调用者少打印
   一份不算解决服务端输出膨胀。

### B. 连贯操作层，而不是业务流程引擎

增加可选 Host-side JS/TS SDK/REPL facade，后端仍为 Rust MCP + Rust agent。
JS 只编排受控 typed API，不执行 OS FFI、Shell 或远程任意 JS。
标准 MCP 客户端仍能使用相同的通用能力，不强依赖 Codex 私有 runtime。

- SDK 维护已授权 device/window/frame 的服务返回句柄，短别名映射保留
  完整 epoch/principal/resource 绑定，禁止生成或猜测 `operation_ref`。
- 提供有界的“执行已确定动作，再观察一次”的通用 typed 组合能力。
  每步重新验证必要的焦点/窗口/元素/租约；出现异常、取消、弹窗或不确定
  结果立即停止后续动作，并返回逐步执行位置与原执行引用。
- 同一桌面不能用多窗口/`Promise.all` 并发输入；独立设备只读可并发。
- 短期租约在组合操作中申请并及时释放，不让持久 SDK 对象永久占据桌面。
- 动态页面需要观察后再决策，不能为了“一次调用完成”预先猜后面的控件。
- 业务目标验证留在调用者层；批次结束、成功发送输入都不是业务成功。

### C. 复用认证连接，保留取消与 no-replay

双方配套改成长连接、请求 ID 路由和有界并发；控制/查询不被长 native 操作
堵在同一全局串行队列。Prepare/Execute、真实 principal 与 agent/context
epoch 仍受校验。断线时未确认动作保持 uncertain，重连不得把队列自动重发。
关闭旧连接也不得意外关闭另一个 Host 的上下文或释放别人的 lease。

不能用 Mac 本机 socket 延迟与 Mac→Windows VM TCP 延迟直接作性能排名。

### D. 观察与稳定检测

把原生 `AX only`、图像与二者联合观察做成明确能力，而不是改一个输出布尔值。
差分需带 `base_frame/next_frame`、完整性、失效与重新全量获取规则；未变化
也应返回新鲜度依据，避免重用旧元素。稳定等待应有原生事件/有界采样依据，
返回 timeout/unstable，而不是成功假象；截图-only 后恢复 AX 索引需全量。

## 5. 验收门槛

- 将“工具输出完成 → 下一次正确工具调用提交”作为独立主指标，分别统计
  目录理解、状态理解、动作后决策、错误恢复及最终报告阶段的 p50/p95。
  同时报旁白字符数、无操作空档、无必要的额外检查数、重复观察数与总回合数。
  若可取得 Host 分段遥测，再细分队列/模型首 token/调用参数生成；拿不到的
  部分保留为合并间隔，不能删掉或归到服务端。
- 可理解性 A/B：相同模型/effort、任务和可比起始状态，分别使用当前输出与
  精简输出，多次重复并报告样本数。检查下一步选择、参数/引用正确率、对
  partial/unsupported/uncertain 的解释、错误重试与重放事件；不能靠降低
  模型设置、删掉关键状态或跳过验证换取表面速度。
- 固定同一个任务、模型设置、目标和状态边界，比对模型回合数、UTF-8 输出量、
  图片字节、MCP p50/p95、网络连接/握手数、native queue/UIA/capture/encode
  分段时间及完成率。未采集的分段明确标为未知，不反推成“思考时间”。
- 第一层用离线协议/fixture 测重复输出、部分失败、差分恢复、批次中途取消、
  断线不重放、同桌面竞争与控制通道响应。
- 第二层用真实 MCP 进程，分别加入显式 RTT/带宽条件，独立报告冷/热连接；
  fixture 性能不能替代 native 图像与 UIA 性能。
- 第三层在明确分配的 Windows 窗口做实际观察/短动作组合/独立结果观察，
  再用真实新 Codex 任务验收。保留失败样本，不只统计成功路径。
- Rust行为变更按项目要求跑 fmt、workspace locked tests、Clippy；本轮仅
  分析文档和记录，不运行或声称重新通过产品回归。

这份分析不关闭此前的 owned-worker 隔离、完整多平台、安装器、Plugin/App
接入及 native 失败矩阵缺口，也不把新的 facade 设计算作已实现。
