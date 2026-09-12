# 流式分层与 Nemessix 真实应用验收

日期：2026-09-12。记录用户新增的一期验收标准；不把测试窗口当最终产品验收。

更新：下文保留当时的待办状态。其后的实际结果及纠偏见
`2026-09-12-cua-rcu-drawing-comparison.md` 和
`../validation/2026-09-12-paint-prebaseline.md`：连接复用已落地；Notes 路线改为
Paintbrush / JS Paint 独立对照，但完整入口、下载和 Pointer 验收仍未全过。

## 1. 参考图标和目标

本机正式版 `/Applications/Nemessix.app`，bundle ID
`io.github.vincentadamnemessisx.nemessix`；不是 NemessixPaste。
实际图标是深色圆角底、银色盾牌与顶帽、橙色破裂 X 和碎片。
正式版与 `Nemessix-DV.app` 的 icns SHA 均为：
`5a1168e004af006f91fd5185ba18ffa822a8911b049cac37062caa4050f4636c`。
已通过 iconutil 提取原始图标尺寸并实际查看，提取只用作参考，不当成绘图产物。

目标是使用软件自身的形状、描边、填色、绘笔等工具重绘这个图标，再保存并
重新打开验证。不能导入原图、脚本生成成品、贴 SVG 或用测试窗画圆冒充完成。
默认使用同一 512×512 参考和目标画布，逐项记录轮廓、比例、颜色、破裂细节、
编辑过程和最终成品；不预先宣称像素级一致。

## 2. 必测组与对照限制

| 组 | 实际路径 | 状态 |
|---|---|---|
| A | Mac 内置 Computer Use → Notes 的附件 Markup → 图标 | 已连 Notes、建立独立画稿标题；绘图尚未开始 |
| B | Mac Codex → 全局 RCU MCP → Windows agent → Paint → 同一图标 | 尚未开始 |
| C | 同一应用中的连续轨迹与分步工具流程 | 待记录；不能把不同 OS 的耗时当受控 A/B |
| D | 持久连接、断连/重连、取消/恢复、不重放 | 取消/恢复仅通过底层预检；连接复用未实现 |
| E | 实际 Pointer、方向/动作反馈、截图中的坐标与光标 | 尚未完成 |
| F | Mac 同平台 RCU 对照 | production Mac device agent 尚未实现，不能启用 legacy 桥接冒充 |

Mac 已实际通过 CUA 访问 Notes，用户新建的空白 note 已命名为
`Nemessix — Computer Use drawing benchmark`。其他原有笔记未修改。
Apple 官方说明 Notes 可在图像/PDF 附件中使用 Markup 的 Sketch、Shapes、
描边及填色；Draw 另受 Force Touch 条件限制。这与从 iPhone 导入 Add Sketch
不是同一条链路。下一步需用空白附件实际确认 Markup，不预判不存在绘图功能，
也不为此直接安装额外应用。
[Apple 官方说明](https://support.apple.com/en-gb/guide/notes/apdef7f74190/mac)。

各组记录 Host/工具/版本、开始结束时间、完整模型轮次、工具结果→下一调用
间隔、MCP 与内部 RPC 次数、连接/握手次数、图片/文本量、错误恢复和用户介入。
保留每个关键节点的实际 UI/图像及文件证据。不同应用工具能力、OS、人工置前
和预置画布时间分别列出。最终成品与链路均要通过，不能只看一张像的截图。

GUI 和 SSH 始终是并列互补通道。若启动、保存核对等节点用了 SSH，就明确写出；
没有执行或尚未实现的 MCP 节点保留为缺口，不以 Shell 成功覆盖 GUI 未验证。

## 3. OpenAI 官方“流式”不是一个单一概念

### 模型响应层

普通 HTTP `stream=true` 用 SSE 增量返回模型输出。它不能单独证明客户端可以
持续向当前生成过程送入任意桌面帧。
[Streaming API responses](https://developers.openai.com/api/docs/guides/streaming-responses)。

Responses WebSocket 使用持久连接、增量输入及 `previous_response_id`；当前
文档还说明 `stream_id` 可复用一条连接处理不同会话。它针对模型 API 的续接，
不是 RCU 的 Windows 控制 socket。
[WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)。

GPT-6 Astra 的 `response.steer` 可在响应进行时提交用户更新。accepted 只是
排队确认；服务在相应边界继续，不撤销已输出内容，也不取消已开始的工具。
因此模型收到“停”与设备实际停止/抬键必须各自有证据。
[Mid-turn steering](https://developers.openai.com/api/docs/guides/steering)。

实时音频连接还有自身的双向音频/JSON 事件契约，不能与 Responses 或 MCP
传输混同，也不能据此宣称 Computer Use 在连续消费桌面视频。
[Voice WebSockets](https://developers.openai.com/api/docs/guides/voice-websockets)。

### 工具传输与 Computer Use 层

OpenAI 明确支持远程 MCP 的 Streamable HTTP 或 HTTP/SSE。这里说明的是
工具接入的传输兼容性，不等于宣布所有 MCP 模型输入和输出都是实时双工。
[MCP and Connectors](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)。

公开 Computer Use 示例描述：模型给动作，应用按序执行，回传新的 screenshot，
再继续。工具 call 的 completed 可仅表示模型已生成调用，不表示应用已执行。
当前资料不足以把 Codex 桌面私有实现等同于连续视频推理协议。
[Computer use](https://developers.openai.com/api/docs/guides/tools-computer-use)。

## 4. RCU 当前事实与下一步

本机 `computer.js` 是持久编排环境，Host 使用 stdio MCP；**这不是流式模型 API**。
`crates/rcu-native-link/src/device.rs` 的 `Client::call` 仍每次创建 TCP/TLS，服务端
每次回复后关闭连接。gateway 的 Connection 存的是逻辑 context token/instance，
不是可复用 socket。不能拿持久 JS、低 RTT 或 Codex 的 IPC 探针替它证明复用。

接下来应分别交付和测量：设备连接复用（控制/取消不能排在长动作后面）、有界
轨迹原生执行、可视 Pointer/光标采集、画面反馈背压/过期帧、Host 模型续接。
传输的持续性、原生输入的连续性、视觉反馈和模型输入更新是不同节点。

所有更新继续遵循 ADR 0002：服务、发现、通信不依赖图标、Paint、Notes、测试
任务或某个 oracle；具体绘图计划只在调用者/验收层。现阶段只有原生轨迹预检
通过，以上真实应用矩阵与“替代 Computer Use”的一期结论仍然 **pending**。
