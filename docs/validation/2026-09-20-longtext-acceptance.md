# 长文本与零派发回执验收

## 实现
- Core 仅在已知且未饱和的原生副作用计数前后相同、后端返回失败时，将回执标为 `rejected_before_dispatch`。其他失败仍 `uncertain`，不根据错误名猜测。Observed 转发计数；最终拒绝结果写入 durable journal，重启后同 ID 仍不重放。
- 同一次 Text 调用分批发送时，以 retained AX 对象 CFEqual、原焦点、身份/进程/窗口/位置作为连续性依据。同对象原地调整尺寸允许继续；目标替换、位置/窗口/焦点变化、安全/未知字段、权限或可写性丢失仍停止。Pointer/KeyChord 的几何要求不放宽，不刷新外部引用。
- input_anchor_changed 增加不含输入文字的变化诊断。

## 真实测试时间顺序
1. 原版本长文本只出现首 20 个 UTF-16 单元，第二批因几何派生 ID 改变失败。查询原请求后，原生恢复检查通过，Escape/Undo 清理；没有重发原请求。

![初次失败](../../outputs/validation/2026-09-20-longtext/01-before-fix.png)

2. 第一个尺寸增长候选仍失败。新增原生诊断确认 AX 对象、位置、窗口/焦点等未变；矩形由 [581,1718.5,7.5,16.5] 变为 [581,1718.5,242.25,15.75]。宽度增长伴随空编辑框高度小幅缩小，因此采用原地尺寸调整而非单调增长。失败对象逐次退出/撤销，未保存。

3. 最终部署后，输入“RWI 长文本验收：第一批输入后继续第二批，验证中文与英文 ABC 123 完整保留。”共 43 UTF-16 单元、3 批，715ms returned。AXTextArea.value 与预期全文精确匹配；fence=false。

![完整长文本](../../outputs/validation/2026-09-20-longtext/02-complete.png)

4. RWI Escape 676ms，Undo 632ms；截图显示临时文本层消失。

![恢复画布](../../outputs/validation/2026-09-20-longtext/03-restored.png)

5. 已在前一候选（相同 Core 回执修复）实机验证不支持键名：217ms，`rejected_before_dispatch`、`native_dispatch_entered=false`、`input_domain_fenced=false`。保留错误原因、禁止重放，不需要恢复输入。

## 回归、部署、边界
最终 fmt/check、workspace locked offline test **465 passed / 10 ignored**（38 suites）、all-target/all-feature clippy -D warnings 及 release 构建通过。回归覆盖已知/未知/饱和计数、实际派发后失败、拒绝结果跨重启去重，以及编辑框尺寸变化时原生对象替换必须拒绝。

Agent 已按原稳定证书部署，最终实例 `3463cc61-c09d-4698-857f-7578e01cb41d`，四权限 true，journal/profile 保留。备份 work/v14-longtext-final-20260920/previous-bundle；证据 accepted.json、work/v14-longtext-20260920/anchor-diagnostic.json 与 zero-dispatch-receipt.json。

本轮 GUI 全部通过 RWI Plugin，无 CUA。没有保存测试图层、提交或推送；原文件 SHA256 仍 67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e。当前 Host 旧 MCP 进程的异常摘要仍需下一次自然重连更新；不影响新 Agent 修复生效。

该验收不覆盖 4096 字节极限、多行/IME、用户同时编辑同一文本框或任意后台失焦场景；也不代表 V14 视觉复刻完成。下一步继续真实 V14 编辑、增量观察和阶段耗时优化。
