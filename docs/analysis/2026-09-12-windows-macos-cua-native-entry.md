# Windows / macOS Computer Use：已安装入口与 SDK 代码核对

日期：2026-09-12。本文为只读配置、随应用交付的技能/API 文档及 JS 客户端检查。
不是抓包报告，也不是 Windows 绘图或 Pointer 动态验收。

## 用户完成安装后的状态

- Windows 包已变为 `OpenAI.Codex 26.908.4834.0`，ARM64，`Status=Ok`。
  这与首次安装的 `26.903.8094.0` 不同；只确认当前版本，不推断更新由谁触发。
- Windows `ChatGPT` 与 `codex` 进程均存在于交互 Session 1；SSH 检查仍是
  Session 0，不把它当作 GUI 执行会话。
- `unified-computer-use@openai-bundled` 和 `computer-use@openai-bundled`
  配置均为 `enabled=true`，缓存版本均为 `26.908.40834`。
- 只检查登录状态文件是否存在，没有读取其内容。存在不能替代登录服务成功或
  特定模型/工具可用性的验证。
- Mac 的 `list_projects` 未提供 Windows Host 项目。已请用户在 Windows
  本机 Codex 新任务发送桌面验收说明；未通过 UI、隐藏 RPC 或另一台主机代发。
- 最后一次输出目录检查只有参考图和说明文档。尚无 Windows 成图/报告文件；
  不能据此证明任务未开始，也不能声称已经执行了绘图。

## 真实入口不相同

| 层级 | Mac 本机已安装配置/代码 | Windows 本机已安装配置/代码 |
|---|---|---|
| 统一 REPL 插件 | `cua_repl`，`CUA_REPL_ENABLED_SURFACES=browser,computer` | 同名插件，但 surfaces **只有 `browser`** |
| 原生 GUI 技能 | 本任务的统一 `cua` API | 独立 `computer-use` 技能，明确要求 `node_repl` 内导入 `@oai/sky` |
| 对象模型 | app 对象与 app action；原生请求可带 API version、deadline、turn metadata | `sky` window2 API；输入携带实际发现的 `Window`，含 app 和 window id |
| 原生获取状态 | Mac SDK 有 app skyshot 与差异控制字段；不是持续视频证明 | `get_window_state` 默认截图开启、AX 文本关闭；需要文本时独立请求 |
| 公开 drag 参数 | 起点和终点 | `from_x/from_y/to_x/to_y`，可携带 screenshotId；没有公开多路点/时间表参数 |

两端 `@oai/sky` package version 均为 `0.6.32`。同 package version 不意味着
走同一平台后端，也不意味着拥有相同公开工具入口。

因此，在 Windows 看到统一 `cua_repl` 没有原生 app API，不能直接判定整个
Computer Use 安装失败。需要检查 Windows 实际加载的独立技能和 `node_repl`
工具；更不能自行把 surfaces 改成 `computer` 来冒充受支持配置。

## 连接与消息：能由代码证明什么

检查的是随应用安装的 JS 客户端，不调用私有 pipe，也不自行启动 helper。

- Mac `MacComputerUseClient` 按 API version 保存 transport promise；
  `MacNativePipeTransport` 使用宿主提供的 nativePipe 接口，包含连接、ping、
  顺序请求队列、request-id → pending result 映射和关闭后的失败处理。
- Mac 的此路径以 **4 字节 little-endian 长度 + UTF-8 JSON-RPC** 分帧；单帧
  上限 8 MiB。请求包含 deadline/turn metadata。这是本机 RPC 客户端协议，
  不是模型输入输出的流式协议。
- Windows `WindowsComputerUseClient` 同时实现可信 `node_repl.nativePipe`
  路径和独立 helper transport 路径。nativePipe 路径同样缓存连接、串行排队、
  用请求 ID 分派返回，使用长度前缀 JSON-RPC；发送上限 8 MiB，接收上限
  64 MiB。默认请求等待 10 s、launch 15 s；它们是超时，不是实际延迟。
