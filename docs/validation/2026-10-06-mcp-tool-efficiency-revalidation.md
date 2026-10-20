# RWI MCP 成功率与输出效率复核

日期：2026-10-06

## 范围与证据

只检查 RWI 相关 Codex 会话线索和仓库中的既有审计记录，不展开读取无关个人会话，不向其他会话发送消息。主要依据：

- [2026-10-02 macOS Computer Use 对照审计](2026-10-02-macos-computer-use-comparison.md)及其脱敏调用表。审计明确比较了飞书/Chrome、游戏两个不同工作流，不能把失败的 MCP envelope 当成输入失败或目标错误，也不能把不同任务样本包装成 RWI/CUA 公平 A/B。
- [2026-10-03 MCP 输出与效率候选](2026-10-03-mcp-output-and-efficiency.md)。候选此前已覆盖 typed-first、错误与不确定回执分层、计时按需、局部引用缩短和保留完整结果；当前 Host 尚未加载这一候选。
- [10/06 动态地图锚点交接记录](2026-10-06-creaturecurios-map-anchor.md)。游戏会话摘要补充了某次调用 `act=2430ms` 后拒绝、`content_changed=true`、`input=0`。该耗时来自交接，不是此任务独立读取的底层日志。当前本机只读截图不是该全地图界面，不能据此定因或做点击实测。

## 分析结论

1. **工具调用成功率不是删几段 JSON 就能改善。** 历史飞书/Chrome样本包含 RWI JS 临时编排、手动分页、绑定替换和“动作返回后观察失败”等成本。一次 cell 顶层失败可能发生在原生派发之后；必须分别看 RPC envelope、dispatch receipt、目标状态读回。已有 JS `execution_summary` 把这些状态分开，不赋予回滚/重放含义。
2. **减少模型临时代码与跨窗口猜测更可能降低调用错误。** 当前候选把日常路径收敛到 typed `snapshot/controls/click/scroll/type_text`；已知 device_id 时定向查询，不让无关离线 endpoint 拉长状态发现。窗口关系未实现时明确拒绝，不能让技能文案鼓励尝试不存在的 owned-popup 路径。
3. **输出缩减采取白名单，而不是删关键字段。** 默认成功不重复输出内部阶段耗时；`include_timings=true` 和 `details_ref` 仍可读取。只删除完全相等的 name/description 重复项、缩短 adapter-local result/details UUID；保留控件名称/角色、Close/X、原生ID、frame/window、view 坐标 mapping、焦点/可见/遮挡/disabled、coverage、mutation/fence 和 input/receipt 状态。复核时发现 typed `computer.controls` 投影漏掉元素 `point`；已纳入并断言保留原观察 `[x,y]`，但它不是绕过 frame/window 重新校验的权限。拒绝/不确定结果不套成功压缩。
4. **新发现的测试故障阻塞了候选验收。** 修复后才完成全仓门禁：新 `red_marker_cluster` 参数的离线测试初始化补上明确 `false`；纠正动态画布测试夹具的重复 PNG 编码并加入非红色纹理；修正像素锚点代码警告并格式化；`computer.controls` 坐标保留新增测试通过。该路径只允许显式 opt-in 的红点簇，普通 Pixel 仍严格，移动/缺失/遮挡负例拒绝且不派发。它是离线候选，不是游戏实机成功的证据。

## 变更与回归结果

- 该候选的成功输出：默认隐藏 `stage_timings`、`elapsed_ms`，读取原始诊断需明确 `include_timings` 或用 `details_ref`；错误、拒绝及不确定派发保留完整诊断。成功回执仍标记 `goal_verified=false`，避免把派发成功混成目标达成。
- 保护字段：只压缩 adapter-local 引用；目标/窗口/Frame/Element 原生身份不短化。截图 preview 默认 2048边长；超清局部字仍用 `computer.image` 原图 crop。坐标 mapping、原始 PNG 与解析图保持可区分。
- 同命令基线 `rtk cargo test -p rwi-mcp --locked --offline` 在补字段前退出 101，报告 3 个缺少 `red_marker_cluster` 字段的测试结构体；修正后同一命令 `96 passed / 1 ignored`。该编译修复不放宽解析或输入策略。
- 最终验收：`rtk cargo fmt --all -- --check` 退出0；`rtk cargo test --workspace --locked --offline` 为 **585 passed / 10 ignored（41 suites）**；`rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` 退出0；`rtk proxy node --test integrations/javascript/rwi-client.test.mjs` 为 **50 passed**；`rtk git diff --check` 退出0。

## 用户授权后的部署与只读验收（2026-10-06）

- 用户明确允许部署/重启/提交后，从工作树构建 `rtk cargo build --release -p rwi-mcp --locked --offline`，exit 0（release构建用时约68秒）。候选二进制 SHA256 `4bd4eedef223a2fd1137668a2b856f30bac150fc3c7fee849ba861bd44e486ef`，`codesign --verify --strict` 通过；其 adhoc linker signature 与现有 MCP binary 的签名方式相同。
- 通过同卷临时文件原子替换已配置 RWI MCP 路径 `/Users/vincentadamnemessis/Library/Application Support/RealWorldInteraction/client-v1/bin/rwi-mcp`。部署前备份的原二进制 SHA256 `b478b93561a97bc31c100478ace4302ae8652c48ede911c6d93aede9e7cd2f76`；部署后哈希与候选相同。未安装 Agent、未重启 LaunchAgent、未改 MCP profile/Host配置/权限/其他进程。
- 对同一现有私有profile，以旧二进制备份与已安装新二进制分别起独立 stdio MCP 进程；两者均执行 `initialize`、`tools/list`、`computer.targets(device_id=mac-host)`，共38个tool，Agent healthy/available、PID 976。旧/新该次工具结果分别 **32,110 bytes / result_ref 36** 与 **32,106 bytes / result_ref 32**。Metadata本身分别 21,249/21,253 bytes，存在运行状态差异；本次只证明引用长度减少及单次完整结果实测差4 bytes，不宣称通用token节省或速度优势。两次均未截图或输入；进程在响应后由测试脚本显式SIGTERM清理（exit -15、stderr 0，不是MCP请求失败）。
- 安装后通过当前 Codex 已加载的旧 Host 再调用 `computer.targets`，仍见36字符引用、32,278 wrapper bytes；这证明当前活动会话的MCP子进程/schema尚未刷新，不能称作Host端部署验收完成。Codex app工具没有安全的应用重启API；本机有多个会话MCP子进程，未强杀Codex或猜测杀其中一个。需要用户重启Codex，再回到本任务，我才可验证已加载Host是否返回32字符引用。Controls坐标当前只完成合成离线回归；实际窗口输出验收仍需用户明确分配安全窗口。
- 没有提交：此次只部署用户批准的adapter，工作树包含大量与本次无关的未提交/未跟踪改动；整体提交风险高。若后续需提交，应先单独整理可审查提交集。

## 当前不能据此声称的结果

- 没有模型token计数、通用成功率提升、整体耗时下降或同任务实机动作→独立目标读回A/B。10/02历史中不同任务的中位值仍只是定位线索。
- 当前活动 Codex Host 仍可能使用旧加载进程；重启后应检查 typed `computer.controls` 的坐标字段、X/Close标签、原生ID和状态，并在分配窗口上做同态只读/动作读回验收。任何动作不自动重试。

本次已部署RWI MCP二进制并做独立只读stdio验证；未重启Codex、未向业务窗口发送输入、未提交/推送。
