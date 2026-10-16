# Paint S V14：占位弧线诊断、头像对齐与第 7 行（2026-09-25）

## 范围与真实来源

本轮沿用 BetterDisplay 虚拟屏 UUID `13E4E9B0-47F2-4043-9CDC-878D5441BD41` 上的正式 Paint S 分层文档；当前任务由 RWI Plugin MCP 重新发现并绑定 PID 14404 的主窗口 `cgwindow:2189`，窗口当时为 `[1536,24,1434,660]`。所有 GUI 导入、选层、拖动、微调、结束编辑与保存都经 RWI MCP；终端只做原图确定性裁切/缩放、只读归档检查和阶段备份。没有改写 `.paint` 内部结构或把整幅参考图烤成新层。

本轮输入前的第 6 行快照为 133 对象/10 Image、SHA-256 `b804a35ad576296a170e07fb29eb72bc24345f6a51f20f5a74f73b8f9449c652`；先前阶段文件未覆盖。

## 按时间顺序的结果

1. **查明酒红色占位环的来源。**只读解开 `.paint` 中底层 `DrawImage2` 的嵌入 PNG，得到 2000×679 原始底图（中间件 `work/v14-base-layer-readonly-20260925.png`，SHA-256 `cbd4a791242368f074a6bf62d5a1d73fa5edf6ef91e99646409e68b4cdeb6293`）。占位环在底图像素里，不是可直接删除的独立 Ellipse；因此没有盲删 11 个现存 Ellipse。底图头像列的第3/4/6行环大致落在 x478..526、top-down y272..320 / 347..395 / 497..545。只读解析新增图层却发现第4行图片 top-down 起点 `[476,339]`，第6行 `[478,504]`，明显与底图环错开；这解释了大块外露弧线。
2. **在实际 Paint GUI 微调，而非修改归档。**RWI 逐次选择并核对高亮 Image 层，用独立请求的方向键移动：第4行下8/右2个画布像素，第6行上7，第3行下2/右2；第5行原本与环近似同心，不改。每次按键后取新画面，最终 Escape、`cmd+s`；只读归档复核第3–6行 Image 的 bottom-left bounds 依次为 `[478,359,48,48]`、`[478,284,48,48]`、`[478,208,48,48]`、`[478,134,48,48]`。对象数保持133/10 Image。对齐阶段快照 [`paint-v14-20260925-avatar-aligned.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-aligned.paint) 为466241 bytes、SHA-256 `d127d91ab7991a0dd96e31ddf0060176ab5b4bf4197e0f53b9701f6111602114`；[原生分辨率局部截图](../../outputs/validation/2026-09-25-v14-alignment/native-row3-row6-aligned.png) 为450×500、SHA-256 `704582bed959a9b230447926bddaa80b47ee8806d86fe61ddd3acf62809ba078`。第3/4/6行先前大块露出的弧线消失，仍有细约1px边线及第5行选中行的外轮廓，不能称1:1。
3. **第7行蓝色游戏手柄头像。**从既有参考 `work/replica-verify-20260915/attachments/attach-3-bkkRer32.png` 的 x162/y979 裁82×82（SHA-256 `019fb47812ad4561247f25782c2f051661a18b71f182d69ed5f59834c5826728`），经 `sips` 缩为48×48（SHA-256 `ca69709da8bd0fa94afcdc27ab587eeaa49e7fa49699488ea4b88427917e803e`），存 `work/replica-verify-20260915/assets/`。导入前复制对齐版工作源至 `work/paint-v14-before-row7-20260925.paint`，SHA与对齐快照一致。
4. **RWI GUI 导入及定位。**RWI 对正式文档 `Add Image` 执行一次 AXPress，独立发现虚拟屏上的 Open 面板并明确选中 `avatar-row7-import48.png`。AXOpen 回执为 `native_action_unconfirmed` / `-25205`，因此**没有重放**；新发现表明面板已关闭，主画面出现第11个 Image/48×48并被选中。RWI 从默认画布中心拖至第7行；新截图证明图层确实移动。Escape、`cmd+s` 后只读归档为134对象/11 Image，新增最上层 bottom-left `[478,60,48,48]`。比较底图环后重新选中该 Image，用 RWI `down` 一次，Escape、`cmd+s`，落盘 bounds `[478,59,48,48]`（top-down `[478,572,48,48]`），嵌入PNG为48×48。最新分层快照 [`paint-v14-20260925-avatar-row7.paint`](../../outputs/paint-v14-source/paint-v14-20260925-avatar-row7.paint) 为472829 bytes、SHA-256 `7332d4c2033f73ac4fad4a53116bbc0ae6b785b91ce63a690eba2510856e4730`，与正式工作源字节一致。RWI [原生分辨率局部截图](../../outputs/validation/2026-09-25-v14-row7/native-row6-row8-after-row7.png) 380×225、SHA-256 `5cc636b3d43adb71342002a7d788ced5293f22a8c891c428235786ffc7666560`：第7行已有蓝色头像，第8行占位环仍空白。
5. **第8行来源限制检查。**参考图第8行仅显示头像上半部，余下被原始截图中的窗口底缘/黑色区域截断。虽可裁 x162/y1103 的82×82，但缩至48×48后的下半部分是参考图黑色遮挡，不是头像真实内容；本轮不将黑块误导入正式 `.paint`。准备的中间候选仅在 `work/`，未作为成果。下一步可用只含真实可见像素的局部图层及画布裁切处理，不臆造隐藏部分。

## 边界与后续

- 底图的占位环为栅格，48×48头像与约49px环仍可能留细边；选择行的大酒红椭圆、残余轮廓、侧栏/会话文字和整体布局未达到参考1:1。第8行仍未导入。
- 阶段 `.paint` 均保留独立可编辑 Image 及旧快照；逐操作 Undo 历史能否跨关闭/重启保持仍未证明。保存回执本身不是目标证明，证据是主画面变化、落盘SHA、归档对象数/几何与原生截图。
- 本轮无产品 Rust 源码修改、无 Agent/MCP 部署、无 Git 提交/推送；未碰 Windows VM 或用户主屏。
