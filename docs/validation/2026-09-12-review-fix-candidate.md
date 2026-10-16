# R01–R03 修复候选与明确保留的边界

基于 Control Center 对 `7fbbeeb` 的[固定独立复审](2026-09-12-service-independent-review-7fbbeeb.md)。机械拆分 `2a37999`；实现 `a5e41be`；增强 EOF 断言 `78f9df6`。**待新一轮独立复审，不是完整 ADR 0002 验收通过。**

## R01：持久 ID 交叉案例已修正，历史引用代际仍待完善

查询增加 `id_kind=monitor/request`；默认 `auto` 对多个不同操作明确拒绝，不再由查找顺序决定。在检查 live registry 前先排除耐久别名歧义。adapter 同样分开 request/monitor 的路由命名空间，包括旧 reservation 的字段命名适配。

真实 MCP 进程＋实际 Runtime＋mTLS＋显式 fixture 测试构造 A `(req-a,op-b)`、B `(op-b,op-c)`：原 Host 的 auto 查询拒绝歧义，明确类型分别选对；关闭原 Host，新 Host 在打开 context 之前仍能正确查询两个耐久记录；只收到两次原始 mutation Invoke，没有重放。另测不同设备、主体和同字符串不同类型。

**尚不关闭 R01 整体引用生命周期问题。** 旧只读操作的 monitor ID 未持久化，也没有带 instance/context 的限定引用；在后续 context/instance 中重用该字符串，仍可能与一个持久 request 或 monitor 混淆。`id_kind` 修正了已记录的持久交叉案例，但不能凭空恢复已遗失的代际信息。Control Center 已确认[服务签发 operation_ref＋独立耐久 request_id 的规范契约](../adr/0003-execution-reference-and-durable-request-identity.md)；必须继续实现，不能再叠加标签查找规则或业务特例。

## R02：可以保留坏记录，再提交新恢复证明

不再覆盖固定 snapshot 文件。新记录先写唯一 `.pending` 文件，完整写入并 fsync 后，以不可覆盖的 hard link 发布新的 committed JSON。读取忽略未提交文件，保留兼容的完整 v1 证明；空/截断 v1 或 v2 记录保留为证据，新检查可产生新提交。坏记录仍计入有界容量，没有删除任何操作预约或重发原动作。

在 create、部分 write、完整 write、file sync、publish、directory sync 六处真实退出子进程；重启验证未提交记录不解除隔离、再次恢复成功、坏证据原字节保留、旧操作不可重放、其他 domain 不变。Runtime 测试另检查只读授权、忙碌写域会拒绝恢复，重新核查后才可恢复。上述单测亦在 Windows ARM64 上实际执行，8 passed；这是进程崩溃/文件系统证据，不是物理断电或真实按键资源检查的 N 验收。

## R03：EOF 状态不再依赖订阅者是否已经存在

Stop guard 使用 `send_replace(true)` 保留关闭状态，并优先处理已就绪的取消/EOF。确定性测试先构造未 poll 的有效 action future，此时 receiver 数为 0；触发 Stop 后才放行调度。配对的真实认证 loopback peer 收到的原始 Invoke 为 0，后续调用同样被挡住。跨平台纯状态测试与原有“已开始 Invoke 后取消/EOF”的实际 MCP 进程测试都保留。没有将取消 ACK 解释为 native worker 已停止。

## 更新与证据范围

- 默认 workspace：205 passed，1 ignored；全 feature：292 passed，2 ignored。
- fmt、Host all-target/all-feature Clippy、Windows agent all-target Clippy、架构 ratchet 通过。
- Windows 仅替换已核对 PID/创建时点/镜像的自有 RCU agent：6820 → 4808，仍无主窗口；4 份原操作预约/结果文件 SHA 全部不变。配置、配对与用户应用未改动。
- Mac 稳定 adapter 原子替换。新的真实 Codex Host 看到 20 个工具及新查询 schema；按 request/monitor 查询得到同一条旧动作回执，另一条旧回执也可查，`replay_allowed=false`。本轮未发送新的原生 GUI 输入。
- [A01 owned worker 隔离](../migrations/2026-09-12-owned-worker-isolation-gap.md) 已明确列为**尚未实现**；当前仍是同进程 `spawn_blocking`。compact、launch、完整原生/平台矩阵与 M 层也没有因此完成。

结构化索引：[JSON](2026-09-12-review-fix-candidate.json)。原始日志、RPC 和部署审计留在忽略的工作目录，索引记录大小与 SHA，不包含密钥或截图。
