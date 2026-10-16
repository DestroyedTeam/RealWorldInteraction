# e2f3598 保存的平台证据：补充核对

本补充针对产品 `e2f35980f641337b32fc65fc3ca9f594e0863c4e` 的后来交付记录，不修改已封存的 2f0f774 / e2f3598 定向复核或其中的失败历史。

## 已核实

- 实现方最终索引列出的 **18 份证据，大小与 SHA 全部一致**。
- Windows 保存结果为三个 executable：execution_refs **6/0**、service **5/0**、rcu_agent **14/0**；合计 **25 passed / 0 failed**，三个进程均 exit 0。stderr 中的 synthetic panic 对应 catch_unwind 单测，不是生产 agent 崩溃。
- 离线 CLI 合成恢复结果保存为 namespace_repaired=true、ledger_modified=false、input_sent=false。它使用合成元数据；不等同对 live namespace 执行过修复。
- 保存的部署审计中，**4 个原账本文件与 1 个 namespace 的 before/after SHA 映射完全一致**。
- Host E 保存的 inventory 为 **20 个工具**；旧请求的 reservation/result 与早期原记录完全相同，replay_allowed=false；旧执行引用返回 execution_ref_expired；设备 metadata 返回 healthy。

## 边界

这些是对已有文件的独立读取、比较和统计。本轮没有运行 Windows executable、CLI、Host 或 native，没有连接 VM，也没有采集屏幕或发送输入；不声称 PID 2024 此刻仍存活。

25 项结果属于 fixture/服务/文件系统测试，Host E 属于保存的接入与 metadata 证据。它们不证明真实 OS provider 挂死、强制终止、GUI 输入、完整多平台或模型 M 层已经通过。ER01/ER02 的定向关闭结论不扩大，R01 综合裁决与 A01 仍单列。
