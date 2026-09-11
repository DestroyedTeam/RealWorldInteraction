# A01：原生 worker 隔离是尚未实现的架构要求

依据：ADR 0002 与固定候选 `7fbbeeb` 的独立复审。状态：**未实现**，不是“只差补测”。

当前 `rcu-agent::Runtime` 在同一 agent 进程内通过 `tokio::task::spawn_blocking`
调用 native backend。上下文 gate、桌面输入域 gate、UIA timeout 和 cooperative
checkpoint 可以限制后续派发，但无法隔离 native access violation，也不能让一个
已经卡住的原生调用安全退出。Rust panic 的 JoinError 不是完整原生崩溃隔离。

必须落地的边界：

- device service 保留认证、授权、统一租约/预约账本、状态查询与恢复判定，不把它们
  交给每个 worker 各自决定，更不能重新引入业务任务、Stage 或 oracle。
- 原生执行进入可独立识别和管理的 owned process。IPC 校验实际 peer 身份、
  PID/创建时点、会话与权限、服务/worker 代际；数据帧与排队有界。
- 取消请求、worker 已退出、输入资源已回收是三个不同事实。超时不能伪造 terminal
  或释放仍在使用的资源。必要时仅结束精确拥有的 worker，不按名称批量结束进程。
- worker 丢失时保留副作用预约与未知状态，仅限制受影响的输入域；其他设备及服务
  控制面仍响应。恢复/重新观察不得自动重放输入，旧窗口/frame 引用必须失效。
- 普通/Admin worker 在正确的交互会话内运行；不能把 Session 0 进程冒充用户桌面。

关闭此缺口需要实现证据与测试，而不只是文档：独立进程 IPC/所有权代码；受控
provider 挂死和进程崩溃注入；父服务、其他 worker/设备的存活与响应；真实 Windows
上的 PID/创建时点、取消、强停后资源核查、旧预约不重放和代际失效。必须区分
合成故障 U/P 与原生 N，也不将这些结果扩大为业务任务或多平台验收。
