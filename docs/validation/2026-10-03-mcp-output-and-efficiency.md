# RWI MCP 调用效率与输出减负（源码候选）

日期：2026-10-03。回看当前项目相关 Codex 会话历史，并对照已完成的 macOS/Computer Use 原始调用审计；本轮只改 MCP 指引与输出整形，不改原生输入、重试、焦点、授权或窗口准入策略。

## 历史证据与边界

- [2026-10-02 真实会话审计](2026-10-02-macos-computer-use-comparison.md)覆盖授权读取的飞书/Chrome 与游戏历史日志。飞书/Chrome 中 RWI 为 75 次顶层调用、12 次 `failed`；Computer Use 为 87 次、17 次 `failed`。嵌套 JS 的耗时中位/P95 分别为 RWI 1075/5228 ms、CUA 494/1825 ms；返回文本字节中位数为 3834/1358。任务、目标窗、步骤和验证方式不相同，`failed` 也可能表示“输入已返回、后续读取失败”，因此这些数不是成功率或公平 A/B 性能结论。
- 对最近的 [RWI Mac 任务](codex://threads/01a0b8fc-631c-7cc0-bbeb-c8681babfabf)与 Paint S 接续会话 [RWI 重启验收](codex://threads/01a0fd6b-7892-7f12-9485-41760122a1bc)读取最近 10 轮 `read_thread` 的 MCP 调用元数据：前者 5 次 `computer.snapshot` 中位 1165 ms、P95 1262 ms（均完成），另有 1 次 click 为 2114 ms；后者 34 次 snapshot 中位 600 ms、P95 826 ms，单次 `computer.targets` 为 5005 ms。该 Paint 样本的 `computer.controls` 3/9、click 1/4、press_key 1/3 返回 `failed` 状态；它们包括精确焦点/过期引用/当前 UI 状态等不同路径，不等于这些次数都发生了已派发后无效果。
- 该 `computer.targets` 样本与会话报告中的 `win11-vm` `device_connect_timeout` 同时出现；gateway 对无筛选的 targets 会并发检查并等待所有配置 endpoint。虽未有同一 request 的阶段计时，不能把5秒唯一归因于 VM，但精确已知 `device_id` 路由可避免探测无关 endpoint。MCP server instructions 现在明确要求“已知时传 `device_id`，仅在确需枚举设备时省略”，不会猜测或静默换设备。
- 该 Paint 会话明确记录：字体点击被派发前安全门拒绝；保存键因窗口未聚焦被派发前拒绝，磁盘文件保持 219 对象原样。这是正确的 fail-closed 行为，不应靠降低门槛换“成功率”。另一条历史飞书/Chrome 调用曾出现 act 已返回、后续 observe 失败导致整个 JS cell 被标 failed；应将工具状态、原生派发结果、目标读回分别记录。旧审计已指出这也是优先使用 typed MCP、减少临时 JS 和保留 dispatch/verification 分层的原因。

上述读取是 Codex MCP 调用元数据的观测时间，不是原生 CPU 时间；当前样本包含不同应用、不同交互状态，不能据此宣称 RWI/CUA 胜负或某个单项真实失败率。

## 本轮源码变更

1. `computer.snapshot/click/controls/scroll/...` 这组 typed convenience tools 的成功默认回复不再重复附带 `stage_timings` 与 `elapsed_ms`。可选 `include_timings=true` 按需请求；错误、拒绝、围栏或不确定的变更回执仍默认带完整时间诊断。计时仍随完整数据放入 `details_ref`，可用 `computer.result` 读取。为了避免把长说明复制到每个工具 schema，计时语义只在 MCP server instructions 中说明一次：`stage_timings` 是 adapter 对内部 RPC 的墙钟时间；`elapsed_ms` 到最终 progress 通知为止、在响应缓存/序列化之前计时。二者均不冒充 native CPU 时间。
2. 控件 `name` 与 `description` **仅在完全相同**时去掉重复描述；没有名称时仍把描述提升为可见名称，名称不同则保留两者。`X`/`Close` 等名称、原生 `AX*` action、值、焦点/禁用/选中/可见性状态未裁剪。
3. 仅将 adapter-local `result_ref`/`details_ref` UUID 从 36 字符改成同一 128-bit UUID 的 32 位无分隔十六进制形式。native target、window、frame、element、operation ID 不缩短；截图 view 的坐标 mapping、window title、controls 覆盖/完整性、`mutation_ready`、fence 和派发状态均不变。
4. MCP server instructions 将“已知 device_id 时定向查询 `computer.targets`、否则才枚举所有 endpoint”写成一句明确路由规则，避免仅因查状态而等待无关离线设备；底层targets与超时/并发逻辑不变。

这些改动减少默认成功响应中的诊断重复和少量重复标签，不改 action 语义；**当前不能据此宣称工具成功率已经提升**。历史显示需要修复的是具体动态观察、AX 覆盖/目标重验证、多窗绑定和失败后的真实读回路径；这些保留在完整计划中，不能由 token 压缩替代。

## 离线验证与未完成项

- `cargo fmt --all -- --check`：通过。
- `cargo test -p rwi-mcp --locked --offline`：96 passed / 1 ignored。
- `cargo test --workspace --locked --offline`：580 passed / 10 ignored（41 suites）。
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`：通过。
- `integrations/javascript/rwi-client.test.mjs`：通过（0 failures）。
- 新测试覆盖默认成功隐去计时、显式计时、错误/不确定诊断保留、详情仍可从 `details_ref` 读取、重复描述裁剪以及短引用可解析。

本轮未替换已安装 MCP、未部署、未重启 Codex、未使用 GUI 或输入。下一步应在安装/Host 刷新后，用相同 read-only fixture 记录每次 MCP JSON 字节数、工具调用耗时、RPC envelope 状态、dispatch receipt 和独立目标读回；再针对历史失败路径做受控 A/B。未经该实测，不报告线上 token 节省比例或成功率提升。
