# Paint S V14：侧栏底色与遮盖层的渲染色校准（2026-09-25）

接续 [Live 浮层与 V14 视觉核对](2026-09-25-live-panel-polish-and-v14-audit.md)。正式 `.paint` 工作源在 BetterDisplay 虚拟屏的 Paint S 主文档，编辑前已复制 `work/paint-v14-before-sidebar-bg-20260925.paint`，SHA-256 `3f4c26dc9517edc512c832df417965ef2124825a2246bd0269dbc76d092d6423`，并保留此前第8行阶段快照。GUI 图层/颜色更改以真实 RWI 与必要的 Computer Use 辅助完成，终端只做参考裁切、只读归档/像素核验和阶段复制。

## 按时间顺序的实际结果

1. **先比较再编辑。**参考与 RWI 原生画面以420×680的近似几何映射对照：左侧栏平坦样本 `(20,210)` 参考 `#F5F5F5`，原画稿 `#E0E0E1`；原有酒红占位圈与数枚图标不符。对照只作为该区域设计基准，不宣称全图已精确配准。RWI 色值控件当时为旧遮盖色 `#D9D9DA`。
2. **验证颜色入口/跨屏。**RWI 对填色色井 AXPress 单次没有证明弹窗可操作；经新帧做一次原生双击后，Paint 的色板 popover 确实出现。CG目录可发现其独立瞬态窗口，但 RWI 对它精确绑定后的 `snapshot` 返回 `window_missing`，不能据 CG存在就伪称 RWI已操作。Computer Use 在 Paint AX 树读到该 popover 的 `Input Color Manually` 按钮并点击；随后 RWI 独立发现真正的 `Input Color` 对话框起初在**内建物理屏**，用 `computer.place_window` 按 BetterDisplay UUID 移至虚拟屏并稳定读回，再由 RWI 在对话框里切十六进制、输入/确认。取色板 popover 的最后收起由 Computer Use 执行。后续相同颜色入口均用这条已验证的分工，没有跨屏盲点或不确定输入重放。
3. **新增可编辑背景。**RWI 核对 Rectangle 工具已选、Fill=on、Border=off 后，在实际画布拖出64×627 Rectangle；GUI截图和只读归档核验 top-left `[401,49,64,627]`。新矩形最初覆盖现存侧栏图标；Computer Use 按真实 `Object > Send to Back`、再 `Bring Forward` 一层，RWI截图确认现有矢量人形/星形/#/邮件/手机/菜单轮廓重新露出，底图酒红占位圈被遮盖。归档新层 order2，在底层图片之上且保留其余独立层。RWI Escape/`cmd+s`，初版阶段 [`paint-v14-20260925-sidebar-background.paint`](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-background.paint) 为136对象/12 Image/28 Rectangle、477735 bytes、SHA-256 `8e06c81823d8df73bc8b93cf43045a755b60b46ac7789870c3da34fa9248baff`。
4. **修整三个既有局部底色块。**选中并逐个核对 bounds 后，联系人、星形、# 的三个现存 Rectangle `[414,105,42,42]`、`[416,154,38,38]`、`[419,201,32,36]` 改为与新背景同一输入色；未删层，也未把图标烤成图。每个 RGB 值均由 Paint 色控件回读和落盘归档复核，RWI保存。此时[第一次原生截图](../../outputs/validation/2026-09-25-v14-sidebar-color/actual-sidebar-after-masks-420x680.png) 显示侧栏比参考亮；五个平坦点的参考都是 `#F5F5F5`，第一次渲染却是 `#F7F7F7`。因此**不能**以输入框显示的 `#F5F5F5` 宣称颜色已匹配。
5. **以渲染像素而非输入值校准。**只读归档发现此前联系人底块的 `NSComponents≈0.9490196`（输入 `#F2F2F2`）在 RWI原生截图实际渲染为 `#F5F5F5`；当前 `#F5F5F5` 输入值则渲染为 `#F7F7F7`。因此把新背景和三个局部块分别改为输入 `#F2F2F2`。一次 RWI 对十六进制输入框的像素聚焦因 `content_changed` 报 `stale_reference`，原请求查询证明 `native_input_dispatched=false`、未围栏，未重放；该次改由 Computer Use 对已明确选中的输入框设值，RWI重新观察 `#f2f2f2` 后才 AXPress OK。其余 Dialog 的旧值经 RWI直接核对/确认。所有真实 Input Color 弹窗先移回虚拟屏；关闭色板后 RWI结束编辑并 `cmd+s`。
6. **最终原生核验。**正式工作源与新快照 [`paint-v14-20260925-sidebar-render-calibrated.paint`](../../outputs/paint-v14-source/paint-v14-20260925-sidebar-render-calibrated.paint) 均477879 bytes、SHA-256 `c8b0e73295e8e928c98174e245efe11f59be311c864fd05ec76f6cf2a978f55c`；只读两层 NSKeyedArchiver 为**136对象/12 Image/28 Rectangle**，新背景在order2，三个既有块在order110/113/118，四者 `NSComponents` 同为 `0.9490196078`/无描边/有填色。[最终 RWI 原生420×680截图](../../outputs/validation/2026-09-25-v14-sidebar-color/actual-sidebar-final-420x680.png)，SHA-256 `b56bb72e719c61be97ab4c5a3c955672715553c13af5f65a9b9fe4c0410fac07`；五个远离文字/图标的点 `(20,210)`、`(26,156)`、`(26,205)`、`(26,110)`、`(20,300)` **参考和实际均为 `#F5F5F5`**。这是这五点的匹配，不是全区域、全作品逐像素等同。

## 仍待处理

- 背景校准后，消息气泡、文件/应用/九宫格等侧栏图标仍空白或轮廓不准；星/#/联系人只有现存轮廓，不等于全部形态1:1。应依参考逐个重绘并检查实际渲染色。**后续勘误**：这里早先记录的消息蓝色 `#1697FF` 未经可靠源像素核验，2026-09-25 对原参考 PNG 与实际 RWI 原像素局部再测，主体占优色均为 `#1A83FE`；见[色板绑定与消息底形](2026-09-25-popup-binding-and-blue-base.md)。
- 选中行旧酒红大椭圆与横线仍在底图像素内，部分文字重叠/模糊；后续用可编辑遮盖和 Text/矢量层逐项修，不覆写底图 PNG。
- 保存的是可编辑 `.paint` 图层和阶段源，不承诺逐操作 Undo 历史跨重启持久化。此轮无产品 Rust 代码变更、无 Git 提交/推送。
