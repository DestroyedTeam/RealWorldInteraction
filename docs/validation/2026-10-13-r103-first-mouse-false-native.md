# R103／R104：同一二进制的首次后台点击 YES／NO 实机对照

## 结果与限度

只操作独立分配在 BetterDisplay 虚拟屏的 Receiver，不操作用户游戏、Chrome、飞书或已有文档，也不要求人工配合。两次均用已加载 RWI MCP 的默认 click，value 字段完全省略；每个新窗口只发一对按下／松开，不激活、不预热、不补点、不更换发送路径。

两次使用同一个 R102 实际编译并冻结的 Receiver，SHA-256：9122f2611d80116e6d42e87c75b2d96c17f45b479d0b444c4a260793ccaae5c3。

| 独立冷启动 | 实际 selector 调用 | Application／Window 回调 | Safe 按钮 | 应用 active／窗口 key |
|---|---|---|---|---|
| R103，first-mouse=false | 0→1，返回 false | 2／2，down／up 各一次 | 0→0，没有执行 | false／false 保持 |
| R104，first-mouse=true | 0→1，返回 true | 2／2，down／up 各一次 | 0→1，已执行 | false／false 保持 |

两次完整前台 tuple 均保持为 PID 71613／cgwindow:46769／process_start 71613:1791835842:356984／native_epoch db95e441844f25b1a9b6b5eb603119b7be4368f29ea56be1f161c6a93d813446。原生环境定时采样、无丢记录、无 borrow skip、无无效事件、无输出错误；这不是连续物理用户输入／光标来源证明。

这个对照补齐了测试靶场的负向接收策略，证明对这个自有 AppKit 按钮不能把“送达”当成“动作成功”。它不证明 Chrome 的失败已定位，不证明 NSEvent 构造器优于默认路径，也不保证所有 Mac 应用都接受真后台事件。

## 按实际执行顺序的截图

1. R103 冷后台 NO，发送前 Safe 0。
![R103 NO 前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r103-cold-first-mouse-false-2026-10-13/screens/01-before.png)
2. 同一窗口实际收到 down／up，selector 返回 false，发送后仍 Safe 0。前后图片相同（SHA 0ddf34cfcb120c9d3b978b3afd246d42eb1047af25d576091684a5831189dcda），没有伪造变化。
![R103 NO 后](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r103-cold-first-mouse-false-2026-10-13/screens/02-after.png)
3. 关闭并清理 R103 后，新建同二进制 R104 冷后台 YES，发送前 Safe 0。
![R104 YES 前](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r104-cold-first-mouse-true-2026-10-13/screens/01-before.png)
4. 实际 selector 返回 true，发送后 Safe 1；未切走用户前台。
![R104 YES 后](/Volumes/ChatGPT/Real World Interaction/outputs/validation/r104-cold-first-mouse-true-2026-10-13/screens/02-after.png)

PNG 均来自对应实际 MCP 返回的 image block，写盘后重新打开逐字节相同；具体字节数、SHA、原始 ledger index 见各 ACTUAL_SCREENSHOT_MANIFEST 文件。Native screenshots 的两次 elapsed_ms 为 R103 357／184、R104 563／154；只是一组单次观察，不是系统／模型性能优越性的统计证明。

## 已执行清理与无重放

- R103 PID 29950，出生标记 29950:1791853201:435292，窗口 cgwindow:47203；R104 PID 35121，出生标记 35121:1791853598:752583，窗口 cgwindow:47222。
- 每次原始 click 和 own-close 的两条 durable request 均 query 为 terminal／resources_released，pending／uncertainty 为 false，replay_allowed=false。
- 仅对新鲜观察到的自有 Close window 控件发 AXPress，再 detach 独立 target。两个 child 与两个 supervisor 均实际 exit 0；自有 PID 已不存在。没有把 TERM 或 PID 不见冒充成功退出。
- R103 原始15条、R104 原始14条工具调用逐条写盘；未重复已完成输入或旧脚本。

## 已执行源码行为及回滚

精确改动是 Receiver 的 --first-mouse=true|false 与 first_mouse 字段／acceptsFirstMouse: 分支；生产 Agent/MCP/代码签名／权限未改变。R102 默认仍 YES；独立 Cell 保存配置和 selector 调用，未调用时 last_returned_accept 为 null。

同一个 --first-mouse=false --check-only 输入：
- BASELINE：编译 exit 0，旧代码拒绝未知 flag，CLI exit 1。
- MODIFIED：编译 exit 0，识别 false；gui_created/input_sent/native binding 均 false，CLI exit 0。
- ROLLBACK：同一 ROLLBACK.sh 在独立副本恢复三个预 R102 fixture 文件、移除新 helper，恢复哈希／ABSENT；编译 exit 0，同输入再拒绝，CLI exit 1。
- REAPPLY：重新应用四处源码改动，编译 exit 0、CLI exit 0。
6 项纯策略测试及 ROOT 九项离线 gate 的实际执行均 exit 0；本轮只消费既有记录，没有重跑 Cargo 或原生输入。

原全局251基线及新3个ABSENT已在 R102 同角色254证明保留；这两份报告扩展同一角色集合，不建立另一开发源、不重基。完整字面 stdout/stderr、命令、退出码、源码／程序／图片哈希以及补录的同输入证明保存在以下角色。

- [MODIFIED_FILE](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE)
- [DIFF_FILE.patch](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch)
- [VERIFICATION.txt](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt)
- [ROLLBACK.sh](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh)

## 原始事实

- [R103 实机结果](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-false-native-r103/RESULT_NATIVE_R103.json)
- [R103 原始工具记录](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-false-native-r103/ACTUAL_NATIVE_TOOL_LEDGER_R103.json)
- [R103 实际图片字节索引](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-false-native-r103/ACTUAL_SCREENSHOT_MANIFEST_R103.json)
- [R103 独立校验命令](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-false-native-r103/INDEPENDENT_NATIVE_CHECK_R103.command_execution.json)
- [R104 实机结果](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-true-native-r104/RESULT_NATIVE_R104.json)
- [R104 原始工具记录](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-true-native-r104/ACTUAL_NATIVE_TOOL_LEDGER_R104.json)
- [R104 实际图片字节索引](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-true-native-r104/ACTUAL_SCREENSHOT_MANIFEST_R104.json)
- [R104 独立校验命令](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-true-native-r104/INDEPENDENT_NATIVE_CHECK_R104.command_execution.json)
- [R102 实际四相回滚](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/cumulative-four-roles-r102/RESULT.json)

完整 F01–F16、Chrome 后台接收边界、Live 44pt 按钮实际绘制／命中、新 MCP 低冗余输出及最终合理 commit／PR 推送仍未全部完成。Live 已部署的280×300pt上限保留；44pt命中区域方案还不是已部署/已原生验收的功能。
