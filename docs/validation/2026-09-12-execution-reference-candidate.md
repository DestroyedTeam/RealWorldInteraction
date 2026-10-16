# ADR 0003 执行引用候选：源代码与 Windows/Host 验证

固定产品提交：`839e81b4a2a124b94cbc2026fddedb51ac5a5ca3`。
状态：**实现候选与下述有限范围验证通过；独立复核待结论，完整架构未完成。**

## 契约变化

- Device RPC v2；MCP 协议版本、现有安装入口和配对配置不变。
- Agent 签发执行引用；设备、agent instance、context generation、服务 nonce
  由记录校验，真实 principal 来自 mTLS，不信任调用者声明。
- 模型仍只调用普通工具；adapter 内部 Prepare → Execute。Prepare 不调用 native
  provider，不截图、不输入、不获取输入资源；Execute/Cancel 竞争同一单次状态。
- `computer.operation` 为互斥 execution/request selector；取消仅接受完整引用。
  旧 monitor/operation 标签不再查询、取消或路由，关联标签不参与防重放指纹。
- 耐久 request 命名空间不含 agent/context epoch。状态根的设备命名空间固定；
  旧账本保持原字节。记录中的旧 monitor 或原执行引用只是证据，不复活执行。
- 未结束 native 所有权不因 TTL/配额回收。单次 accepted TTL 为 30 秒，终结
  历史按有界容量和 300 秒年龄回收；过期不证明未发生副作用或 native 停止。
- Cached artifact/resolve 读走单轮 RPC，不创建 native 执行记录。

## 回归证据

| 范围 | 结果 |
|---|---|
| 默认 workspace | 215 passed，1 ignored |
| all-features workspace | 302 passed，2 ignored |
| Windows ARM64 实际 unit/integration 可执行文件 | 20 passed，0 failed，3 个 suite |
| fmt、Host all-targets/all-features Clippy、Windows agent Clippy | 通过 |
| architecture ratchet | 通过；不是完整架构验收 |

`work/execution-ref-v2-20260912/default-tests.raw.log`、
`all-features-tests.raw.log`、`clippy.raw.log` 是 `rtk proxy cargo` 未过滤输出。
`raw-gates-commit.txt` / `raw-gates-end-commit.txt` 绑定相同固定 SHA。
all-features 显式提供历史 fixture worker；它不是部署产品。
Windows `windows-test-run.stdout` 保存三个 executable 的逐测试结果与退出码。

覆盖旧 read ref → 新 context/agent、关联标签跨读/耐久请求复用、跨主体/设备与
篡改字段、取消旧 ref、配额下 pending native 保留、终结历史回收、原请求跨重启
查询与 fixture 派发计数不增加。真实 MCP stdio + mTLS 测试覆盖接受响应前 EOF、
派发中 Cancel/EOF；确定性组件测试覆盖已经取得 ref 后 ready Stop 不轮询 Execute。
组件测试不冒充外部 Host 调度窗口测试。

## 实际 Codex Host → Windows 原生验证

使用现有项目配置和 `codex_host_probe`，真实 Codex `0.153.4` app-server，三个
临时测试任务，无模型推理；其他 MCP/plugin 仅在测试任务中禁用，保存配置未改动。

1. 新 Host 发现 **20 个工具**，状态 connected，Skill 被发现，device wire = 2。
2. 在已分配的 Task Manager 上完成 native discover、bind、observe；原图
   **1538 × 1169 PNG** 已实际查看，返回 terminal/resources_released，未发送新 GUI 输入。
3. 第二个新 Host **不先打开 context** 也能查询第一个 Host 的有效完整执行引用。
4. 该次 app-server 退出没有立即关闭 agent context，旧 ref 仍有效；这不证明
   context 已结束，不记为“Host 退出导致 ref 过期”的成功案例。
5. 对精确 PID/birth/path/hash 的自有 agent 做空闲重启后，第三个 Host 的旧 ref
   查询和取消都返回 `execution_ref_expired`；不匹配新实例、不退回旧 monitor。
6. v1 原请求在升级前后和重启后仍按 request selector 读取同一耐久结果；4 个
   原账本文件 SHA 全部不变。裸 monitor 查询返回 `invalid_arguments`。
7. Cached artifact 请求成功且声明 `native_execution: false`，无执行引用。

两次早期 operator 调用分别因错误 bind 字段、catalog 过期被拒绝；其后的 observe
也因尚未绑定而拒绝。原始失败证据保留，不计为成功，也没有降级或发送输入。

当前部署 agent PID `10908`、instance `2f392e85-e399-4538-a71d-8477092e08c6`，
MainWindowHandle = 0，无黑色常驻控制台。程序路径仍为用户 LocalAppData 的
`RealComputerUse/agent-v1/rcu-agent.exe`，目录名不是 wire 版本。
本次没有修改防火墙：原精确规则仍只放行该 exe 的
Mac `10.211.55.2` → Windows `10.211.55.3:45128/TCP`，其他程序/规则未改动。

二进制与原始证据 SHA、回包来源见同名 JSON；截图与私有部署/配对文件不进入 Git。

## 尚未完成

- R01 全部关闭需 Control Center 独立结论；A01 owned-process native worker
  隔离仍未实现，当前仍是进程内 blocking worker。
- 本次 live 是 **H + 有限 native read**，不是 M（模型自主调用），不是新 native
  输入/卡死 native 强取消验收，也不是全局 Plugin/projectless 安装验收。
- 输出压缩、严格延迟基准未完成。证据写入时间粗测包含整个 Host/RPC/native/序列化：
  observe 单样本约 369 ms、cached artifact 约 16 ms；不能据此推导额外 RTT 精确
  成本或性能优越。完整 observe 回包约 230 KB，仍有明显压缩空间。
- 完整 ADR 0002 始终是强制架构，不因本候选通过而改称首期/试用架构。