- Windows 独立 helper transport 代码包含持久子进程、逐行 JSON、pending map
  和 turn 结束/用户 Escape 处理。**没有因此手动调用这个备用路径。**
- Windows 统一插件配置的 `SKY_CUA_NATIVE_PIPE=1` 已核对；但统一插件这里只
  启用 browser，不能仅凭这份配置断言独立 GUI `node_repl` 的某次实测必然走
  nativePipe。需要 Windows 原生任务记录或受支持的运行态证据进一步确认。

结论：两端代码已有可复用本机通道，不能把流畅度差异简化为“Windows 流式、
Mac 非流式”。MCP 外层、模型通信、本机 IPC、设备输入节拍、绘图应用重绘和
可见 Pointer 是不同层。本文没有连续视频订阅、逐帧输入模型、实际连接数量或
请求 RTT 的证据；未用 TCP/HTTP 抓包结果假称捕获 native pipe。

## 操作约束与 RCU 的取舍

Windows 随包 guidance 明确采用观察/检查、单次动作/立即复查的两 cell 流程，
并强调截图与 AX 按需二选一。因此不能以这份代码为依据，声称 Windows 官方
鼓励任意连续盲操作。真实基线应遵守其当前工具和技能，而非为了速度改掉它们。

RCU 可以借鉴状态对象、按需观察、紧凑返回和持久连接，但仍需保留自己独立的
设备身份、权限、lease、durable request 与不确定输入不重放约束；不复制某个
Paint 场景进入产品服务，不复制安装包代码作为产品实现。

当前 RCU Rust 输入实际使用 `SendInput` 及绝对桌面 MOVE 事件，并已有设备节拍
轨迹。**这不等于用户要求的可见 Pointer 反馈完成。** GDI 采集报告也明确没有
cursor fidelity 保证。后续必须分别验证：

1. Windows 真实屏幕上的指针运动及按下/拖动/释放反馈；
2. 截图中是否包含对应反馈及其坐标准确性；
3. 连续路径的效果、取消清理、不抢焦点/拦截输入；
4. 模型决策间隔、工具耗时、设备执行耗时和可见效果的不同时间边界。

尚未读取到 Windows native helper 内部绘制循环源码，JS 客户端也未公开
Pointer overlay API。不能从文件名或两点 drag 推导其内部插值、刷新率或动画。

## 复核来源

官方说明支持 Windows 活动桌面、Mac 后台操作的区别，以及正常插件安装路径：
[Computer Use](https://learn.chatgpt.com/docs/computer-use)。

本机安装件（只读；配置仅提取相关字段，未保留 env 值全量）：

- Mac：`/Applications/ChatGPT.app/Contents/Resources/cua_node/lib/node_modules/@oai/sky/dist/project/cua/sky_js/src/targets/mac/` 中
  `client.js`、`native-pipe.js`、`drag.js`。
- Windows：`C:\Users\vincentadamnemessis\AppData\Local\OpenAI\Codex\runtimes\cua_node\55ee6ac13082db76\bin\node_modules\@oai\sky\dist\project\cua\sky_js\src\targets\windows\internal\` 中
  `computer_use_client.js`、`computer_use_client_base.js`、`helper_transport.js`。
- Windows 插件 `computer-use/26.908.40834/skills/computer-use/SKILL.md` 及
  `docs/{guidance,api,confirmations}.md`。
- 本机 `work/windows-codex-baseline-20260912/` 中的脱敏入口元数据、文件哈希
  `source-fingerprints.json` 和只读 SDK 摘取。安装包源文件不提交 Git。

诊断过程有三类本地检查错误：PowerShell `$HOME` 只读变量冲突、路径正则转义、
文档 JSON 传输解析错误，均没有触发 GUI 输入。读取长文档最终改用已有 SSH 的
SCP 文件复制。另一次 PowerShell 字符串附带元数据导致输出膨胀，已收敛为显式
标量/结构字段；不能把这些本地编排故障归因于 Computer Use 或 RCU MCP。
