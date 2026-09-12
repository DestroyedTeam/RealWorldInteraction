# 紧凑交互部署与 GUI / SSH 独立交叉验证

日期：2026-09-12。产品实现提交 `e6a394d`、`bbbbe77`；部署来源
`bbbbe77efaf26abb361707864e354e6f6025a086`。本文件如实区分实现、部署、
原生操作、业务结果和整轮效率。完整结构化记录见
[验证 JSON](2026-09-12-interaction-remediation.json)。

## 1. 实际交付

- JS `snapshot()` 返回短语义控件和 frame-scoped `eN` 引用；`summary()`
  只分页已加载数据。过期 frame/ref 在派发前拒绝，原生身份和检查不变。
- `snapshot({preview:true})` 默认长边 1024，保留原图 SHA、坐标映射和覆盖边界。
- SDK / MCP 明确显示嵌套的原生拒绝，保留原始 operation_ref、native dispatch
  与不确定性；缺失的 request_id 不再回显 null。
- Windows 原生 planner 支持 F1–F24，拒绝无效表示和原有系统切换组合。
  单测/编译已过；本轮现场 F5 没有实际发出，详见第 5 节。

同一帧 metadata 的两次只读实测：

| 样本 | 原 compact JSON | 新 summary | 减少 | 控件数 |
|---|---:|---:|---:|---:|
| 候选二进制 + 更新前 agent | 16,184 B | 3,771 B | 76.7% | 53 |
| 全局安装二进制 + 更新后 agent | 16,087 B | 4,136 B | 74.3% | 58 |

两次预览均由 1538×1169 → **1024×778**，像素数减少 55.7%。原始截图未改写；
预览 PNG 分别 70,948 / 98,021 B，窗口内容不同，不能跨样本推导压缩比或网络
提速。两次 `native_complete:false` 均如实保留，不能根据摘要判定所有进程。
首张预览已人工视觉检查，文字/PID 可读。原图与截图文件留在忽略目录，不提交 Git。

## 2. Windows SSH：确实连接，不拿 VM 命令冒充

用户批准专用密钥及 Mac `10.211.55.2` → Windows `10.211.55.3:22`。
初查虚拟机没有 sshd，OpenSSH.Server 为 NotPresent。VM 管理命令仅用于这次
明确授权的安装，不是 RCU GUI 实现，也不是 SSH 验收证据。

