# R92 后台点击实机记录（2026-10-13）

**结论：专用 AppKit canvas 接收通过，Chrome 后台目标仍失败，不宣称通用支持。**

默认路线不变；显式 `Intent::Click` JSON 选择单 down/up。保留窗口、权限、lease、no-replay；无激活、defocus、primer、Command、时间戳覆盖或 fallback。

先验出 attached target 缺 `pointer_click`，修复共享能力声明后重新签名部署；九项 gate 全零，workspace **859/0/11 ignored，43 suites**。

Receiver2 **PID98840/CG46844** inactive、key=false、前台50648：原生 down1/up1/click_without_drag1，window点[150,252]、canvas点[126,54]。

明确置前台后 `RWI_R92_AB→RWI_R92_A`（全局 Backspace）；新私有 B 请求 `r92-private-background-counter-b-2` 返回，未纠正 refocus，再 Backspace 得 `RWI_R92_`，changes3、前台98840/key=true。B 的 AX counter0；DOM352/drop0/click0。旧 A 的键盘检查因 Receiver 寿命到期不完整，不判“偷焦点”。

MCP 原子部署零退出，三次新进程 initialize/tools42/只读 targets 正常；Agent与配置未变。当前 Host 旧 schema 未刷新，仅证明 generic 调用与新进程 schema，不称 typed GUI 已验。

清理后两专用窗口各0、19请求 terminal/released/replay=false；Receiver2、Chrome退出0。Oracle SIGTERM/-15（外层241），监听已消失，不称优雅退出0；用户三进程仍存活。

最终事务证明以 [VERIFICATION.txt](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt>) 为准；本报告记录写入时验收，不用离线策略升级 GUI 结论。

## 按实际阶段的本机截图

04 未保存；只列真实五张，未复制或纳入提交。

1. 默认路线前，counter0。
![01 默认前](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/01-r92-default-before.png>)

2. 默认后台点击后，仍0。
![02 默认未消费](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/02-r92-default-background-unchanged.png>)

3. 首 Receiver；后续测量寿命到期。
![03 首Receiver](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/03-r92-front-receiver-before.png>)

4. Receiver2 canvas 点击前；接收由回执证明。
![05 Receiver2准备](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/05-r92-receiver2-before-private-canvas.png>)

5. 私有 B 点击后，counter0。
![06 B未消费](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/06-r92-private-b-counter-no-effect.png>)

[原生调用][calls] · [canvas回执][canvas] · [B独立DOM][dom] · [完整gates][gates] · [MCP实际部署][mcp] · [清理][cleanup] · [监听消失][listener]

[calls]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/LOADED_NATIVE_CALLS_R92.json>
[canvas]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/AFTER_PRIVATE_RECEIVER2_CANVAS_R92.json>
[dom]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/AFTER_PRIVATE_B_CUSTODY_R92.json>
[gates]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/OFFLINE_FINAL_GATES_R92.json>
[mcp]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/mcp-deployment/installed-mcp-r92/DEPLOYED_MCP_ONLY_R92.json>
[cleanup]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/OWNED_PROCESS_CLEANUP_R92.command_execution.json>
[listener]: </Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/background-pointer-route-r92/ORACLE_LISTENER_GONE_R92.command_execution.json>
