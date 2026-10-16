# R96 · 私有前台与暖态后台对照（2026-10-13）

只在虚拟屏操作独立 Chrome85754／CG47016、专用 oracle 与 Receiver91524；不改生产源码、Agent/MCP或默认投递。沿用 R95 已验371源、九门禁和 source contract1/0/1/0、check-only0/0/0/0，不重复 Cargo／旧请求。

## 实际结果与限制

- 原图固定 pixel **[211,594]**、桌面点 **[1929.5,321]**。前台 default 新pair：counter **0→1**，document down/up/click各+1；之后的新 private pair：counter **1→2**，各再+1，独立AX同样读到1／2。两请求终态、释放、禁止重放；receipt的 `goal_verified=false` 保持，效果来自独立DOM/AX，不来自returned。
- foreground PID／birth／window／epoch三次回读一致，无额外hover／primer／refocus。但 default 已使 `activeElementId` **空→counter**：private阳性是**默认点击后的暖态前台**，不是cold-private-first，也不是排除全部内部状态差异的唯一因子实验。
- 同一Chrome、点、private路由转后台后，counter **2→2**、document三项新增均 **0**；`activeElementId=counter`仍在、`documentFocused=false`。暖态控制焦点本身不足以让本次后台消费成立；不能据此证明通用根因或宣称后台修复。
- 前台Receiver键盘托管：`r96WXYZ→r96WXY→r96WX`，private前后无补focus；NSTextView／key／active／foreground保留。50ms采样仅记录两次setup变化、无drop，不是连续物理人工证明；比较阶段oracle ring drop0。
- 首focus其实 `already_foreground`／native0，旧operator误抛 `focus not observed`；保留原错误，查询原终态＋新鲜exact foreground后继续，未重发focus。fresh原desktop五字段守卫补齐仅改work operator，不改产品。
- 完整81 raw调用、12原请求均 terminal／released／无pending或uncertainty／no-replay。精确close/detach后 Chrome／Receiver退出0，oracle保留SIGTERM **-15**；两supervisor session终态0，owned父PID消失，用户896／907／71613仍活跃。四权限、12protected hashes、Agent88136/d9e与371源未变，无Host重启或人工配合。

## 按时序的原生截图

1. [01 · 前台初始counter0](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r96-private-foreground-and-warm-background-2026-10-13/chronological/01-ready-front-counter0-r96.png)。
2. [02 · default前台counter1](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r96-private-foreground-and-warm-background-2026-10-13/chronological/02-default-front-counter1-r96.png)。
3. [03 · 后续private前台counter2](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r96-private-foreground-and-warm-background-2026-10-13/chronological/03-private-front-counter2-r96.png)。
4. [04 · private暖态后台仍counter2](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r96-private-foreground-and-warm-background-2026-10-13/chronological/04-private-warm-background-counter2-r96.png)。
5. [05 · private之后Receiver键盘r96WX](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r96-private-foreground-and-warm-background-2026-10-13/chronological/05-receiver-keycustody-r96WX.png)。

五图原bytes已复制、hash/reopen；只作本地证据，不stage、不纳source map。

## 证据与四角色

[原生结果](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/NATIVE_OUTCOME_FINAL_R96.json)、[清理](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/NATIVE_CLEANUP_FINAL_R96.json)、[81 raw调用](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/RAW_NATIVE_CALL_LEDGER_R96.json)、[12原请求查询](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/ALL_ORIGINAL_OPERATION_QUERIES_R96.json)保留原始实录。

同四角色证据扩展的真实终态仅由 [独立RESULT](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/cumulative-four-role-evidence-r96/RESULT.json)、[final reopen](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/chrome-private-foreground-route-r96/FINAL_ROLE_REOPEN_R96.json) 与固定 [VERIFICATION.txt](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt) 陈述，本文不预称通过；244原基线保持，两新报告原ABSENT。**未证明Chrome后台支持、cold private首次消费或完整F01–F16封版。**
