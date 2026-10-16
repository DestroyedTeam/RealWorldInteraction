# R102：显式首次后台点击接收策略（测试靶场，不是Chrome修复）

## 已执行结果

R100新factory与R101默认路径在明确acceptsFirstMouse=YES校准客户端均冷后台0→1，且应用/窗口inactive和前台tuple保持。为了避免固定YES策略形成乐观验收偏置，本轮只改专用Rust Receiver测试程序，不改生产Agent、MCP、输入策略或权限。

新增显式 --first-mouse=false，缺省及显式true仍为原YES行为。first_mouse selector从独立thread-local Cell读取固定策略并饱和记录调用数；不借UI_STATE RefCell、不发送事件、不激活应用。JSON分别记录configured_accept、selector_calls、last_returned_accept（未调用为null）、counter_saturated。

真实ROOT匹配CLI输入：旧代码compile0／未知flag拒绝1；新代码compile0／识别false且check-only gui_created/input_sent/native_binding均false／exit0。状态目录保持ABSENT。6项纯策略测试通过；fmt、workspace test、browser example test/build、Receiver example test、Clippy全features全targets -Dwarnings、架构检查/单测、git diff --check共9真实命令均0。源码账本373项已逐哈希冻结。

## 未完成、不应混淆的项目

本轮尚未运行firstMouseFALSE真实窗口；配置false不是selector被查询的证据。下一次实机必须看selector_calls增量/last_returned、Application/Window回调及按钮动作，并分别核对前台/窗口key状态。零回调不可直接判WindowServer拒收，NO校准结果也不可当Chrome源码或安装binary同一因果。完整F01–F16验收、Chrome后台、Live44pt命中区及最终PR推送仍未完成。生产签名Agent/配对证书、四项权限未变，无Codex重启要求。

## 四条精确源码路径

- /Volumes/ChatGPT/Real World Interaction/crates/rwi-macos/examples/receiver_fixture.rs
- /Volumes/ChatGPT/Real World Interaction/crates/rwi-macos/examples/receiver_fixture/model.rs
- /Volumes/ChatGPT/Real World Interaction/crates/rwi-macos/examples/receiver_fixture/appkit.rs
- /Volumes/ChatGPT/Real World Interaction/crates/rwi-macos/examples/receiver_fixture/first_mouse.rs

## 证据

- [candidate-transaction-r102/RESULT.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/candidate-transaction-r102/RESULT.json)
- [candidate-transaction-r102/BASELINE_CLI.command_execution.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/candidate-transaction-r102/BASELINE_CLI.command_execution.json)
- [candidate-transaction-r102/MODIFIED_CLI.command_execution.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/candidate-transaction-r102/MODIFIED_CLI.command_execution.json)
- [candidate-transaction-r102/MODIFIED_SOURCE_FROZEN_R102.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/candidate-transaction-r102/MODIFIED_SOURCE_FROZEN_R102.json)
- [proposal-first-mouse-policy-r102.patch](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/proposal-first-mouse-policy-r102.patch)
- [proof-preparation-r102/ROLLBACK_GENERATED_PARSE_R102.command_execution.json](/Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/implementation-2026-10-10/comprehensive-native-public-readiness/fixture-first-mouse-policy-r102/proof-preparation-r102/ROLLBACK_GENERATED_PARSE_R102.command_execution.json)

同四角色254项补录及scoped/default回滚将由独立副本实际执行；原251（包含三个fixture源码全局ABSENT）不得重基。这里不把尚待完成的rollback或真实NO窗口验收写成已通过。


## 后续实际补齐（R103／R104，2026-10-13）
R102 同四角色254的实际默认配置及独立fixture CLI四相均1／0／1／0、各相编译0；全局原251及三个新ABSENT真实恢复后重应用。真实NO已验：selector0→1并返回false、Application2／Window2、Safe0→0。用同一个instrumented9122…二进制独立冷YES：selector0→1／true、Safe0→1，均未激活／改变原前台tuple。两个窗口已关闭、child／supervisor实际0，未重放输入。此处更新的是此前明确pending项，旧段落保留为当时状态，不以自有策略结果宣称Chrome修复。
详情：[/Volumes/ChatGPT/Real World Interaction/docs/validation/2026-10-13-r103-first-mouse-false-native.md](/Volumes/ChatGPT/Real World Interaction/docs/validation/2026-10-13-r103-first-mouse-false-native.md)
