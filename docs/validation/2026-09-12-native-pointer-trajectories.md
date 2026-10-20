# 原生轨迹预检：绘制、取消、抬键与恢复

日期：2026-09-12。部署实现 `52ee7b4`，legacy schema 隔离修复 `4f2d5c3`。
**只通过底层预检，不代表 Paint/Notes 图标验收通过，也不代表已替代 Computer Use。**

## 实际部署与范围

- Mac 用户级安装 adapter SHA：
  `e62bb97bff0d3d948123fe94fe3e21f20826a71dec282d3e6fbf15ff8db1f024`。
- Windows ARM64 agent SHA：
  `449f3bc1d12093c02bbe4cb78feb3bed508a76bd4b13b2b03c5f7567cbc1d0e5`，
  PID 4184、Session 1；原配置和启动任务保留，无黑控制台。
- 9 个 Mac profile / identity / SSH / Codex 配置文件 hash 未变；两个安装均有
 独立 `before-drag-v1` / `rollback-drag-v1` 回滚副本。
- 轨迹通过新启动的**实际安装 Rust MCP 进程**、持久 JS SDK、mTLS 设备 RPC 和
  Windows SendInput 执行。没有 Python/PowerShell 鼠标注入或生成图像冒充绘制。
- 独立 Rust 测试窗只绘制它收到的窗口事件，并写 create-new 日志；日志通过
  真 SSH 读取。SSH Shell Session 0 读取的是 Session 1 应用的记录，不是查询
  Session 0 的鼠标状态来冒充 GUI 状态。

## 结果

| 用例 | 计划 | 测试窗实际 down→up | 接收到的事件 | 结论 |
|---|---|---:|---|---|
| 闭合矩形 | 4 边、1200ms | 1197.815ms | down 1 / move 78 / up 1 | 图像显示闭合矩形 |
| 多顶点圆周折线 | 33 顶点、1600ms | 1603.597ms | down 1 / move 125 / up 1 | 图像显示闭合圆形 |
| 中途取消斜线 | 原定 5000ms | 361.990ms | down 1 / move 15 / up 1 | 提前停止，保留短线，没有续画 |

每个绘图用例只有一个 checked intent / durable request，不是模型逐点发 MCP。
圆形原生计数为 129 个计划采样点、128 次 move 插入、一次 release；执行器
记录 1610ms。**插入数、窗口事件数和渲染帧数是三件事**；本轮未确定每个计数
差异的具体原因，也没有将它们换算成“保证 60fps”。截图已逐张实际查看。

取消是在 SSH 日志已确认按钮 down 后，查询原始 durable request 得到确切
operation_ref，再请求 `computer.cancel`。取消调用约 1.826ms，仅表示请求往返；
不是完整抬键延迟。随后原执行状态为 terminal / resources_released，应用收到
up，50ms 后的应用记录显示 left_down=false。原生诊断保留了 partial / uncertain
动作和已插入 22 次 move 的记录，没有以取消确认抹掉此前影响。

取消后 input domain 仍按策略 fenced。显式 `computer.recover_input` 使用原生
Session 1 的 GetAsyncKeyState 确认无按键/按钮按下、无活跃 writer，再解除该
资源域的隔离；它没有发送输入。再次查询原 request，**replay_allowed 仍为 false**。
之后对新观察的确切 focused 元素发送一次 F5，测试窗独立记录到一个 F5 事件。
这关闭的是 Windows F5 编码/派发预检，不是 Task Manager 刷新业务效果证明。

## 没有隐藏的失败和环境变化

- 初次只读发现早于测试窗启动完成，返回无匹配；没有输入。等待启动确认后
  重新做只读发现。
- 初次 `lifecycle_preflight` 误用了 activation-only 授权；该接口需要 launch
  permission，已记录为调用者错误，不修改产品去迎合测试流程。
- 一次正式 `activate_window` 请求返回 `api_accepted=false` / `not_foreground`；
  没有强抢焦点或重放。用户将测试窗置前并移动后，重新绑定和观察，才开始画图。
  路径使用原生 reference 的相对坐标，不依赖测试窗原来的桌面位置。
- 新连接首次 recovery 缺少 targets 上下文，拒绝且未派发；取得新授权上下文后
  才执行原生恢复。不是让旧不确定动作重新执行。
- 全功能回归发现新 Intent 泄漏到冻结 legacy schema，握手表面报 TLS EOF。
  通过独立 LegacyIntent 保持旧协议，未改 golden 预期或把新能力写进旧场景。
- SSH 的跨 session CloseMainWindow 未确认关闭。最终用当前任务加载的原生
  RCU 点击确切 Close 控件；SSH 独立确认测试进程退出并移除临时启动任务。
  正常 RCU agent 和启动任务继续保留，旧 CrossMacro / NemessixPaste 未改动。

## 回归与未验收项

fmt / all-targets all-features Clippy 通过；默认 Rust 252 passed、2 ignored；
all-features 339 passed、3 ignored；JS SDK 15 passed。架构 ratchet 通过但不代表
架构债务全部解决。原始日志、截图、测试启动脚本留在忽略目录；精简事件与引用
见 [验证 JSON](2026-09-12-native-pointer-trajectories.json)。

本轮不是完整模型绘图流程计时，没有把写代码、等待用户、编译时间与原生笔画
时间混为一谈。仍需完成真实 Paint/Notes 的 Nemessix 图标对照、连接复用、可视
Pointer、模型返回后间隔统计，以及缺失平台能力。验收标准见
[真实应用与流式分层](../analysis/2026-09-12-streaming-and-icon-acceptance.md)。
