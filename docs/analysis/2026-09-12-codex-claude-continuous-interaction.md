# Codex / Claude 连贯交互机制：证据与落地边界

日期：2026-09-12。继续遵循 ADR 0002 多端、无业务 UI 依赖的架构；不是改成
仅 Mac→Windows 的双端产品，也不是把清理 CrossMacro 固化为能力。

## 已确认的实现，而不是对“丝滑”的猜测

### Codex：实际安装的客户端 + 隔离 Unix socket 探针

读取本机 `@oai/cua` 0.2.4、`@oai/sky` 0.6.32 的可读 JS，沿用
[源码对照与来源清单](2026-09-12-codex-computer-use-reference.md)。本轮进一步
导入实际安装的 `MacComputerUseClient`，连接测试专用 Unix socket 对端：

- `drag({app,fromX,fromY,toX,toY})` 实际发出 **一个**
  `ComputerUseIPCAppPerformActionRequest`，携带起点和终点。
- 探针确认连接复用、请求串行化、断开的原请求不重放，以及后续请求重新连接。
- 持久 JS / app 对象与按需输出，让确定的动作无需每一步经过模型重新规划。

探针结果纳入本轮[验证 JSON](../validation/2026-09-12-interaction-remediation.json)。
这是**真实客户端接合成对端**，不是实际 native 服务的抓包或原生输入验收。
最初探针对起终点误用对象形状的断言失败；实测线格式是数组，修正后通过。
没有用 Proxyman 假装捕获 Unix socket，也没有重新尝试已被拒绝的 Codex UI 权限。
闭源原生侧怎样插值、多少 Hz、采用什么轨迹，以及肉眼绘制流畅程度，均未测得。

### Claude：官方接口与官方参考执行器

官方 Computer Use 是客户端执行的工具循环。模型可一次给出短动作序列，应用
按顺序执行；截图/zoom 返回图像，其他动作可返回短确认。失败后的依赖动作
不能继续当作成功执行。这不是模型持续接收桌面视频的证据。
见 [Claude 官方 Computer Use 文档](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)。

官方 Linux 演示的拖动将移动、按下、移动、抬起组合在一次执行中，执行器使用
Python/xdotool；这证明演示如何组合动作，不证明所有 Claude 产品的原生轨迹
或帧率。RCU 不复制该 Shell 后端，仍由 Rust 设备 agent 做原生执行。
见 [官方演示执行器](https://github.com/anthropics/claude-quickstarts/blob/main/computer-use-demo/computer_use_demo/tools/computer.py)。

演示同时提供 VNC/noVNC 给人观看桌面。人看到的持续画面，与模型拿到的工具
截图是不同通道，不能混为实时视觉推理流。
见 [官方演示说明](https://github.com/anthropics/claude-quickstarts/blob/main/computer-use-demo/README.md)。

## 应保留的机制与当前实现

| 机制 | RCU 当前状态 |
|---|---|
| 持久对象、一次执行确定动作、按需输出 | Rust/QuickJS `computer.js` 已有；无 Shell/network JS API |
| 紧凑语义观察 | 本轮新增 `snapshot` / `summary`、frame-scoped `eN` 引用 |
| 可读的小图与精确映射 | 本轮默认预览长边 1024，保留原图与像素中心映射 |
| 错误可理解且不掩盖执行不确定性 | 嵌套拒绝升级为 MCP 错误，保留原始执行引用与状态 |
| 原生连续拖动/轨迹 | **尚未实现**，不能宣传丝滑画图或零延迟 |
| GUI 与 terminal 互相执行、验证、反馈 | Windows 原生 GUI + 真正 SSH 已交叉验证；SSH 仍是 Host 独立工具 |
| 模态观察、跨应用发现/切换 | 仍有覆盖预算和绑定范围问题，未完成整改 |

`snapshot` 是模型呈现层优化。设备依然捕获并传输原始图像；缩小预览不等于
降低了设备链路带宽。`inline_image:false` 也不等于原生端不截图。

## 后续实现的明确边界与验收

以下是既定多端架构下的待交付项，**不是已经完成的功能**：

1. 在公共协议中定义有界 gesture/trajectory：时长、点数、坐标空间、按钮和
   modifier 状态、目标/session/seat、授权、原始 operation_ref。每个设备 agent
   实现自己的原生执行；不让模型逐像素发 RPC，不引入 Task Manager 等业务分支。
2. 设备内按节奏插值；取消、异常或失联时释放本次持有的按键/按钮。保留已执行
   部分和不确定性；重连不重放。串行桌面输入不能因多开窗口而假装并行。
3. 视觉反馈可独立流式/按变化推送，需处理背压、过期帧、时钟、坐标映射和
   同一设备身份。人用预览与模型观察分别标识，不把视频刷新率当控制完成率。
4. terminal 与 GUI 是并列的 capability family，各自保留认证、user/session、
   资源引用和结果；允许同一任务交叉核对，但不能拿 SSH 的 Session 0 冒充
   GUI 的 Session 1。当前尚无 RCU `terminal.*` API，不能仅安装 sshd 就宣布完成。
5. 连贯性要分别测：模型整轮/返回后下一调用间隔、链路 RTT、native gesture
   起始/持续时间、丢点/取消/按钮释放、视觉反馈延迟，以及真实绘制效果。
   持久 socket、SSE 或 WebSocket 的名字本身不构成流畅性证明。

当前完成的是两类参考的可核实机制分析，以及紧凑交互的一部分落地；不是
Claude/Codex 闭源 native 行为的完全还原，也没有声称 Rust 天然更快。
