# R95 · Chrome 原生 document 接收对照（2026-10-13）

本轮只增强被动浏览器验收 fixture：document capture 的 down/up/move 与 hover 家族分离，保留 raw 字段、各类型累计计数及有界 ring；不派发 DOM 事件、不改 Agent／MCP 生产投递。

## 实际结果

- 源码 contract **BASELINE1→MODIFIED0**，同 `--check-only` **0→0**（无 listener／GUI／输入）；候选九门禁实际全0。源码标记不是原生接收证明。
- 独立 Chrome57537 留在后台：private pair、move、再一条新 pair 后，document down/up/move 均 **0**、counter **0**。returned 不等于应用效果，也不证明整个 Chrome 进程未收事件。
- Receiver60860 前台键盘托管：`r95ABCD→r95ABC→r95AB`；private 输入前后未补 focus／点字段，NSTextView 接收仍正常。
- 明确切到专用 Chrome 的前台正对照：document down1/up1/move1、click1，AX counter **0→1**。`isTrusted=true` 是浏览器标志，不认证物理人工来源，更不是 private 后台成功。
- 首次 focus 错用 desktop lease，`lease_required`／零 native dispatch；随后 default pair 仍后台 counter0。更正 exact Chrome lease，使用新 focus/input ID 才通过前台对照；原错误不删、不重放。
- 102 项原 raw 调用已保存；16 原请求全部 terminal／released／no-replay。专用 Receiver／Chrome 退出0，oracle 受限 SIGTERM **-15**，窗口关闭／detach、owned PID 消失；用户896／907／71613仍活跃。Node64k保存限制及 FileNotFound exit1保留；分块补存既有原字节，未重做输入／capture。

## 按时序的实际截图

1. [01 · 专用 Chrome ready](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r95-chrome-document-reception-2026-10-13/chronological/01-own-chrome-ready-r95.png)。
2. [01b · private 后 counter0](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r95-chrome-document-reception-2026-10-13/chronological/01b-own-chrome-private-counter0-r95.png)。
3. [02 · private 后、前台对照前的 Receiver 键盘托管](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r95-chrome-document-reception-2026-10-13/chronological/02-own-receiver-keycustody-r95.png)。
4. [03 · Chrome 前台正对照 counter1](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r95-chrome-document-reception-2026-10-13/chronological/03-own-chrome-foreground-counter1-r95.png)。

四张原 PNG 已复制、hash/reopen；只作本地证据，不 stage、不纳累计 source map。

## 证据与边界

原生结果：[NATIVE_OUTCOME_FINAL_R95.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-document-mouse-reception-r95/NATIVE_OUTCOME_FINAL_R95.json)；清理：[NATIVE_CLEANUP_FINAL_R95.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-document-mouse-reception-r95/NATIVE_CLEANUP_FINAL_R95.json)；102 原调用：[RAW_NATIVE_CALL_LEDGER_R95.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-document-mouse-reception-r95/RAW_NATIVE_CALL_LEDGER_R95.json)。512 ring 实际淘汰90，但累计 mouse/click计数保留。

同四角色 source-copy rollback 的真实终态以 [独立 RESULT](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-document-mouse-reception-r95/cumulative-four-roles-r95/RESULT.json)、[最终角色回读](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-document-mouse-reception-r95/FINAL_ROLE_REOPEN_R95.json) 与固定 [VERIFICATION.txt](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt) 为准；本文不预称该 proof 通过。保留242累计原基线，三 fixture 的全局 original 仍 ABSENT。**Chrome 真后台接收缺口未修复；不是广泛浏览器兼容或完整 F01–F16 封版。** 无用户配合、Host／Agent重启或生产部署。
