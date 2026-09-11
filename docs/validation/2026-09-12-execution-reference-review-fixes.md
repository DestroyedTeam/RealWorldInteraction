# 执行引用独立复核后的修复

最终产品候选：`e2f35980f641337b32fc65fc3ca9f594e0863c4e`。
独立原始基线仍是 `839e81b`，报告保留，不因修复移动基线。
**修复后的独立结论待确认；R01、A01 和完整架构不在本文自行结案。**

## ER01：已接受后的取消失败仍保留控制引用

`06ebeb7` / `d1f237d`：取消超时、断连以及 Execute 被设备拒绝时，回包均保留
已取得的 operation_ref。送达确认与取消接受分别报告，不推断原生终止。

除 accepted-only 测试外，`2f0f774` 增加组合 P 测试：真实 adapter + mTLS +
Core Operation/NativeCall 的合成 held provider。Execute 已进入 pending scope 后
Host EOF，Cancel 超时的回包仍能供新控制连接查询同一 pending owner；控制端
路由断开也不丢 ref。恢复后 Cancel 不提前报告资源释放，显式释放合成 scope 后
才 terminal/resources_released；Execute 计数始终为 1。
它不是生产 Runtime/真实 OS provider 卡死注入，也不是 A01 强终止验收。

## ER02：namespace 首次写入与旧损坏权威恢复

`dba5965`：完整 staging + sync + 原子 no-overwrite 发布；六个子进程退出点
验证同设备重启恢复、不同设备拒绝，保留未完成尝试，不污染最终权威文件。
旧损坏 final 需精确 SHA 与先前可信 committed backup；离线 Rust operator
恢复保留损坏 inode，不删除/修改账本，且活服务独占锁会阻止恢复。
维护步骤见 `docs/migrations/2026-09-12-device-namespace-recovery.md`。

另有 `e64a8e7` 将 panic fence 与两个 permit 放进同一所有权 guard，先 fence
再释放字段，避免 unwind 与其他 writer 竞争。仍没有实现跨进程 native 隔离。

## 保留失败与后续锁修正

`2f0f774` 的首轮 all-features 在既有 recovery 测试最后一次 drop/reopen 处
出现一次 `service_state_already_owned`；原始失败完整保留于
`work/execution-ref-v2-20260912/review-final/all-features-tests.raw.log`。
单独 serial/parallel 复查均通过，没有用这些复查抹掉首轮失败。

文件锁在 duplicate/inherited 描述符全部关闭或显式 unlock 时才释放。
[Rust File 官方文档](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)。
并发子进程创建导致短暂继承是解释该失败的推断，具体 fork 时序没有捕获。
`e2f3598` 在正常服务所有者最后析构时明确 unlock，而不是等待偶然副本关闭；
新测试验证活 owner 仍互斥、旧副本不延迟新 owner、关闭旧副本不解锁新 owner。
没有删除锁文件、添加重试、跳过互斥或放宽输入恢复判定。

## 最终候选验证

| 范围 | 结果 |
|---|---|
| workspace 默认 | 223 passed / 0 failed / 1 ignored |
| workspace all-features | 310 passed / 0 failed / 2 ignored |
| Windows ARM64 实际 executable | 25 passed / 0 failed；3 suites |
| fmt、Host Clippy all-targets/all-features、Windows agent Clippy | 通过 |
| architecture ratchet / source hygiene | 通过，不等于完整架构验收 |

未过滤日志与起止 SHA 在 `work/execution-ref-v2-20260912/owner-final/`。
Windows suite 的 stderr 有被测试正常捕获的 synthetic panic，不是生产 agent
异常；三个 executable 均 exit 0。实际离线 CLI 合成恢复也 exit 0，没启 TLS/provider。

最新 agent 与 adapter 都已按同一产品候选构建、更新既有安装路径；防火墙、
配对配置未扩大或替换。Windows agent PID `2024`，instance
`079a5177-6192-4b1f-8d78-aa05047b5320`，MainWindowHandle = 0。
升级保留 **4 个原账本文件 + 1 个 namespace** 的全部 SHA；未运行 live namespace 修复。

更新后的新 Codex Host 再次发现 20 工具、读取同一旧请求、拒绝旧执行代际，
metadata/native identity 正常，Host exit 0。原生 PNG/跨 Host 有效引用/实际 agent
重启的更早验证范围见 `2026-09-12-execution-reference-candidate.md`。
本轮新增 live 验证是 H/metadata，未发送新 GUI 输入，不是 M 或全局 Plugin 安装验收。

进一步直接探测**当前调用任务**的 `computer.targets`，仍收到旧 admin5 连接的
`diagnostic_only / connect_and_tls_deadline`。它不是新独立服务的回包；没有按其
旧指令准备业务 task 或重置 HOLD。该任务当前直接工具入口尚未切换。
新启动 app-server 的 H 证据不能证明现有 GUI Host 已热加载，也不能证明 GUI 中
新任务已做 M 级直接调用。已询问用户是否允许创建实际只读验证任务，未获明确
创建请求前不会自动新建。证据：`owner-final/current-task-old-entry.json`。

同名 JSON 固定二进制与证据 SHA。A01 worker 进程隔离、输出压缩、严格额外 RTT
基准、多平台驱动/完整 ADR 0002 仍需继续实现与各自验收。
