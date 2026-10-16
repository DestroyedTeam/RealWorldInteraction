# R93 窗口坐标内存校准（2026-10-13）

当前：RWI / 无输入构造校准完成 / Chrome 后台消费仍未通过。

- 原版没有新 example，实际运行退出101；新增后五项专测、fmt与example Clippy通过，内存运行退出0。
- `CGEventCreateCopy` 产生六个独立副本，两原件与六副本同时存活，八个指针互异；每变体前后及全部阶段结束，原 Quartz 坐标／受保护字段均保持。
- 输入point[3186,804]、frame[3036,504,660,552]；比较不调用setter、local[150,300]、screen[3186,804]。两阶段的公开 NSEvent 读回均为[3186,365]，timestamp均0，未更改field58。
- **window91234567是合成身份，不存在真实窗口**。因此此结果只证明内存独立与实际读回，不能证明screen/local生产规则、跨进程消费或解释Chrome失败。R92真实AppKit的window[150,252]/canvas[126,54]证据保持，不修改正确生产坐标。
- 无post、GUI、截图、光标移动、激活或权限请求；不重放R92输入。初始retain-clone提案缺陷在执行前发现并修正，旧提案／审计记录保留。

完整工程门槛与四角色复制／回滚结论以同一验证账本为准，原始运行不被副本验证替代。下一实际客户端实验将补document级mouse接收观察，再试单变量投递；MCP输出去重与短ID保持独立计划，不偷换整个前后台范围。

## 原始证据

- [实际内存结果](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/actual-run-r93/RESULT.json>)
- [原版运行](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/actual-run-r93/BASELINE_MISSING_EXAMPLE.command_execution.json>)
- [修改版运行](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/actual-run-r93/MODIFIED_MEMORY_PROBE.command_execution.json>)
- [专测](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/actual-run-r93/EXAMPLE_TESTS.command_execution.json>)
- [原始审计缺陷](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/AUDIT_BLOCKED_R93.json>)
- [修正提案静态审计](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/AUDIT_CORRECTED_STATIC_CLEAR_R93.json>)
- [工作区门槛](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/window-location-memory-calibration-r93/OFFLINE_FINAL_GATES_R93.json>)
- [同一验证账本](</Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt>)
