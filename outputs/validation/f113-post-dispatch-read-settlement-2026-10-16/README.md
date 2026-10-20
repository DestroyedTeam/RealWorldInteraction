# F113：显式切窗后的保留对象单次补读

## 精确变更

`crates/rwi-macos/src/native/activation.rs` 的 `focus_exact_window`：只有 AXRaise/AXFrontmost 请求发出后的两处前台观察调用 `post_dispatch_foreground_state`；所有前置观察和发出下一次请求前的检查不变。AXRaise、AXFrontmost各最多一次，无鼠标/键盘/应用启动替代路线。

`activation/readback.rs` 仍验证两次公开前台应用对象、PID、出生时间、AX应用对象、AX窗口对象、CG窗口编号、几何及会话；补读仍作用于同一保留对象的同一属性，不重选窗口。属性返回成功后仍需通过全部原有检查。

`activation/readback/settlement.rs` 仅接受读取 `AXFocusedWindow`、`AXPosition`、`AXSize` 时完整诊断匹配 -25204 的错误。整次显式激活共享一个补读名额；申请20ms间隔，额外100ms软预算，前后检查原操作取消/期限、会话/进程及用户活动。成功快路径不启动补读计时、等待或额外属性读取；迟到成功和上下文变化仍不接受。原始错误与最终错误都保留。回复增加实际 `native_request_counts`，仅进入补读时附 `readback_settlement`，不提升 `goal_verified` 或 `input_authority_granted`。

## 依据与边界

Apple 对 [AXUIElementCopyAttributeValue](https://developer.apple.com/documentation/applicationservices/1462085-axuielementcopyattributevalue?changes=_5) 的说明把 cannotComplete 归于消息未能完成；**它不能证明 F105 的错误一定是瞬态，更不能证明本补读必然解决它**。这是根据现有失败证据做的有界只读处理。

[AXUIElementSetMessagingTimeout](https://developer.apple.com/documentation/applicationservices/1459345-axuielementsetmessagingtimeout?changes=l_4) 的对象级超时仅属于那个具体对象；本变更保留现有每对象50ms设置，不改系统级/进程默认超时。100ms是补读阶段的软接纳预算，不是可强抢占系统RPC的硬期限。

## 实际验证与回滚

- 同一个 Native 单测过滤命令：BASELINE 0项、MODIFIED 12项、独立副本 ROLLBACK 0项、重应用12项，退出均0。新增用例覆盖成功快路径、允许属性、其他错误拒绝、共享名额、取消/上下文变化、暂停前后耗尽预算、二次错误、迟到成功及原始诊断保存。
- 冻结策略输入：F105 真实原始 AXFocusedWindow -25204 错误，下一次值42是**明确假设的模拟读值**。BASELINE直接返回原错误/1读/0ms → MODIFIED有值/2读/模拟20ms → ROLLBACK原错误/1读 → 重应用有值/2读，退出均0。候选使用真实新策略模块；旧分支模拟旧的错误直接传播。这个对照不是原生应用验证。
- 独立副本恢复全部483项原始源码哈希及新模块缺失；严格补丁重建484项候选；原全局81项存在、305项缺失的基线也已通过恢复/重建。工作根与永久修改件保持候选状态，回滚脚本拒绝直接作用于这两个源位置。
- 全量1375通过/11忽略；fmt、workspace test、Clippy全目标全特性-D warnings、架构检查全部退出0。原编译101（诊断Box类型）、记录脚本1（JSON行解析）已保存并修正，不掩盖成原先成功。
- 新Agent已实际稳定签名部署，四项权限保持，Host/MCP/CA/配对及密钥未变。
- [F116实际加载工具验收](../f116-foreground-read-native-acceptance-2026-10-16/README.md)：原生/Chrome首次显式切窗成功并独立回读相符，但**没有遇到-25204、没有进入补读分支**。因此实机错误恢复仍待独立原生延迟/失败用例，F95/F99/F105失败仍有效。

## 同一四角色（已重新打开核对）

- /Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/MODIFIED_FILE
- /Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/DIFF_FILE.patch
- /Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/VERIFICATION.txt
- /Volumes/ChatGPT/Real World Interaction/work/transactions/2026-10-06-mcp-output-compile-fix/ROLLBACK.sh
