# ADR 0003：执行引用与耐久请求身份分离

状态：**Control Center 已确认的 ADR 0002 契约细化；839e81b 的固定复核发现 ER01/ER02，e2f3598 为后续修复候选，等待修复复核。**
日期：2026-09-12。完整架构约束持续生效，不是临时替代架构。

## 决策

使用服务签发的规范 `operation_ref` 表示一次执行实例。服务侧权威记录必须绑定
reference version、device、真实认证 principal、agent instance/epoch、context generation
及不可复用的 execution nonce；只读执行同样适用。随机 token 加服务侧记录即可，
不要求新建 PKI。引用可以携带可解码的路由 envelope，但不能用其中的声明代替授权。

`request_id` 单独表示需要耐久防重放的逻辑副作用，去重/查询命名空间是设备与真实
认证主体。不得把会变化的 agent/context epoch 混进耐久去重键。耐久记录可保留原
operation_ref 作为证据，但不能使已过期的执行实例复活。

现有 caller-supplied operation_id/monitor_id 如保留，只作为关联标签；不再用来
查询、取消或路由。不能通过永久保存所有只读标签来掩盖缺少代际身份的问题。

## 公开接口与边界

`computer.operation` 使用互斥 tagged selector：

```json
{"selector":{"kind":"execution","operation_ref":{}}}
{"selector":{"kind":"request","device_id":"paired-device","request_id":"logical-id"}}
```

上面的空对象仅表示 operation_ref 的占位，不能作为合法引用。正式字段由共享
类型契约确定；调用方必须使用服务返回的引用，不能自行拼接。

`computer.cancel` 只接受 operation_ref。拒绝裸标签、request_id、auto lookup、
混合选择字段或优先级猜测。adapter 不再递归扫描任意回包的监控标签建立操作路由，
而从明确的 typed execution/receipt envelope 登记；路由只能指向已配对设备。

agent 重启、context 真正结束或已终结控制记录被有界回收之后，旧 ref 明确过期。
不得退回同字符串的新 monitor/request 查询，即使当前只有一个候选。过期不表示
动作没有发生，也不表示 native 已停止。尚未结束的 native 工作必须继续保有精确
控制记录和所有权，直到可核验终结或进入隔离恢复。

## 接受与取消竞态

在可能长时间运行的可取消工作/原生派发开始之前，必须先建立可精确取消的接受记录。
只在最终结果中补 ref 不符合要求。

当前单最终回包 RPC 可在 adapter 内部采用最小 Prepare → Execute：Prepare 仅校验、
签发和登记，不触发 UIA、截图、输入、应用启动或占用输入资源。Host 在收到引用前
停止则不得 Execute；收到引用之后 Cancel 与 Execute 竞争同一个一次性状态。也可
使用先行 accepted 帧或完整限定的私有 transport cancellation ticket，但必须证明
同样的接受前/接受后竞态。不得把内部生命周期变成新的模型业务 Stage/准备工具。

快速只读的额外 RTT/存储开销必须按场景测量，不强制全部操作增加第二轮 RPC。

## 迁移与当前状态

wire/schema 使用版本或能力声明，不暗改 MCP 协议版本，也不增加第二套安装入口。
旧耐久账本保持原字节/SHA，通过明确 request 查询。缺少代际的旧监控标签明确拒绝
或标为 legacy unresolved，不能按当前同名记录恢复原执行身份。

`a5e41be` 的 id_kind 修复解决已知耐久 request/monitor 交叉案例，但不是最终契约；
R01 的历史执行引用生命周期继续保持未关闭。A01 owned-process native worker
隔离仍是独立的未实现结构项。

`839e81b` 已替换上述中间契约：device wire v2 + service-issued operation_ref +
内部 Prepare/Execute；关联标签不再是身份。源码回归、Windows executable 与真实
Codex Host 的有限 native read 验证见
`docs/validation/2026-09-12-execution-reference-candidate.md`，不据此自行关闭 R01
或宣告完整架构完成。

后续修复与未过滤回归、Windows executable、最新 Host 部署证据见
`docs/validation/2026-09-12-execution-reference-review-fixes.md`。保留原独立报告与
首轮失败记录，后续成功不覆盖历史基线。

## R01 整体关闭的最低测试范围

- 旧只读 ref → 同 instance 新 context；旧 ref → 新 instance。
- 旧标签被新 request_id、新只读 monitor、新 durable monitor 分别复用。
- 同类/跨类、跨设备、跨认证主体冲突；旧 ref 取消不得命中新执行。
- Host Stop 在接受前、接受后派发前、派发中三个窗口的竞态。
- 耐久 request 查询跨重启保持同一记录，派发计数不增加。
- 所有过期/不确定路径不得通过换 ID、换任务或自动重试输入恢复。

实施顺序：共享类型与版本声明 → agent 权威接受/执行记录 → adapter 内部接受/取消
路径及公开 selectors → 兼容旧耐久记录 → 上述 U/P 与真实 Host/Windows 验证。