Windows 内置可选功能安装因文件占用失败，未强杀 servicing 进程或修改更新策略。
随后使用微软发布的 ARM64 OpenSSH **10.0.0.0p2-Preview**，Mac/Windows 均核验
发行资产 SHA-256，Windows 核验 Microsoft Authenticode。该版属于验证环境预览
包，不列为生产部署基线。见
[微软官方发行说明](https://github.com/PowerShell/Win32-OpenSSH/releases/tag/10.0.0.0p2-Preview)。

- 仅新建 sshd 服务和专用配置，不运行会删除/替换 ssh-agent、修改机器 PATH 的
  整套安装脚本；未替换已有 ssh-agent。
- 公钥认证，禁止密码认证、agent/TCP forwarding；限制账户与来源地址。
- sshd 仅监听 10.211.55.3:22；防火墙规则
  `RealComputerUse-SSH-Mac-22` 精确限定程序、两端 IP 和 TCP 22。
- Mac key/config/known_hosts 放在 RCU 用户级私有目录；未改 `~/.ssh/config`。
  主机公钥经已核实 VM 管理通道取回，再用严格主机密钥校验连接，并非盲目接受。
- 真正 `ssh` 登录和远程命令成功；SSH Shell 在 **Session 0**，GUI agent /
  CrossMacro 在 **Session 1**。计算机和有效账户均已读取并核对。
- 原 GUI 45128 防火墙规则、mTLS identity/profile 与授权保留。

当前是 **Host 层的 RCU GUI + 标准 SSH 两条真实互补通道**；RCU 23 个 MCP
工具里仍没有 `terminal.*`，`computer.js` 不具备 SSH API。这一层尚待产品化。

## 3. 用户要求的独立任务

任务 **验证 GUI 与 SSH 通道及 CrossMacro 重启现象**：
`01a0943f-80eb-7f01-85a8-10e44f73bc64`，projectless，未通过项目配置安装 MCP。
两阶段测试期间没有更换被测 GUI agent；其结束后才部署本轮二进制。

1. GUI 经原生选择、结束、确认，结束 `CrossMacro.exe` **PID 10524**；随后约
   2 分 15 秒未见新 PID。当时还有 7 个实例，不是清空进程。
   另一 PID 4920 在确认前消失，退出原因未确定。
2. SSH 就绪后，核对进程名、完整创建时间、用户、session、parent，再持有
   已打开的进程对象，仅结束 **PID 4728**。SSH 确认退出，紧接的 RCU 原生 GUI
   截图确认消失。任务管理器 PID / 完整创建时间在两端吻合。
3. **14:57:37–15:00:26 UTC+8** 后续检查仍为原有 7 个实例，没有新 PID。
   父 PowerShell **5820** 存活；没有结束它或修改旧桥接/启动源。
4. 读取启动脚本及其加载的 `WindowsBridge.cs`，发现其接收新管道连接时创建
   `CrossMacro.exe mcp`。支持“连接触发创建”，**未证明固定周期重启**；是否有
   外部客户端持续重连尚未确认。不得为了制造重启结果调用/修改旧桥接。

结论：GUI 单独结束与 SSH 结束后 GUI 核对通过；“结束后自动重启”未证实。
本轮双通道任务不是再次清空所有 CrossMacro 的任务。

## 4. 完整计时，包含工具结果之后的下一步间隔

按明确 task + turn 读取 Host 记录，不读取/导出私有思考正文：

| 指标 | 第一阶段 GUI | 第二阶段 SSH + GUI |
|---|---:|---:|
| 开始（UTC+8） | 14:13:05.495 | 14:54:08.431 |
| 结束（日志） | 14:19:52.961 | 15:02:12.135 |
| Host 整轮 | **407.469s** | **483.753s** |
| 外层 RCU MCP 调用 | 15 | 4 |
| RCU MCP 耗时之和 | 17.103s | 2.981s |
| 全部 Host 工具批次 | 16 | 10 |
| 结果→下一工具/最终报告间隔合计 | **369.159s** | **462.714s** |
| 该间隔 p50 / p95 | 21.209 / 43.076s | 38.965 / 71.791s |
| 模型可见文本 / 图像块 | 113,880 B / 5 | 39,890 B / 3 |

RCU MCP 耗时不含 SSH 工具耗时；完整 Host 区间包含所有工具。两阶段 Host
计时合计 891.222s；从第一阶段开始到第二阶段结束是 **49 分 6.640 秒**，其中
阶段间隔约 34 分 15.470 秒，包含授权安装、协调和等待，不是“执行任务只花
20 秒”。这两阶段目的和观察时间不同，不是 GUI/SSH 速度 A/B。

间隔包含模型处理/生成、Host 调度、刻意观察间隔等，不等于纯推理计算时间。
本轮也没有重新做新 summary 的整轮受控模型验收，故**仍未证明端到端效率改善**。
完整逐批次数据在本轮 `-gui-handoffs.csv` / `-ssh-handoffs.csv`；外层 MCP 的
嵌套路由耗时不能再与外层相加。

## 5. 部署与剩余现场问题

- 用户级安装 Mac `rcu-mcp` SHA：
  `6d33069f8a797f00fd05d6ef7be9093531f213bdc71c2508aac315169389eef4`。
  原子替换并保留 `before-snapshot-v1` 回滚副本；9 个 profile / identity / SSH /
  Codex 配置文件逐字节 hash 未变。未强停其他任务的 adapter。
- Windows ARM64 agent SHA：
  `43a2f0180f151106432c309aac2e8d911b07e439335620ed4cfad2467e327f02`。
  真 SSH/SCP 更新，原 exe 和配置已核验；新 PID 8576，Session 1，PE GUI
  subsystem，无常驻黑控制台。`RealComputerUse-Interactive-Agent` 以交互用户
  Highest 直接启动 Rust agent，**零触发器、手动启动**，不依赖业务窗口/脚本。
- 新建实际安装 MCP 进程发现 23 工具，新摘要/预览以及省略 null 回执均通过。
  当前任务 reset 后也可使用新 SDK 并观察新 agent。但当前任务旧 adapter 父进程
  仍会输出旧版回执字段，不能将 worker 更新宣称为所有旧 MCP 进程热升级。
- F5 的第一次调用者预检错误地排除了 focused pane，未 claim/派发；修正调用者
  后，一次 checked action 返回 `checked_context_unavailable`、
  `native_dispatch_entered:false`。原 durable request 已查询，terminal / resources
  released，`replay_allowed:false`；没有重放或转用 Shell 注入。
- 后续原生观察确认 Task Manager **enabled=true、foreground=false**，
  `blocked=true` / `mutation_ready=false`。这是明确的前台条件；不能仅依据笼统
  “document context”错误就归因于文档文本或 UIA 不完整。native-element policy
  本来不要求文档文本。该错误表达仍需进一步区分不可用原因。

真实跨端认证、读取和双通道操作已通过；F5 现场派发、原生轨迹绘制、通用窗口
切换/模态覆盖、RCU terminal API、全新摘要的模型整轮性能仍未验收。
有关 Codex / Claude 参考与后续边界，见
[连贯交互分析](../analysis/2026-09-12-codex-claude-continuous-interaction.md)。

## 6. 回归与工程记录

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace --locked --offline`：241 passed，0 failed，2 ignored。
- 显式 fixture worker 的 all-features 测试：328 passed，0 failed，3 ignored。
- all-targets / all-features Clippy `-D warnings`：通过。
- JS SDK：14 tests passed；包含别名/frame 校验、预览映射、错误和分页。
- 架构 ratchet 与 staged hygiene：通过；ratchet 不代表架构债务全部解决。
- Mac release、Windows ARM64 release 均构建成功。Windows ring 构建通过仅本次
  构建环境把 xwin include flags 转为 clang 格式，使用缓存 lld-link `/lib`；未改
  全局工具链配置。先前失败构建没有当成成功二进制部署。

原始日志、截图、官方安装包、操作脚本和回滚二进制都留在本地受控目录；这里只
提交精简验证数据与文档，不提交密钥、用户配置、二进制或完整任务历史。
