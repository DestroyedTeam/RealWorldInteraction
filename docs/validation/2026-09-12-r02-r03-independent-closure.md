# R02 / R03 定向独立复核

## 裁决

**R02、R03 的原报告缺陷可关闭。R01 整体继续开放，ADR 0002 / 0003 的完整架构验收仍未完成。**

本轮固定检查：

- 修复实现：`a5e41be1b679cca210a846f4c0c784afc35bc2f5`。
- EOF 增强测试：`78f9df6ee9dd13392f94d26021fc5dae473a88af`。
- 候选证据索引：`a9d68f9b932a11f9ce4154f1be8484ba0f01b12f` 中的 review-fix-candidate JSON。
- ADR 0003 状态记录：`23fa01bff905fcdd67ec203d2e42edd58e726953`，明确为已确认、尚未实现。

方法为固定 Git object 的静态检查及已有测试/部署记录核对。**没有重新运行 Cargo、启动 Host/native 进程、连接 VM、操作 GUI、修改产品源码、配置或账本。** 新增文件仅为本次评审与证据副本。

## R02：半写恢复记录导致永久封锁——原缺陷关闭

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-agent/src/ledger/recovery.rs:18–102,132–229`。

原来同一 pending snapshot 只能写固定文件名，坏文件存在后永远得到 AlreadyExists。现在每次尝试生成独立 UUID，先写 `.pending`，完整写入并同步后，使用不可覆盖的 hard link 发布新的 committed JSON。读取只信任有效的 committed 记录；坏的旧 v1/v2 记录和未提交文件不阻止在容量范围内提交新的合法证明，也没有被删除或覆盖。

已核对测试源码与成功日志：

- 六个边界都真的启动子进程，并检查其在选定阶段以退出码 73 结束，而不是只返回一个模拟错误。
- create、partial write、complete write、file sync、publish、directory-sync 路径分别覆盖。
- 未发布的记录不解除隔离；重开 Ledger 后再次恢复成功。
- 已有中断文件原字节保留，旧 request 仍返回 Existing，另一个 domain 仍保持 fenced。
- 空/截断/无效旧固定文件名可由新证明取代其授权作用，但原坏文件不被改写。
- Runtime 测试另检查只读主体和忙碌写域都被拒绝；授权及资源检查成功后才解除域封锁。

**关闭范围：** 原报告的“单份半写恢复文件使后续合法恢复永远失败”。不是全部磁盘故障、任意账本损坏或资源恢复正确性的总验收。

**保留限制：** 恢复目录仍有 256 项容量限制；容量耗尽继续拒绝，需要后续保留/运维策略。目录同步代码仅在 `cfg(unix)` 执行，因此 Windows 上名为 `directory_synced` 的第六个故障点不能解释成实际执行了 Unix 目录 fsync。现有 Windows 证据是进程退出与文件系统行为，不是物理断电测试，也不是原生按键释放状态的 N 验收。

## R03：无 receiver 时 EOF 状态丢失——原缺陷关闭

**位置：** `/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway/shutdown.rs:1–18,36–114`；`/Users/vincentadamnemessis/AI Projects/Real Computer Use/crates/rcu-mcp/src/gateway.rs:165–185,248–256,454–465`。

Stop guard 改成 `send_replace(true)`，停止状态不再依赖当前是否有订阅者。生产 stdin feed 使用这个 guard；Invoke 在准入处检查已保存状态，select 也优先处理已经就绪的 EOF/取消。

增强测试先确认 receiver 数为零，构造但不 poll 一个参数合法的 action future；触发 Stop 后再 poll。已排队和后续 Invoke 均返回 `host_disconnected`，配对的认证 loopback test peer 收到的原始 Invoke 计数为零。旧的“Invoke 已开始后 cancellation/EOF”实际 MCP 子进程测试在现有日志中继续通过。

**证据分层：** 新的确定性零 receiver 用例直接调用 Gateway 方法，并设置真实认证 test peer，属于组件/集成测试；它不是在同一个用例里通过外部 MCP stdio 子进程制造该调度窗口。保留的外部 MCP 进程测试覆盖另一类已进入调用后的 EOF。两者结合源码足以关闭原状态丢失缺陷，不能扩展成全部 Host Stop 时序、原生强制抢占或取消后资源释放均已验收。

## 证据核对结果

候选索引中的 **14 份证据，其大小和 SHA-256 全部一致**。原日志按 `test result` 汇总：

- 默认 workspace：**205 passed / 0 failed / 1 ignored**。
- 全 feature：**292 passed / 0 failed / 2 ignored**。
- Windows 保存的单测结果：**8 passed / 0 failed**，包括六阶段进程退出循环。不是 8 项 GUI 测试。

上述是对实现方原日志的独立核对，没有由本评审者重新跑测试。

附带核对了历史部署审计：before/after 的 **4 份 ledger JSON SHA 完全一致**；保存的 request 和 monitor 两种查询返回同一 reservation/result。这里只确认保存记录的一致性，没有重新读取 Windows 上的文件或验证 PID 4808 当下存活，也不将这一无冲突查询实例用于关闭 R01 的代际问题。

## 明确保留的未完成项

- **R01：仍开放。** 服务签发的代际绑定 `operation_ref`、独立耐久 `request_id`、互斥 selector、旧瞬时引用明确过期和接受前取消，须按 ADR 0003 实现并另行验收。Skill 的规避说明不是服务正确性。
- **A01：仍未实现。** 当前同进程 `spawn_blocking` 不等于 owned worker 的挂死/崩溃隔离。
- compact/完整证据取回、通用 launch、legacy 迁出、完整原生/多平台矩阵和自主模型 M 层，不因本轮关闭两个缺陷而完成。

先前固定候选评审保持为历史记录；本文件只更新 R02、R03 的定向裁决，不回写旧证据或扩大既有验收范围。
