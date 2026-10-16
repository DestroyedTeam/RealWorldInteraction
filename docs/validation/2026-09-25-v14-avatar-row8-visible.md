# Paint S V14：第 8 行仅可见头像的分层导入（2026-09-25）

接续 [第3–7行头像对齐与导入](2026-09-25-v14-avatar-alignment-and-row7.md)。正式 Paint S 文档仍在 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41`；本任务 RWI MCP 重新发现/绑定 PID14404 主窗 `cgwindow:2189`。导入前工作源复制为 `work/paint-v14-before-row8-20260925.paint`，SHA-256 `7332d4c2033f73ac4fad4a53116bbc0ae6b785b91ce63a690eba2510856e4730`，与第7行阶段快照相同。

## 按时间顺序的实际步骤

1. 参考 `work/replica-verify-20260915/attachments/attach-3-bkkRer32.png` 第8行 x162/y1103 的82×82头像区域，底部是**参考截图自身**的黑色截断；只读像素检查显示在 x0/20/40/60/81 五个取样列，黑色均从相对 y56 开始。没有把黑块解释为头像，也没有凭空补全其不可见下半部。
2. `sips` 仅截原图 x162/y1103 的82×56真实可见部分（SHA-256 `35f98617e978bf5ebbddc7a20aa90a7529a175379cc2a9d1f13b14bb87f51915`），按48/82比例缩为48×33，再取上方48×32作为 GUI 导入素材（SHA-256 `949ecdf79d4a2d7ea1746c9f659f08d22243b62d12f11c348e740a0ea2d26767`）。两个中间资产均在 `work/replica-verify-20260915/assets/`，只作为独立局部头像素材，不合成整幅界面。
3. RWI 在正式文档对 `Add Image` 发一次 AXPress；独立发现 Open 面板 `cgwindow:8233` 位于虚拟屏、明确选择 `avatar-row8-visible48x32.png`。AXOpen 返回 `native_action_unconfirmed` / `-25205`，**没有重放**；随后重新发现面板关闭，正式文档多出选中的48×32 Image，截图也显示实际嵌入。
4. RWI 在新帧将新 Image 从默认画布中心拖至第8行可见窗口底部；新截图显示选框已移到目标，而不是只凭返回回执。Escape、`cmd+s` 后只读解开 `.paint`：135绘图对象/12 Image，新顶层 embedded PNG 48×32、archive bottom-left `[478,0,48,32]`，换算 canvas top-left `[478,647,48,32]`，正好止于679px画布下缘。无需假设或存储被截断下半部。
5. 正式工作源与 [`paint-v14-20260925-avatar-row8-visible.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-row8-visible.paint) 同为477415 bytes、SHA-256 `3f4c26dc9517edc512c832df417965ef2124825a2246bd0269dbc76d092d6423`。RWI [原生380×225截图](../../outputs/validation/2026-09-25-v14-row8/native-row6-row8-after-row8.png)，SHA-256 `c3a1c6b908c672def0cd05c66bc9a756a11cb2626a1fb7b41e2a06acc7138bae`，确认第8行在画布底缘露出参考里的蓝色 SUI 图案，并且没有黑色带导入。旧第7行快照未覆盖。

## 质量边界

- 当前只复刻**原参考实际可见的上部头像**，不推断隐藏像素；第2–8行头像为各自可编辑 Image，但第1行来源/分层仍待逐层核实。底图约1px酒红边、选中行外椭圆、文字与其他形状仍非1:1。
- 当前 Paint 会话的 Undo 栈存在，但 `.paint` 是否持久化逐操作撤销史未证实；可编辑层与分阶段快照已落盘。
- 本轮 GUI 由已部署 RWI Plugin MCP 执行，终端仅做确定性素材准备/只读归档核验/阶段复制；无产品 Rust 代码变更、部署或 Git 提交/推送。
