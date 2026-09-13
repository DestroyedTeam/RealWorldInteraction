# Mac 通用 agent：基础实现与独立验证

日期：2026-09-13。源码提交 `3426811`。
**基础代码已落地，不等于 Mac 实机绘图或整体可替代性完成。**

## 本批结果

| 检查 | 结果 |
|---|---|
| 独立服务架构 | 同一 `rcu-agent` 和 mTLS 协议；启动、身份、工具目录不依赖任务/应用/窗口 |
| 默认公开回归 | 424通过，5忽略，0失败 |
| 全feature公开回归 | 511通过，6忽略，0失败；使用本次独立构建的fixture worker |
| fmt / 严格clippy / diff | 全部通过 |
| 架构 | 门禁及4项负向回归通过；仅批准增加agent→macOS后端依赖边，其他阈值/checker未改 |
| 生产依赖图 | Mac/Windows agent各引自己的GUI后端；MCP与transport不引两平台GUI执行后端 |
| 真实Mac进程预检 | UID/EUID/console UID=501、实际安全会话100002、on-console/login-done为真；无选窗targets可构造 |
| Host Mac GUI与全局安装 | 未做，不冒称通过；现有Windows安装未动 |

架构门禁仍报告 `architecture_complete=false`。独立测试前后27文件冻结清单、
235文件源码快照及新fixture worker SHA均一致；不把保留的旧worker当作本次构建。
Mac进程预检没有读窗表/AX内容、截图或输入，TCC为true只属于该次测试进程，
不能推广到以后安装的agent。硬件console-set字段缺失仍为null；seat仅为逻辑冲突域。

## 没有藏掉的失败

1. 原生预检先因旧实现强求硬件console-set编号而失败。核对实际字段类型和Apple
   会话定义后，保留真实UID/安全会话/boot校验；使用保守逻辑锁域，不捏造硬件ID。
2. R1公开测试全绿，独立审查仍发现P1：长文本的第二块或双击第二击，会被第一块
   自身造成的值/焦点变化拒绝。R2保留首块完整grounding，用本次调用内的原生AX
   anchor检查后续块；新增7项有状态测试，真的在模拟post后改变值/焦点/标题。
   身份、原布局、精确元素、安全/可写、权限、取消与租约等边界变化仍停止后续输入。

## 剩余项

需完成Mac安装件/实际进程权限、Host原生MCP配对与真实Paintbrush验收。
本批尚不支持Mac Pointer、多点拖拽、capture_visual、原生ROI、激活/启动及owner链
迁移，不从Windows已经可用的功能推断这些能力也已经可用。

设计说明：[Mac受控端](../architecture/macos-device-agent.md)。
独立证据位于 `work/resume-and-performance-20260913/`：
`macos-self-review-r2.json`、`macos-verifier-r2-report.json`、
`macos-verifier-r2-command-log.json`；旧R1失败与审查均保留。
本次fixture worker SHA：`da79642edcaff2cf1f2727924294b16df07c88d3dfeb4b1a45f8b23de70c9316`。
