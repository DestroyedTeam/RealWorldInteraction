# Paint S 文本直调修复与实机验收

## 修正与依据
- `AXSubrole` 为可空的专用子类型；普通 TextField/TextArea 不应因明确无子类型被判为读取失败。仅原生读取返回 AttributeUnsupported(-25205)/NoValue(-25212) 或成功空字符串，才表示明确无子类型。超时、无效对象、类型错误仍 Unknown；AXUnknown 及安全子类型仍拒绝。
- Apple 文档：https://developer.apple.com/documentation/appkit/nsaccessibility-c.protocol/accessibilitysubrole （nullable）；https://developer.apple.com/documentation/applicationservices/kaxsecuretextfieldsubrole （安全子类型）。
- 键盘 payload、精确原生目标及焦点检查移到 AppKit 后台激活事件之前；每个按下/释放批次仍再次检查，不重放。
- MCP 摘要只压缩无 dispatch_error 且未 fenced 的 returned 回执；快照 fenced 时 mutation_ready=false，原始证据保留。

## 回归与部署
fmt/check、workspace locked offline tests **463 passed / 10 ignored**（38 suites）、all-target/all-feature clippy -D warnings 通过，release 构建成功。
使用原本地签名证书更新 RWI Agent，保留 journal/profile/配对身份，四权限仍 true。Agent 实例 `81a811a6-9018-4f48-b3ef-2eef75de1de1`。
MCP 稳定二进制也原子更新，但当前 Host 仍持有旧进程：本轮实际调用中的摘要仍是旧版本，新摘要经过单测，须下一次自然重连生效；不要求再次重启来测试已部署 Agent。

## 按时间顺序的真实 GUI 结果
靶场 Paint S PID 2578 / cgwindow:6491，左侧空白画布新建独立临时文本。全过程使用已加载 RWI 直调工具，无 computer.js、CUA 或外部输入桥。

1. 新建文本对象，type_text `RWI 验收`：680ms，截图与 AXTextArea.value 精确匹配；fence=false。

![输入结果](../../outputs/validation/2026-09-20-text-input/typed.png)

2. press_key Escape：706ms，编辑器 AXTextArea 消失，对象保持选中；这是可见效果验收，而非仅 returned。

![退出编辑](../../outputs/validation/2026-09-20-text-input/escaped.png)

3. 点击原生 Undo：624ms，截图显示 Undo Add “RWI 验收”，临时层消失，原成果保留。

![撤销恢复](../../outputs/validation/2026-09-20-text-input/restored.png)

4. 向已聚焦 scroll area 发送不支持的键名：240ms 拒绝，原请求查询确认 `unsupported_key`、`native_dispatch_entered=false`、`input_domain_fenced=false`，无需恢复输入。回执 `work/v14-text-classification-20260920/negative-receipt.json`。

## 限制与后续
- core durable receipt 目前仍将后端 Err 标为 dispatch=uncertain，即使 Agent 零事件计数证明没有派发；本轮修复消除了误 fence，但回执状态命名还要基于正面零派发证据细化，不能按错误码猜测。
- 成功样本是短 Unicode 文本；多批长文本、IME、动态编辑框尺寸及后台失焦连续操作仍需单独验收。
- 没有保存测试文本；原文件 SHA256 保持 `67c340468b4ca1bcdc452a1f9a29083739563ec378327553bc2e858d7aa04e3e`。
- 未提交/推送；Paint V14 的图形复刻并未因本轮输入验收而宣称完成。
